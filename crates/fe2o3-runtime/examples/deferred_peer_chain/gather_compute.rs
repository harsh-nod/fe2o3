//! Ordered gather into the original R57 C input, followed by its unchanged second gate.

use super::*;

const USAGE: &str = "usage: gfx942-runtime-deferred-peer-chain-smoke <--gather-compute|--gather-compute-overlap> <0xsource-id> <0xsource-id> [more source IDs] <0xsink-id>";

#[derive(Debug, Eq, PartialEq)]
struct Options {
    ids: Vec<u64>,
    overlap: bool,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    let overlap = match arguments.first().map(String::as_str) {
        Some("--gather-compute") => false,
        Some("--gather-compute-overlap") => true,
        _ => return Err(USAGE.into()),
    };
    if !(3..=8).contains(&(arguments.len() - 1)) {
        return Err(USAGE.into());
    }
    let mut ids = Vec::new();
    for argument in &arguments[1..] {
        let value = argument
            .strip_prefix("0x")
            .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        let id = u64::from_str_radix(value, 16).map_err(|_| USAGE)?;
        if id == 0 || ids.contains(&id) {
            return Err(USAGE.into());
        }
        ids.push(id);
    }
    Ok(Options { ids, overlap })
}

#[derive(Clone, Copy, Debug)]
struct Window {
    source: usize,
    destination: usize,
    bytes: usize,
}

fn windows(count: usize, overlap: bool) -> Vec<Window> {
    assert!((2..=7).contains(&count));
    let mut end = 64;
    (0..count)
        .map(|index| {
            let source = 4 * (1 + 7 * index);
            let destination = if index == 0 {
                end
            } else if overlap {
                end - 48
            } else {
                end + 28
            };
            let bytes = (((BYTES - 512) / count) & !3) - 4 * index;
            assert!(source + bytes <= BYTES && destination + bytes < BYTES);
            end = destination + bytes;
            Window {
                source,
                destination,
                bytes,
            }
        })
        .collect()
}

fn expected(checked: &[Window]) -> Vec<Vec<u8>> {
    let source = expected_d();
    let mut gathered = filled(0.25);
    for window in checked {
        gathered[window.destination..window.destination + window.bytes]
            .copy_from_slice(&source[window.source..window.source + window.bytes]);
    }
    let computed: Vec<u8> = gathered
        .chunks_exact(4)
        .enumerate()
        .flat_map(|(index, bytes)| {
            let input = f32::from_le_bytes(bytes.try_into().expect("one f32"));
            (input + (index & 31) as f32 * 0.5).to_le_bytes()
        })
        .collect();
    let layout = ReturnLayout::new(true);
    let mut returned = layout.returned_initial();
    returned[layout.returned_offset..layout.returned_offset + layout.copy_bytes]
        .copy_from_slice(&computed[layout.source_offset..layout.source_offset + layout.copy_bytes]);
    let mut host = vec![0x5a; layout.host_bytes];
    host[layout.host_offset..layout.host_offset + returned.len()].copy_from_slice(&returned);
    let mut all = vec![source; checked.len()];
    all.extend([gathered, computed, returned, host]);
    all
}

fn verify(checked: &[Window], snapshots: &[Vec<u8>]) -> ResultV1<String> {
    if snapshots != expected(checked) {
        return Err(failure(
            "gather-compute-bytes",
            "full independent snapshots differ",
        ));
    }
    let mut framed = b"fe2o3.gather-compute.v1\0".to_vec();
    for bytes in snapshots {
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(bytes);
    }
    Ok(digest(&framed))
}

struct Resources {
    runs: Vec<DeviceRun>,
    windows: Vec<Window>,
    peer_stream: RuntimeStreamIdV1,
    return_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    returned: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
}

