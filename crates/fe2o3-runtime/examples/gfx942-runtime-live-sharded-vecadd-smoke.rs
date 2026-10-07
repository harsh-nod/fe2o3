//! Two finite sharded compute/peer/readback batches inside one live Context.

use std::mem::ManuallyDrop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fe2o3_runtime::qualification_gfx942_sharded_vecadd_rounds_v1::{
    Gfx942ShardedVecaddRoundsQualificationArgumentsV1 as Arguments,
    Gfx942ShardedVecaddRoundsQualificationRecipeV1 as Recipe,
    admit_gfx942_sharded_vecadd_rounds_qualification_v1,
};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, RuntimeAccessV1, RuntimeAllocationIdV1,
    RuntimeAsyncCurrentThreadOwnedEngineV1, RuntimeAsyncDrainOutcomeV1, RuntimeAsyncDriveErrorV1,
    RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedDispositionV1, RuntimeAsyncProgressConfigV1,
    RuntimeAsyncProgressHandleV1, RuntimeCompletionStatusV1, RuntimeContextV1, RuntimeCopyV1,
    RuntimeLaunchGeometryV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1, RuntimeModuleIdV1,
    RuntimePeerCopyV1, RuntimePollV1, RuntimeStreamIdV1, RuntimeStreamObservationV1,
    RuntimeSubmissionIdV1, RuntimeSubmissionV1, TypedRuntimeKernelV1,
};
use sha2::{Digest, Sha256};

#[path = "live_sharded_vecadd/data.rs"]
mod data;
use data::*;

#[path = "live_sharded_vecadd/gather.rs"]
mod gather;

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<KfdMultiDeviceRuntimeBackendV1>;
type Handle = RuntimeAsyncProgressHandleV1<KfdMultiDeviceRuntimeBackendV1>;
type ResultV1<T> = Result<T, String>;
const WAIT: Duration = Duration::from_secs(30);
const TICKS: usize = 30_000;
const USAGE: &str = "usage: gfx942-runtime-live-sharded-vecadd-smoke <0xunique-id> <0xunique-id> [up to eight total]";

struct Buffers {
    compute_stream: RuntimeStreamIdV1,
    peer_stream: RuntimeStreamIdV1,
    readback_stream: RuntimeStreamIdV1,
    host: [RuntimeAllocationIdV1; 3],
    device: [RuntimeAllocationIdV1; 3],
    incoming_host: RuntimeAllocationIdV1,
    incoming: RuntimeAllocationIdV1,
    module: RuntimeModuleIdV1,
    kernel: TypedRuntimeKernelV1<Arguments>,
}

struct Batch {
    compute: Vec<RuntimeSubmissionV1<Arguments>>,
    peers: Vec<RuntimeSubmissionV1<RuntimePeerCopyV1>>,
    readbacks: Vec<RuntimeSubmissionV1<RuntimeCopyV1>>,
    ids: Vec<RuntimeSubmissionIdV1>,
}

struct Report {
    digests: [(String, String); 2],
}

fn failure(stage: &str, error: impl core::fmt::Debug) -> String {
    let detail = format!("stage={stage} {error:?}");
    eprintln!("live sharded vecadd diagnostic: {detail}");
    detail
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    bytes: usize,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: bytes as u64,
    }
}

fn handles(buffers: &[Buffers]) -> String {
    buffers
        .iter()
        .map(|buffer| {
            format!(
                "{:?}",
                (
                    buffer.host,
                    buffer.device,
                    buffer.incoming_host,
                    buffer.incoming,
                    buffer.compute_stream,
                    buffer.peer_stream,
                    buffer.readback_stream,
                    buffer.module,
                    buffer.kernel.model_identity(),
                )
            )
        })
        .collect::<Vec<_>>()
        .join(";")
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
    // An expired observation must retain this exact accepted command and reply.
    if !matches!(
        engine.drive_until_ready(future.as_mut(), Instant::now()),
        Err(RuntimeAsyncDriveErrorV1::DeadlineExceeded)
    ) {
        return Err(failure(
            stage,
            "fresh command unexpectedly completed without an owner tick",
        ));
    }
    let result = engine.drive_until_ready(future.as_mut(), deadline);
    drop(future);
    result
        .map_err(|error| failure(stage, error))?
        .map_err(|error| failure(stage, error))?
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
        return Err(failure("initial-bytes", "full allocation differs"));
    }
    Ok(())
}

