//! Pending or released source compute through unchanged two-step R57 device gates.

use super::*;
use fe2o3_runtime::{RuntimePeerCopySegmentV1, RuntimePeerCopySegmentsV1};

const USAGE: &str = "usage: gfx942-runtime-deferred-peer-chain-smoke <--segments-compute|--late-segments-compute|--segments-readback|--settled-segments-compute|--late-settled-segments-compute|--settled-segments-readback> <4|65|4096> <0xsource-id> <0xdestination-id>";
const SOURCE_OFFSET: usize = 8;
const SOURCE_LEN: usize = BYTES - 40;
const DESTINATION_OFFSET: usize = 20;
const DESTINATION_LEN: usize = BYTES - 76;
const PIPELINE_WAIT: Duration = Duration::from_secs(180);
const PIPELINE_TICKS: usize = 60_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    ids: [u64; 2],
    count: usize,
    late: bool,
    direct: bool,
    settled: bool,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    if arguments.len() != 4 {
        return Err(USAGE.into());
    }
    let (late, direct, settled) = match arguments[0].as_str() {
        "--segments-compute" => (false, false, false),
        "--late-segments-compute" => (true, false, false),
        "--segments-readback" => (false, true, false),
        "--settled-segments-compute" => (false, false, true),
        "--late-settled-segments-compute" => (true, false, true),
        "--settled-segments-readback" => (false, true, true),
        _ => return Err(USAGE.into()),
    };
    let count = match arguments[1].as_str() {
        "4" => 4,
        "65" => 65,
        "4096" => 4096,
        _ => return Err(USAGE.into()),
    };
    Ok(Options {
        ids: unique_ids(&arguments[2..])?,
        count,
        late,
        direct,
        settled,
    })
}

fn descriptors(count: usize) -> Vec<RuntimePeerCopySegmentV1> {
    (0..count)
        .map(|index| {
            let (source_offset, destination_offset, byte_len) = match index {
                0 | 2 => (0, 0, 128),
                1 => (128, 64, 128),
                3 => (512, 96, 68),
                _ => (
                    4 * (1 + index * 13 % 10_000),
                    4 * (257 + index * 17 % 40_000),
                    4 * (1 + index % 17),
                ),
            };
            RuntimePeerCopySegmentV1 {
                source_offset: source_offset as u64,
                destination_offset: destination_offset as u64,
                byte_len: byte_len as u64,
            }
        })
        .collect()
}

fn expected(count: usize) -> [Vec<u8>; 5] {
    let source = expected_d();
    let mut input = filled(0.25);
    for segment in descriptors(count) {
        let source_start = SOURCE_OFFSET + segment.source_offset as usize;
        let destination_start = DESTINATION_OFFSET + segment.destination_offset as usize;
        let bytes = segment.byte_len as usize;
        input[destination_start..destination_start + bytes]
            .copy_from_slice(&source[source_start..source_start + bytes]);
    }
    let output = input
        .chunks_exact(4)
        .enumerate()
        .flat_map(|(index, bytes)| {
            let value = f32::from_le_bytes(bytes.try_into().expect("one f32"));
            (value + (index & 31) as f32 * 0.5).to_le_bytes()
        })
        .collect::<Vec<_>>();
    let layout = ReturnLayout::new(true);
    let mut returned = layout.returned_initial();
    returned[layout.returned_offset..layout.returned_offset + layout.copy_bytes]
        .copy_from_slice(&output[layout.source_offset..layout.source_offset + layout.copy_bytes]);
    let mut host = vec![0x5a; layout.host_bytes];
    host[layout.host_offset..layout.host_offset + layout.returned_bytes].copy_from_slice(&returned);
    [source, input, output, returned, host]
}

fn verify(count: usize, snapshots: &[Vec<u8>; 5]) -> ResultV1<String> {
    if snapshots != &expected(count) {
        return Err(failure(
            "segments-bytes",
            "full independent snapshots differ",
        ));
    }
    let mut framed = b"fe2o3.pending-segments-compute.v1\0".to_vec();
    for bytes in snapshots {
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(bytes);
    }
    Ok(digest(&framed))
}

fn direct_layout() -> ReturnLayout {
    ReturnLayout {
        source_offset: 0,
        returned_offset: 0,
        host_offset: 97,
        copy_bytes: BYTES,
        returned_bytes: BYTES,
        host_bytes: BYTES + 273,
    }
}

fn expected_direct(count: usize) -> [Vec<u8>; 3] {
    let [source, input, _, _, _] = expected(count);
    let layout = direct_layout();
    let mut host = vec![0x5a; layout.host_bytes];
    host[layout.host_offset..layout.host_offset + BYTES].copy_from_slice(&input);
    [source, input, host]
}

fn verify_direct(count: usize, snapshots: &[Vec<u8>; 3]) -> ResultV1<String> {
    if snapshots != &expected_direct(count) {
        return Err(failure(
            "segments-direct-bytes",
            "full source/frame/host differ",
        ));
    }
    let mut framed = b"fe2o3.pending-segments-readback.v1\0".to_vec();
    for bytes in snapshots {
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(bytes);
    }
    Ok(digest(&framed))
}