fn setup(options: &Options) -> ResultV1<(ManuallyDrop<Context>, Arc<Resources>)> {
    let admitted = admit_gfx942_r57_n3_qualification_v2()
        .map_err(|error| failure("gather-artifact", error))?;
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_peer_qualification_v2(&options.ids)
            .map_err(|error| failure("gather-devices", error))?;
    let mut context = ManuallyDrop::new(
        Context::open_with_version_journal_members_v1(backend, 128, 128, 128)
            .map_err(|error| failure("gather-context", error))?,
    );
    if context.devices().len() != options.ids.len()
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure("gather-devices", "ordered gfx942 roster differs"));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let deadline = Instant::now() + WAIT;
    let mut runs = Vec::new();
    for &device in &devices {
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("gather-stream", error))?;
        let module = context
            .load_module(device, admitted.hsaco())
            .map_err(|error| failure("gather-module", error))?;
        let kernel = context
            .resolve_kernel::<Arguments>(module, admitted.kernel_name())
            .map_err(|error| failure("gather-kernel", error))?;
        let host = context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                BYTES as u64,
                PAGE as u64,
            )
            .map_err(|error| failure("gather-upload", error))?;
        let mut allocations = Vec::new();
        for bytes in inputs() {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    BYTES as u64,
                    PAGE as u64,
                )
                .map_err(|error| failure("gather-allocation", error))?;
            upload(&mut context, stream, host, allocation, &bytes, deadline)?;
            allocations.push(allocation);
        }
        let run = DeviceRun {
            stream,
            upload: host,
            allocations: allocations
                .try_into()
                .map_err(|_| "four allocations required")?,
            kernel,
        };
        setup_compute(&mut context, &run, false, deadline)?;
        // Preserve source C's actual device-produced identity for its second gate.
        runs.push(run);
    }
    let sink = runs.last().ok_or("missing sink")?;
    upload(
        &mut context,
        sink.stream,
        sink.upload,
        sink.allocations[2],
        &filled(0.25),
        deadline,
    )?;
    let layout = ReturnLayout::new(true);
    let returned = context
        .allocate(
            devices[0],
            RuntimeMemoryKindV1::DeviceLocal,
            layout.returned_bytes as u64,
            PAGE as u64,
        )
        .map_err(|error| failure("gather-returned", error))?;
    let host = context
        .allocate(
            devices[0],
            RuntimeMemoryKindV1::HostVisible,
            layout.host_bytes as u64,
            PAGE as u64,
        )
        .map_err(|error| failure("gather-host", error))?;
    upload(
        &mut context,
        runs[0].stream,
        host,
        returned,
        &layout.returned_initial(),
        deadline,
    )?;
    context
        .write_allocation(host, 0, &vec![0x5a; layout.host_bytes])
        .map_err(|error| failure("gather-host-initial", error))?;
    let peer_stream = context
        .create_stream(*devices.last().ok_or("missing sink")?)
        .map_err(|error| failure("gather-peer-stream", error))?;
    let return_stream = context
        .create_stream(devices[0])
        .map_err(|error| failure("gather-return-stream", error))?;
    let readback_stream = context
        .create_stream(devices[0])
        .map_err(|error| failure("gather-readback-stream", error))?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure("gather-setup", "unexpected native peer completion"));
    }
    Ok((
        context,
        Arc::new(Resources {
            runs,
            windows: windows(devices.len() - 1, options.overlap),
            peer_stream,
            return_stream,
            readback_stream,
            returned,
            host,
        }),
    ))
}

#[derive(Default)]
struct Receipts {
    observations: Vec<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>,
}

fn callback<A>(
    context: &mut Context,
    submission: &RuntimeSubmissionV1<A>,
    receipts: &Arc<Mutex<Receipts>>,
) -> ResultV1<()> {
    let id = submission.id();
    let receipts = Arc::clone(receipts);
    context
        .on_completion(submission, move |status| {
            receipts
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .observations
                .push((id, status));
        })
        .map_err(|error| failure("gather-callback", error))
}

