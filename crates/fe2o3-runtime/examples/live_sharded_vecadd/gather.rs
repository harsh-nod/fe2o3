//! Ordered partial writes share one initialized sink and one prequeued readback.

use super::*;

#[derive(Clone, Copy, Debug)]
struct Window {
    source_offset: usize,
    destination_offset: usize,
    bytes: usize,
}

struct Layout {
    windows: Vec<Window>,
    bytes: usize,
}

impl Layout {
    fn new(shards: &[Shard], overlap: bool) -> Self {
        let mut end = 17;
        let windows = shards[..shards.len() - 1]
            .iter()
            .enumerate()
            .map(|(index, shard)| {
                let destination_offset = if index == 0 {
                    end
                } else if overlap {
                    end - 31
                } else {
                    end + 23
                };
                let window = Window {
                    source_offset: 4 + index * 4,
                    destination_offset,
                    bytes: shard.logical_bytes() - 20 - index * 8,
                };
                end = destination_offset + window.bytes;
                window
            })
            .collect();
        Self {
            windows,
            bytes: end + 97,
        }
    }

    fn expected(&self, shards: &[Shard], round: usize) -> (Vec<Vec<u8>>, Vec<u8>, Vec<u8>) {
        let sources: Vec<_> = shards[..shards.len() - 1]
            .iter()
            .map(|shard| {
                let mut bytes = (-1.0_f32).to_le_bytes().repeat(shard.padded_bytes / 4);
                for (index, word) in bytes[..shard.logical_bytes()]
                    .chunks_exact_mut(4)
                    .enumerate()
                {
                    word.copy_from_slice(
                        &((shard.offset + index + 3 + 131_073 * round) as f32).to_le_bytes(),
                    );
                }
                bytes
            })
            .collect();
        let mut destination = vec![0xa5; self.bytes];
        for (source, window) in sources.iter().zip(&self.windows) {
            destination[window.destination_offset..window.destination_offset + window.bytes]
                .copy_from_slice(
                    &source[window.source_offset..window.source_offset + window.bytes],
                );
        }
        let mut host = vec![0x5a; self.bytes + 42];
        host[13..13 + self.bytes].copy_from_slice(&destination);
        (sources, destination, host)
    }
}

struct Sink {
    device: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
    peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
}

pub(super) fn options(arguments: &[String]) -> ResultV1<(Option<bool>, &[String])> {
    let mode = match arguments.first().map(String::as_str) {
        Some("--gather") => Some(false),
        Some("--gather-overlap") => Some(true),
        _ => None,
    };
    let ids = &arguments[usize::from(mode.is_some())..];
    if mode.is_some() && !(3..=8).contains(&ids.len()) {
        return Err("gather requires two to seven source GPUs followed by one sink GPU".into());
    }
    Ok((mode, ids))
}

fn window(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: usize,
    bytes: usize,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        byte_offset: offset as u64,
        ..region(allocation, access, bytes)
    }
}

fn allocate(context: &mut Context, bytes: usize) -> ResultV1<Sink> {
    let device = context.devices().last().ok_or("missing sink device")?.id();
    Ok(Sink {
        device: context
            .allocate(
                device,
                RuntimeMemoryKindV1::DeviceLocal,
                bytes as u64,
                PAGE_BYTES as u64,
            )
            .map_err(|error| failure("gather-destination", error))?,
        host: context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                (bytes + 42) as u64,
                PAGE_BYTES as u64,
            )
            .map_err(|error| failure("gather-host", error))?,
        peer_stream: context
            .create_stream(device)
            .map_err(|error| failure("gather-stream", error))?,
        readback_stream: context
            .create_stream(device)
            .map_err(|error| failure("gather-readback-stream", error))?,
    })
}