fn setup(ids: &[u64], shards: &[Shard]) -> ResultV1<(ManuallyDrop<Context>, Arc<Vec<Buffers>>)> {
    let admitted = (0..ids.len())
        .map(|index| {
            admit_gfx942_sharded_vecadd_rounds_qualification_v1(ids.len(), index)
                .map_err(|error| failure("artifact-admission", error))
        })
        .collect::<ResultV1<Vec<_>>>()?;
    for round in 0..2 {
        for (index, &shard) in shards.iter().enumerate() {
            recipe(ids.len(), index, round, shard)?;
        }
    }
    let backend =
        KfdMultiDeviceRuntimeBackendV1::open_gfx942_sharded_vecadd_rounds_peer_qualification_v1(
            ids,
        )
        .map_err(|error| failure("device-admission", error))?;
    // Setup failures retain native owners until this bounded process exits.
    let mut context = ManuallyDrop::new(
        Context::open_with_version_journal_members_v1(backend, 128, 128, 128)
            .map_err(|error| failure("context-open", error))?,
    );
    if context.devices().len() != ids.len()
        || context
            .devices()
            .iter()
            .any(|device| device.target() != "gfx942:xnack-")
    {
        return Err(failure(
            "device-roster",
            "exact ordered gfx942:xnack- roster required",
        ));
    }
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    let mut buffers = Vec::with_capacity(ids.len());
    for (index, device) in devices.into_iter().enumerate() {
        let incoming = shards[(index + ids.len() - 1) % ids.len()];
        let mut allocate = |kind, bytes: usize| {
            context
                .allocate(device, kind, bytes as u64, PAGE_BYTES as u64)
                .map_err(|error| failure("allocation", error))
        };
        let mut host = Vec::with_capacity(3);
        let mut local = Vec::with_capacity(3);
        for _ in 0..3 {
            host.push(allocate(
                RuntimeMemoryKindV1::HostVisible,
                shards[index].padded_bytes,
            )?);
            local.push(allocate(
                RuntimeMemoryKindV1::DeviceLocal,
                shards[index].padded_bytes,
            )?);
        }
        let incoming_host = allocate(RuntimeMemoryKindV1::HostVisible, incoming.padded_bytes)?;
        let incoming_device = allocate(RuntimeMemoryKindV1::DeviceLocal, incoming.padded_bytes)?;
        let compute_stream = context
            .create_stream(device)
            .map_err(|error| failure("compute-stream", error))?;
        let peer_stream = context
            .create_stream(device)
            .map_err(|error| failure("peer-stream", error))?;
        let readback_stream = context
            .create_stream(device)
            .map_err(|error| failure("readback-stream", error))?;
        let module = context
            .load_module(device, admitted[index].hsaco())
            .map_err(|error| failure("module-load", error))?;
        let kernel = context
            .resolve_kernel::<Arguments>(module, admitted[index].kernel_name())
            .map_err(|error| failure("kernel-resolve", error))?;
        buffers.push(Buffers {
            compute_stream,
            peer_stream,
            readback_stream,
            host: host.try_into().map_err(|_| "host roster differs")?,
            device: local.try_into().map_err(|_| "device roster differs")?,
            incoming_host,
            incoming: incoming_device,
            module,
            kernel,
        });
    }
    Ok((context, Arc::new(buffers)))
}