struct Chain {
    sources: Vec<RuntimeSubmissionV1<Arguments>>,
    peers: Vec<RuntimeSubmissionV1<RuntimePeerCopyV1>>,
    compute: RuntimeSubmissionV1<Arguments>,
    returned: RuntimeSubmissionV1<RuntimePeerCopyV1>,
    readback: RuntimeSubmissionV1<RuntimeCopyV1>,
    ids: Vec<RuntimeSubmissionIdV1>,
}

fn admit(
    context: &mut Context,
    resources: &Resources,
    receipts: &Arc<Mutex<Receipts>>,
) -> ResultV1<Chain> {
    let count = resources.windows.len();
    let sink = &resources.runs[count];
    let mut ids = Vec::new();
    let mut sources = Vec::new();
    let mut events = Vec::new();
    for run in &resources.runs[..count] {
        let arguments = Arguments::new(run.allocations[2], run.allocations[1], run.allocations[3])
            .map_err(|error| failure("gather-source-arguments", error))?;
        let submission = context
            .launch_producer_aware_v1(
                run.stream,
                &run.kernel,
                &arguments,
                GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
                &[],
            )
            .map_err(|error| failure("gather-source-compute", error))?;
        callback(context, &submission, receipts)?;
        ids.push(submission.id());
        events.push(
            context
                .record_event(&submission)
                .map_err(|error| failure("gather-source-event", error))?,
        );
        sources.push(submission);
    }
    let mut peers = Vec::new();
    let mut predecessor = None;
    for (index, window) in resources.windows.iter().enumerate() {
        let mut dependencies = vec![events[index]];
        dependencies.extend(predecessor);
        let submission = context
            .peer_copy(
                resources.peer_stream,
                range(
                    resources.runs[index].allocations[3],
                    RuntimeAccessV1::Read,
                    window.source,
                    window.bytes,
                ),
                range(
                    sink.allocations[2],
                    RuntimeAccessV1::Write,
                    window.destination,
                    window.bytes,
                ),
                &dependencies,
            )
            .map_err(|error| failure("gather-peer", error))?;
        callback(context, &submission, receipts)?;
        ids.push(submission.id());
        context
            .release_event(events[index])
            .map_err(|error| failure("gather-source-event-release", error))?;
        if let Some(event) = predecessor {
            context
                .release_event(event)
                .map_err(|error| failure("gather-peer-event-release", error))?;
        }
        predecessor = Some(
            context
                .record_event(&submission)
                .map_err(|error| failure("gather-peer-event", error))?,
        );
        peers.push(submission);
    }
    let event = predecessor.ok_or("missing gather tail")?;
    let arguments = Arguments::new(
        sink.allocations[2],
        sink.allocations[1],
        sink.allocations[3],
    )
    .map_err(|error| failure("gather-consumer-arguments", error))?;
    let compute = context
        .launch_producer_aware_v1(
            sink.stream,
            &sink.kernel,
            &arguments,
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[event],
        )
        .map_err(|error| failure("gather-consumer-compute", error))?;
    callback(context, &compute, receipts)?;
    ids.push(compute.id());
    context
        .release_event(event)
        .map_err(|error| failure("gather-tail-event-release", error))?;
    let event = context
        .record_event(&compute)
        .map_err(|error| failure("gather-consumer-event", error))?;
    let layout = ReturnLayout::new(true);
    let returned = context
        .peer_copy(
            resources.return_stream,
            range(
                sink.allocations[3],
                RuntimeAccessV1::Read,
                layout.source_offset,
                layout.copy_bytes,
            ),
            range(
                resources.returned,
                RuntimeAccessV1::Write,
                layout.returned_offset,
                layout.copy_bytes,
            ),
            &[event],
        )
        .map_err(|error| failure("gather-return-peer", error))?;
    callback(context, &returned, receipts)?;
    ids.push(returned.id());
    context
        .release_event(event)
        .map_err(|error| failure("gather-consumer-event-release", error))?;
    let event = context
        .record_event(&returned)
        .map_err(|error| failure("gather-return-event", error))?;
    let readback = context
        .copy_async(
            resources.readback_stream,
            range(
                resources.returned,
                RuntimeAccessV1::Read,
                0,
                layout.returned_bytes,
            ),
            range(
                resources.host,
                RuntimeAccessV1::Write,
                layout.host_offset,
                layout.returned_bytes,
            ),
            &[event],
        )
        .map_err(|error| failure("gather-full-readback", error))?;
    callback(context, &readback, receipts)?;
    ids.push(readback.id());
    context
        .release_event(event)
        .map_err(|error| failure("gather-return-event-release", error))?;
    for submission in &sources {
        require(context, submission, RuntimeCompletionStatusV1::Pending)?;
    }
    for submission in &peers {
        require(context, submission, RuntimeCompletionStatusV1::Pending)?;
    }
    require(context, &compute, RuntimeCompletionStatusV1::Pending)?;
    require(context, &returned, RuntimeCompletionStatusV1::Pending)?;
    require(context, &readback, RuntimeCompletionStatusV1::Pending)?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure(
            "gather-admission",
            "peer completed before explicit progress",
        ));
    }
    Ok(Chain {
        sources,
        peers,
        compute,
        returned,
        readback,
        ids,
    })
}