fn setup(options: Options) -> ResultV1<(ManuallyDrop<Context>, Arc<Resources>)> {
    let admitted = admit_gfx942_r57_n3_qualification_v2()
        .map_err(|error| failure("segments-artifact", error))?;
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_peer_qualification_v2(&options.ids)
            .map_err(|error| failure("segments-devices", error))?;
    let mut context = ManuallyDrop::new(
        Context::open_with_version_journal_members_v1(backend, 32, 32, 32)
            .map_err(|error| failure("segments-context", error))?,
    );
    if context.devices().len() != 2
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure("segments-devices", "ordered gfx942 roster differs"));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let deadline = Instant::now() + WAIT;
    let mut runs = Vec::new();
    for &device in &devices {
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("segments-stream", error))?;
        let module = context
            .load_module(device, admitted.hsaco())
            .map_err(|error| failure("segments-module", error))?;
        let kernel = context
            .resolve_kernel::<Arguments>(module, admitted.kernel_name())
            .map_err(|error| failure("segments-kernel", error))?;
        let host = context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                BYTES as u64,
                PAGE as u64,
            )
            .map_err(|error| failure("segments-upload", error))?;
        let mut allocations = Vec::new();
        for bytes in inputs() {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    BYTES as u64,
                    PAGE as u64,
                )
                .map_err(|error| failure("segments-allocation", error))?;
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
        runs.push(run);
    }
    if options.settled {
        // Normal setup launch is completed and released; no source event is created.
        setup_compute(&mut context, &runs[0], true, deadline)?;
        require_settled_source(&context, &runs[0])?;
        verify_initial(&mut context, runs[0].allocations[3], &expected_d())?;
    }
    // Sink's second gate remains unconsumed. Only C is refreshed; B/D stay intact.
    upload(
        &mut context,
        runs[1].stream,
        runs[1].upload,
        runs[1].allocations[2],
        &filled(0.25),
        deadline,
    )?;
    verify_initial(&mut context, runs[1].allocations[2], &filled(0.25))?;
    let layout = if options.direct {
        direct_layout()
    } else {
        ReturnLayout::new(true)
    };
    let returned = if options.direct {
        runs[1].allocations[2]
    } else {
        context
            .allocate(
                devices[0],
                RuntimeMemoryKindV1::DeviceLocal,
                layout.returned_bytes as u64,
                PAGE as u64,
            )
            .map_err(|error| failure("segments-returned", error))?
    };
    let host_output = context
        .allocate(
            devices[usize::from(options.direct)],
            RuntimeMemoryKindV1::HostVisible,
            layout.host_bytes as u64,
            PAGE as u64,
        )
        .map_err(|error| failure("segments-host", error))?;
    if !options.direct {
        upload(
            &mut context,
            runs[0].stream,
            host_output,
            returned,
            &layout.returned_initial(),
            deadline,
        )?;
        verify_initial(&mut context, returned, &layout.returned_initial())?;
    }
    context
        .write_allocation(host_output, 0, &vec![0x5a; layout.host_bytes])
        .map_err(|error| failure("segments-host-initial", error))?;
    let initial_peer_stream = context
        .create_stream(devices[1])
        .map_err(|error| failure("segments-peer-stream", error))?;
    let return_peer_stream = context
        .create_stream(devices[0])
        .map_err(|error| failure("segments-return-stream", error))?;
    let readback_stream = context
        .create_stream(devices[usize::from(options.direct)])
        .map_err(|error| failure("segments-readback-stream", error))?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure("segments-setup", "unexpected native completion"));
    }
    Ok((
        context,
        Arc::new(Resources {
            runs: runs.try_into().map_err(|_| "two runs required")?,
            initial_peer_stream,
            return_peer_stream,
            readback_stream,
            returned,
            host_output,
            layout,
        }),
    ))
}

#[derive(Default)]
struct Receipts {
    observations: [Option<(RuntimeSubmissionIdV1, RuntimeCompletionStatusV1)>; 5],
    deliveries: [usize; 5],
}

impl Receipts {
    fn succeeded(&self, ids: &[RuntimeSubmissionIdV1]) -> bool {
        ids.iter().enumerate().all(|(index, &id)| {
            self.deliveries[index] == 1
                && self.observations[index] == Some((id, RuntimeCompletionStatusV1::Succeeded))
        }) && self.deliveries[ids.len()..].iter().all(|&count| count == 0)
    }
}

fn callback<A>(
    context: &mut Context,
    submission: &RuntimeSubmissionV1<A>,
    index: usize,
    receipts: &Arc<Mutex<Receipts>>,
) -> ResultV1<()> {
    let id = submission.id();
    let receipts = Arc::clone(receipts);
    context
        .on_completion(submission, move |status| {
            let mut receipts = receipts.lock().unwrap_or_else(|error| error.into_inner());
            receipts.deliveries[index] = receipts.deliveries[index].saturating_add(1);
            receipts.observations[index] = Some((id, status));
        })
        .map_err(|error| failure("segments-callback", error))
}

