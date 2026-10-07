//! Genuine legacy-artifact qualification of receipt custody, not GPU time overlap.
use super::{Artifact, checked, take_result};
use crate::coexistence_case::{self as case, Case, Report};
use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    ChargedTypedResultV1, GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1,
    GeneratedRuntimeWriteSlice,
};
use fe2o3_runtime::*;
use std::{future::Future, pin::Pin, sync::Arc, time::Instant};

type Backend = KfdRuntimeBackendV1;
type Context = RuntimeContextV1<Backend>;
type Engine = RuntimeAsyncCurrentThreadOwnedEngineV1<Backend>;
type Handle = RuntimeAsyncProgressHandleV1<Backend>;
type Result<T> = std::result::Result<T, String>;

fn drive<F: Future + Unpin>(
    engine: &mut Engine,
    mut future: F,
    deadline: Instant,
) -> Result<F::Output> {
    checked(
        "drive single-device owner",
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

fn settle(
    context: &mut Context,
    mut submission: RuntimeSubmissionV1<RuntimeCopyV1>,
    deadline: Instant,
) -> Result<()> {
    if Instant::now() >= deadline {
        return Err("copy settlement deadline expired".into());
    }
    checked(
        "wait exact copy",
        context.wait(
            &mut submission,
            deadline.saturating_duration_since(Instant::now()),
        ),
    )?;
    if checked("query exact copy", context.query_submission(&submission))?
        != RuntimeCompletionStatusV1::Succeeded
    {
        return Err("copy did not conclusively succeed".into());
    }
    checked(
        "release exact settled copy",
        context.release_submission(submission),
    )
}

pub fn run(artifact: Arc<Artifact>, case: Case, deadline: Instant) -> Result<Report> {
    let budget = checked(
        "result budget",
        GeneratedRuntimeResultBudgetV1::new((case::ELEMENTS * 8) as u64, 1),
    )?;
    let (mut engine, handle) = checked(
        "single-device runtime initialization",
        Engine::new_with_progress(
            || {
                let backend = checked(
                    "open selected generated-only device",
                    Backend::open_worker_v3_generated_only_v1(case.device),
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
    let work = execute(&mut engine, &handle, &artifact, &budget, case, deadline);
    let shutdown = engine.shutdown();
    let released = shutdown.disposition == RuntimeAsyncOwnedDispositionV1::Released
        && shutdown
            .cleanup
            .as_ref()
            .is_some_and(|report| report.is_complete())
        && !shutdown.worker_panicked
        && shutdown.native_failure.is_none()
        && handle.observer().reply_cells_in_use() == 0
        && handle.observer().snapshot_bytes_in_use() == 0;
    let result = match work {
        Ok((output, report)) if released => {
            drop(output);
            let usage = budget.usage();
            if usage.reserved_peak_bytes != 0
                || usage.retained_members != 0
                || usage.quarantined_members != 0
                || usage.unissued_members != 0
                || usage.poisoned
            {
                Err(format!("result budget not fully refunded: {usage:?}"))
            } else {
                Ok(report)
            }
        }
        Ok(_) => Err(format!("runtime shutdown retained custody: {shutdown:?}")),
        Err(error) if released => Err(error),
        Err(error) => Err(format!(
            "{error}; runtime shutdown retained custody: {shutdown:?}"
        )),
    };
    drop(artifact);
    result
}

fn execute(
    engine: &mut Engine,
    handle: &Handle,
    artifact: &Arc<Artifact>,
    budget: &GeneratedRuntimeResultBudgetV1,
    case: Case,
    deadline: Instant,
) -> Result<(ChargedTypedResultV1<u32>, Report)> {
    let (device, streams, allocations) = with_context(engine, handle, deadline, move |context| {
        let [selected] = context.devices() else {
            return Err("expected exactly one admitted device".into());
        };
        if selected.target() != "gfx942:xnack-" {
            return Err("receipt profile requires gfx942:xnack-".into());
        }
        let device = selected.id();
        let identity = checked(
            "selected physical identity",
            context.with_gfx942_preparation_device_v1(device, |owner| {
                Ok::<_, String>(owner.observation().unique_id())
            }),
        )?;
        if *identity.value() != case.device {
            return Err("selected physical GPU differs".into());
        }
        let streams = [
            checked("compute stream", context.create_stream(device))?,
            checked("copy stream", context.create_stream(device))?,
        ];
        let mut allocate = |kind| {
            checked(
                "independent copy allocation",
                context.allocate(device, kind, case::FRAME_BYTES as u64, 4096),
            )
        };
        let allocations = [
            allocate(RuntimeMemoryKindV1::HostVisible)?,
            allocate(RuntimeMemoryKindV1::DeviceLocal)?,
            allocate(RuntimeMemoryKindV1::HostVisible)?,
        ];
        checked(
            "stage independent payload",
            context.write_host_visible_allocation_v1(allocations[0], &case::source_frame()),
        )?;
        checked(
            "stage destination guards",
            context.write_host_visible_allocation_v1(allocations[2], &case::destination_frame()),
        )?;
        let warm = checked(
            "initialize complete destination",
            context.copy_async(
                streams[1],
                region(allocations[2], RuntimeAccessV1::Read, 0, case::FRAME_BYTES),
                region(allocations[1], RuntimeAccessV1::Write, 0, case::FRAME_BYTES),
                &[],
            ),
        )?;
        checked("publish initialization", context.flush_stream(streams[1]))?;
        settle(context, warm, deadline)?;
        Ok((device, streams, allocations))
    })?;
    let (output, mut observer) =
        GeneratedRuntimeWriteSlice::new_charged(vec![u32::MAX; case::ELEMENTS].into_boxed_slice());
    let prepare = checked(
        "prepare genuine conditional fill",
        artifact.prepare_generated_context_invocation_async(
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
    let prepared = checked("prepare reply", drive(engine, prepare, deadline)?)?;
    let prepared = checked("prepare fill", prepared)?;
    let reserve = handle
        .try_reserve_prepared_v1(prepared)
        .map_err(|error| format!("reserve fill: {:?}", error.error))?;
    let reserved = checked("reserve reply", drive(engine, reserve, deadline)?)?
        .map_err(|error| format!("reserve fill: {:?}", error.error))?;

    // Use ordinary public Context custody to retain the copy's original receipt.
    // Deliberately do not poll it before the generated publication. This proves
    // host receipt coexistence, not that SDMA is still executing on the GPU.
    let copy = with_context(engine, handle, deadline, move |context| {
        checked(
            "arm exact two-publication witness",
            context.arm_generated_copy_coexistence_qualification_v1(),
        )?;
        let copy = checked(
            "submit independent guarded copy",
            context.copy_async(
                streams[1],
                region(
                    allocations[0],
                    RuntimeAccessV1::Read,
                    case::GUARD_BYTES,
                    case::COPY_BYTES,
                ),
                region(
                    allocations[1],
                    RuntimeAccessV1::Write,
                    case::GUARD_BYTES,
                    case::COPY_BYTES,
                ),
                &[],
            ),
        )?;
        checked("publish independent copy", context.flush_stream(streams[1]))?;
        Ok(copy)
    })?;
    let activate = handle
        .try_activate_generated_v1(reserved, streams[0])
        .map_err(|error| format!("activate fill: {:?}", error.error))?;
    let completion = checked("activate reply", drive(engine, activate, deadline)?)?
        .map_err(|error| format!("activate fill: {:?}", error.error))?;
    let receipt = checked(
        "fill completion",
        checked("completion reply", drive(engine, completion, deadline)?)?,
    )?;
    let output = take_result(&mut observer, &receipt, deadline)?;
    let report = with_context(engine, handle, deadline, move |context| {
        // Take before readback, which may submit additional native copies.
        let witness = checked(
            "exact second-publication witness",
            context.take_generated_copy_coexistence_qualification_v1(),
        )?;
        let report = case.check_witness(case::ReceiptObservation {
            device: witness.device_unique_id(),
            copy_first: witness.first_publication()
                == KfdGeneratedCopyPublicationKindV1::DirectionalCopy,
            packets: witness.copy_packets(),
            identities: [
                witness.compute_receipt_sha256(),
                witness.compute_membership_sha256(),
                witness.copy_receipt_sha256(),
                witness.copy_membership_sha256(),
            ],
        })?;
        settle(context, copy, deadline)?;
        Ok(report)
    })?;
    let (source, destination) = with_context(engine, handle, deadline, move |context| {
        let mut source = vec![0; case::FRAME_BYTES];
        let mut destination = vec![0; case::FRAME_BYTES];
        checked(
            "full independent source readback",
            context.read_allocation(allocations[0], 0, &mut source),
        )?;
        checked(
            "full guarded destination readback",
            context.read_allocation(allocations[1], 0, &mut destination),
        )?;
        for allocation in allocations.into_iter().rev() {
            checked(
                "release copy storage",
                context.release_allocation(allocation),
            )?;
        }
        for stream in streams.into_iter().rev() {
            checked("destroy independent stream", context.destroy_stream(stream))?;
        }
        Ok((source, destination))
    })?;
    case::check_results(output.as_slice(), &source, &destination)?;
    checked(
        "retained artifact after execution",
        artifact.revalidate(deadline),
    )?;
    let drain = checked("begin owned drain", handle.begin_drain(1024))?;
    let drain = checked("owned drain reply", drive(engine, drain, deadline)?)?;
    if drain.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent {
        return Err(format!("native drain incomplete: {drain:?}"));
    }
    Ok((output, report))
}