fn pipeline(engine: &mut Engine, handle: &Handle, resources: Arc<Resources>) -> ResultV1<String> {
    let deadline = Instant::now() + WAIT;
    let mut future = Box::pin(
        handle
            .enqueue_stream_registration(resources.readback_stream)
            .map_err(|error| failure("gather-register", error))?,
    );
    let result = engine.drive_until_ready(future.as_mut(), deadline);
    drop(future);
    let registration = result
        .map_err(|error| failure("gather-register-drive", error))?
        .map_err(|error| failure("gather-register-reply", error))?
        .map_err(|error| failure("gather-register-result", error))?;
    let receipts = Arc::new(Mutex::new(Receipts::default()));
    let observed = Arc::clone(&receipts);
    let owned = Arc::clone(&resources);
    let mut chain = command(engine, handle, deadline, "gather-admit", move |context| {
        admit(context, &owned, &observed)
    })?;
    let mut completed = false;
    for _ in 0..TICKS {
        if Instant::now() >= deadline {
            break;
        }
        let result = command(engine, handle, deadline, "gather-poll", move |context| {
            let complete = match context
                .poll(&mut chain.readback)
                .map_err(|error| failure("gather-final-poll", error))?
            {
                RuntimePollV1::Succeeded => true,
                RuntimePollV1::Pending => false,
                status => return Err(failure("gather-final-status", status)),
            };
            Ok((chain, complete))
        })?;
        chain = result.0;
        if result.1 {
            completed = true;
            break;
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    if !completed {
        return Err(failure("gather-deadline", "final-only progress exhausted"));
    }
    {
        let receipts = receipts.lock().unwrap_or_else(|error| error.into_inner());
        if receipts.observations.len() != chain.ids.len()
            || chain.ids.iter().any(|id| {
                receipts
                    .observations
                    .iter()
                    .filter(|(seen, status)| {
                        seen == id && *status == RuntimeCompletionStatusV1::Succeeded
                    })
                    .count()
                    != 1
            })
        {
            return Err(failure(
                "gather-receipts",
                "exact successful callback roster differs",
            ));
        }
    }
    let owned = Arc::clone(&resources);
    let snapshots = command(
        engine,
        handle,
        deadline,
        "gather-snapshot-release",
        move |context| {
            for submission in &chain.sources {
                require(context, submission, RuntimeCompletionStatusV1::Succeeded)?;
            }
            for submission in &chain.peers {
                require(context, submission, RuntimeCompletionStatusV1::Succeeded)?;
            }
            require(
                context,
                &chain.compute,
                RuntimeCompletionStatusV1::Succeeded,
            )?;
            require(
                context,
                &chain.returned,
                RuntimeCompletionStatusV1::Succeeded,
            )?;
            require(
                context,
                &chain.readback,
                RuntimeCompletionStatusV1::Succeeded,
            )?;
            if context.backend().completed_compute_xgmi_copies_v1()
                != (owned.windows.len() + 1) as u64
                || context.backend().retained_compute_xgmi_copies_v1() != 0
            {
                return Err(failure(
                    "gather-native-counter",
                    "completion or retained native count differs",
                ));
            }
            let sink = owned.runs.last().ok_or("missing sink")?;
            let layout = ReturnLayout::new(true);
            let mut allocations: Vec<_> = owned.runs[..owned.windows.len()]
                .iter()
                .map(|run| (run.allocations[3], BYTES))
                .collect();
            allocations.extend([
                (sink.allocations[2], BYTES),
                (sink.allocations[3], BYTES),
                (owned.returned, layout.returned_bytes),
                (owned.host, layout.host_bytes),
            ]);
            let mut snapshots = Vec::new();
            for (allocation, bytes) in allocations {
                let mut snapshot = vec![0; bytes];
                context
                    .read_allocation(allocation, 0, &mut snapshot)
                    .map_err(|error| failure("gather-snapshot", error))?;
                snapshots.push(snapshot);
            }
            context
                .release_submission(chain.readback)
                .map_err(|error| failure("gather-release-readback", error))?;
            context
                .release_submission(chain.returned)
                .map_err(|error| failure("gather-release-return", error))?;
            context
                .release_submission(chain.compute)
                .map_err(|error| failure("gather-release-consumer", error))?;
            for submission in chain.peers.into_iter().rev() {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("gather-release-peer", error))?;
            }
            for submission in chain.sources {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("gather-release-source", error))?;
            }
            Ok(snapshots)
        },
    )?;
    let output = verify(&resources.windows, &snapshots)?;
    if handle.observer().reply_cells_in_use() != 0 {
        return Err(failure("gather-replies", "retained reply credit"));
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
        return Err(failure("gather-drained", drained));
    }
    drop(registration);
    Ok(output)
}

