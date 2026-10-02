//! Unchanged R57 V2 peer -> deferred compute -> peer -> readback qualification.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_runtime::qualification_gfx942_r57_n3_v1::{
    GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1, GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1,
    GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1, GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
    Gfx942R57N3QualificationArgumentsV2 as Arguments, admit_gfx942_r57_n3_qualification_v2,
};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, RuntimeAccessV1, RuntimeAllocationIdV1,
    RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncDrainOutcomeV1, RuntimeAsyncEngineConfigV1,
    RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1, RuntimeAsyncProgressHandleV1,
    RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeCopyV1, RuntimeDirectedScalarPeerCopyV1,
    RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimePeerCopyV1, RuntimePollV1,
    RuntimeStreamIdV1, RuntimeStreamObservationV1, RuntimeSubmissionIdV1, RuntimeSubmissionV1,
    TypedRuntimeKernelV1,
};
use sha2::{Digest, Sha256};

#[path = "deferred_peer_chain/data.rs"]
mod data;
use data::*;

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<KfdMultiDeviceRuntimeBackendV1>;
type Handle = RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const WAIT: Duration = Duration::from_secs(30);
const TICKS: usize = 30_000;
const USAGE: &str = "usage: gfx942-runtime-deferred-peer-chain-smoke [--late-compute] [--return-window] <0xsource-unique-id> <0xdestination-unique-id>";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    ids: [u64; 2],
    late_compute: bool,
    return_window: bool,
}

fn options(arguments: &[String]) -> ResultV1<Options> {
    let mut late_compute = false;
    let mut return_window = false;
    let mut flags = 0;
    for argument in arguments {
        match argument.as_str() {
            "--late-compute" if !late_compute => late_compute = true,
            "--return-window" if !return_window => return_window = true,
            value if value.starts_with("--") => return Err(USAGE.into()),
            _ => break,
        }
        flags += 1;
    }
    Ok(Options {
        ids: unique_ids(&arguments[flags..])?,
        late_compute,
        return_window,
    })
}

struct DeviceRun {
    stream: RuntimeStreamIdV1,
    upload: RuntimeAllocationIdV1,
    allocations: [RuntimeAllocationIdV1; 4],
    kernel: TypedRuntimeKernelV1<Arguments>,
}

struct Resources {
    runs: [DeviceRun; 2],
    initial_peer_stream: RuntimeStreamIdV1,
    return_peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    returned: RuntimeAllocationIdV1,
    host_output: RuntimeAllocationIdV1,
    layout: ReturnLayout,
}

enum InitialPeer {
    Ordinary(RuntimeSubmissionV1<RuntimePeerCopyV1>),
    Directed(RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>),
}

impl InitialPeer {
    fn id(&self) -> RuntimeSubmissionIdV1 {
        match self {
            Self::Ordinary(submission) => submission.id(),
            Self::Directed(submission) => submission.id(),
        }
    }

    fn require(&self, context: &Context, status: RuntimeCompletionStatusV1) -> ResultV1<()> {
        match self {
            Self::Ordinary(submission) => require(context, submission, status),
            Self::Directed(submission) => require(context, submission, status),
        }
    }

    fn release(self, context: &mut Context) -> ResultV1<()> {
        match self {
            Self::Ordinary(submission) => context
                .release_submission(submission)
                .map_err(|error| failure("initial-peer-release", error)),
            Self::Directed(submission) => context
                .release_submission(submission)
                .map_err(|error| failure("initial-peer-release", error)),
        }
    }
}

struct Chain {
    initial_peer: InitialPeer,
    compute: RuntimeSubmissionV1<Arguments>,
    return_peer: RuntimeSubmissionV1<RuntimePeerCopyV1>,
    readback: RuntimeSubmissionV1<RuntimeCopyV1>,
    ids: [RuntimeSubmissionIdV1; 4],
}

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let detail = format!("stage={stage} {error:?}");
    eprintln!("deferred peer chain diagnostic: {detail}");
    detail
}