struct Chain {
    producer: Option<RuntimeSubmissionV1<Arguments>>,
    list: RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>,
    consumer: Option<RuntimeSubmissionV1<Arguments>>,
    returned: Option<RuntimeSubmissionV1<RuntimePeerCopyV1>>,
    readback: RuntimeSubmissionV1<RuntimeCopyV1>,
    ids: Vec<RuntimeSubmissionIdV1>,
}

fn require_settled_source(context: &Context, source: &DeviceRun) -> ResultV1<()> {
    let observation = context
        .query_stream(source.stream)
        .map_err(|error| failure("segments-settled-source-query", error))?;
    if observation != RuntimeStreamObservationV1::default() {
        return Err(failure("segments-settled-source-results", observation));
    }
    Ok(())
}

fn publication_gate(retained: usize, completed: u64, pending: bool) -> ResultV1<bool> {
    if completed != 0 || !pending || retained > 1 {
        return Err(failure(
            "segments-publication-state",
            (retained, completed, pending),
        ));
    }
    Ok(retained == 1)
}

fn observed(
    context: &Context,
    list: &RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>,
) -> ResultV1<bool> {
    publication_gate(
        context.backend().retained_compute_xgmi_copies_v1(),
        context.backend().completed_compute_xgmi_copies_v1(),
        context
            .query_submission(list)
            .map_err(|error| failure("segments-publication-query", error))?
            == RuntimeCompletionStatusV1::Pending,
    )
}

