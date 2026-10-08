//! Opt-in genuine Singleton DATA-to-device-local copy; no captured native authority.
mod native_bootstrap;
mod native_data_copy_case;

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1, GeneratedRuntimeWriteSlice,
    NativeConditionalFillIntakeErrorV1 as IntakeError,
    ProvedNativeConditionalFillApplicationV1 as Proved,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_runtime::{
    KfdRuntimeBackendV1 as Backend, RuntimeAccessV1, RuntimeAllocationVersionObservationV1,
    RuntimeContextV1, RuntimeMemoryKindV1, RuntimeMemoryRegionV1,
};
use native_bootstrap::{Result, Summary, checked};
use native_data_copy_case::{BYTES, ELEMENTS, Verified, Version};
use std::time::{Duration, Instant};
type Context = RuntimeContextV1<Backend>;

fn main() -> Result<()> {
    let case = native_data_copy_case::parse(std::env::args_os().skip(1))?;
    let executor = checked(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    )?;
    executor.block_on(native_bootstrap::run_async(
        &case.producer_source,
        async |application, budget, deadline| {
            let verified = run(application, budget, case.device, deadline).await?;
            Ok(Summary {
                schema: "fe2o3.native-generated-data-copy.v1",
                fields: verified.fields(case.device)?,
            })
        },
    ))
}

async fn run<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    selected: u64,
    deadline: Instant,
) -> Result<Verified> {
    let backend = checked(Backend::open_worker_v3_generated_only_v1(selected))?;
    let mut context = match Context::open_with_version_journal_v1(backend, 16, 16) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("DATA-copy Context initialization retains backend: {error:?}");
            std::process::abort();
        }
    };
    let result = execute(application, budget, &mut context, selected, deadline).await;
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("DATA-copy Context cleanup retains custody: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("DATA-copy backend cleanup retains custody: {error:?}");
        std::process::abort();
    }
    drop(backend);
    result
}

async fn execute<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    context: &mut Context,
    selected: u64,
    deadline: Instant,
) -> Result<Verified> {
    let [device] = context.devices() else {
        return Err("DATA-copy requires one device".into());
    };
    if device.target() != "gfx942:xnack-" {
        return Err("DATA-copy requires gfx942:xnack-".into());
    }
    let device = device.id();
    let observed = checked(context.with_gfx942_preparation_device_v1(device, |owner| {
        Ok::<_, String>(owner.observation().unique_id())
    }))?;
    if *observed.value() != selected {
        return Err("DATA-copy original device identity differs".into());
    }
    drop(observed);

    let producer_stream = checked(context.create_stream(device))?;
    let copy_stream = checked(context.create_stream(device))?;
    if producer_stream == copy_stream {
        return Err("DATA-copy requires distinct original streams".into());
    }
    let allocation =
        checked(context.allocate(device, RuntimeMemoryKindV1::DeviceLocal, BYTES as u64, 8))?;
    let destination = RuntimeMemoryRegionV1 {
        allocation,
        access: RuntimeAccessV1::Write,
        byte_offset: 0,
        byte_len: BYTES as u64,
    };
    let before = checked(context.observe_allocation_version_qualification_v1(destination))?;
    let results = checked(GeneratedRuntimeResultBudgetV1::new(4096, 1))?;
    let (output, mut observer) =
        GeneratedRuntimeWriteSlice::new_charged(vec![u32::MAX; ELEMENTS].into_boxed_slice());
    let geometry = checked(AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]))?;
    let typed = checked(
        application
            .with_native_invocation_scope_async_v1(budget, deadline, async |native| {
                context
                    .with_generated_gfx942_scope_async_v1(1, deadline, caller_wake, async |scope| {
                        let ticket = scope
                            .try_submit_sdma_backed_v1(device, producer_stream, |checked_device| {
                                native
                                    .prepare_generated_invocation::<fill_write_only_gpu::Marker, _>(
                                        fill_write_only_gpu::RuntimeArguments::new(output),
                                        checked_device,
                                        geometry,
                                        30_000,
                                        GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
                                        &results,
                                    )
                            })
                            .map_err(rejected)?;
                        // Registration precedes adoption. The existing owner path retains DATA,
                        // its promotion bridge and the destination writer through physical release.
                        scope
                            .copy_generated_output_to_v1(&ticket, copy_stream, destination)
                            .map_err(rejected)?;
                        let completion = scope.completion_future_v1(&ticket).map_err(rejected)?;
                        scope
                            .drive_with_wake_v1(caller_wake)
                            .await
                            .map_err(rejected)?;
                        completion.await.map_err(rejected)?;
                        observer
                            .take_scoped_completed_v1(scope, &ticket)
                            .map_err(rejected)?
                            .ok_or_else(|| {
                                IntakeError::Rejected("DATA-copy result contention".into())
                            })
                    })
                    .await
                    .map_err(rejected)?
            })
            .await,
    )?;
    let after = checked(context.observe_allocation_version_qualification_v1(destination))?;
    if before.allocation() != allocation
        || after.allocation() != allocation
        || before.device() != device
        || after.device() != device
        || before.byte_extent() != BYTES as u64
        || after.byte_extent() != BYTES as u64
    {
        return Err("DATA-copy observation identity or extent changed".into());
    }
    // A snapshot is not read permission. The ordinary API revalidates the live
    // allocation and absence of a writer before the actual destination readback.
    let mut bytes = [0xff; BYTES];
    checked(context.read_allocation(allocation, 0, &mut bytes))?;
    if checked(context.observe_allocation_version_qualification_v1(destination))? != after {
        return Err("DATA-copy journal changed during readback".into());
    }
    let verified =
        native_data_copy_case::verify(typed.as_slice(), &bytes, version(before), version(after))?;
    drop(typed);
    drop(observer);
    let usage = results.usage();
    if usage.reserved_peak_bytes != 0
        || usage.retained_members != 0
        || usage.quarantined_members != 0
        || usage.unissued_members != 0
        || usage.poisoned
    {
        return Err(format!(
            "DATA-copy original result credits not refunded: {usage:?}"
        ));
    }
    checked(context.release_allocation(allocation))?;
    checked(context.destroy_stream(copy_stream))?;
    checked(context.destroy_stream(producer_stream))?;
    Ok(verified)
}

fn version(value: RuntimeAllocationVersionObservationV1) -> Version {
    Version {
        attempt: value.attempt_epoch(),
        lineage: value.content_lineage(),
    }
}

fn rejected(error: impl std::fmt::Debug) -> IntakeError {
    IntakeError::Rejected(format!("native DATA-copy: {error:?}"))
}

fn caller_wake(deadline: Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(
        std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
    )
}