fn initialize(engine: &mut Engine, handle: &Handle, sink: Arc<Sink>, bytes: usize) -> ResultV1<()> {
    let deadline = Instant::now() + WAIT;
    let owned = Arc::clone(&sink);
    let mut upload = command(
        engine,
        handle,
        deadline,
        "gather-initialize",
        move |context| {
            context
                .write_allocation(owned.host, 0, &vec![0xa5; bytes])
                .map_err(|error| failure("gather-initial-host", error))?;
            context
                .copy_async(
                    owned.peer_stream,
                    region(owned.host, RuntimeAccessV1::Read, bytes),
                    region(owned.device, RuntimeAccessV1::Write, bytes),
                    &[],
                )
                .map_err(|error| failure("gather-upload", error))
        },
    )?;
    for _ in 0..TICKS {
        if Instant::now() >= deadline {
            break;
        }
        let owned = Arc::clone(&sink);
        let (returned, complete) = command(
            engine,
            handle,
            deadline,
            "gather-upload-progress",
            move |context| {
                context
                    .flush_stream(owned.peer_stream)
                    .map_err(|error| failure("gather-upload-flush", error))?;
                let complete = match context
                    .poll(&mut upload)
                    .map_err(|error| failure("gather-upload-poll", error))?
                {
                    RuntimePollV1::Pending => false,
                    RuntimePollV1::Succeeded => true,
                    status => return Err(failure("gather-upload-status", status)),
                };
                Ok((upload, complete))
            },
        )?;
        upload = returned;
        if complete {
            return command(
                engine,
                handle,
                deadline,
                "gather-upload-release",
                move |context| {
                    context
                        .release_submission(upload)
                        .map_err(|error| failure("gather-upload-release", error))?;
                    verify_initial(context, sink.device, &vec![0xa5; bytes])?;
                    context
                        .write_allocation(sink.host, 0, &vec![0x5a; bytes + 42])
                        .map_err(|error| failure("gather-host-guards", error))?;
                    verify_initial(context, sink.host, &vec![0x5a; bytes + 42])
                },
            );
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    Err(failure("gather-upload-deadline", "bounded setup exhausted"))
}

fn admit(
    context: &mut Context,
    buffers: &[Buffers],
    shards: &[Shard],
    layout: &Layout,
    sink: &Sink,
    round: usize,
    receipts: &Arc<Mutex<Receipts>>,
) -> ResultV1<Batch> {
    let count = layout.windows.len();
    let offset = round * (2 * count + 1);
    let mut batch = Batch {
        compute: Vec::new(),
        peers: Vec::new(),
        readbacks: Vec::new(),
        ids: Vec::new(),
    };
    let mut events = Vec::new();
    for (index, buffer) in buffers[..count].iter().enumerate() {
        let recipe = recipe(buffers.len(), index, round, shards[index])?;
        let [a, b, c] = buffer.device;
        let arguments =
            Arguments::new(recipe, a, b, c).map_err(|error| failure("gather-arguments", error))?;
        let submission = context
            .launch_producer_aware_v1(
                buffer.compute_stream,
                &buffer.kernel,
                &arguments,
                recipe.geometry(),
                &[],
            )
            .map_err(|error| failure("gather-compute", error))?;
        callback(context, &submission, offset + index, receipts)?;
        batch.ids.push(submission.id());
        events.push(
            context
                .record_event(&submission)
                .map_err(|error| failure("gather-compute-event", error))?,
        );
        batch.compute.push(submission);
    }
    let mut predecessor = None;
    for (index, checked) in layout.windows.iter().enumerate() {
        let mut dependencies = vec![events[index]];
        dependencies.extend(predecessor);
        let submission = context
            .peer_copy(
                sink.peer_stream,
                window(
                    buffers[index].device[2],
                    RuntimeAccessV1::Read,
                    checked.source_offset,
                    checked.bytes,
                ),
                window(
                    sink.device,
                    RuntimeAccessV1::Write,
                    checked.destination_offset,
                    checked.bytes,
                ),
                &dependencies,
            )
            .map_err(|error| failure("gather-peer", error))?;
        callback(context, &submission, offset + count + index, receipts)?;
        batch.ids.push(submission.id());
        if let Some(event) = predecessor {
            context
                .release_event(event)
                .map_err(|error| failure("gather-predecessor-release", error))?;
        }
        predecessor = Some(
            context
                .record_event(&submission)
                .map_err(|error| failure("gather-peer-event", error))?,
        );
        batch.peers.push(submission);
    }
    let event = predecessor.ok_or("no gather peers")?;
    let readback = context
        .copy_async(
            sink.readback_stream,
            region(sink.device, RuntimeAccessV1::Read, layout.bytes),
            window(sink.host, RuntimeAccessV1::Write, 13, layout.bytes),
            &[event],
        )
        .map_err(|error| failure("gather-whole-readback", error))?;
    callback(context, &readback, offset + 2 * count, receipts)?;
    batch.ids.push(readback.id());
    batch.readbacks.push(readback);
    events.push(event);
    for event in events {
        context
            .release_event(event)
            .map_err(|error| failure("gather-event-release", error))?;
    }
    for submission in &batch.compute {
        pending(context, submission)?;
    }
    for submission in &batch.peers {
        pending(context, submission)?;
    }
    pending(context, &batch.readbacks[0])?;
    if context.backend().completed_compute_xgmi_copies_v1() != (round * count) as u64 {
        return Err(failure(
            "gather-prequeued",
            "transfer completed during admission",
        ));
    }
    Ok(batch)
}

fn pending<A>(context: &Context, submission: &RuntimeSubmissionV1<A>) -> ResultV1<()> {
    if context
        .query_submission(submission)
        .map_err(|error| failure("gather-pending", error))?
        != RuntimeCompletionStatusV1::Pending
    {
        return Err(failure("gather-pending", "submission is not pending"));
    }
    Ok(())
}

fn verify(
    layout: &Layout,
    shards: &[Shard],
    round: usize,
    sources: &[Vec<u8>],
    destination: &[u8],
    host: &[u8],
) -> ResultV1<(String, String)> {
    let expected = layout.expected(shards, round);
    if sources != expected.0 || destination != expected.1 || host != expected.2 {
        return Err(failure(
            "gather-bytes",
            "full sources, destination, or host differ",
        ));
    }
    let mut framed = b"fe2o3.queued-gather.v1\0".to_vec();
    for bytes in sources.iter().map(Vec::as_slice).chain([destination, host]) {
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(bytes);
    }
    Ok((hex_digest(destination), hex_digest(&framed)))
}

pub(super) fn run_batches(
    engine: &mut Engine,
    handle: &Handle,
    buffers: Arc<Vec<Buffers>>,
    shards: Arc<Vec<Shard>>,
    overlap: bool,
) -> ResultV1<Report> {
    let layout = Arc::new(Layout::new(&shards, overlap));
    let bytes = layout.bytes;
    let sink = Arc::new(command(
        engine,
        handle,
        Instant::now() + WAIT,
        "gather-allocate",
        move |context| allocate(context, bytes),
    )?);
    let mut registration = Box::pin(
        handle
            .enqueue_stream_registration(sink.readback_stream)
            .map_err(|error| failure("gather-registration", error))?,
    );
    let registered = engine.drive_until_ready(registration.as_mut(), Instant::now() + WAIT);
    drop(registration);
    let registration = registered
        .map_err(|error| failure("gather-registration-drive", error))?
        .map_err(|error| failure("gather-registration-result", error))?
        .map_err(|error| failure("gather-registration-admission", error))?;
    let count = layout.windows.len();
    let receipts = Arc::new(Mutex::new(Receipts::new(2 * (2 * count + 1))));
    let original = handles(&buffers);
    let mut digests = Vec::new();
    let mut ids = Vec::new();
    for round in 0..2 {
        refresh(engine, handle, &buffers, &shards, round)?;
        initialize(engine, handle, Arc::clone(&sink), bytes)?;
        let deadline = Instant::now() + WAIT;
        let owned = Arc::clone(&buffers);
        let partition = Arc::clone(&shards);
        let checked = Arc::clone(&layout);
        let destination = Arc::clone(&sink);
        let observed = Arc::clone(&receipts);
        let batch = command(
            engine,
            handle,
            deadline,
            "gather-admission",
            move |context| {
                admit(
                    context,
                    &owned,
                    &partition,
                    &checked,
                    &destination,
                    round,
                    &observed,
                )
            },
        )?;
        let batch = settle_batch(engine, handle, batch, count, round, deadline)?;
        ids.extend_from_slice(&batch.ids);
        if !receipts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .succeeded(round * (2 * count + 1), &batch.ids)
        {
            return Err(failure(
                "gather-callbacks",
                "exact original completion receipts differ",
            ));
        }
        let destination = Arc::clone(&sink);
        let owned = Arc::clone(&buffers);
        let partition = Arc::clone(&shards);
        let checked = Arc::clone(&layout);
        digests.push(command(
            engine,
            handle,
            deadline,
            "gather-snapshot-release",
            move |context| {
                if context.backend().retained_compute_xgmi_copies_v1() != 0 {
                    return Err(failure("gather-restoration", "native custody remains"));
                }
                for (stream, total) in [
                    (destination.peer_stream, count),
                    (destination.readback_stream, 1),
                ] {
                    let state = context
                        .query_stream(stream)
                        .map_err(|error| failure("gather-settled-stream", error))?;
                    if state.total_submissions != total
                        || state.succeeded != total
                        || !state.is_quiescent()
                        || state.failed != 0
                        || state.quiescent_without_result != 0
                        || state.first_failure.is_some()
                    {
                        return Err(failure("gather-settled-stream", state));
                    }
                }
                let mut sources = Vec::new();
                for index in 0..count {
                    let mut bytes = vec![0; partition[index].padded_bytes];
                    context
                        .read_allocation(owned[index].device[2], 0, &mut bytes)
                        .map_err(|error| failure("gather-source-snapshot", error))?;
                    sources.push(bytes);
                }
                let mut sink_bytes = vec![0; checked.bytes];
                let mut host = vec![0; checked.bytes + 42];
                context
                    .read_allocation(destination.device, 0, &mut sink_bytes)
                    .map_err(|error| failure("gather-device-snapshot", error))?;
                context
                    .read_allocation(destination.host, 0, &mut host)
                    .map_err(|error| failure("gather-host-snapshot", error))?;
                let result = verify(&checked, &partition, round, &sources, &sink_bytes, &host)?;
                for submission in batch.readbacks.into_iter().rev() {
                    context
                        .release_submission(submission)
                        .map_err(|error| failure("gather-readback-release", error))?;
                }
                for submission in batch.peers.into_iter().rev() {
                    context
                        .release_submission(submission)
                        .map_err(|error| failure("gather-peer-release", error))?;
                }
                for submission in batch.compute.into_iter().rev() {
                    context
                        .release_submission(submission)
                        .map_err(|error| failure("gather-compute-release", error))?;
                }
                Ok(result)
            },
        )?);
        if handle.observer().reply_cells_in_use() != 0 || handles(&buffers) != original {
            return Err(failure(
                "gather-reuse",
                "reply credits or logical handles differ",
            ));
        }
    }
    if !receipts
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .succeeded(0, &ids)
    {
        return Err(failure(
            "gather-final-callbacks",
            "original receipts differ",
        ));
    }
    let mut future = Box::pin(
        handle
            .begin_drain(TICKS)
            .map_err(|error| failure("gather-drain", error))?,
    );
    let result = engine.drive_until_ready(future.as_mut(), Instant::now() + WAIT);
    drop(future);
    let drained = result
        .map_err(|error| failure("gather-drain-drive", error))?
        .map_err(|error| failure("gather-drain-result", error))?;
    if drained.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
        || !drained.queued_commands_exhausted
        || drained.operations_remaining != 0
        || drained.graph_active
        || drained.retained_submissions != RuntimeStreamObservationV1::default()
        || drained.ticks == 0
        || drained.ticks > TICKS
    {
        return Err(failure("gather-drain", drained));
    }
    drop(registration);
    Ok(Report {
        digests: digests
            .try_into()
            .map_err(|_| "two gather rounds required")?,
    })
}

pub(super) fn report(ids: &[u64], shards: &[Shard], overlap: bool, result: &Report) -> String {
    let layout = Layout::new(shards, overlap);
    let count = layout.windows.len();
    let devices = ids.len();
    let ids = ids
        .iter()
        .map(|id| format!("0x{id:016x}"))
        .collect::<Vec<_>>()
        .join(",");
    let windows = layout
        .windows
        .iter()
        .map(|w| format!("{}:{}:{}", w.source_offset, w.destination_offset, w.bytes))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "PASS schema=fe2o3.queued-gather.v1 authority=qualification-sharded-vecadd-rounds-v1 devices={devices} sources={count} unique_ids={ids} rounds=2 overlap={overlap} windows={windows} destination_bytes={} host_bytes={} launches={} peer_copies={} dependent_readbacks=2 completion_receipts={} admission=all-before-explicit-progress progress=final-readback-stream-only public_events=released-after-admission native_transport=NATIVE-XGMI native_counter=0,{count},{} output=full-byte-pass source_preservation=full-byte-pass untouched_destination=full-byte-pass host_guards=full-byte-pass round0_sha256={} round1_sha256={} round0_framed_sha256={} round1_framed_sha256={} pipeline_host_joins=0 host_output_installations=0 contexts=1 owners=1 allocation_reuse=true input_refresh=full-padded-per-batch deadline_resumption=same-command-future intermediate_cutoffs=0 final_drain=completed-only cleanup=owned-shutdown-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
        layout.bytes,
        layout.bytes + 42,
        2 * count,
        2 * count,
        2 * (2 * count + 1),
        2 * count,
        result.digests[0].0,
        result.digests[1].0,
        result.digests[0].1,
        result.digests[1].1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_cli_requires_explicit_mode_and_at_least_two_sources() {
        for flag in ["--gather", "--gather-overlap"] {
            assert!(options(&[flag.into(), "0x1".into(), "0x2".into()]).is_err());
            let args = [flag.into(), "0x3".into(), "0x2".into(), "0x1".into()];
            let (mode, ids) = options(&args).unwrap();
            assert_eq!(mode, Some(flag == "--gather-overlap"));
            assert_eq!(unique_ids(ids).unwrap(), [3, 2, 1]);
        }
    }

    #[test]
    fn gather_oracle_checks_preservation_order_guards_and_both_rounds() {
        for count in 3..=8 {
            let shards = partition(count).unwrap();
            for overlap in [false, true] {
                let layout = Layout::new(&shards, overlap);
                for pair in layout.windows.windows(2) {
                    assert_eq!(
                        pair[1].destination_offset < pair[0].destination_offset + pair[0].bytes,
                        overlap
                    );
                }
                for round in 0..2 {
                    let (sources, sink, host) = layout.expected(&shards, round);
                    assert!(verify(&layout, &shards, round, &sources, &sink, &host).is_ok());
                    assert!(verify(&layout, &shards, 1 - round, &sources, &sink, &host).is_err());
                    for position in [0, 16, 17, sink.len() - 98, sink.len() - 97, sink.len() - 1] {
                        let mut wrong = sink.clone();
                        wrong[position] ^= 1;
                        assert!(verify(&layout, &shards, round, &sources, &wrong, &host).is_err());
                    }
                    let mut wrong = sources.clone();
                    wrong[0][0] ^= 1;
                    assert!(verify(&layout, &shards, round, &wrong, &sink, &host).is_err());
                    let mut wrong = host.clone();
                    wrong[12] ^= 1;
                    assert!(verify(&layout, &shards, round, &sources, &sink, &wrong).is_err());
                    if overlap {
                        let mut reversed = vec![0xa5; layout.bytes];
                        for (source, w) in sources.iter().zip(&layout.windows).rev() {
                            reversed[w.destination_offset..w.destination_offset + w.bytes]
                                .copy_from_slice(
                                    &source[w.source_offset..w.source_offset + w.bytes],
                                );
                        }
                        assert!(
                            verify(&layout, &shards, round, &sources, &reversed, &host).is_err()
                        );
                    }
                }
            }
        }
    }
}
