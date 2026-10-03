//! Ordered gather into the original R57 C input, followed by its unchanged second gate.

use super::*;
use fe2o3_runtime::{RuntimePeerCopySegmentV1, RuntimePeerCopySegmentsV1};

const USAGE: &str = "usage: gfx942-runtime-deferred-peer-chain-smoke <--gather-compute|--gather-compute-overlap|--late-gather-compute|--late-gather-compute-overlap> <0xsource-id> <0xsource-id> [more source IDs] <0xsink-id>";
const SEGMENT_USAGE: &str = "usage: gfx942-runtime-deferred-peer-chain-smoke <--destination-segments-compute|--late-destination-segments-compute> <4|65|4096> <0xsource-id> <0xsource-id> <0xsink-id>";
const PENDING_SEGMENT_USAGE: &str = "usage: gfx942-runtime-deferred-peer-chain-smoke <--pending-destination-segments-compute|--late-pending-destination-segments-compute> <first|second|both> <4|65|4096> <0xsource-id> <0xsource-id> <0xsink-id>";

#[derive(Debug, Eq, PartialEq)]
struct Options {
    ids: Vec<u64>,
    overlap: bool,
    late: bool,
    segments: Option<usize>,
    pending_sources: u8,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    if matches!(
        arguments.first().map(String::as_str),
        Some(
            "--pending-destination-segments-compute"
                | "--late-pending-destination-segments-compute"
        )
    ) {
        if arguments.len() != 6 {
            return Err(PENDING_SEGMENT_USAGE.into());
        }
        let mask = match arguments[1].as_str() {
            "first" => 1,
            "second" => 2,
            "both" => 3,
            _ => return Err(PENDING_SEGMENT_USAGE.into()),
        };
        let mut settled = vec!["--destination-segments-compute".to_owned()];
        settled.extend_from_slice(&arguments[2..]);
        let mut parsed = options(&settled).map_err(|_| PENDING_SEGMENT_USAGE)?;
        parsed.pending_sources = mask;
        parsed.late = arguments[0] == "--late-pending-destination-segments-compute";
        return Ok(parsed);
    }
    if matches!(
        arguments.first().map(String::as_str),
        Some("--destination-segments-compute" | "--late-destination-segments-compute")
    ) {
        if arguments.len() != 5 {
            return Err(SEGMENT_USAGE.into());
        }
        let count = match arguments[1].as_str() {
            "4" => 4,
            "65" => 65,
            "4096" => 4096,
            _ => return Err(SEGMENT_USAGE.into()),
        };
        let mut legacy = vec!["--gather-compute".to_owned()];
        legacy.extend_from_slice(&arguments[2..]);
        let mut parsed = options(&legacy)?;
        parsed.late = arguments[0] == "--late-destination-segments-compute";
        parsed.segments = Some(count);
        return Ok(parsed);
    }
    let (overlap, late) = match arguments.first().map(String::as_str) {
        Some("--gather-compute") => (false, false),
        Some("--gather-compute-overlap") => (true, false),
        Some("--late-gather-compute") => (false, true),
        Some("--late-gather-compute-overlap") => (true, true),
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
    Ok(Options {
        ids,
        overlap,
        late,
        segments: None,
        pending_sources: 0,
    })
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
    finish_expected(vec![source; checked.len()], gathered)
}

fn finish_expected(mut sources: Vec<Vec<u8>>, gathered: Vec<u8>) -> Vec<Vec<u8>> {
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
    sources.extend([gathered, computed, returned, host]);
    sources
}

fn descriptors(source: usize, count: usize) -> Vec<RuntimePeerCopySegmentV1> {
    assert!(source < 2 && [4, 65, 4096].contains(&count));
    (0..count)
        .map(|index| {
            let (from, to, bytes) = match index {
                0 | 2 => (4 * (1 + source * 9), 64, 128),
                1 => (4 * (65 + source * 5), 96, 128),
                3 => (4 * (257 + source * 7), 192, 68),
                _ => (
                    4 * (1 + (index * 13 + source * 29) % 10_000),
                    4 * (257 + (index * 17 + source * 19) % 40_000),
                    4 * (1 + index % 17),
                ),
            };
            RuntimePeerCopySegmentV1 {
                source_offset: from as u64,
                destination_offset: to as u64,
                byte_len: bytes as u64,
            }
        })
        .collect()
}

fn expected_segments(count: usize) -> Vec<Vec<u8>> {
    let sources = vec![expected_c(), expected_d()];
    let mut gathered = filled(0.25);
    for (source, bytes) in sources.iter().enumerate() {
        for segment in descriptors(source, count) {
            let from = segment.source_offset as usize;
            let to = segment.destination_offset as usize;
            let len = segment.byte_len as usize;
            gathered[to..to + len].copy_from_slice(&bytes[from..from + len]);
        }
    }
    finish_expected(sources, gathered)
}

fn verify_segments(count: usize, snapshots: &[Vec<u8>]) -> ResultV1<String> {
    if snapshots != expected_segments(count) {
        return Err(failure(
            "destination-segments-bytes",
            "full snapshots differ",
        ));
    }
    let mut framed = b"fe2o3.destination-segments-compute.v1\0".to_vec();
    for bytes in snapshots {
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(bytes);
    }
    Ok(digest(&framed))
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
    segments: Option<usize>,
    pending_sources: u8,
    peer_stream: RuntimeStreamIdV1,
    return_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    returned: RuntimeAllocationIdV1,
    host: RuntimeAllocationIdV1,
}

impl Resources {
    fn source_count(&self) -> usize {
        self.runs.len() - 1
    }