fn admit(
    context: &mut Context,
    resources: &Resources,
    options: Options,
    receipts: &Arc<Mutex<Receipts>>,
    deadline: Instant,
) -> ResultV1<Chain> {
    let source = &resources.runs[0];
    let sink = &resources.runs[1];
    let producer = if options.settled {
        require_settled_source(context, source)?;
        None
    } else {
        let arguments = Arguments::new(
            source.allocations[2],
            source.allocations[1],
            source.allocations[3],
        )
        .map_err(|error| failure("segments-producer-arguments", error))?;
        let submission = context
            .launch_producer_aware_v1(
                source.stream,
                &source.kernel,
                &arguments,
                GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
                &[],
            )
            .map_err(|error| failure("segments-producer", error))?;
        callback(context, &submission, 0, receipts)?;
        Some(submission)
    };
    let event = producer
        .as_ref()
        .map(|submission| {
            context
                .record_event(submission)
                .map_err(|error| failure("segments-producer-event", error))
        })
        .transpose()?;
    let first = usize::from(producer.is_some());
    let mut supplied = descriptors(options.count);
    let list = context
        .peer_copy_segments(
            resources.initial_peer_stream,
            range(
                source.allocations[3],
                RuntimeAccessV1::Read,
                SOURCE_OFFSET,
                SOURCE_LEN,
            ),
            range(
                sink.allocations[2],
                RuntimeAccessV1::Write,
                DESTINATION_OFFSET,
                DESTINATION_LEN,
            ),
            &supplied,
            event.as_slice(),
        )
        .map_err(|error| failure("segments-list", error))?;
    callback(context, &list, first, receipts)?;
    if let Some(event) = event {
        context
            .release_event(event)
            .map_err(|error| failure("segments-producer-event-release", error))?;
    } else {
        require_settled_source(context, source)?;
    }
    // Both caller order and descriptor bytes cease being a source of authority.
    supplied.reverse();
    supplied.fill(RuntimePeerCopySegmentV1 {
        source_offset: u64::MAX,
        destination_offset: u64::MAX,
        byte_len: 0,
    });
    drop(supplied);
    if options.late {
        for _ in 0..PIPELINE_TICKS {
            if observed(context, &list)? || Instant::now() >= deadline {
                break;
            }
            context
                .progress_stream_v1(resources.initial_peer_stream)
                .map_err(|error| failure("segments-seed", error))?;
            if !observed(context, &list)? {
                std::thread::sleep(Duration::from_micros(50));
            }
        }
        if !observed(context, &list)? {
            return Err(failure(
                "segments-publication",
                "exact sole list publication not observed",
            ));
        }
    }
    let event = context
        .record_event(&list)
        .map_err(|error| failure("segments-list-event", error))?;
    if options.direct {
        let layout = resources.layout;
        let readback = context
            .copy_async(
                resources.readback_stream,
                region(sink.allocations[2], RuntimeAccessV1::Read),
                range(
                    resources.host_output,
                    RuntimeAccessV1::Write,
                    layout.host_offset,
                    BYTES,
                ),
                &[event],
            )
            .map_err(|error| failure("segments-direct-readback", error))?;
        callback(context, &readback, first + 1, receipts)?;
        context
            .release_event(event)
            .map_err(|error| failure("segments-direct-event-release", error))?;
        if let Some(submission) = &producer {
            require(context, submission, RuntimeCompletionStatusV1::Pending)?;
        } else {
            require_settled_source(context, source)?;
        }
        require(context, &list, RuntimeCompletionStatusV1::Pending)?;
        require(context, &readback, RuntimeCompletionStatusV1::Pending)?;
        if context.backend().completed_compute_xgmi_copies_v1() != 0
            || context.backend().retained_compute_xgmi_copies_v1() != 0
        {
            return Err(failure(
                "segments-direct-admission",
                "unexpected native peer progress",
            ));
        }
        let mut ids: Vec<_> = producer
            .as_ref()
            .map(RuntimeSubmissionV1::id)
            .into_iter()
            .collect();
        ids.extend([list.id(), readback.id()]);
        return Ok(Chain {
            producer,
            list,
            consumer: None,
            returned: None,
            readback,
            ids,
        });
    }
    let arguments = Arguments::new(
        sink.allocations[2],
        sink.allocations[1],
        sink.allocations[3],
    )
    .map_err(|error| failure("segments-consumer-arguments", error))?;
    let consumer = context
        .launch_producer_aware_v1(
            sink.stream,
            &sink.kernel,
            &arguments,
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[event],
        )
        .map_err(|error| failure("segments-consumer", error))?;
    if options.late && !observed(context, &list)? {
        return Err(failure("segments-late-admission", "paired custody changed"));
    }
    callback(context, &consumer, first + 1, receipts)?;
    context
        .release_event(event)
        .map_err(|error| failure("segments-list-event-release", error))?;
    let event = context
        .record_event(&consumer)
        .map_err(|error| failure("segments-consumer-event", error))?;
    let layout = resources.layout;
    let returned = context
        .peer_copy(
            resources.return_peer_stream,
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
        .map_err(|error| failure("segments-return", error))?;
    callback(context, &returned, first + 2, receipts)?;
    context
        .release_event(event)
        .map_err(|error| failure("segments-consumer-event-release", error))?;
    let event = context
        .record_event(&returned)
        .map_err(|error| failure("segments-return-event", error))?;
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
                resources.host_output,
                RuntimeAccessV1::Write,
                layout.host_offset,
                layout.returned_bytes,
            ),
            &[event],
        )
        .map_err(|error| failure("segments-readback", error))?;
    callback(context, &readback, first + 3, receipts)?;
    context
        .release_event(event)
        .map_err(|error| failure("segments-return-event-release", error))?;
    if let Some(submission) = &producer {
        require(context, submission, RuntimeCompletionStatusV1::Pending)?;
    } else {
        require_settled_source(context, source)?;
    }
    require(context, &list, RuntimeCompletionStatusV1::Pending)?;
    require(context, &consumer, RuntimeCompletionStatusV1::Pending)?;
    require(context, &returned, RuntimeCompletionStatusV1::Pending)?;
    require(context, &readback, RuntimeCompletionStatusV1::Pending)?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0
        || (!options.late && context.backend().retained_compute_xgmi_copies_v1() != 0)
        || (options.late && !observed(context, &list)?)
    {
        return Err(failure(
            "segments-admission",
            "native counter or custody differs",
        ));
    }
    let mut ids: Vec<_> = producer
        .as_ref()
        .map(RuntimeSubmissionV1::id)
        .into_iter()
        .collect();
    ids.extend([list.id(), consumer.id(), returned.id(), readback.id()]);
    Ok(Chain {
        producer,
        list,
        consumer: Some(consumer),
        returned: Some(returned),
        readback,
        ids,
    })
}