fn region(allocation: RuntimeAllocationIdV1, access: RuntimeAccessV1) -> RuntimeMemoryRegionV1 {
    range(allocation, access, 0, BYTES)
}

fn range(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: usize,
    bytes: usize,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset as u64,
        byte_len: bytes as u64,
    }
}

fn verify_initial(
    context: &mut Context,
    allocation: RuntimeAllocationIdV1,
    expected: &[u8],
) -> ResultV1<()> {
    let mut observed = vec![0; expected.len()];
    context
        .read_allocation(allocation, 0, &mut observed)
        .map_err(|error| failure("initial-readback", error))?;
    if observed != expected {
        return Err(failure("initial-bytes", "full initial extent differs"));
    }
    Ok(())
}

fn join_setup<A>(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    submission: &mut RuntimeSubmissionV1<A>,
    deadline: Instant,
) -> ResultV1<()> {
    for _ in 0..TICKS {
        if Instant::now() >= deadline {
            break;
        }
        context
            .flush_stream(stream)
            .map_err(|error| failure("setup-flush", error))?;
        match context
            .poll(submission)
            .map_err(|error| failure("setup-poll", error))?
        {
            RuntimePollV1::Succeeded => return Ok(()),
            RuntimePollV1::Pending => std::thread::sleep(Duration::from_micros(50)),
            status => return Err(failure("setup-status", status)),
        }
    }
    Err(failure(
        "setup-deadline",
        "bounded setup progress exhausted",
    ))
}

fn upload(
    context: &mut Context,
    stream: RuntimeStreamIdV1,
    host: RuntimeAllocationIdV1,
    device: RuntimeAllocationIdV1,
    bytes: &[u8],
    deadline: Instant,
) -> ResultV1<()> {
    if bytes.is_empty() {
        return Err(failure("upload-extent", bytes.len()));
    }
    context
        .write_allocation(host, 0, bytes)
        .map_err(|error| failure("upload-write", error))?;
    verify_initial(context, host, bytes)?;
    let mut submission = context
        .copy_async(
            stream,
            range(host, RuntimeAccessV1::Read, 0, bytes.len()),
            range(device, RuntimeAccessV1::Write, 0, bytes.len()),
            &[],
        )
        .map_err(|error| failure("upload-admission", error))?;
    join_setup(context, stream, &mut submission, deadline)?;
    context
        .release_submission(submission)
        .map_err(|error| failure("upload-release", error))
}

fn setup_compute(
    context: &mut Context,
    run: &DeviceRun,
    second: bool,
    deadline: Instant,
) -> ResultV1<()> {
    let [a, b, c, d] = run.allocations;
    let arguments = Arguments::new(if second { c } else { a }, b, if second { d } else { c })
        .map_err(|error| failure("setup-arguments", error))?;
    let mut submission = context
        .launch(
            run.stream,
            &run.kernel,
            &arguments,
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[],
        )
        .map_err(|error| failure("setup-compute", error))?;
    join_setup(context, run.stream, &mut submission, deadline)?;
    context
        .release_submission(submission)
        .map_err(|error| failure("setup-compute-release", error))
}