    fn source_pending(&self, index: usize) -> bool {
        self.segments.is_none() || self.pending_sources & (1 << index) != 0
    }

    fn source_allocation(&self, index: usize) -> RuntimeAllocationIdV1 {
        self.runs[index].allocations[if self.segments.is_some() && index == 0 {
            2
        } else {
            3
        }]
    }
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
    for (index, &device) in devices.iter().enumerate() {
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
        if index != 0 || options.pending_sources & 1 == 0 {
            setup_compute(&mut context, &run, false, deadline)?;
        }
        // Preserve source C's actual device-produced identity for its second gate.
        runs.push(run);
    }
    if options.segments.is_some() {
        // Selected source gates are admitted only after setup, with their lists.
        if options.pending_sources & 2 == 0 {
            setup_compute(&mut context, &runs[1], true, deadline)?;
        }
        for (index, (run, allocation, expected)) in [
            (&runs[0], runs[0].allocations[2], expected_c()),
            (&runs[1], runs[1].allocations[3], expected_d()),
        ]
        .into_iter()
        .enumerate()
        {
            if context
                .query_stream(run.stream)
                .map_err(|error| failure("list-source-settled", error))?
                != RuntimeStreamObservationV1::default()
            {
                return Err(failure("list-source-settled", "retained setup result"));
            }
            // Native readback consumes H2dReady. Leave pending outputs ready for
            // their producing gate; the final oracle checks every source byte.
            if options.pending_sources & (1 << index) == 0 {
                verify_initial(&mut context, allocation, &expected)?;
            }
        }
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
            windows: if options.segments.is_none() {
                windows(devices.len() - 1, options.overlap)
            } else {
                Vec::new()
            },
            segments: options.segments,
            pending_sources: options.pending_sources,
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
    peers: Vec<Peer>,
    compute: RuntimeSubmissionV1<Arguments>,
    returned: RuntimeSubmissionV1<RuntimePeerCopyV1>,
    readback: RuntimeSubmissionV1<RuntimeCopyV1>,
    ids: Vec<RuntimeSubmissionIdV1>,
}

enum Peer {
    Scalar(RuntimeSubmissionV1<RuntimePeerCopyV1>),
    Segments(RuntimeSubmissionV1<RuntimePeerCopySegmentsV1>),
}

impl Peer {
    fn id(&self) -> RuntimeSubmissionIdV1 {
        match self {
            Self::Scalar(peer) => peer.id(),
            Self::Segments(peer) => peer.id(),
        }
    }

    fn status(&self, context: &Context) -> ResultV1<RuntimeCompletionStatusV1> {
        match self {
            Self::Scalar(peer) => context.query_submission(peer),
            Self::Segments(peer) => context.query_submission(peer),
        }
        .map_err(|error| failure("gather-peer-status", error))
    }

    fn observe(
        &self,
        context: &mut Context,
        receipts: &Arc<Mutex<Receipts>>,
    ) -> ResultV1<fe2o3_runtime::RuntimeEventIdV1> {
        match self {
            Self::Scalar(peer) => callback(context, peer, receipts)?,
            Self::Segments(peer) => callback(context, peer, receipts)?,
        }
        match self {
            Self::Scalar(peer) => context.record_event(peer),
            Self::Segments(peer) => context.record_event(peer),
        }
        .map_err(|error| failure("gather-peer-event", error))
    }

    fn require(&self, context: &Context, status: RuntimeCompletionStatusV1) -> ResultV1<()> {
        match self {
            Self::Scalar(peer) => require(context, peer, status),
            Self::Segments(peer) => require(context, peer, status),
        }
    }