fn report(options: &Options, output: &str) -> String {
    let count = options.ids.len() - 1;
    let ids = options
        .ids
        .iter()
        .map(|id| format!("0x{id:016x}"))
        .collect::<Vec<_>>()
        .join(",");
    let windows = windows(count, options.overlap)
        .iter()
        .map(|window| format!("{}:{}:{}", window.source, window.destination, window.bytes))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "PASS schema=fe2o3.gather-compute.v1 authority=qualification-r57-n3-v2 devices={} sources={count} unique_ids={ids} overlap={} windows={windows} elements={ELEMENTS} bytes={BYTES} setup_launches={} pipeline_launches={} peer_copies={} dependent_readbacks=1 completion_receipts={} pipeline=compute-gather-compute-peer-readback admission=all-before-explicit-progress progress=final-readback-stream-only public_events=released-after-dependent-admission consumer_dependencies=latest-gather-only consumer_bindings=full-frame-read-stable-read-full-write native_transport=NATIVE-XGMI native_counter=0,{} output=full-byte-pass source_preservation=full-byte-pass gathered_frame=full-byte-pass return_guards=full-byte-pass host_guards=full-byte-pass output_sha256={output} digest=length-prefixed-sources-C-D-E-host host_output_installations=0 pipeline_host_joins=0 journal=enabled contexts=1 owners=1 batches=1 results_release=reverse-dependencies final_drain=completed-only cleanup=owned-shutdown-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
        options.ids.len(),
        options.overlap,
        options.ids.len(),
        count + 1,
        count + 1,
        2 * count + 3,
        count + 1
    )
}