fn setup(options: Options) -> ResultV1<(ManuallyDrop<Context>, Arc<Resources>)> {
    let ids = options.ids;
    let layout = ReturnLayout::new(options.return_window);
    let admitted =
        admit_gfx942_r57_n3_qualification_v2().map_err(|error| failure("artifact", error))?;
    let fixture = admitted
        .host_buffers()
        .map_err(|error| failure("fixture", error))?;
    let inputs = inputs();
    if BYTES != GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1
        || ELEMENTS != GFX942_R57_N3_QUALIFICATION_ELEMENTS_V1
        || BYTES.div_ceil(PAGE) * PAGE != BYTES
        || GFX942_R57_N3_QUALIFICATION_ARGUMENTS_V1[2].access != RuntimeAccessV1::Write
        || inputs.iter().map(Vec::as_slice).ne([
            fixture.a(),
            fixture.b(),
            fixture.c_initial(),
            fixture.d_initial(),
        ])
    {
        return Err(failure(
            "fixture",
            "independent inputs or full Write shape differ",
        ));
    }
    let backend = KfdMultiDeviceRuntimeBackendV1::open_gfx942_r57_n3_peer_qualification_v2(&ids)
        .map_err(|error| failure("device-admission", error))?;
    // A failed setup cannot unwind over native owners; the bounded process retains them.
    let mut context = ManuallyDrop::new(
        Context::open_with_version_journal_members_v1(backend, 32, 32, 32)
            .map_err(|error| failure("context-open", error))?,
    );
    if context.devices().len() != 2
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure(
            "devices",
            "exact ordered two-device gfx942:xnack- roster required",
        ));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let deadline = Instant::now() + WAIT;
    let mut runs = Vec::with_capacity(2);
    for &device in &devices {
        let stream = context
            .create_stream(device)
            .map_err(|error| failure("compute-stream", error))?;
        let module = context
            .load_module(device, admitted.hsaco())
            .map_err(|error| failure("module", error))?;
        let kernel = context
            .resolve_kernel::<Arguments>(module, admitted.kernel_name())
            .map_err(|error| failure("kernel", error))?;
        let host = context
            .allocate(
                device,
                RuntimeMemoryKindV1::HostVisible,
                BYTES as u64,
                PAGE as u64,
            )
            .map_err(|error| failure("upload-host", error))?;
        let mut allocations = Vec::with_capacity(4);
        for bytes in &inputs {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    BYTES as u64,
                    PAGE as u64,
                )
                .map_err(|error| failure("compute-allocation", error))?;
            upload(&mut context, stream, host, allocation, bytes, deadline)?;
            allocations.push(allocation);
        }
        runs.push(DeviceRun {
            stream,
            upload: host,
            allocations: allocations
                .try_into()
                .map_err(|_| "four compute allocations required")?,
            kernel,
        });
    }
    // Complete the source gate before the pipeline; destination's second gate stays pending.
    setup_compute(&mut context, &runs[0], false, deadline)?;
    setup_compute(&mut context, &runs[1], false, deadline)?;
    setup_compute(&mut context, &runs[0], true, deadline)?;
    verify_initial(&mut context, runs[0].allocations[2], &expected_c())?;
    upload(
        &mut context,
        runs[1].stream,
        runs[1].upload,
        runs[1].allocations[2],
        &filled(-1.0),
        deadline,
    )?;
    verify_initial(&mut context, runs[1].allocations[2], &filled(-1.0))?;
    // The auxiliary return destination has no kernel authority and distinct identity.
    let returned = context
        .allocate(
            devices[0],
            RuntimeMemoryKindV1::DeviceLocal,
            layout.returned_bytes as u64,
            PAGE as u64,
        )
        .map_err(|error| failure("return-allocation", error))?;
    let host_output = context
        .allocate(
            devices[0],
            RuntimeMemoryKindV1::HostVisible,
            layout.host_bytes as u64,
            PAGE as u64,
        )
        .map_err(|error| failure("output-host", error))?;
    upload(
        &mut context,
        runs[0].stream,
        host_output,
        returned,
        &layout.returned_initial(),
        deadline,
    )?;
    verify_initial(&mut context, returned, &layout.returned_initial())?;
    if options.return_window {
        let initial = vec![0x5a; layout.host_bytes];
        context
            .write_allocation(host_output, 0, &initial)
            .map_err(|error| failure("host-sentinel", error))?;
        verify_initial(&mut context, host_output, &initial)?;
    }
    let initial_peer_stream = context
        .create_stream(devices[1])
        .map_err(|error| failure("initial-peer-stream", error))?;
    let return_peer_stream = context
        .create_stream(devices[0])
        .map_err(|error| failure("return-peer-stream", error))?;
    let readback_stream = context
        .create_stream(devices[0])
        .map_err(|error| failure("readback-stream", error))?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure("setup-native-count", "unexpected peer completion"));
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