    fn release(self, context: &mut Context) -> ResultV1<()> {
        match self {
            Self::Scalar(peer) => context
                .release_submission(peer)
                .map_err(|error| failure("gather-release-peer", error)),
            Self::Segments(peer) => context
                .release_submission(peer)
                .map_err(|error| failure("gather-release-peer", error)),
        }
    }
}

fn publication_gate(retained: usize, completed: u64, all_pending: bool) -> ResultV1<bool> {
    if completed != 0 || !all_pending || retained > 1 {
        return Err(failure(
            "gather-late-publication-state",
            (retained, completed, all_pending),
        ));
    }
    Ok(retained == 1)
}

fn publication_observation(context: &Context, peers: &[Peer]) -> ResultV1<bool> {
    if peers.len() < 2 {
        return Err(failure("gather-late-roster", "ordered peers required"));
    }
    let mut all_pending = true;
    for peer in peers {
        all_pending &= peer.status(context)? == RuntimeCompletionStatusV1::Pending;
    }
    publication_gate(
        context.backend().retained_compute_xgmi_copies_v1(),
        context.backend().completed_compute_xgmi_copies_v1(),
        all_pending,
    )
}

fn seed_oldest(
    context: &mut Context,
    peers: &[Peer],
    peer_stream: RuntimeStreamIdV1,
    deadline: Instant,
) -> ResultV1<()> {
    if publication_observation(context, peers)? {
        return Err(failure(
            "gather-late-baseline",
            "native peer already retained",
        ));
    }
    for _ in 0..TICKS {
        if Instant::now() >= deadline {
            break;
        }
        // KfdMulti permits one peer leaf per call, so publication and retirement
        // cannot occur in the same attempt. The actual custody check is decisive.
        context
            .progress_stream_v1(peer_stream)
            .map_err(|error| failure("gather-late-oldest-progress", error))?;
        if publication_observation(context, peers)? {
            // These are the only peer roots. Shared-destination ordering and zero
            // completions identify the oldest; the counter alone exposes no pair.
            return Ok(());
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    Err(failure(
        "gather-late-publication-deadline",
        "retained publication not observed",
    ))
}

fn admit(
    context: &mut Context,
    resources: &Resources,
    receipts: &Arc<Mutex<Receipts>>,
    late: bool,
    deadline: Instant,
) -> ResultV1<Chain> {
    let count = resources.source_count();
    let sink = &resources.runs[count];
    let mut ids = Vec::new();
    let mut sources = Vec::new();
    let mut events = vec![None; count];
    for (index, run) in resources.runs[..count].iter().enumerate() {
        if !resources.source_pending(index) {
            continue;
        }
        let first_gate = resources.segments.is_some() && index == 0;
        let arguments = Arguments::new(
            run.allocations[if first_gate { 0 } else { 2 }],
            run.allocations[1],
            resources.source_allocation(index),
        )
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
        events[index] = Some(
            context
                .record_event(&submission)
                .map_err(|error| failure("gather-source-event", error))?,
        );
        sources.push(submission);
    }
    let mut peers = Vec::new();
    let mut predecessor = None;
    for (index, source_event) in events.into_iter().enumerate() {
        let mut dependencies = source_event.into_iter().collect::<Vec<_>>();
        dependencies.extend(predecessor);
        let submission = if let Some(count) = resources.segments {
            let mut list = descriptors(index, count);
            let peer = context
                .peer_copy_segments(
                    resources.peer_stream,
                    region(resources.source_allocation(index), RuntimeAccessV1::Read),
                    region(sink.allocations[2], RuntimeAccessV1::Write),
                    &list,
                    &dependencies,
                )
                .map_err(|error| failure("destination-segments-peer", error))?;
            list.fill(RuntimePeerCopySegmentV1 {
                source_offset: u64::MAX,
                destination_offset: u64::MAX,
                byte_len: 0,
            });
            Peer::Segments(peer)
        } else {
            let window = resources.windows[index];
            Peer::Scalar(
                context
                    .peer_copy(
                        resources.peer_stream,
                        range(
                            resources.source_allocation(index),
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
                    .map_err(|error| failure("gather-peer", error))?,
            )
        };
        ids.push(submission.id());
        let event = submission.observe(context, receipts)?;
        if let Some(event) = source_event {
            context
                .release_event(event)
                .map_err(|error| failure("gather-source-event-release", error))?;
        }
        if let Some(event) = predecessor {
            context
                .release_event(event)
                .map_err(|error| failure("gather-peer-event-release", error))?;
        }
        predecessor = Some(event);
        peers.push(submission);
    }
    for submission in &sources {
        require(context, submission, RuntimeCompletionStatusV1::Pending)?;
    }
    if late {
        seed_oldest(context, &peers, resources.peer_stream, deadline)?;
    }
    let event = predecessor.ok_or("missing gather tail")?;
    let arguments = Arguments::new(
        sink.allocations[2],
        sink.allocations[1],
        sink.allocations[3],
    )
    .map_err(|error| failure("gather-consumer-arguments", error))?;
    if late && !publication_observation(context, &peers)? {
        return Err(failure(
            "gather-late-before-consumer",
            "native custody changed",
        ));
    }
    let compute = context
        .launch_producer_aware_v1(
            sink.stream,
            &sink.kernel,
            &arguments,
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[event],
        )
        .map_err(|error| failure("gather-consumer-compute", error))?;
    // Normal late admission authenticates the exact ancestor and both paired
    // reservations/InFlight slots; no witness-only native inspection is used.
    if late && !publication_observation(context, &peers)? {
        return Err(failure(
            "gather-late-after-consumer",
            "native custody changed",
        ));
    }
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
        if resources.pending_sources != 0 && late {
            match context.query_submission(submission) {
                Ok(RuntimeCompletionStatusV1::Pending | RuntimeCompletionStatusV1::Succeeded) => {}
                status => return Err(failure("pending-source-after-seed", status)),
            }
        } else {
            require(context, submission, RuntimeCompletionStatusV1::Pending)?;
        }
    }
    for submission in &peers {
        submission.require(context, RuntimeCompletionStatusV1::Pending)?;
    }
    require(context, &compute, RuntimeCompletionStatusV1::Pending)?;
    require(context, &returned, RuntimeCompletionStatusV1::Pending)?;
    require(context, &readback, RuntimeCompletionStatusV1::Pending)?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure(
            "gather-admission",
            "peer completed before final-readback progress",
        ));
    }
    if late && !publication_observation(context, &peers)? {
        return Err(failure("gather-late-admission", "native custody changed"));
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

fn pipeline_wait(segments: Option<usize>) -> Duration {
    match segments {
        // Two serialized maximum-size lists retain the prior allowance per list.
        Some(4096) => Duration::from_secs(360),
        Some(_) => Duration::from_secs(180),
        None => WAIT,
    }
}

fn pipeline(
    engine: &mut Engine,
    handle: &Handle,
    resources: Arc<Resources>,
    late: bool,
) -> ResultV1<String> {
    let pipeline_wait = pipeline_wait(resources.segments);
    let pipeline_ticks = if resources.segments.is_some() {
        60_000
    } else {
        TICKS
    };
    let started = Instant::now();
    let deadline = started + pipeline_wait;
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
        admit(context, &owned, &observed, late, deadline)
    })?;
    let mut completed = false;
    let mut completed_polls = 0;
    let mut last_snapshot = None;
    for tick in 0..pipeline_ticks {
        if Instant::now() >= deadline {
            break;
        }
        let sample = resources.segments.is_some() && tick % 256 == 0;
        let result = command(engine, handle, deadline, "gather-poll", move |context| {
            let complete = match context
                .poll(&mut chain.readback)
                .map_err(|error| failure("gather-final-poll", error))?
            {
                RuntimePollV1::Succeeded => true,
                RuntimePollV1::Pending => false,
                status => return Err(failure("gather-final-status", status)),
            };
            let snapshot = sample.then(|| {
                format!(
                    "elapsed={:?} native_completed={} native_retained={} peers={:?} compute={:?} return={:?} readback={:?}",
                    started.elapsed(),
                    context.backend().completed_compute_xgmi_copies_v1(),
                    context.backend().retained_compute_xgmi_copies_v1(),
                    chain.peers.iter().map(|peer| (peer.id(), peer.status(context))).collect::<Vec<_>>(),
                    (chain.compute.id(), context.query_submission(&chain.compute)),
                    (chain.returned.id(), context.query_submission(&chain.returned)),
                    (chain.readback.id(), context.query_submission(&chain.readback)),
                )
            });
            Ok((chain, complete, snapshot))
        })?;
        chain = result.0;
        completed_polls += 1;
        if result.2.is_some() {
            last_snapshot = result.2;
        }
        if result.1 {
            completed = true;
            break;
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    if !completed {
        return Err(failure(
            "gather-deadline",
            (
                pipeline_wait,
                started.elapsed(),
                completed_polls,
                last_snapshot,
            ),
        ));
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
                submission.require(context, RuntimeCompletionStatusV1::Succeeded)?;
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
                != (owned.source_count() + 1) as u64
                || context.backend().retained_compute_xgmi_copies_v1() != 0
            {
                return Err(failure(
                    "gather-native-counter",
                    "completion or retained native count differs",
                ));
            }
            let sink = owned.runs.last().ok_or("missing sink")?;
            let layout = ReturnLayout::new(true);
            let mut allocations: Vec<_> = (0..owned.source_count())
                .map(|index| (owned.source_allocation(index), BYTES))
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
                submission.release(context)?;
            }
            for submission in chain.sources {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("gather-release-source", error))?;
            }
            Ok(snapshots)
        },
    )?;
    let output = match resources.segments {
        Some(count) => verify_segments(count, &snapshots)?,
        None => verify(&resources.windows, &snapshots)?,
    };
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
    if let Some(segments) = options.segments {
        let (schema, pipeline, producers, source_scope) = match options.pending_sources {
            0 => (
                "fe2o3.destination-segments-compute.v1",
                "settled-list-list-compute-peer-readback",
                "completed-and-released-before-list-admission",
                String::new(),
            ),
            mask => (
                "fe2o3.pending-destination-segments-compute.v1",
                "source-compute-list-list-compute-peer-readback",
                "selected-pending-others-completed-and-released-at-list-admission",
                format!(
                    " pending_sources={}",
                    match mask {
                        1 => "first",
                        2 => "second",
                        3 => "both",
                        _ => unreachable!(),
                    }
                ),
            ),
        };
        let pending_count = options.pending_sources.count_ones();
        let (admission, progress, publication) = if options.late {
            (
                "lists-preadmitted-consumer-after-oldest-publication",
                "oldest-list-seed-then-final-readback-stream-only",
                " publication_observed=true publication_identity=ordered-roster-inference paired_custody=validated-by-consumer-admission retained_native_at_consumer_admission=1 retained_native_counter=0,1,0 publication_capture=single-peer-leaf-quantum oldest_seed=bounded-context-progress",
            )
        } else {
            (
                "all-before-explicit-progress",
                "final-readback-stream-only",
                "",
            )
        };
        return format!(
            "PASS schema={schema} authority=qualification-r57-n3-v2 devices=3 sources=2 unique_ids={ids} lists=2 segments_per_list={segments} descriptors={} elements={ELEMENTS} bytes={BYTES} setup_launches={} pipeline_launches={} peer_copies=3 dependent_readbacks=1 completion_receipts={} pipeline={pipeline} source_payloads=C,D source_producers={producers} admission={admission} progress={progress} public_events=released-after-dependent-admission consumer_dependencies=latest-list-only consumer_bindings=full-frame-read-stable-read-full-write descriptor_snapshot=caller-overwritten-after-admission native_transport=NATIVE-XGMI native_counter=0,3 output=full-byte-pass source_preservation=full-byte-pass gathered_frame=full-byte-pass return_guards=full-byte-pass host_guards=full-byte-pass output_sha256={output} digest=domain-and-u64le-length-prefixed-C-source-D-source-C-D-E-host host_output_installations=0 pipeline_host_joins=0 journal=enabled contexts=1 owners=1 batches=1 results_release=reverse-dependencies final_drain=completed-only cleanup=owned-shutdown-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false{publication}{source_scope}",
            segments * 2,
            4 - pending_count,
            1 + pending_count,
            5 + pending_count,
        );
    }
    let windows = windows(count, options.overlap)
        .iter()
        .map(|window| format!("{}:{}:{}", window.source, window.destination, window.bytes))
        .collect::<Vec<_>>()
        .join(",");
    let (schema, admission, progress, publication) = if options.late {
        (
            "fe2o3.late-gather-compute.v2",
            "gather-preadmitted-consumer-after-oldest-publication",
            "oldest-peer-seed-then-final-readback-stream-only",
            " publication_observed=true publication_identity=ordered-roster-inference paired_custody=validated-by-consumer-admission retained_native_at_consumer_admission=1 retained_native_counter=0,1,0 publication_capture=single-peer-leaf-quantum oldest_seed=bounded-context-progress",
        )
    } else {
        (
            "fe2o3.gather-compute.v1",
            "all-before-explicit-progress",
            "final-readback-stream-only",
            "",
        )
    };
    format!(
        "PASS schema={schema} authority=qualification-r57-n3-v2 devices={} sources={count} unique_ids={ids} overlap={} windows={windows} elements={ELEMENTS} bytes={BYTES} setup_launches={} pipeline_launches={} peer_copies={} dependent_readbacks=1 completion_receipts={} pipeline=compute-gather-compute-peer-readback admission={admission} progress={progress} public_events=released-after-dependent-admission consumer_dependencies=latest-gather-only consumer_bindings=full-frame-read-stable-read-full-write native_transport=NATIVE-XGMI native_counter=0,{} output=full-byte-pass source_preservation=full-byte-pass gathered_frame=full-byte-pass return_guards=full-byte-pass host_guards=full-byte-pass output_sha256={output} digest=length-prefixed-sources-C-D-E-host host_output_installations=0 pipeline_host_joins=0 journal=enabled contexts=1 owners=1 batches=1 results_release=reverse-dependencies final_drain=completed-only cleanup=owned-shutdown-explicit physical_overlap=unmeasured performance_acceptance=false formal_refinement=false{publication}",
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
    let result = pipeline(&mut engine, &handle, resources, options.late);
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
    fn destination_segment_cli_keeps_two_settled_sources_and_three_devices_explicit() {
        assert_eq!(pipeline_wait(None), WAIT);
        for mode in [
            "--destination-segments-compute",
            "--late-destination-segments-compute",
        ] {
            for count in [4, 65, 4096] {
                let parsed = options(&[
                    mode.into(),
                    count.to_string(),
                    "0x3".into(),
                    "0x2".into(),
                    "0x1".into(),
                ])
                .unwrap();
                assert_eq!(parsed.ids, [3, 2, 1]);
                assert_eq!(parsed.segments, Some(count));
                assert_eq!(parsed.pending_sources, 0);
                assert_eq!(parsed.late, mode.starts_with("--late-"));
                assert!(!parsed.overlap);
                assert_eq!(
                    pipeline_wait(parsed.segments),
                    Duration::from_secs(if count == 4096 { 360 } else { 180 }),
                );
            }
            for suffix in [
                vec!["4", "0x1", "0x2"],
                vec!["4", "0x1", "0x2", "0x3", "0x4"],
                vec!["4", "0x1", "0x2", "0x01"],
                vec!["4", "0x0", "0x2", "0x3"],
                vec!["3", "0x1", "0x2", "0x3"],
                vec!["4097", "0x1", "0x2", "0x3"],
                vec!["04", "0x1", "0x2", "0x3"],
                vec!["4", "0x1", "0x2", "--gather-compute"],
            ] {
                let arguments = std::iter::once(mode)
                    .chain(suffix)
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                assert!(options(&arguments).is_err());
            }
        }
    }

    #[test]
    fn pending_destination_segment_cli_preserves_each_selected_source_gate() {
        for mode in [
            "--pending-destination-segments-compute",
            "--late-pending-destination-segments-compute",
        ] {
            for (selector, mask) in [("first", 1), ("second", 2), ("both", 3)] {
                for count in [4, 65, 4096] {
                    let parsed = options(&[
                        mode.into(),
                        selector.into(),
                        count.to_string(),
                        "0x3".into(),
                        "0x2".into(),
                        "0x1".into(),
                    ])
                    .unwrap();
                    assert_eq!(parsed.ids, [3, 2, 1]);
                    assert_eq!(parsed.pending_sources, mask);
                    assert_eq!(parsed.segments, Some(count));
                    assert_eq!(parsed.late, mode.starts_with("--late-"));
                    assert!(!parsed.overlap);
                }
            }
            for suffix in [
                vec!["none", "4", "0x1", "0x2", "0x3"],
                vec!["both", "4", "0x1", "0x2"],
                vec!["both", "4", "0x1", "0x2", "0x3", "0x4"],
                vec!["first", "4", "0x1", "0x2", "0x01"],
                vec!["second", "4097", "0x1", "0x2", "0x3"],
                vec!["4", "first", "0x1", "0x2", "0x3"],
            ] {
                let args = std::iter::once(mode)
                    .chain(suffix)
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                assert!(options(&args).is_err());
            }
        }
    }

    #[test]
    fn pending_destination_segment_report_counts_real_source_work_without_new_authority() {
        for late in [false, true] {
            for (pending_sources, name, pending_count) in
                [(1, "first", 1), (2, "second", 1), (3, "both", 2)]
            {
                let parsed = Options {
                    ids: vec![1, 2, 3],
                    overlap: false,
                    late,
                    segments: Some(65),
                    pending_sources,
                };
                let line = report(&parsed, &"0".repeat(64));
                let fields: std::collections::BTreeMap<_, _> = line
                    .strip_prefix("PASS ")
                    .unwrap()
                    .split_whitespace()
                    .map(|field| field.split_once('=').unwrap())
                    .collect();
                assert_eq!(fields.len(), line.split_whitespace().count() - 1);
                assert_eq!(
                    fields["schema"],
                    "fe2o3.pending-destination-segments-compute.v1"
                );
                assert_eq!(fields["pending_sources"], name);
                assert_eq!(fields["setup_launches"], (4 - pending_count).to_string());
                assert_eq!(fields["pipeline_launches"], (1 + pending_count).to_string());
                assert_eq!(
                    fields["completion_receipts"],
                    (5 + pending_count).to_string()
                );
                assert_eq!(
                    fields["source_producers"],
                    "selected-pending-others-completed-and-released-at-list-admission"
                );
                assert_eq!(fields["consumer_dependencies"], "latest-list-only");
                assert_eq!(fields["native_counter"], "0,3");
                assert_eq!(fields["authority"], "qualification-r57-n3-v2");
                assert_eq!(fields["host_output_installations"], "0");
                assert_eq!(fields["pipeline_host_joins"], "0");
                assert_eq!(fields["formal_refinement"], "false");
                assert_eq!(fields["performance_acceptance"], "false");
                assert_eq!(fields.contains_key("publication_observed"), late);
            }
        }
    }

    #[test]
    fn destination_segment_oracle_distinguishes_sources_list_order_and_full_guards() {
        for count in [4, 65, 4096] {
            let snapshots = expected_segments(count);
            assert_eq!(snapshots.len(), 6);
            assert_ne!(snapshots[0], snapshots[1]);
            assert_eq!(&snapshots[2][..64], &filled(0.25)[..64]);
            assert_eq!(&snapshots[2][BYTES - 4..], &0.25_f32.to_le_bytes());
            let mut reversed = filled(0.25);
            for source in (0..2).rev() {
                for segment in descriptors(source, count) {
                    let from = segment.source_offset as usize;
                    let to = segment.destination_offset as usize;
                    let len = segment.byte_len as usize;
                    assert!(from.is_multiple_of(4) && to.is_multiple_of(4));
                    assert!(len.is_multiple_of(4) && from + len <= BYTES && to + len <= BYTES);
                    reversed[to..to + len].copy_from_slice(&snapshots[source][from..from + len]);
                }
            }
            assert_ne!(reversed, snapshots[2]);
            assert_eq!(descriptors(0, count)[0], descriptors(0, count)[2]);
            assert_eq!(verify_segments(count, &snapshots).unwrap().len(), 64);
            for index in 0..snapshots.len() {
                for offset in [0, snapshots[index].len() - 1] {
                    let mut corrupt = snapshots.clone();
                    corrupt[index][offset] ^= 1;
                    assert!(verify_segments(count, &corrupt).is_err());
                }
            }
        }
    }

    #[test]
    fn destination_segment_report_does_not_invent_pending_source_or_kernel_authority() {
        for late in [false, true] {
            let options = Options {
                ids: vec![1, 2, 3],
                overlap: false,
                late,
                segments: Some(65),
                pending_sources: 0,
            };
            let line = report(&options, &"0".repeat(64));
            let fields: std::collections::BTreeMap<_, _> = line
                .strip_prefix("PASS ")
                .unwrap()
                .split_whitespace()
                .map(|field| field.split_once('=').unwrap())
                .collect();
            assert_eq!(fields.len(), line.split_whitespace().count() - 1);
            assert_eq!(fields["schema"], "fe2o3.destination-segments-compute.v1");
            assert_eq!(fields["authority"], "qualification-r57-n3-v2");
            assert_eq!(fields["source_payloads"], "C,D");
            assert_eq!(fields["segments_per_list"], "65");
            assert_eq!(fields["descriptors"], "130");
            assert_eq!(fields["setup_launches"], "4");
            assert_eq!(fields["pipeline_launches"], "1");
            assert_eq!(fields["completion_receipts"], "5");
            assert_eq!(fields["native_counter"], "0,3");
            assert_eq!(fields["consumer_dependencies"], "latest-list-only");
            assert_eq!(fields["pipeline_host_joins"], "0");
            assert_eq!(fields["performance_acceptance"], "false");
            assert_eq!(fields["formal_refinement"], "false");
            assert_eq!(fields.contains_key("publication_observed"), late);
            assert!(!fields.contains_key("windows"));
        }
    }

    #[test]
    fn gather_compute_cli_preserves_sources_and_sink_without_new_authority() {
        for mode in ["--gather-compute", "--gather-compute-overlap"] {
            let parsed = options(&[mode.into(), "0x3".into(), "0x2".into(), "0x1".into()]).unwrap();
            assert_eq!(parsed.ids, [3, 2, 1]);
            assert_eq!(parsed.overlap, mode.ends_with("overlap"));
            assert!(!parsed.late);
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
    fn late_gather_compute_cli_is_distinct_and_preserves_the_complete_roster() {
        for mode in ["--late-gather-compute", "--late-gather-compute-overlap"] {
            for devices in 3..=8 {
                let mut arguments = vec![mode.to_owned()];
                arguments.extend((1..=devices).rev().map(|id| format!("0x{id:x}")));
                let parsed = options(&arguments).unwrap();
                assert_eq!(parsed.ids, (1..=devices).rev().collect::<Vec<_>>());
                assert!(parsed.late);
                assert_eq!(parsed.overlap, mode.ends_with("overlap"));
            }
            for suffix in [
                vec!["0x1", "0x2"],
                vec!["0x1", "0x2", "0x01"],
                vec!["0x1", "0x2", "0x0"],
                vec!["0x1", "0x2", "0x10000000000000000"],
                vec!["0x1", "0x2", "0xg"],
                vec!["0x1", "0x2", "--gather-compute"],
                vec!["0x1", "0x2", "0x3", "--late-gather-compute"],
                vec![
                    "0x1", "0x2", "0x3", "0x4", "0x5", "0x6", "0x7", "0x8", "0x9",
                ],
            ] {
                let arguments = std::iter::once(mode)
                    .chain(suffix)
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                assert!(options(&arguments).is_err());
            }
        }
    }

    #[test]
    fn late_gather_publication_gate_never_infers_publication_from_pending_alone() {
        assert!(!publication_gate(0, 0, true).unwrap());
        assert!(publication_gate(1, 0, true).unwrap());
        for (retained, completed, pending) in [
            (0, 1, true),
            (1, 1, true),
            (2, 0, true),
            (usize::MAX, 0, true),
            (0, 0, false),
            (1, 0, false),
            (1, u64::MAX, true),
        ] {
            assert!(publication_gate(retained, completed, pending).is_err());
        }
    }

    #[test]
    fn late_gather_report_keeps_the_full_oracle_and_distinguishes_publication_evidence() {
        fn fields(line: &str) -> std::collections::BTreeMap<&str, &str> {
            let fields: std::collections::BTreeMap<_, _> = line
                .strip_prefix("PASS ")
                .unwrap()
                .split_whitespace()
                .map(|field| field.split_once('=').unwrap())
                .collect();
            assert_eq!(fields.len(), line.split_whitespace().count() - 1);
            fields
        }
        for devices in [3, 4, 8] {
            for overlap in [false, true] {
                let mut options = Options {
                    ids: (1..=devices).collect(),
                    overlap,
                    late: false,
                    segments: None,
                    pending_sources: 0,
                };
                let checked = windows(options.ids.len() - 1, overlap);
                let oracle = verify(&checked, &expected(&checked)).unwrap();
                let old = report(&options, &oracle);
                options.late = true;
                let late = report(&options, &oracle);
                let old = fields(&old);
                let late = fields(&late);
                assert_eq!(old["schema"], "fe2o3.gather-compute.v1");
                assert_eq!(old["admission"], "all-before-explicit-progress");
                assert_eq!(old["progress"], "final-readback-stream-only");
                assert_eq!(late["schema"], "fe2o3.late-gather-compute.v2");
                assert_eq!(
                    late["admission"],
                    "gather-preadmitted-consumer-after-oldest-publication"
                );
                assert_eq!(
                    late["progress"],
                    "oldest-peer-seed-then-final-readback-stream-only"
                );
                let new_fields = [
                    ("publication_observed", "true"),
                    ("publication_identity", "ordered-roster-inference"),
                    ("paired_custody", "validated-by-consumer-admission"),
                    ("retained_native_at_consumer_admission", "1"),
                    ("retained_native_counter", "0,1,0"),
                    ("publication_capture", "single-peer-leaf-quantum"),
                    ("oldest_seed", "bounded-context-progress"),
                ];
                assert_eq!(late.len(), old.len() + new_fields.len());
                for (name, value) in new_fields {
                    assert_eq!(late[name], value);
                    assert!(!old.contains_key(name));
                }
                for (name, value) in old {
                    if !["schema", "admission", "progress"].contains(&name) {
                        assert_eq!(late[name], value);
                    }
                }
                assert_eq!(late["output_sha256"], oracle);
                assert_eq!(late["pipeline_host_joins"], "0");
                assert_eq!(late["formal_refinement"], "false");
            }
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
