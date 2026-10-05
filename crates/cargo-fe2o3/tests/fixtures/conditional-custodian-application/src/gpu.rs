use std::{fmt::Debug, future::Future, pin::Pin, sync::Arc, time::Instant};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    ChargedTypedResultV1, GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeChargedResultV1,
    GeneratedRuntimeResultBudgetV1, GeneratedRuntimeWriteSlice, RemoteConditionalFillArtifactV1,
};
use fe2o3_runtime::*;

use crate::native_case::{self, ELEMENTS, FRAME_BYTES, GUARD_BYTES, PAYLOAD_BYTES};

type Backend = KfdMultiDeviceRuntimeBackendV1;
type Context = RuntimeContextV1<Backend>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<Backend>;
type Handle = RuntimeAsyncProgressHandleV1<Backend>;
type Artifact = RemoteConditionalFillArtifactV1<fill_write_only_gpu::Marker>;
type Result<T> = std::result::Result<T, String>;

fn checked<T, E: Debug>(stage: &str, result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| format!("{stage}: {error:?}"))
}

fn drive<F: Future + Unpin>(
    engine: &mut Engine,
    mut future: F,
    deadline: Instant,
) -> Result<F::Output> {
    // Consume each future so its bounded reply permit is released before the next stage.
    checked(
        "drive",
        engine.drive_until_ready(Pin::new(&mut future), deadline),
    )
}

fn with_context<T: Send + 'static>(
    engine: &mut Engine,
    handle: &Handle,
    deadline: Instant,
    operation: impl FnOnce(&mut Context) -> Result<T> + Send + 'static,
) -> Result<T> {
    let future = checked(
        "enqueue context operation",
        handle.observer().enqueue_with_context(operation),
    )?;
    checked("context operation", drive(engine, future, deadline)?)?
}

pub fn run(artifact: Arc<Artifact>, ids: [u64; 2], deadline: Instant) -> Result<()> {
    let budget = checked(
        "result budget",
        GeneratedRuntimeResultBudgetV1::new(1040, 2),
    )?;
    let (mut engine, handle) = checked(
        "runtime initialization",
        Engine::new_with_progress(
            || {
                let backend = checked(
                    "open native devices",
                    Backend::open_worker_v3_generated_only_with_native_peer_copy_v1(ids.to_vec()),
                )?;
                Context::open_with_version_journal_v1(backend, 16, 16).map_err(|failure| {
                    let (backend, error) = failure.into_parts();
                    std::mem::forget(backend);
                    format!("context initialization retained backend until process exit: {error:?}")
                })
            },
            RuntimeAsyncEngineConfigV1::default(),
            RuntimeAsyncProgressConfigV1::default(),
        ),
    )?;
    let work = execute(&mut engine, &handle, &artifact, &budget, ids, deadline);
    // Never propagate an operation error before inspecting owned shutdown. A timeout is not
    // settlement; the owner retains uncertain native custody instead of dropping it as quiescent.
    let shutdown = engine.shutdown();
    let cleanup = if shutdown.disposition == RuntimeAsyncOwnedDispositionV1::Released
        && shutdown
            .cleanup
            .as_ref()
            .is_some_and(|report| report.is_complete())
        && !shutdown.worker_panicked
        && shutdown.native_failure.is_none()
    {
        Ok(())
    } else {
        Err(format!(
            "runtime shutdown did not release all custody: {shutdown:?}"
        ))
    };
    let result = match (work, cleanup) {
        (Ok(results), Ok(())) => {
            drop(results);
            let usage = budget.usage();
            if usage.reserved_peak_bytes != 0
                || usage.retained_members != 0
                || usage.quarantined_members != 0
                || usage.unissued_members != 0
                || usage.poisoned
            {
                Err(format!("result budget was not fully refunded: {usage:?}"))
            } else {
                Ok(())
            }
        }
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(work), Err(cleanup)) => Err(format!("{work}; {cleanup}")),
    };
    drop(artifact);
    result
}