fn command<T: Send + 'static>(
    engine: &mut Engine,
    handle: &Handle,
    deadline: Instant,
    stage: &str,
    operation: impl FnOnce(&mut Context) -> ResultV1<T> + Send + 'static,
) -> ResultV1<T> {
    let mut future = Box::pin(
        handle
            .observer()
            .enqueue_with_context(operation)
            .map_err(|error| failure(stage, error))?,
    );
    let result = engine.drive_until_ready(future.as_mut(), deadline);
    drop(future);
    result
        .map_err(|error| failure(stage, error))?
        .map_err(|error| failure(stage, error))?
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
            receipts
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .record(index, id, status);
        })
        .map_err(|error| failure("callback", error))
}

fn require<A>(
    context: &Context,
    submission: &RuntimeSubmissionV1<A>,
    status: RuntimeCompletionStatusV1,
) -> ResultV1<()> {
    let observed = context
        .query_submission(submission)
        .map_err(|error| failure("submission-status", error))?;
    if observed != status {
        return Err(failure("submission-status", observed));
    }
    Ok(())
}

fn admit_chain(
    context: &mut Context,
    resources: &Resources,
    receipts: &Arc<Mutex<Receipts>>,
    late_compute: bool,
    deadline: Instant,
) -> ResultV1<Chain> {
    let [source, destination] = &resources.runs;
    if late_compute
        && (context.backend().retained_compute_xgmi_copies_v1() != 0
            || context.backend().completed_compute_xgmi_copies_v1() != 0)
    {
        return Err(failure(
            "late-baseline",
            "no earlier native peer may remain",
        ));
    }
    let (first, event) = if late_compute {
        let mut first = context
            .directed_peer_copy_v1(
                resources.initial_peer_stream,
                region(source.allocations[2], RuntimeAccessV1::Read),
                region(destination.allocations[2], RuntimeAccessV1::Write),
                &[],
            )
            .map_err(|error| failure("initial-directed-peer", error))?;
        callback(context, &first, 0, receipts)?;
        let event = context
            .record_event(&first)
            .map_err(|error| failure("initial-peer-event", error))?;
        let mut published = false;
        for _ in 0..TICKS {
            if Instant::now() >= deadline {
                break;
            }
            let status = context
                .progress_directed_peer_copy_v1(&mut first)
                .map_err(|error| failure("initial-peer-publication", error))?;
            if status != RuntimePollV1::Pending
                || context.backend().completed_compute_xgmi_copies_v1() != 0
            {
                return Err(failure("late-publication-status", status));
            }
            match context.backend().retained_compute_xgmi_copies_v1() {
                0 => std::thread::sleep(Duration::from_micros(50)),
                1 => {
                    published = true;
                    break;
                }
                count => return Err(failure("late-retained-count", count)),
            }
        }
        if !published {
            return Err(failure(
                "late-publication-deadline",
                "retained publication not observed",
            ));
        }
        // Only this peer exists. Its retained publication is observed, not GPU activity.
        (InitialPeer::Directed(first), event)
    } else {
        let first = context
            .peer_copy(
                resources.initial_peer_stream,
                region(source.allocations[2], RuntimeAccessV1::Read),
                region(destination.allocations[2], RuntimeAccessV1::Write),
                &[],
            )
            .map_err(|error| failure("initial-peer", error))?;
        callback(context, &first, 0, receipts)?;
        let event = context
            .record_event(&first)
            .map_err(|error| failure("initial-peer-event", error))?;
        (InitialPeer::Ordinary(first), event)
    };
    let [_, b, c, d] = destination.allocations;
    let arguments =
        Arguments::new(c, b, d).map_err(|error| failure("deferred-arguments", error))?;
    let compute = context
        .launch_producer_aware_v1(
            destination.stream,
            &destination.kernel,
            &arguments,
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[event],
        )
        .map_err(|error| failure("deferred-compute", error))?;
    callback(context, &compute, 1, receipts)?;
    context
        .release_event(event)
        .map_err(|error| failure("initial-event-release", error))?;
    let event = context
        .record_event(&compute)
        .map_err(|error| failure("deferred-event", error))?;
    let second = context
        .peer_copy(
            resources.return_peer_stream,
            range(
                d,
                RuntimeAccessV1::Read,
                resources.layout.source_offset,
                resources.layout.copy_bytes,
            ),
            range(
                resources.returned,
                RuntimeAccessV1::Write,
                resources.layout.returned_offset,
                resources.layout.copy_bytes,
            ),
            &[event],
        )
        .map_err(|error| failure("return-peer", error))?;
    callback(context, &second, 2, receipts)?;
    context
        .release_event(event)
        .map_err(|error| failure("deferred-event-release", error))?;
    let event = context
        .record_event(&second)
        .map_err(|error| failure("return-peer-event", error))?;
    let readback = context
        .copy_async(
            resources.readback_stream,
            range(
                resources.returned,
                RuntimeAccessV1::Read,
                resources.layout.returned_offset,
                resources.layout.copy_bytes,
            ),
            range(
                resources.host_output,
                RuntimeAccessV1::Write,
                resources.layout.host_offset,
                resources.layout.copy_bytes,
            ),
            &[event],
        )
        .map_err(|error| failure("readback", error))?;
    callback(context, &readback, 3, receipts)?;
    context
        .release_event(event)
        .map_err(|error| failure("return-event-release", error))?;
    first.require(context, RuntimeCompletionStatusV1::Pending)?;
    require(context, &compute, RuntimeCompletionStatusV1::Pending)?;
    require(context, &second, RuntimeCompletionStatusV1::Pending)?;
    require(context, &readback, RuntimeCompletionStatusV1::Pending)?;
    if context.backend().completed_compute_xgmi_copies_v1() != 0 {
        return Err(failure(
            "admission-native-count",
            "unexpected peer completion",
        ));
    }
    if late_compute && context.backend().retained_compute_xgmi_copies_v1() != 1 {
        return Err(failure(
            "late-admission-custody",
            "initial peer must remain retained",
        ));
    }
    let ids = [first.id(), compute.id(), second.id(), readback.id()];
    Ok(Chain {
        initial_peer: first,
        compute,
        return_peer: second,
        readback,
        ids,
    })
}