pub(super) fn main(arguments: &[String]) -> ResultV1<()> {
    let options = options(arguments)?;
    let (context, resources) = setup(&options)?;
    let config = RuntimeAsyncEngineConfigV1::new(64, 64, 64, 64, Duration::from_micros(50))
        .and_then(|config| config.with_reply_capacity(64))
        .map_err(|error| failure("gather-owner-config", error))?;
    let progress = RuntimeAsyncProgressConfigV1::new(1, 1)
        .map_err(|error| failure("gather-progress-config", error))?;
    let (mut engine, handle) = Engine::new_with_progress(
        || Ok::<_, String>(ManuallyDrop::into_inner(context)),
        config,
        progress,
    )
    .map_err(|error| failure("gather-owner", error))?;
    let result = pipeline(&mut engine, &handle, resources);
    let shutdown = engine.shutdown();
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|report| !report.is_complete() || !report.failures().is_empty())
    {
        return Err(failure("gather-shutdown", (result.err(), shutdown)));
    }
    println!("{}", report(&options, &result?));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gather_compute_cli_preserves_sources_and_sink_without_new_authority() {
        for mode in ["--gather-compute", "--gather-compute-overlap"] {
            let parsed = options(&[mode.into(), "0x3".into(), "0x2".into(), "0x1".into()]).unwrap();
            assert_eq!(parsed.ids, [3, 2, 1]);
            assert_eq!(parsed.overlap, mode.ends_with("overlap"));
            let line = report(
                &parsed,
                &verify(
                    &windows(2, parsed.overlap),
                    &expected(&windows(2, parsed.overlap)),
                )
                .unwrap(),
            );
            assert!(line.contains("authority=qualification-r57-n3-v2"));
            assert!(line.contains("consumer_dependencies=latest-gather-only"));
            assert!(line.contains("formal_refinement=false"));
        }
        for args in [
            vec![],
            vec!["--gather-compute", "0x1", "0x2"],
            vec!["--gather-compute", "0x1", "0x2", "0x1"],
            vec!["--gather-compute", "0x0", "0x2", "0x3"],
            vec!["--gather-compute", "0x1", "0x2", "--late-compute"],
            vec!["--gather-compute", "0x1", "0x2", "3"],
            vec!["--unknown", "0x1", "0x2", "0x3"],
        ] {
            assert!(options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn gather_compute_oracle_covers_preserved_frame_consumer_and_all_guards() {
        for count in [2, 3, 7] {
            for overlap in [false, true] {
                let checked = windows(count, overlap);
                let snapshots = expected(&checked);
                assert!(verify(&checked, &snapshots).is_ok());
                for (index, snapshot) in snapshots.iter().enumerate() {
                    for offset in [0, snapshot.len() / 2, snapshot.len() - 1] {
                        let mut corrupted = snapshots.clone();
                        corrupted[index][offset] ^= 1;
                        assert!(verify(&checked, &corrupted).is_err());
                    }
                    let mut wrong = snapshots.clone();
                    wrong[index].pop();
                    assert!(verify(&checked, &wrong).is_err());
                }
                for window in &checked {
                    for offset in [
                        window.destination - 1,
                        window.destination,
                        window.destination + window.bytes - 1,
                        window.destination + window.bytes,
                    ] {
                        let mut wrong = snapshots.clone();
                        wrong[count][offset] ^= 1;
                        assert!(verify(&checked, &wrong).is_err());
                    }
                }
                let mut wrong = snapshots.clone();
                wrong[count] = filled(0.25);
                assert!(verify(&checked, &wrong).is_err());
                wrong = snapshots.clone();
                wrong[count + 1] = expected_d();
                assert!(verify(&checked, &wrong).is_err());
                if overlap {
                    let mut reversed = checked.clone();
                    reversed.reverse();
                    assert_ne!(snapshots[count], expected(&reversed)[count]);
                }
            }
        }
    }
}