fn execute(
    engine: &mut Engine,
    handle: &Handle,
    artifact: &Arc<Artifact>,
    budget: &GeneratedRuntimeResultBudgetV1,
    ids: [u64; 2],
    deadline: Instant,
) -> Result<Arc<Vec<ChargedTypedResultV1<u32>>>> {
    let (devices, streams) = with_context(engine, handle, deadline, move |context| {
        let [first, second] = context.devices() else {
            return Err("native context did not expose exactly two devices".into());
        };
        if first.target() != "gfx942:xnack-" || second.target() != "gfx942:xnack-" {
            return Err("native fixture requires two gfx942:xnack- devices".into());
        }
        let devices = [first.id(), second.id()];
        for (device, expected) in devices.into_iter().zip(ids) {
            let observed = checked(
                "hardware identity",
                context.with_gfx942_preparation_device_v1(device, |owner| {
                    Ok::<_, String>(owner.observation().unique_id())
                }),
            )?;
            if *observed.value() != expected {
                return Err("context device order differs from requested hardware IDs".into());
            }
        }
        let streams = [
            checked("first stream", context.create_stream(devices[0]))?,
            checked("second stream", context.create_stream(devices[1]))?,
        ];
        Ok((devices, streams))
    })?;
    let mut pending = Vec::with_capacity(2);
    for (device, stream) in devices.into_iter().zip(streams) {
        let (output, observer) =
            GeneratedRuntimeWriteSlice::new_charged(vec![u32::MAX; ELEMENTS].into_boxed_slice());
        let prepare = checked(
            "prepare fill",
            artifact.prepare_generated_multi_context_invocation_async(
                fill_write_only_gpu::RuntimeArguments::new(output),
                handle,
                device,
                checked(
                    "fill geometry",
                    AqlDispatchGeometryV1::new([128, 1, 1], [64, 1, 1]),
                )?,
                0,
                30_000,
                GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
                budget,
                deadline,
            ),
        )?;
        let prepared = checked(
            "prepare fill",
            checked("prepare reply", drive(engine, prepare, deadline)?)?,
        )?;
        let reserve = handle
            .try_reserve_prepared_v1(prepared)
            .map_err(|e| format!("reserve fill: {:?}", e.error))?;
        let reserved = checked("reserve reply", drive(engine, reserve, deadline)?)?
            .map_err(|e| format!("reserve fill: {:?}", e.error))?;
        let activate = handle
            .try_activate_generated_v1(reserved, stream)
            .map_err(|e| format!("activate fill: {:?}", e.error))?;
        let completion = checked("activate reply", drive(engine, activate, deadline)?)?
            .map_err(|e| format!("activate fill: {:?}", e.error))?;
        pending.push((completion, observer));
    }
    let mut results = Vec::with_capacity(2);
    for (completion, mut observer) in pending {
        let receipt = checked(
            "fill completion",
            checked("completion reply", drive(engine, completion, deadline)?)?,
        )?;
        let result = take_result(&mut observer, &receipt, deadline)?;
        native_case::check_fill(result.as_slice())?;
        results.push(result);
    }
    let results = Arc::new(results);
    transport(engine, handle, devices, streams, &results, deadline)?;
    checked(
        "retained artifact after transport",
        artifact.revalidate(deadline),
    )?;
    with_context(engine, handle, deadline, move |context| {
        for stream in streams.into_iter().rev() {
            checked("destroy stream", context.destroy_stream(stream))?;
        }
        Ok(())
    })?;
    let drain = checked("begin drain", handle.begin_drain(1024))?;
    let report = checked("drain reply", drive(engine, drain, deadline)?)?;
    if report.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent {
        return Err(format!("native drain incomplete: {report:?}"));
    }
    Ok(results)
}

fn take_result(
    observer: &mut GeneratedRuntimeChargedResultV1<u32>,
    receipt: &RuntimeGeneratedCompletionReceiptV1,
    deadline: Instant,
) -> Result<ChargedTypedResultV1<u32>> {
    loop {
        if Instant::now() >= deadline {
            return Err("completed result contention deadline".into());
        }
        if let Some(result) = checked(
            "take exact completed result",
            observer.take_completed_v1(receipt),
        )? {
            return Ok(result);
        }
        std::thread::yield_now();
    }
}

#[derive(Clone, Copy)]
struct Buffers {
    host: RuntimeAllocationIdV1,
    source: RuntimeAllocationIdV1,
    guard_host: RuntimeAllocationIdV1,
    destination: RuntimeAllocationIdV1,
}

fn region(
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

fn finish_transfer<A: Send + 'static>(
    engine: &mut Engine,
    handle: &Handle,
    future: RuntimeAsyncOperationFutureV1<A, KfdRuntimeBackendErrorV1>,
    deadline: Instant,
) -> Result<()> {
    let outcome = checked("transfer reply", drive(engine, future, deadline)?)?;
    if outcome.rejected_observations != 0 || outcome.last_rejected_observation.is_some() {
        return Err(format!(
            "transfer rejected observations: count={}, last={:?}",
            outcome.rejected_observations, outcome.last_rejected_observation
        ));
    }
    if checked("transfer observation", outcome.observation)? != RuntimeCompletionStatusV1::Succeeded
    {
        return Err("transfer did not succeed; custody requires owned shutdown".into());
    }
    let submission = outcome
        .submission
        .ok_or("successful transfer omitted its submission")?;
    with_context(engine, handle, deadline, move |context| {
        checked(
            "release settled transfer",
            context.release_submission(submission),
        )
    })
}