fn pipeline(
    engine: &mut Engine,
    handle: &Handle,
    resources: Arc<Resources>,
    late_compute: bool,
) -> ResultV1<String> {
    let deadline = Instant::now() + WAIT;
    let mut registration = Box::pin(
        handle
            .enqueue_stream_registration(resources.readback_stream)
            .map_err(|error| failure("progress-register", error))?,
    );
    let result = engine.drive_until_ready(registration.as_mut(), deadline);
    drop(registration);
    let registration = result
        .map_err(|error| failure("progress-drive", error))?
        .map_err(|error| failure("progress-reply", error))?
        .map_err(|error| failure("progress-admission", error))?;
    let receipts = Arc::new(Mutex::new(Receipts::new()));
    let observed = Arc::clone(&receipts);
    let owned = Arc::clone(&resources);
    let mut chain = command(
        engine,
        handle,
        deadline,
        "chain-admission",
        move |context| admit_chain(context, &owned, &observed, late_compute, deadline),
    )?;
    let mut complete = false;
    for _ in 0..TICKS {
        if Instant::now() >= deadline {
            break;
        }
        let result = command(engine, handle, deadline, "chain-observe", move |context| {
            let complete = match context
                .poll(&mut chain.readback)
                .map_err(|error| failure("final-poll", error))?
            {
                RuntimePollV1::Succeeded => true,
                RuntimePollV1::Pending => false,
                status => return Err(failure("final-status", status)),
            };
            Ok((chain, complete))
        })?;
        chain = result.0;
        complete = result.1;
        if complete {
            break;
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    if !complete {
        return Err(failure(
            "pipeline-deadline",
            "final-only progress exhausted",
        ));
    }
    if !receipts
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .succeeded(&chain.ids)
    {
        return Err(failure(
            "pipeline-callbacks",
            "four exact successful receipts required",
        ));
    }
    let owned = Arc::clone(&resources);
    let (chain, snapshots) = command(
        engine,
        handle,
        deadline,
        "settled-snapshot",
        move |context| {
            chain
                .initial_peer
                .require(context, RuntimeCompletionStatusV1::Succeeded)?;
            require(
                context,
                &chain.compute,
                RuntimeCompletionStatusV1::Succeeded,
            )?;
            require(
                context,
                &chain.return_peer,
                RuntimeCompletionStatusV1::Succeeded,
            )?;
            require(
                context,
                &chain.readback,
                RuntimeCompletionStatusV1::Succeeded,
            )?;
            if context.backend().completed_compute_xgmi_copies_v1() != 2 {
                return Err(failure(
                    "native-counter",
                    "exactly two completed native copies required",
                ));
            }
            if late_compute && context.backend().retained_compute_xgmi_copies_v1() != 0 {
                return Err(failure(
                    "settled-native-custody",
                    "native peer remains retained",
                ));
            }
            for stream in [
                owned.initial_peer_stream,
                owned.runs[1].stream,
                owned.return_peer_stream,
                owned.readback_stream,
            ] {
                let observation = context
                    .query_stream(stream)
                    .map_err(|error| failure("settled-stream", error))?;
                if observation.total_submissions != 1
                    || observation.succeeded != 1
                    || !observation.is_quiescent()
                    || observation.failed != 0
                    || observation.quiescent_without_result != 0
                    || observation.first_failure.is_some()
                {
                    return Err(failure("settled-stream", observation));
                }
            }
            let mut snapshots = [Vec::new(), Vec::new(), vec![0; owned.layout.host_bytes]];
            if owned.layout != ReturnLayout::new(false) {
                snapshots[0].resize(BYTES, 0);
                snapshots[1].resize(owned.layout.returned_bytes, 0);
            }
            for (allocation, bytes) in [
                owned.runs[1].allocations[3],
                owned.returned,
                owned.host_output,
            ]
            .into_iter()
            .zip(&mut snapshots)
            {
                if !bytes.is_empty() {
                    context
                        .read_allocation(allocation, 0, bytes)
                        .map_err(|error| failure("snapshot-read", error))?;
                }
            }
            Ok((chain, snapshots))
        },
    )?;
    let output_digest = if resources.layout == ReturnLayout::new(false) {
        verify_output(&snapshots[2])?
    } else {
        verify_window_output(&snapshots)?
    };
    command(
        engine,
        handle,
        deadline,
        "pipeline-release",
        move |context| {
            context
                .release_submission(chain.readback)
                .map_err(|error| failure("readback-release", error))?;
            context
                .release_submission(chain.return_peer)
                .map_err(|error| failure("return-peer-release", error))?;
            context
                .release_submission(chain.compute)
                .map_err(|error| failure("deferred-release", error))?;
            chain.initial_peer.release(context)?;
            Ok(())
        },
    )?;
    if handle.observer().reply_cells_in_use() != 0 {
        return Err(failure(
            "reply-credits",
            "completed acknowledgment retained",
        ));
    }
    let mut future = Box::pin(
        handle
            .begin_drain(TICKS)
            .map_err(|error| failure("drain-admission", error))?,
    );
    let result = engine.drive_until_ready(future.as_mut(), Instant::now() + WAIT);
    drop(future);
    let drained = result
        .map_err(|error| failure("drain-drive", error))?
        .map_err(|error| failure("drain-result", error))?;
    if drained.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
        || !drained.queued_commands_exhausted
        || drained.operations_remaining != 0
        || drained.graph_active
        || drained.retained_submissions != RuntimeStreamObservationV1::default()
        || drained.ticks == 0
        || drained.ticks > TICKS
    {
        return Err(failure("completed-drain", drained));
    }
    drop(registration);
    Ok(output_digest)
}

fn run(options: Options) -> ResultV1<String> {
    let (context, resources) = setup(options)?;
    let config = RuntimeAsyncEngineConfigV1::new(16, 16, 16, 16, Duration::from_micros(50))
        .and_then(|config| config.with_reply_capacity(16))
        .map_err(|error| failure("owner-config", error))?;
    let progress = RuntimeAsyncProgressConfigV1::new(1, 1)
        .map_err(|error| failure("progress-config", error))?;
    let (mut engine, handle) = Engine::new_with_progress(
        || Ok::<_, String>(ManuallyDrop::into_inner(context)),
        config,
        progress,
    )
    .map_err(|error| failure("owner-open", error))?;
    let result = pipeline(&mut engine, &handle, resources, options.late_compute);
    let shutdown = engine.shutdown();
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|report| !report.is_complete() || !report.failures().is_empty())
    {
        return Err(failure("native-shutdown", (result.err(), shutdown)));
    }
    result
}

fn report(options: Options, output: &str) -> String {
    let padding_field = if options.return_window {
        "kernel_padding_bytes=0"
    } else {
        "padding_bytes=0"
    };
    let (schema, admission, compute_admission, progress, retained) = if options.late_compute {
        (
            "fe2o3.late-deferred-peer-chain-smoke.v1",
            "consumer-tail-after-first-native-publication",
            "deferred-after-first-peer-publication",
            "first-peer-until-retained-then-final-readback-only",
            " initial_peer=directed retained_native_at_compute_admission=1 retained_native_counter=0,1,0 publication_observed=true",
        )
    } else {
        (
            "fe2o3.deferred-peer-chain-smoke.v1",
            "all-four-before-explicit-progress",
            "deferred-before-first-peer-completion",
            "final-readback-stream-only",
            "",
        )
    };
    let (schema, padding, snapshot, preservation, window) = if options.return_window {
        (
            "fe2o3.pending-compute-return-window.v1",
            "logical-guards-checked",
            "settled-full-D-E-host-owner-command",
            "full-computed-source-byte-pass",
            " return_window=true logical_extents=262144,262913,263297 peer_offsets=20,131 readback_offsets=131,97 copy_bytes=262080 outside_destination_ranges=full-byte-pass readback_guards=full-byte-pass digest=length-prefixed-D-E-host",
        )
    } else {
        (
            schema,
            "exact-page-extent",
            "settled-host-visible-owner-command",
            "unobserved",
            "",
        )
    };
    format!(
        "PASS schema={schema} authority=qualification-r57-n3-v2 devices=2 unique_ids=0x{:016x},0x{:016x} elements={ELEMENTS} bytes={BYTES} {padding_field} launches=4 setup_launches=3 pipeline_launches=1 peer_copies=2 dependent_readbacks=1 completion_receipts=4 pipeline=peer-deferred-compute-peer-readback admission={admission} compute_admission={compute_admission} progress={progress} public_events=released-after-dependent-admission native_transport=NATIVE-XGMI native_counter=0,2 initial_peer_sentinel=full-byte-pass final_peer_sentinel=full-byte-pass output=full-byte-pass padding={padding} output_sha256={output} snapshot={snapshot} host_output_installations=0 journal=enabled retained_results=4 allocations=12 modules=2 streams=5 contexts=1 owners=1 pipeline_host_joins=0 results_release=readback-peer-compute-peer final_drain=completed-only cleanup=owned-shutdown-explicit source_preservation={preservation} physical_overlap=unmeasured performance_acceptance=false formal_refinement=false{retained}{window}",
        options.ids[0], options.ids[1]
    )
}

fn main() -> Result<(), String> {
    let options = options(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let output = run(options)?;
    println!("{}", report(options, &output));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_window_cli_combines_with_late_compute_without_changing_authority() {
        for flags in [
            vec!["--return-window"],
            vec!["--late-compute", "--return-window"],
            vec!["--return-window", "--late-compute"],
        ] {
            let mut args: Vec<_> = flags.iter().map(|s| (*s).to_owned()).collect();
            args.extend(["0x2".into(), "0x1".into()]);
            let parsed = options(&args).unwrap();
            assert_eq!(parsed.ids, [2, 1]);
            assert!(parsed.return_window);
            assert_eq!(parsed.late_compute, flags.contains(&"--late-compute"));
            let line = report(
                parsed,
                &verify_window_output(&expected_window_output()).unwrap(),
            );
            let fields: Vec<_> = line.split_whitespace().skip(1).collect();
            assert_eq!(fields.len(), if parsed.late_compute { 54 } else { 50 });
            assert!(fields.contains(&"schema=fe2o3.pending-compute-return-window.v1"));
            assert!(fields.contains(&"authority=qualification-r57-n3-v2"));
            assert!(fields.contains(&"pipeline_host_joins=0"));
            assert!(fields.contains(&"formal_refinement=false"));
        }
        for args in [
            vec!["--return-window", "--return-window", "0x1", "0x2"],
            vec!["0x1", "--return-window", "0x2"],
            vec!["--unknown", "0x1", "0x2"],
        ] {
            assert!(options(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn late_compute_cli_is_explicit_and_preserves_device_order() {
        for late_compute in [false, true] {
            let mut arguments = Vec::new();
            if late_compute {
                arguments.push("--late-compute".into());
            }
            arguments.extend(["0x2".into(), "0x1".into()]);
            assert_eq!(
                options(&arguments).unwrap(),
                Options {
                    ids: [2, 1],
                    late_compute,
                    return_window: false,
                }
            );
        }
        for arguments in [
            vec!["--late-compute"],
            vec!["--late-compute", "--late-compute", "0x1", "0x2"],
            vec!["0x1", "--late-compute", "0x2"],
            vec!["0x1", "0x2", "--late-compute"],
            vec!["--late-compute", "0x1", "0x01"],
            vec!["--late-compute", "0x0", "0x2"],
        ] {
            assert!(
                options(&arguments.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err()
            );
        }
    }

    #[test]
    fn late_report_preserves_legacy_authority_and_distinguishes_retained_publication() {
        let digest = digest(&expected_d());
        let fields = |late_compute| {
            let text = report(
                Options {
                    ids: [2, 1],
                    late_compute,
                    return_window: false,
                },
                &digest,
            );
            let pairs = text
                .split_whitespace()
                .skip(1)
                .map(|field| {
                    let (key, value) = field.split_once('=').unwrap();
                    (key.to_owned(), value.to_owned())
                })
                .collect::<Vec<_>>();
            let result = pairs
                .iter()
                .cloned()
                .collect::<std::collections::BTreeMap<_, _>>();
            assert_eq!(pairs.len(), result.len());
            result
        };
        let ordinary = fields(false);
        let late = fields(true);
        assert_eq!(ordinary.len(), 42);
        assert_eq!(late.len(), 46);
        assert_eq!(ordinary["schema"], "fe2o3.deferred-peer-chain-smoke.v1");
        assert_eq!(late["schema"], "fe2o3.late-deferred-peer-chain-smoke.v1");
        assert_eq!(
            late["compute_admission"],
            "deferred-after-first-peer-publication"
        );
        assert_eq!(late["retained_native_at_compute_admission"], "1");
        assert_eq!(late["retained_native_counter"], "0,1,0");
        assert_eq!(late["publication_observed"], "true");
        assert_eq!(late["initial_peer"], "directed");
        for (key, value) in &ordinary {
            if !["schema", "admission", "compute_admission", "progress"].contains(&key.as_str()) {
                assert_eq!(&late[key], value);
            }
        }
        assert_eq!(late["physical_overlap"], "unmeasured");
        assert_eq!(late["host_output_installations"], "0");
    }
}
