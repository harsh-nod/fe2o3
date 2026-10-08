//! Genuine bounded reuse caller. Compilation is not protected/native execution.
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
    let case = native_registry4_case::parse_repeat2(std::env::args_os().skip(1))?;
    let executor = checked(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    )?;
    executor.block_on(native_bootstrap::run_async(&case.producer_source, async |proved, budget, deadline| {
        run_gpu(proved, budget, case.device, deadline).await?;
        Ok(Summary {
            schema: "fe2o3.native-conditional-fill-registry4-repeat2.v1",
            fields: format!(concat!(
                "\"device\":\"0x{:016x}\",\"target\":\"gfx942:xnack-\",",
                "\"outputs_per_cycle\":[1,37,65,129],\"grid_x\":[64,64,128,192],",
                "\"native_publications\":8,\"max_outstanding_bound\":4,\"cycles\":2,",
                "\"copied_results\":8,\"old_results_retained_through_second_cycle\":4,",
                "\"old_results_retained_through_common_destroy\":4,",
                "\"original_sources_retained_until_common_destroy\":true,",
                "\"common_registry_destroyed\":true,\"metadata_credits\":\"refunded\",",
                "\"rolling_new_source_admission\":false,\"native_out_of_order_measured\":false,",
                "\"thousands_inflight_qualified\":false"), case.device),
        })
    }))
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
            eprintln!("repeat2 Context initialization refused: {error:?}");
            std::process::abort();
        }
    };
    let operation = execute(application, budget, &mut context, selected, deadline).await;
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("repeat2 Context cleanup did not settle: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("repeat2 native cleanup did not settle: {error:?}");
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
        return Err("repeat2 expected one device".into());
    };
    if device.target() != "gfx942:xnack-" {
        return Err("repeat2 requires gfx942".into());
    }
    let device = device.id();
    let observed = checked(context.with_gfx942_preparation_device_v1(device, |owner| {
        Ok::<_, String>(owner.observation().unique_id())
    }))?;
    if *observed.value() != selected {
        return Err("repeat2 original GPU identity differs".into());
    }
    drop(observed);
    let stream = checked(context.create_stream(device))?;
    let metadata = checked(ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, 1024 * 1024),
        6,
    ))?;
    // Both cycles are prepaid before native entry. The same ledger remains live;
    // a previous typed result can retain its indivisible credit across rearm.
    let results = checked(GeneratedRuntimeResultBudgetV1::new(7 * SOURCE_BYTES, 20))?;
    let [(a, oa), (b, ob), (c, oc), (d, od)] = OUTPUT_ELEMENTS.map(|count| {
        GeneratedRuntimeWriteSlice::new_charged(vec![u32::MAX; count].into_boxed_slice())
    });
    let arguments = [a, b, c, d].map(fill_write_only_gpu::RuntimeArguments::new);
    let mut first_observers = [oa, ob, oc, od];
    let mut retained_first = [None, None, None, None];
    let [a, b, c, d] = GRID_X.map(|grid| AqlDispatchGeometryV1::new([grid, 1, 1], [64, 1, 1]));
    let geometries = [checked(a)?, checked(b)?, checked(c)?, checked(d)?];
    checked(application.with_native_invocation_scope_async_v1(budget, deadline, async |native| {
        context.with_generated_gfx942_registry4_repeat2_scope_async_v1(metadata.clone(), deadline, caller_wake, async |scope| {
            let mut second_observers = None;
            let [first, second] = scope.try_submit_repeat2_v1(device, stream, |device| {
                let (owner, observers) = native.prepare_generated_registry4_repeat2::<fill_write_only_gpu::Marker, _>(
                    arguments, device, geometries, 30_000,
                    GeneratedRuntimeArgumentLimitsV1::new(16384, 16384, 1), &results,
                )?;
                second_observers = Some(observers);
                Ok::<_, IntakeError>(owner)
            }).map_err(refused)?;
            let mut second_observers = second_observers.ok_or_else(|| refused("second frame absent"))?;
            let first_futures = [
                scope.result_future_v1(&first[0]).map_err(refused)?, scope.result_future_v1(&first[1]).map_err(refused)?,
                scope.result_future_v1(&first[2]).map_err(refused)?, scope.result_future_v1(&first[3]).map_err(refused)?,
            ];
            let second_futures = [
                scope.result_future_v1(&second[0]).map_err(refused)?, scope.result_future_v1(&second[1]).map_err(refused)?,
                scope.result_future_v1(&second[2]).map_err(refused)?, scope.result_future_v1(&second[3]).map_err(refused)?,
            ];
            observation::all_copied_before_close(scope.drive_with_wake_v1(caller_wake), first_futures).await.map_err(refused)?;
            if scope.pending_v1() != 1 { return Err(refused("first cycle lost common root")) }
            for member in 0..4 {
                let result = first_observers[member].take_registry4_result_v1(scope, &first[member]).map_err(refused)?
                    .ok_or_else(|| refused("first result contention"))?;
                native_registry4_case::check_output(member, result.as_slice()).map_err(refused)?;
                retained_first[member] = Some(result);
            }
            check_results(&results, 6 * SOURCE_BYTES, 16).map_err(refused)?;
            observation::all_copied_before_close(scope.drive_with_wake_v1(caller_wake), second_futures).await.map_err(refused)?;
            if scope.pending_v1() != 1 { return Err(refused("second cycle lost common root")) }
            for member in 0..4 {
                let result = second_observers[member].take_registry4_result_v1(scope, &second[member]).map_err(refused)?
                    .ok_or_else(|| refused("second result contention"))?;
                native_registry4_case::check_output(member, result.as_slice()).map_err(refused)?;
                let old = retained_first[member].as_ref().ok_or_else(|| refused("old result lost"))?;
                native_registry4_case::check_output(member, old.as_slice()).map_err(refused)?;
                drop(result);
            }
            check_results(&results, 3 * SOURCE_BYTES, 8).map_err(refused)?;
            let tables = metadata.usage();
            if tables.retained_records != 6 || tables.used == ResourceVectorV1::ZERO || tables.poisoned {
                return Err(refused("common metadata released before destruction"));
            }
            scope.drive_with_wake_v1(caller_wake).await.map_err(refused)?;
            if scope.pending_v1() != 0 { return Err(refused("common root not destroyed")) }
            check_results(&results, 2 * SOURCE_BYTES, 4).map_err(refused)?;
            Ok(())
        }).await.map_err(refused)?
    }).await)?;
    for (member, result) in retained_first.into_iter().enumerate() {
        let result = result.ok_or("missing old typed result after destruction")?;
        native_registry4_case::check_output(member, result.as_slice())?;
        drop(result);
    }
    drop(first_observers);
    check_results(&results, 0, 0)?;
    checked(context.destroy_stream(stream))?;
    let tables = metadata.usage();
    if tables.used != ResourceVectorV1::ZERO
        || tables.retained_records != 0
        || tables.reserved_records != 0
        || tables.quarantined_records != 0
        || tables.poisoned
    {
        return Err(format!(
            "repeat2 metadata remains after destruction: {tables:?}"
        ));
    }
    Ok(())
}

fn check_results(
    results: &GeneratedRuntimeResultBudgetV1,
    bytes: u64,
    records: usize,
) -> Result<()> {
    let usage = results.usage();
    if usage.reserved_peak_bytes != bytes
        || usage.retained_members != records
        || usage.unissued_members != 0
        || usage.quarantined_members != 0
        || usage.poisoned
    {
        return Err(format!("repeat2 original result debit differs: {usage:?}"));
    }
    Ok(())
}

fn refused(error: impl std::fmt::Debug) -> IntakeError {
    IntakeError::Rejected(format!("native Registry4 repeat2: {error:?}"))
}
fn caller_wake(deadline: Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(
        std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
    )
}