fn transport(
    engine: &mut Engine,
    handle: &Handle,
    devices: [RuntimeDeviceIdV1; 2],
    streams: [RuntimeStreamIdV1; 2],
    results: &Arc<Vec<ChargedTypedResultV1<u32>>>,
    deadline: Instant,
) -> Result<()> {
    // Keep caller custody even when the queued staging command is rejected or abandoned.
    let staged_results = Arc::clone(results);
    let (buffers, payloads) = with_context(engine, handle, deadline, move |context| {
        let mut payloads = [vec![0; PAYLOAD_BYTES], vec![0; PAYLOAD_BYTES]];
        if context.backend().completed_compute_xgmi_copies_v1() != 0 {
            return Err("native peer counter was not initially zero".into());
        }
        let mut allocate = |index: usize| -> Result<Buffers> {
            let device = devices[index];
            let host = checked(
                "allocate upload staging",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    PAYLOAD_BYTES as u64,
                    4096,
                ),
            )?;
            let source = checked(
                "allocate PUBLIC source",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    PAYLOAD_BYTES as u64,
                    4096,
                ),
            )?;
            let guard_host = checked(
                "allocate sentinel staging",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    FRAME_BYTES as u64,
                    4096,
                ),
            )?;
            let destination = checked(
                "allocate PUBLIC destination",
                context.allocate(
                    device,
                    RuntimeMemoryKindV1::DeviceLocal,
                    FRAME_BYTES as u64,
                    4096,
                ),
            )?;
            checked(
                "stage completed fill",
                staged_results[index].write_staging_v1(context, host, &mut payloads[index]),
            )?;
            checked(
                "stage sentinel frame",
                context.write_host_visible_allocation_v1(
                    guard_host,
                    &native_case::destination_frame(index),
                ),
            )?;
            Ok(Buffers {
                host,
                source,
                guard_host,
                destination,
            })
        };
        let buffers = [allocate(0)?, allocate(1)?];
        Ok((buffers, payloads))
    })?;
    for (stream, buffers) in streams.into_iter().zip(buffers) {
        for (host, device, bytes) in [
            (buffers.host, buffers.source, PAYLOAD_BYTES),
            (buffers.guard_host, buffers.destination, FRAME_BYTES),
        ] {
            let upload = checked(
                "enqueue upload",
                handle.copy_async(
                    stream,
                    region(host, RuntimeAccessV1::Read, 0, bytes),
                    region(device, RuntimeAccessV1::Write, 0, bytes),
                    Vec::new(),
                ),
            )?;
            finish_transfer(engine, handle, upload, deadline)?;
        }
    }
    for destination in 0..2 {
        let peer = checked(
            "enqueue native peer",
            handle.peer_copy(
                streams[destination],
                region(
                    buffers[1 - destination].source,
                    RuntimeAccessV1::Read,
                    0,
                    PAYLOAD_BYTES,
                ),
                region(
                    buffers[destination].destination,
                    RuntimeAccessV1::Write,
                    GUARD_BYTES,
                    PAYLOAD_BYTES,
                ),
                Vec::new(),
            ),
        )?;
        finish_transfer(engine, handle, peer, deadline)?;
        with_context(engine, handle, deadline, move |context| {
            if context.backend().completed_compute_xgmi_copies_v1() != destination as u64 + 1 {
                return Err("peer transfer did not retire through native XGMI".into());
            }
            Ok(())
        })?;
    }
    with_context(engine, handle, deadline, move |context| {
        let mut sources = [vec![0; PAYLOAD_BYTES], vec![0; PAYLOAD_BYTES]];
        let mut destinations = [vec![0; FRAME_BYTES], vec![0; FRAME_BYTES]];
        for index in 0..2 {
            checked(
                "read full peer source",
                context.read_allocation(buffers[index].source, 0, &mut sources[index]),
            )?;
            checked(
                "read full guarded destination",
                context.read_allocation(buffers[index].destination, 0, &mut destinations[index]),
            )?;
        }
        native_case::check_transport(&payloads, &sources, &destinations)?;
        for buffers in buffers.into_iter().rev() {
            for allocation in [
                buffers.destination,
                buffers.guard_host,
                buffers.source,
                buffers.host,
            ] {
                checked(
                    "release transport allocation",
                    context.release_allocation(allocation),
                )?;
            }
        }
        Ok(())
    })
}