fn pipeline(
    engine: &mut Engine,
    handle: &Handle,
    resources: Arc<Resources>,
    options: Options,
) -> ResultV1<String> {
    let deadline = Instant::now() + PIPELINE_WAIT;
    let mut registration = Box::pin(
        handle
            .enqueue_stream_registration(resources.readback_stream)
            .map_err(|error| failure("segments-register", error))?,
    );
    let result = engine.drive_until_ready(registration.as_mut(), deadline);
    drop(registration);
    let registration = result
        .map_err(|error| failure("segments-register-drive", error))?
        .map_err(|error| failure("segments-register-reply", error))?
        .map_err(|error| failure("segments-register-result", error))?;
    let receipts = Arc::new(Mutex::new(Receipts::default()));
    let retained = Arc::clone(&receipts);
    let owned = Arc::clone(&resources);
    let mut chain = command(engine, handle, deadline, "segments-admit", move |context| {
        admit(context, &owned, options, &retained, deadline)
    })?;
    let mut complete = false;
    for _ in 0..PIPELINE_TICKS {
        if Instant::now() >= deadline {
            break;
        }
        let result = command(engine, handle, deadline, "segments-poll", move |context| {
            let complete = match context
                .poll(&mut chain.readback)
                .map_err(|error| failure("segments-final-poll", error))?
            {
                RuntimePollV1::Succeeded => true,
                RuntimePollV1::Pending => false,
                status => return Err(failure("segments-final-status", status)),
            };
            Ok((chain, complete))
        })?;
        chain = result.0;
        if result.1 {
            complete = true;
            break;
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    if !complete {
        return Err(failure(
            "segments-deadline",
            "final-only progress exhausted",
        ));
    }
    if !receipts
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .succeeded(&chain.ids)
    {
        return Err(failure(
            "segments-receipts",
            "exact successful callback roster required",
        ));
    }
    let owned = Arc::clone(&resources);
    let snapshots = command(
        engine,
        handle,
        deadline,
        "segments-snapshot-release",
        move |context| {
            if let Some(submission) = &chain.producer {
                require(context, submission, RuntimeCompletionStatusV1::Succeeded)?;
            } else {
                require_settled_source(context, &owned.runs[0])?;
            }
            require(context, &chain.list, RuntimeCompletionStatusV1::Succeeded)?;
            if let Some(submission) = &chain.consumer {
                require(context, submission, RuntimeCompletionStatusV1::Succeeded)?;
            }
            if let Some(submission) = &chain.returned {
                require(context, submission, RuntimeCompletionStatusV1::Succeeded)?;
            }
            require(
                context,
                &chain.readback,
                RuntimeCompletionStatusV1::Succeeded,
            )?;
            if context.backend().completed_compute_xgmi_copies_v1()
                != if options.direct { 1 } else { 2 }
                || context.backend().retained_compute_xgmi_copies_v1() != 0
            {
                return Err(failure(
                    "segments-counter",
                    "retired native logical copy count differs",
                ));
            }
            let mut snapshots = Vec::new();
            let mut allocations = vec![
                (owned.runs[0].allocations[3], BYTES),
                (owned.runs[1].allocations[2], BYTES),
            ];
            if !options.direct {
                allocations.extend([
                    (owned.runs[1].allocations[3], BYTES),
                    (owned.returned, owned.layout.returned_bytes),
                ]);
            }
            allocations.push((owned.host_output, owned.layout.host_bytes));
            for (allocation, bytes) in allocations {
                let mut snapshot = vec![0; bytes];
                context
                    .read_allocation(allocation, 0, &mut snapshot)
                    .map_err(|error| failure("segments-full-read", error))?;
                snapshots.push(snapshot);
            }
            context
                .release_submission(chain.readback)
                .map_err(|error| failure("segments-release-readback", error))?;
            if let Some(submission) = chain.returned {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("segments-release-return", error))?;
            }
            if let Some(submission) = chain.consumer {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("segments-release-consumer", error))?;
            }
            context
                .release_submission(chain.list)
                .map_err(|error| failure("segments-release-list", error))?;
            if let Some(submission) = chain.producer {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("segments-release-producer", error))?;
            }
            Ok(snapshots)
        },
    )?;
    let output = if options.direct {
        verify_direct(
            options.count,
            &snapshots
                .try_into()
                .map_err(|_| "three snapshots required")?,
        )?
    } else {
        verify(
            options.count,
            &snapshots
                .try_into()
                .map_err(|_| "five snapshots required")?,
        )?
    };
    if handle.observer().reply_cells_in_use() != 0 {
        return Err(failure(
            "segments-reply-credit",
            "completed replies retained",
        ));
    }
    let mut future = Box::pin(
        handle
            .begin_drain(PIPELINE_TICKS)
            .map_err(|error| failure("segments-drain", error))?,
    );
    let result = engine.drive_until_ready(future.as_mut(), Instant::now() + WAIT);
    drop(future);
    let drained = result
        .map_err(|error| failure("segments-drain-drive", error))?
        .map_err(|error| failure("segments-drain-result", error))?;
    if drained.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
        || !drained.queued_commands_exhausted
        || drained.operations_remaining != 0
        || drained.graph_active
        || drained.retained_submissions != RuntimeStreamObservationV1::default()
        || drained.ticks == 0
        || drained.ticks > PIPELINE_TICKS
    {
        return Err(failure("segments-final-drain", drained));
    }
    drop(registration);
    Ok(output)
}

fn run(options: Options) -> ResultV1<String> {
    let (context, resources) = setup(options)?;
    let config = RuntimeAsyncEngineConfigV1::new(16, 16, 16, 16, Duration::from_micros(50))
        .and_then(|config| config.with_reply_capacity(16))
        .map_err(|error| failure("segments-owner-config", error))?;
    let progress = RuntimeAsyncProgressConfigV1::new(1, 1)
        .map_err(|error| failure("segments-progress-config", error))?;
    let (mut engine, handle) = Engine::new_with_progress(
        || Ok::<_, String>(ManuallyDrop::into_inner(context)),
        config,
        progress,
    )
    .map_err(|error| failure("segments-owner", error))?;
    let result = pipeline(&mut engine, &handle, resources, options);
    let shutdown = engine.shutdown();
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|report| !report.is_complete() || !report.failures().is_empty())
    {
        return Err(failure("segments-shutdown", (result.err(), shutdown)));
    }
    result
}

