//! Genuine protected native V5 entrypoint. No captured artifact or legacy fallback.
mod native_bootstrap;
mod native_v5_case;

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1, GeneratedRuntimeWriteSlice,
    NativeConditionalFillIntakeErrorV1, ProvedNativeConditionalFillApplicationV1 as Proved,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_runtime::{KfdRuntimeBackendV1 as Backend, RuntimeContextV1};
use native_bootstrap::{Result, Summary, checked};
use std::time::{Duration, Instant};
type Context = RuntimeContextV1<Backend>;

fn main() -> Result<()> {
    let case = native_v5_case::parse(std::env::args_os().skip(1))?;
    let executor = checked(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    )?;
    // The sole blocking executor entry is at main, outside every account/owner
    // callback. All three borrowed-owner layers below compose through await.
    executor.block_on(native_bootstrap::run_async(
        &case.producer_source,
        async |proved, budget, deadline| {
            run_gpu(proved, budget, case.device, deadline).await?;
            Ok(Summary {
                schema: "fe2o3.native-conditional-fill.v1",
                fields: format!(
                    "\"device\":\"0x{:016x}\",\"target\":\"gfx942:xnack-\",\"outputs\":[64,37],\"settled_native_launches\":2",
                    case.device
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
            eprintln!("native context initialization refused while retaining backend: {error:?}");
            std::process::abort();
        }
    };
    let operation = execute(application, budget, &mut context, selected, deadline).await;
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("native context cleanup did not settle: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("native backend cleanup did not settle: {error:?}");
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
        return Err("native fixture expected one device".into());
    };
    if device.target() != "gfx942:xnack-" {
        return Err("native fixture rejects non-gfx942 target".into());
    }
    let device = device.id();
    let observed = checked(context.with_gfx942_preparation_device_v1(device, |owner| {
        Ok::<_, String>(owner.observation().unique_id())
    }))?;
    if *observed.value() != selected {
        return Err("native fixture GPU identity differs".into());
    }
    drop(observed);
    let streams = [
        checked(context.create_stream(device))?,
        checked(context.create_stream(device))?,
    ];
    let results = checked(GeneratedRuntimeResultBudgetV1::new(4096, 2))?;
    let (output0, mut observer0) = GeneratedRuntimeWriteSlice::new_charged(
        vec![u32::MAX; native_v5_case::OUTPUT_ELEMENTS[0]].into_boxed_slice(),
    );
    let (output1, mut observer1) = GeneratedRuntimeWriteSlice::new_charged(
        vec![u32::MAX; native_v5_case::OUTPUT_ELEMENTS[1]].into_boxed_slice(),
    );
    let geometry = checked(AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]))?;
    let typed = checked(
        application
            .with_native_invocation_scope_async_v1(budget, deadline, async |native| {
                let nested = context
                    .with_generated_gfx942_scope_async_v1(2, deadline, caller_wake, async |scope| {
                        let first = scope
                            .try_submit_v1(device, streams[0], |checked_device| {
                                native
                                    .prepare_generated_invocation::<fill_write_only_gpu::Marker, _>(
                                        fill_write_only_gpu::RuntimeArguments::new(output0),
                                        checked_device,
                                        geometry,
                                        30_000,
                                        GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
                                        &results,
                                    )
                            })
                            .map_err(|error| {
                                NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                    "first native submission: {error:?}"
                                ))
                            })?;
                        let second = scope
                            .try_submit_v1(device, streams[1], |checked_device| {
                                native
                                    .prepare_generated_invocation::<fill_write_only_gpu::Marker, _>(
                                        fill_write_only_gpu::RuntimeArguments::new(output1),
                                        checked_device,
                                        geometry,
                                        30_000,
                                        GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1),
                                        &results,
                                    )
                            })
                            .map_err(|error| {
                                NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                    "second native submission: {error:?}"
                                ))
                            })?;
                        let first_completion =
                            scope.completion_future_v1(&first).map_err(|error| {
                                NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                    "first native completion observer: {error:?}"
                                ))
                            })?;
                        let second_completion =
                            scope.completion_future_v1(&second).map_err(|error| {
                                NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                    "second native completion observer: {error:?}"
                                ))
                            })?;
                        scope
                            .drive_with_wake_v1(caller_wake)
                            .await
                            .map_err(|error| {
                                NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                    "native settlement: {error:?}"
                                ))
                            })?;
                        first_completion.await.map_err(|error| {
                            NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                "first native completion: {error:?}"
                            ))
                        })?;
                        second_completion.await.map_err(|error| {
                            NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                "second native completion: {error:?}"
                            ))
                        })?;
                        let first = observer0
                            .take_scoped_completed_v1(scope, &first)
                            .map_err(|error| {
                                NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                    "first decoder: {error:?}"
                                ))
                            })?
                            .ok_or_else(|| {
                                NativeConditionalFillIntakeErrorV1::Rejected(
                                    "first result contention".into(),
                                )
                            })?;
                        let second = observer1
                            .take_scoped_completed_v1(scope, &second)
                            .map_err(|error| {
                                NativeConditionalFillIntakeErrorV1::Rejected(format!(
                                    "second decoder: {error:?}"
                                ))
                            })?
                            .ok_or_else(|| {
                                NativeConditionalFillIntakeErrorV1::Rejected(
                                    "second result contention".into(),
                                )
                            })?;
                        Ok((first, second))
                    })
                    .await
                    .map_err(|error| {
                        NativeConditionalFillIntakeErrorV1::Rejected(format!(
                            "native lexical scope: {error:?}"
                        ))
                    })?;
                nested
            })
            .await,
    )?;
    native_v5_case::check_output(0, typed.0.as_slice())?;
    native_v5_case::check_output(1, typed.1.as_slice())?;
    drop(typed);
    drop((observer0, observer1));
    for stream in streams {
        checked(context.destroy_stream(stream))?;
    }
    let usage = results.usage();
    if usage.reserved_peak_bytes != 0
        || usage.retained_members != 0
        || usage.quarantined_members != 0
        || usage.unissued_members != 0
        || usage.poisoned
    {
        return Err(format!(
            "native result credits not fully refunded: {usage:?}"
        ));
    }
    Ok(())
}

fn caller_wake(deadline: Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(
        std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
    )
}
