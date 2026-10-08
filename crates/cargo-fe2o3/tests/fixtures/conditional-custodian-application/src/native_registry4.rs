//! Genuine four-original native caller. Binding-only compilation is not execution.
mod native_bootstrap;
mod native_registry4_case;

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1, GeneratedRuntimeWriteSlice,
    NativeConditionalFillIntakeErrorV1 as IntakeError,
    ProvedNativeConditionalFillApplicationV1 as Proved,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};
use fe2o3_runtime::{KfdRuntimeBackendV1 as Backend, RuntimeContextV1};
use native_bootstrap::{Result, Summary, checked};
use native_registry4_case::{GRID_X, OUTPUT_ELEMENTS, SOURCE_BYTES, observation};
use std::time::{Duration, Instant};

type Context = RuntimeContextV1<Backend>;

fn main() -> Result<()> {
    let case = native_registry4_case::parse(std::env::args_os().skip(1))?;
    let executor = checked(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    )?;
    executor.block_on(native_bootstrap::run_async(
        &case.producer_source,
        async |proved, budget, deadline| {
            run_gpu(proved, budget, case.device, deadline).await?;
            Ok(Summary {
                schema: "fe2o3.native-conditional-fill-registry4.v1",
                fields: format!(
                    concat!(
                        "\"device\":\"0x{:016x}\",\"target\":\"gfx942:xnack-\",",
                        "\"outputs\":[1,37,65,129],\"grid_x\":[64,64,128,192],",
                        "\"copied_results\":4,\"results_dropped_before_common_destroy\":4,",
                        "\"source_credits_retained_after_result_drop\":true,",
                        "\"common_registry_destroyed\":true,\"metadata_credits\":\"refunded\",",
                        "\"rolling_rearm\":false,\"native_out_of_order_measured\":false,",
                        "\"thousands_inflight_qualified\":false"
                    ),
                    case.device,
                ),
            })
        },
    ))
}

async fn run_gpu<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    selected: u64,
    deadline: Instant,
) -> Result<()> {
    let backend = checked(Backend::open_worker_v3_generated_only_v1(selected))?;
    let mut context = match Context::open_with_version_journal_v1(backend, 16, 16) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("registry Context initialization refused with retained backend: {error:?}");
            std::process::abort();
        }
    };
    let operation = execute(application, budget, &mut context, selected, deadline).await;
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("registry Context cleanup did not settle: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("registry native cleanup did not settle: {error:?}");
        std::process::abort();
    }
    drop(backend);
    operation
}

async fn execute<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    context: &mut Context,
    selected: u64,
    deadline: Instant,
) -> Result<()> {
    let [device] = context.devices() else {
        return Err("registry fixture expected one device".into());
    };
    if device.target() != "gfx942:xnack-" {
        return Err("registry fixture rejects non-gfx942 target".into());
    }
    let device = device.id();
    let observed = checked(context.with_gfx942_preparation_device_v1(device, |owner| {
        Ok::<_, String>(owner.observation().unique_id())
    }))?;
    if *observed.value() != selected {
        return Err("registry fixture GPU identity differs".into());
    }
    drop(observed);
    let stream = checked(context.create_stream(device))?;
    // These explicit resource domains precede native admission. Cloning the
    // metadata handle retains the same ledger, not a replacement funded account.
    let metadata = checked(ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1024 * 1024),
        6,
    ))?;
    let results = checked(GeneratedRuntimeResultBudgetV1::new(4096, 8))?;
    let [(a, oa), (b, ob), (c, oc), (d, od)] = OUTPUT_ELEMENTS.map(|count| {
        GeneratedRuntimeWriteSlice::new_charged(vec![u32::MAX; count].into_boxed_slice())
    });
    let arguments = [a, b, c, d].map(fill_write_only_gpu::RuntimeArguments::new);
    let mut observers = [oa, ob, oc, od];
    let [ga, gb, gc, gd] = GRID_X.map(|grid| AqlDispatchGeometryV1::new([grid, 1, 1], [64, 1, 1]));
    let geometries = [checked(ga)?, checked(gb)?, checked(gc)?, checked(gd)?];
    checked(
        application
            .with_native_invocation_scope_async_v1(budget, deadline, async |native| {
                context
                    .with_generated_gfx942_registry4_scope_async_v1(
                        metadata.clone(),
                        deadline,
                        caller_wake,
                        async |scope| {
                            let tickets = scope
                                .try_submit_v1(device, stream, |checked_device| {
                                    native.prepare_generated_registry4::<fill_write_only_gpu::Marker, _>(
                                        arguments,
                                        checked_device,
                                        geometries,
                                        30_000,
                                        GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
                                        &results,
                                    )
                                })
                                .map_err(refused)?;
                            let futures = [
                                scope.result_future_v1(&tickets[0]).map_err(refused)?,
                                scope.result_future_v1(&tickets[1]).map_err(refused)?,
                                scope.result_future_v1(&tickets[2]).map_err(refused)?,
                                scope.result_future_v1(&tickets[3]).map_err(refused)?,
                            ];
                            observation::all_copied_before_close(
                                scope.drive_with_wake_v1(caller_wake),
                                futures,
                            )
                            .await
                            .map_err(refused)?;
                            // All four observers, not merely the first, resolved.
                            // The driver loan has ended; the original scope still
                            // owns common native DATA and all source residuals.
                            if scope.pending_v1() != 1 {
                                return Err(refused("common registry retired before result observation"));
                            }
                            for member in 0..4 {
                                let typed = observers[member]
                                    .take_registry4_result_v1(scope, &tickets[member])
                                    .map_err(refused)?
                                    .ok_or_else(|| refused("registry result contention"))?;
                                native_registry4_case::check_output(member, typed.as_slice())
                                    .map_err(refused)?;
                                drop(typed);
                            }
                            let residual = results.usage();
                            let tables = metadata.usage();
                            if scope.pending_v1() != 1
                                || residual.reserved_peak_bytes != SOURCE_BYTES
                                || residual.retained_members != 4
                                || residual.unissued_members != 0
                                || residual.quarantined_members != 0
                                || residual.poisoned
                                || tables.retained_records != 6
                                || tables.reserved_records != 0
                                || tables.quarantined_records != 0
                                || tables.used == ResourceVectorV1::ZERO
                                || tables.poisoned
                            {
                                return Err(refused("early result disposal lost original residual credits"));
                            }
                            // Resume the same original scope and shared driver;
                            // copied results did not retire any native DATA.
                            scope.drive_with_wake_v1(caller_wake).await.map_err(refused)?;
                            if scope.pending_v1() != 0 {
                                return Err(refused("registry remained pending after common destroy"));
                            }
                            Ok(())
                        },
                    )
                    .await
                    .map_err(refused)?
            })
            .await,
    )?;
    drop(observers);
    checked(context.destroy_stream(stream))?;
    let usage = results.usage();
    let tables = metadata.usage();
    if usage.reserved_peak_bytes != 0
        || usage.retained_members != 0
        || usage.quarantined_members != 0
        || usage.unissued_members != 0
        || usage.poisoned
        || tables.used != ResourceVectorV1::ZERO
        || tables.reserved_records != 0
        || tables.retained_records != 0
        || tables.quarantined_records != 0
        || tables.poisoned
    {
        return Err(format!(
            "registry credits remain after destruction: {usage:?}, {tables:?}"
        ));
    }
    Ok(())
}

fn refused(error: impl std::fmt::Debug) -> IntakeError {
    IntakeError::Rejected(format!("native Registry4: {error:?}"))
}

fn caller_wake(deadline: Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(
        std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
    )
}