fn report(options: Options, output: &str) -> String {
    let useful: u64 = descriptors(options.count)
        .iter()
        .map(|segment| segment.byte_len)
        .sum();
    let (admission, progress, retained) = if options.direct {
        (
            if options.settled {
                "all-two-before-explicit-progress"
            } else {
                "all-three-before-explicit-progress"
            },
            "final-readback-stream-only",
            "0,0",
        )
    } else if options.late {
        (
            "consumer-after-list-publication",
            "list-until-retained-then-final-readback-only",
            "0,1,0",
        )
    } else {
        (
            if options.settled {
                "all-four-before-explicit-progress"
            } else {
                "all-five-before-explicit-progress"
            },
            "final-readback-stream-only",
            "0,0",
        )
    };
    let (schema, launches, computes, copies, results, pipeline, counter, guards, digest, release) =
        if options.direct {
            (
                if options.settled {
                    "fe2o3.settled-segments-readback.v1"
                } else {
                    "fe2o3.pending-segments-readback.v1"
                },
                3,
                usize::from(!options.settled),
                1,
                if options.settled { 2 } else { 3 },
                if options.settled {
                    "segments-readback"
                } else {
                    "compute-segments-readback"
                },
                "0,1",
                "not-applicable",
                "length-prefixed-source-input-host",
                if options.settled {
                    "readback-list"
                } else {
                    "readback-list-producer"
                },
            )
        } else {
            (
                if options.settled {
                    "fe2o3.settled-segments-compute.v1"
                } else {
                    "fe2o3.pending-segments-compute.v1"
                },
                4,
                if options.settled { 1 } else { 2 },
                2,
                if options.settled { 4 } else { 5 },
                if options.settled {
                    "segments-deferred-compute-peer-readback"
                } else {
                    "compute-segments-deferred-compute-peer-readback"
                },
                "0,2",
                "full-byte-pass",
                "length-prefixed-source-input-output-return-host",
                if options.settled {
                    "readback-peer-consumer-list"
                } else {
                    "readback-peer-consumer-list-producer"
                },
            )
        };
    let setup_launches = 2 + usize::from(options.settled);
    let provenance = if options.settled {
        " source_admission=settled-result-released source_results_at_list_admission=0 source_events_supplied=0 source_stream=empty-before-and-after-admission"
    } else {
        ""
    };
    format!(
        "PASS schema={schema} authority=qualification-r57-n3-v2 devices=2 unique_ids=0x{:016x},0x{:016x} elements={ELEMENTS} bytes={BYTES} descriptors={} useful_bytes={useful} source_envelope={SOURCE_OFFSET},{SOURCE_LEN} destination_envelope={DESTINATION_OFFSET},{DESTINATION_LEN} descriptor_order=serial-last-writer-wins duplicates=retained caller_list=mutated-after-admission launches={launches} setup_launches={setup_launches} pipeline_launches={computes} peer_copies={copies} segmented_copies=1 dependent_readbacks=1 completion_receipts={results} pipeline={pipeline} admission={admission} progress={progress} late_consumer={} publication_observed={} retained_native_counter={retained} public_events=released-after-dependent-admission native_transport=NATIVE-XGMI native_counter={counter} output=full-byte-pass source_preservation=full-byte-pass destination_frame=initialized-complement-checked return_guards={guards} host_guards=full-byte-pass output_sha256={output} digest={digest} host_output_installations=0 journal=enabled retained_results={results} contexts=1 owners=1 pipeline_host_joins=0 results_release={release} final_drain=completed-only cleanup=owned-shutdown-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false{provenance}",
        options.ids[0], options.ids[1], options.count, options.late, options.late
    )
}