fn refresh(
    engine: &mut Engine,
    handle: &Handle,
    buffers: &Arc<Vec<Buffers>>,
    shards: &Arc<Vec<Shard>>,
    round: usize,
) -> ResultV1<()> {
    let deadline = Instant::now() + WAIT;
    let owned = Arc::clone(buffers);
    let partition = Arc::clone(shards);
    let mut uploads = command(
        engine,
        handle,
        deadline,
        "refresh-admission",
        move |context| {
            let mut uploads = Vec::with_capacity(4 * owned.len());
            for (index, buffer) in owned.iter().enumerate() {
                for (host, bytes) in buffer
                    .host
                    .iter()
                    .zip(initial_buffers(partition[index], round))
                {
                    context
                        .write_allocation(*host, 0, &bytes)
                        .map_err(|error| failure("input-host-write", error))?;
                    verify_initial(context, *host, &bytes)?;
                }
                let incoming = partition[(index + owned.len() - 1) % owned.len()];
                let sentinel = sentinel(incoming);
                context
                    .write_allocation(buffer.incoming_host, 0, &sentinel)
                    .map_err(|error| failure("sentinel-host-write", error))?;
                for (host, device, bytes) in [
                    (
                        buffer.host[0],
                        buffer.device[0],
                        partition[index].padded_bytes,
                    ),
                    (
                        buffer.host[1],
                        buffer.device[1],
                        partition[index].padded_bytes,
                    ),
                    (
                        buffer.host[2],
                        buffer.device[2],
                        partition[index].padded_bytes,
                    ),
                    (buffer.incoming_host, buffer.incoming, incoming.padded_bytes),
                ] {
                    let upload = context
                        .copy_async(
                            buffer.compute_stream,
                            region(host, RuntimeAccessV1::Read, bytes),
                            region(device, RuntimeAccessV1::Write, bytes),
                            &[],
                        )
                        .map_err(|error| failure("upload-admission", error))?;
                    uploads.push((buffer.compute_stream, upload));
                }
            }
            Ok(uploads)
        },
    )?;
    let mut complete = false;
    for _ in 0..TICKS {
        if Instant::now() >= deadline {
            break;
        }
        let result = command(
            engine,
            handle,
            deadline,
            "upload-progress",
            move |context| {
                let mut complete = true;
                for (stream, upload) in &mut uploads {
                    context
                        .flush_stream(*stream)
                        .map_err(|error| failure("upload-flush", error))?;
                    match context
                        .poll(upload)
                        .map_err(|error| failure("upload-poll", error))?
                    {
                        RuntimePollV1::Succeeded => {}
                        RuntimePollV1::Pending => complete = false,
                        status => return Err(failure("upload-status", status)),
                    }
                }
                Ok((uploads, complete))
            },
        )?;
        uploads = result.0;
        complete = result.1;
        if complete {
            break;
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    if !complete {
        return Err(failure(
            "upload-deadline",
            "bounded upload progress exhausted",
        ));
    }
    let owned = Arc::clone(buffers);
    let partition = Arc::clone(shards);
    command(engine, handle, deadline, "upload-release", move |context| {
        for (_, upload) in uploads.into_iter().rev() {
            context
                .release_submission(upload)
                .map_err(|error| failure("upload-release", error))?;
        }
        // Reading compute A/B/C here would destroy the authenticated H2dReady digests.
        for (index, &shard) in partition.iter().enumerate() {
            verify_initial(
                context,
                owned[(index + 1) % owned.len()].incoming,
                &sentinel(shard),
            )?;
        }
        Ok(())
    })
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

fn admit_batch(
    context: &mut Context,
    buffers: &[Buffers],
    shards: &[Shard],
    round: usize,
    receipts: &Arc<Mutex<Receipts>>,
) -> ResultV1<Batch> {
    let count = buffers.len();
    if context.backend().completed_compute_xgmi_copies_v1() != (round * count) as u64 {
        return Err(failure("native-before", "logical native count differs"));
    }
    let mut compute = Vec::with_capacity(count);
    let mut peers = Vec::with_capacity(count);
    let mut readbacks = Vec::with_capacity(count);
    let mut compute_events = Vec::with_capacity(count);
    let mut peer_events = Vec::with_capacity(count);
    let mut ids = Vec::with_capacity(3 * count);
    let offset = round * 3 * count;
    for (index, buffer) in buffers.iter().enumerate() {
        let recipe = recipe(count, index, round, shards[index])?;
        let [a, b, c] = buffer.device;
        let arguments =
            Arguments::new(recipe, a, b, c).map_err(|error| failure("arguments", error))?;
        let submission = context
            .launch_producer_aware_v1(
                buffer.compute_stream,
                &buffer.kernel,
                &arguments,
                recipe.geometry(),
                &[],
            )
            .map_err(|error| failure("compute-admission", error))?;
        callback(context, &submission, offset + index, receipts)?;
        ids.push(submission.id());
        compute_events.push(
            context
                .record_event(&submission)
                .map_err(|error| failure("compute-event", error))?,
        );
        compute.push(submission);
    }
    for (index, shard) in shards.iter().enumerate() {
        let destination = &buffers[(index + 1) % count];
        let submission = context
            .peer_copy(
                destination.peer_stream,
                region(
                    buffers[index].device[2],
                    RuntimeAccessV1::Read,
                    shard.padded_bytes,
                ),
                region(
                    destination.incoming,
                    RuntimeAccessV1::Write,
                    shard.padded_bytes,
                ),
                &[compute_events[index]],
            )
            .map_err(|error| failure("peer-admission", error))?;
        callback(context, &submission, offset + count + index, receipts)?;
        ids.push(submission.id());
        peer_events.push(
            context
                .record_event(&submission)
                .map_err(|error| failure("peer-event", error))?,
        );
        peers.push(submission);
    }
    for event in compute_events {
        context
            .release_event(event)
            .map_err(|error| failure("compute-event-release", error))?;
    }
    for (index, shard) in shards.iter().enumerate() {
        let destination = &buffers[(index + 1) % count];
        let submission = context
            .copy_async(
                destination.readback_stream,
                region(
                    destination.incoming,
                    RuntimeAccessV1::Read,
                    shard.padded_bytes,
                ),
                region(
                    destination.incoming_host,
                    RuntimeAccessV1::Write,
                    shard.padded_bytes,
                ),
                &[peer_events[index]],
            )
            .map_err(|error| failure("readback-admission", error))?;
        callback(context, &submission, offset + 2 * count + index, receipts)?;
        ids.push(submission.id());
        readbacks.push(submission);
    }
    for event in peer_events {
        context
            .release_event(event)
            .map_err(|error| failure("peer-event-release", error))?;
    }
    for submission in &peers {
        if context
            .query_submission(submission)
            .map_err(|error| failure("peer-pending", error))?
            != RuntimeCompletionStatusV1::Pending
        {
            return Err(failure("peer-pending", "transfer is not pending"));
        }
    }
    for submission in &readbacks {
        if context
            .query_submission(submission)
            .map_err(|error| failure("readback-pending", error))?
            != RuntimeCompletionStatusV1::Pending
        {
            return Err(failure("readback-pending", "transfer is not pending"));
        }
    }
    if context.backend().completed_compute_xgmi_copies_v1() != (round * count) as u64 {
        return Err(failure(
            "native-after-admission",
            "unexpected progress within batch admission",
        ));
    }
    Ok(Batch {
        compute,
        peers,
        readbacks,
        ids,
    })
}

fn settle_batch(
    engine: &mut Engine,
    handle: &Handle,
    mut batch: Batch,
    count: usize,
    round: usize,
    deadline: Instant,
) -> ResultV1<Batch> {
    for _ in 0..TICKS {
        if Instant::now() >= deadline {
            break;
        }
        let (returned, complete) =
            command(engine, handle, deadline, "batch-observe", move |context| {
                let mut complete = true;
                for readback in &mut batch.readbacks {
                    match context
                        .poll(readback)
                        .map_err(|error| failure("readback-poll", error))?
                    {
                        RuntimePollV1::Succeeded => {}
                        RuntimePollV1::Pending => complete = false,
                        status => return Err(failure("readback-status", status)),
                    }
                }
                if complete {
                    for submission in &batch.compute {
                        if context
                            .query_submission(submission)
                            .map_err(|error| failure("compute-status", error))?
                            != RuntimeCompletionStatusV1::Succeeded
                        {
                            return Err(failure(
                                "compute-status",
                                "original ancestor was not reconciled",
                            ));
                        }
                    }
                    for submission in &batch.peers {
                        if context
                            .query_submission(submission)
                            .map_err(|error| failure("peer-status", error))?
                            != RuntimeCompletionStatusV1::Succeeded
                        {
                            return Err(failure(
                                "peer-status",
                                "original ancestor was not reconciled",
                            ));
                        }
                    }
                    if context.backend().completed_compute_xgmi_copies_v1()
                        != ((round + 1) * count) as u64
                    {
                        return Err(failure(
                            "native-after",
                            "exact native logical count differs",
                        ));
                    }
                }
                Ok((batch, complete))
            })?;
        batch = returned;
        if complete {
            return Ok(batch);
        }
        std::thread::sleep(Duration::from_micros(50));
    }
    Err(failure(
        "batch-deadline",
        "bounded readback progress exhausted",
    ))
}

fn run_batches(
    engine: &mut Engine,
    handle: &Handle,
    buffers: Arc<Vec<Buffers>>,
    shards: Arc<Vec<Shard>>,
) -> ResultV1<Report> {
    let count = buffers.len();
    let original_handles = handles(&buffers);
    let receipts = Arc::new(Mutex::new(Receipts::new(6 * count)));
    let mut expected_ids = Vec::with_capacity(6 * count);
    let mut registrations = Vec::with_capacity(count);
    let deadline = Instant::now() + WAIT;
    for buffer in buffers.iter() {
        let mut future = Box::pin(
            handle
                .enqueue_stream_registration(buffer.readback_stream)
                .map_err(|error| failure("registration", error))?,
        );
        let result = engine.drive_until_ready(future.as_mut(), deadline);
        drop(future);
        registrations.push(
            result
                .map_err(|error| failure("registration-drive", error))?
                .map_err(|error| failure("registration-result", error))?
                .map_err(|error| failure("registration-admission", error))?,
        );
    }
    let mut digests = Vec::with_capacity(2);
    for round in 0..2 {
        if handles(&buffers) != original_handles {
            return Err(failure("stable-handles", "logical identity changed"));
        }
        refresh(engine, handle, &buffers, &shards, round)?;
        let deadline = Instant::now() + WAIT;
        let owned = Arc::clone(&buffers);
        let partition = Arc::clone(&shards);
        let received = Arc::clone(&receipts);
        let batch = command(
            engine,
            handle,
            deadline,
            "batch-admission",
            move |context| admit_batch(context, &owned, &partition, round, &received),
        )?;
        let batch = settle_batch(engine, handle, batch, count, round, deadline)?;
        if !receipts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .succeeded(round * 3 * count, &batch.ids)
        {
            return Err(failure(
                "batch-receipts",
                "exact callback identities or dispositions differ",
            ));
        }
        expected_ids.extend_from_slice(&batch.ids);
        let owned = Arc::clone(&buffers);
        let lengths: Vec<_> = shards.iter().map(|shard| shard.padded_bytes).collect();
        let (batch, bytes) = command(
            engine,
            handle,
            deadline,
            "settled-snapshot",
            move |context| {
                for readback in &batch.readbacks {
                    if context
                        .query_submission(readback)
                        .map_err(|error| failure("snapshot-result", error))?
                        != RuntimeCompletionStatusV1::Succeeded
                    {
                        return Err(failure(
                            "snapshot-result",
                            "original readback is not successful",
                        ));
                    }
                }
                for buffer in owned.iter() {
                    for stream in [
                        buffer.compute_stream,
                        buffer.peer_stream,
                        buffer.readback_stream,
                    ] {
                        let state = context
                            .query_stream(stream)
                            .map_err(|error| failure("snapshot-stream", error))?;
                        if state.total_submissions != 1
                            || state.succeeded != 1
                            || !state.is_quiescent()
                            || state.failed != 0
                            || state.quiescent_without_result != 0
                            || state.first_failure.is_some()
                        {
                            return Err(failure("snapshot-stream", state));
                        }
                    }
                }
                let bytes = snapshot(&lengths, |index, bytes| {
                    context
                        .read_allocation(owned[(index + 1) % owned.len()].incoming_host, 0, bytes)
                        .map_err(|error| failure("snapshot-read", error))
                })?;
                Ok((batch, bytes))
            },
        )?;
        digests.push(verify_snapshot(&bytes, &shards, round)?);
        drop(bytes);
        command(engine, handle, deadline, "batch-release", move |context| {
            for submission in batch.readbacks.into_iter().rev() {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("readback-release", error))?;
            }
            for submission in batch.peers.into_iter().rev() {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("peer-release", error))?;
            }
            for submission in batch.compute.into_iter().rev() {
                context
                    .release_submission(submission)
                    .map_err(|error| failure("compute-release", error))?;
            }
            Ok(())
        })?;
        if handle.observer().reply_cells_in_use() != 0 {
            return Err(failure(
                "reply-credits",
                "completed command acknowledgment retained",
            ));
        }
    }
    if handles(&buffers) != original_handles
        || !receipts
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .succeeded(0, &expected_ids)
    {
        return Err(failure(
            "final-identities",
            "stable logical handles or six-N receipts differ",
        ));
    }
    let mut future = Box::pin(
        handle
            .begin_drain(TICKS)
            .map_err(|error| failure("final-drain", error))?,
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
    drop(registrations);
    Ok(Report {
        digests: digests
            .try_into()
            .map_err(|_| "two batch digests required")?,
    })
}

fn run(ids: &[u64], gather: Option<bool>) -> ResultV1<Report> {
    let shards = Arc::new(partition(ids.len())?);
    let (context, buffers) = setup(ids, &shards)?;
    let capacity = 3 * ids.len() + 4;
    let config = RuntimeAsyncEngineConfigV1::new(
        capacity,
        capacity,
        capacity,
        capacity,
        Duration::from_micros(50),
    )
    .and_then(|config| config.with_reply_capacity(capacity))
    .map_err(|error| failure("owner-config", error))?;
    let progress = RuntimeAsyncProgressConfigV1::new(ids.len(), 1)
        .map_err(|error| failure("progress-config", error))?;
    let (mut engine, handle) = Engine::new_with_progress(
        || Ok::<_, String>(ManuallyDrop::into_inner(context)),
        config,
        progress,
    )
    .map_err(|error| failure("owner-open", error))?;
    let result = match gather {
        Some(overlap) => gather::run_batches(&mut engine, &handle, buffers, shards, overlap),
        None => run_batches(&mut engine, &handle, buffers, shards),
    };
    // Every post-engine error path attempts owned cleanup before reporting failure.
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

fn main() -> Result<(), String> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    let (mode, arguments) = gather::options(&arguments)?;
    let ids = unique_ids(arguments)?;
    let shards = partition(ids.len())?;
    let report = run(&ids, mode)?;
    if let Some(overlap) = mode {
        println!("{}", gather::report(&ids, &shards, overlap, &report));
        return Ok(());
    }
    let count = ids.len();
    let ids = ids
        .iter()
        .map(|id| format!("0x{id:016x}"))
        .collect::<Vec<_>>()
        .join(",");
    let ranges = shards
        .iter()
        .map(|shard| format!("{}:{}:{}", shard.offset, shard.elements, shard.padded_bytes))
        .collect::<Vec<_>>()
        .join(",");
    let padded: usize = shards.iter().map(|shard| shard.padded_bytes).sum();
    println!(
        "PASS schema=fe2o3.live-sharded-vecadd-smoke.v1 authority=qualification-sharded-vecadd-rounds-v1 devices={count} unique_ids={ids} rounds=2 logical_elements_per_round={TOTAL_ELEMENTS} logical_bytes_per_round=262148 shard_ranges={ranges} padded_bytes_per_round={padded} launches={} peer_copies={} dependent_readbacks={} completion_receipts={} batch_admission=one-owner-command compute_api=producer-aware compute_dependency=exact-event compute_admission=may-publish-before-progress compute_progress=dependent-readback-streams journal=enabled host_output_installations=0 input_refresh=full-padded-per-batch incoming_sentinel=full-byte-pass output=full-byte-pass padding=full-byte-pass coverage=exact-contiguous-disjoint round0_sha256={} round1_sha256={} round0_padded_sha256={} round1_padded_sha256={} native_transport=authenticated-pending-peer native_counter=0,{count},{} stable_handles=logical-only contexts=1 owners=1 allocation_reuse=true module_reuse=true kernel_reuse=true stream_reuse=true public_events=released-after-admission batch_results=released-before-refresh intermediate_cutoffs=0 snapshot=settled-host-visible-owner-command snapshot_byte_bound={MAX_SNAPSHOT_BYTES} reply_credits_after_batches=0 deadline_resumption=same-command-future final_drain=completed-only cleanup=owned-shutdown-explicit source_preservation=unobserved physical_overlap=unmeasured performance_acceptance=false formal_refinement=false",
        2 * count,
        2 * count,
        2 * count,
        6 * count,
        report.digests[0].0,
        report.digests[1].0,
        report.digests[0].1,
        report.digests[1].1,
        2 * count
    );
    Ok(())
}