pub(super) fn main(arguments: &[String]) -> ResultV1<()> {
    let options = options(arguments)?;
    let output = run(options)?;
    println!("{}", report(options, &output));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_cli_keeps_modes_counts_and_exact_device_order_explicit() {
        for flag in [
            "--segments-compute",
            "--late-segments-compute",
            "--segments-readback",
        ] {
            for count in [4, 65, 4096] {
                let args = [
                    flag.to_owned(),
                    count.to_string(),
                    "0x2".into(),
                    "0x1".into(),
                ];
                assert_eq!(
                    options(&args).unwrap(),
                    Options {
                        ids: [2, 1],
                        count,
                        late: flag.starts_with("--late"),
                        direct: flag == "--segments-readback",
                        settled: false,
                    }
                );
            }
        }
        for args in [
            vec![],
            vec!["--segments-compute", "0", "0x1", "0x2"],
            vec!["--segments-compute", "4097", "0x1", "0x2"],
            vec!["--segments-compute", "4", "0x1", "0x01"],
            vec!["--segments-compute", "4", "0x0", "0x1"],
            vec!["--segments-compute", "4", "1", "0x2"],
            vec!["--late-segments-compute", "4", "0x1"],
            vec!["--gather-compute", "4", "0x1", "0x2"],
        ] {
            assert!(options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn segmented_oracle_preserves_gaps_order_duplicates_and_all_guard_bytes() {
        // Independently derived by the qualification controller using integer quarters.
        for (count, digest) in [
            (
                4,
                "4de38129cad8c31d5dbe953b09f24bf71855eebb7967415a6492fcb30746ebb6",
            ),
            (
                65,
                "655301a53f783b043b146031565e8b39d86a4a367fb2afb8a489735e591d08d4",
            ),
            (
                4096,
                "86cc241bdd9754c65c3b67ff3cd0890d702f626d7d4d1c4f2fd2749cf86ce590",
            ),
        ] {
            let list = descriptors(count);
            assert_eq!(list.len(), count);
            assert_eq!(list[0], list[2]);
            for segment in &list {
                assert!(segment.byte_len > 0);
                assert_eq!(
                    (segment.source_offset | segment.destination_offset | segment.byte_len) % 4,
                    0
                );
                assert!(segment.source_offset + segment.byte_len <= SOURCE_LEN as u64);
                assert!(segment.destination_offset + segment.byte_len <= DESTINATION_LEN as u64);
            }
            let snapshots = expected(count);
            assert_eq!(verify(count, &snapshots).unwrap(), digest);
            assert_eq!(
                &snapshots[1][..DESTINATION_OFFSET],
                &filled(0.25)[..DESTINATION_OFFSET]
            );
            for index in 0..5 {
                for offset in [0, snapshots[index].len() / 2, snapshots[index].len() - 1] {
                    let mut bad = snapshots.clone();
                    bad[index][offset] ^= 1;
                    assert!(verify(count, &bad).is_err());
                }
                let mut bad = snapshots.clone();
                bad[index].pop();
                assert!(verify(count, &bad).is_err());
            }
            let mut reversed = filled(0.25);
            let source = expected_d();
            for segment in list.iter().rev() {
                let from = SOURCE_OFFSET + segment.source_offset as usize;
                let to = DESTINATION_OFFSET + segment.destination_offset as usize;
                reversed[to..to + segment.byte_len as usize]
                    .copy_from_slice(&source[from..from + segment.byte_len as usize]);
            }
            assert_ne!(reversed, snapshots[1]);
            assert_ne!(snapshots[1], filled(0.25));
            assert_ne!(snapshots[2], expected_d());
        }
    }

    #[test]
    fn segmented_publication_gate_requires_real_retained_pending_list() {
        assert!(!publication_gate(0, 0, true).unwrap());
        assert!(publication_gate(1, 0, true).unwrap());
        for (retained, completed, pending) in [
            (2, 0, true),
            (0, 1, true),
            (1, 1, true),
            (1, 0, false),
            (0, 0, false),
        ] {
            assert!(publication_gate(retained, completed, pending).is_err());
        }
    }

    #[test]
    fn segmented_report_is_exact_and_does_not_claim_general_authority() {
        for late in [false, true] {
            let line = report(
                Options {
                    ids: [2, 1],
                    count: 4,
                    late,
                    direct: false,
                    settled: false,
                },
                "digest",
            );
            let pairs: Vec<_> = line
                .split_whitespace()
                .skip(1)
                .map(|word| word.split_once('=').unwrap())
                .collect();
            let fields: std::collections::BTreeMap<_, _> = pairs.iter().copied().collect();
            assert_eq!(pairs.len(), fields.len());
            assert_eq!(pairs.len(), 48);
            assert_eq!(fields["completion_receipts"], "5");
            assert_eq!(fields["native_counter"], "0,2");
            assert_eq!(fields["authority"], "qualification-r57-n3-v2");
            assert_eq!(fields["formal_refinement"], "false");
            assert_eq!(fields["performance_acceptance"], "false");
            assert_eq!(
                fields["publication_observed"],
                if late { "true" } else { "false" }
            );
        }
    }

    #[test]
    fn direct_readback_covers_frame_outside_the_list_envelope() {
        let snapshots = expected_direct(4);
        assert_eq!(
            verify_direct(4, &snapshots).unwrap(),
            "25f8a17ece259bf3ca3866e31d503e4396f35eb7e547645ea550312072d537f2"
        );
        let layout = direct_layout();
        assert_eq!(
            &snapshots[2][layout.host_offset..layout.host_offset + BYTES],
            snapshots[1]
        );
        for (index, offsets) in [
            (0, vec![0, BYTES - 1]),
            (
                1,
                vec![0, DESTINATION_OFFSET - 1, DESTINATION_OFFSET, BYTES - 1],
            ),
            (
                2,
                vec![
                    0,
                    layout.host_offset - 1,
                    layout.host_offset,
                    layout.host_offset + BYTES,
                    layout.host_bytes - 1,
                ],
            ),
        ] {
            for offset in offsets {
                let mut corrupt = snapshots.clone();
                corrupt[index][offset] ^= 1;
                assert!(verify_direct(4, &corrupt).is_err());
            }
        }
        let line = report(
            Options {
                ids: [2, 1],
                count: 4,
                late: false,
                direct: true,
                settled: false,
            },
            "digest",
        );
        for field in [
            "schema=fe2o3.pending-segments-readback.v1",
            "completion_receipts=3",
            "native_counter=0,1",
            "pipeline_launches=1",
            "return_guards=not-applicable",
        ] {
            assert!(line.split_whitespace().any(|word| word == field));
        }
    }

    #[test]
    fn settled_segment_cli_requires_distinct_explicit_provenance_modes() {
        for flag in [
            "--settled-segments-compute",
            "--late-settled-segments-compute",
            "--settled-segments-readback",
        ] {
            for count in [4, 65, 4096] {
                let parsed =
                    options(&[flag.into(), count.to_string(), "0x2".into(), "0x1".into()]).unwrap();
                assert_eq!(
                    parsed,
                    Options {
                        ids: [2, 1],
                        count,
                        late: flag.starts_with("--late"),
                        direct: flag.ends_with("-readback"),
                        settled: true,
                    }
                );
            }
        }
        for arguments in [
            vec!["--settled-segments-compute", "0", "0x1", "0x2"],
            vec!["--settled-segments-compute", "4097", "0x1", "0x2"],
            vec!["--settled-segments-compute", "4", "0x1", "0x01"],
            vec!["--settled-segments-compute", "4", "0x0", "0x2"],
            vec!["--settled-segments-readback", "4", "1", "0x2"],
            vec!["--late-settled-segments-readback", "4", "0x1", "0x2"],
            vec![
                "--settled-segments-compute",
                "--segments-compute",
                "4",
                "0x1",
                "0x2",
            ],
            vec!["--late-settled-segments-compute", "4", "0x1"],
        ] {
            assert!(
                options(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
    }

    #[test]
    fn settled_segment_reports_change_provenance_not_the_complete_byte_oracle() {
        for count in [4, 65, 4096] {
            for (direct, late) in [(false, false), (false, true), (true, false)] {
                let output = if direct {
                    verify_direct(count, &expected_direct(count)).unwrap()
                } else {
                    verify(count, &expected(count)).unwrap()
                };
                let options = Options {
                    ids: [2, 1],
                    count,
                    late,
                    direct,
                    settled: true,
                };
                let line = report(options, &output);
                let pairs: Vec<_> = line
                    .split_whitespace()
                    .skip(1)
                    .map(|word| word.split_once('=').unwrap())
                    .collect();
                let fields: std::collections::BTreeMap<_, _> = pairs.iter().copied().collect();
                assert_eq!(pairs.len(), 52);
                assert_eq!(fields.len(), pairs.len());
                assert_eq!(
                    fields["schema"],
                    if direct {
                        "fe2o3.settled-segments-readback.v1"
                    } else {
                        "fe2o3.settled-segments-compute.v1"
                    }
                );
                assert_eq!(fields["source_admission"], "settled-result-released");
                assert_eq!(fields["source_results_at_list_admission"], "0");
                assert_eq!(fields["source_events_supplied"], "0");
                assert_eq!(fields["source_stream"], "empty-before-and-after-admission");
                assert_eq!(fields["setup_launches"], "3");
                assert_eq!(fields["pipeline_launches"], if direct { "0" } else { "1" });
                assert_eq!(
                    fields["completion_receipts"],
                    if direct { "2" } else { "4" }
                );
                assert_eq!(fields["retained_results"], fields["completion_receipts"]);
                assert_eq!(fields["pipeline_host_joins"], "0");
                assert_eq!(fields["output_sha256"], output);
                assert_eq!(fields["native_counter"], if direct { "0,1" } else { "0,2" });
                assert_eq!(
                    fields["results_release"],
                    if direct {
                        "readback-list"
                    } else {
                        "readback-peer-consumer-list"
                    }
                );
                assert_eq!(
                    fields["retained_native_counter"],
                    if late { "0,1,0" } else { "0,0" }
                );
                assert_eq!(fields["authority"], "qualification-r57-n3-v2");
                assert_eq!(fields["performance_acceptance"], "false");
                assert_eq!(fields["formal_refinement"], "false");
                let previous = report(
                    Options {
                        settled: false,
                        ..options
                    },
                    &output,
                );
                let previous: std::collections::BTreeMap<_, _> = previous
                    .split_whitespace()
                    .skip(1)
                    .map(|word| word.split_once('=').unwrap())
                    .collect();
                assert_eq!(previous.len(), 48);
                assert!(!previous.contains_key("source_admission"));
                for key in [
                    "output_sha256",
                    "digest",
                    "source_envelope",
                    "destination_envelope",
                    "descriptor_order",
                    "duplicates",
                    "caller_list",
                    "source_preservation",
                    "destination_frame",
                    "return_guards",
                    "host_guards",
                    "cleanup",
                ] {
                    assert_eq!(fields[key], previous[key], "oracle/ownership field {key}");
                }
            }
        }
    }
}
