//! Actual original-custody caller; compilation is not native qualification.
mod native_bootstrap;
mod native_independent_arena2048_case;
mod native_registry4_case;

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_conditional_custodian_application::fill_write_only_gpu;
use fe2o3_host::{
    GeneratedRuntimeArgumentLimitsV1, GeneratedRuntimeResultBudgetV1, GeneratedRuntimeWriteSlice,
    NativeConditionalFillIntakeErrorV1 as IntakeError,
    ProvedNativeConditionalFillApplicationV1 as Proved,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_resource_accounting::{
    HostMetadataTableV1, ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1,
};
use fe2o3_runtime::{KfdRuntimeBackendV1 as Backend, RuntimeContextV1};
use native_bootstrap::{Result, Summary, checked};
use native_independent_arena2048_case::{self as case, Report, Witness};
use std::time::{Duration, Instant};

type Context = RuntimeContextV1<Backend>;

fn main() -> Result<()> {
    let selected = case::parse(std::env::args_os().skip(1))?;
    let executor = checked(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    )?;
    executor.block_on(native_bootstrap::run_async(
        &selected.producer_source,
        async |proved, budget, deadline| {
            let report = run_gpu(proved, budget, selected.device, deadline).await?;
            Ok(Summary {
                schema: "fe2o3.native-independent-fill-arena2048.v1",
                fields: report.fields(selected.device)?,
            })
        },
    ))
}

async fn run_gpu<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    selected: u64,
    deadline: Instant,
) -> Result<Report> {
    let backend = checked(Backend::open_worker_v3_generated_only_v1(selected))?;
    let mut context = match Context::open_with_version_journal_v1(backend, 32, 32) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("arena Context initialization retained backend: {error:?}");
            std::process::abort();
        }
    };
    let result = execute(application, budget, &mut context, selected, deadline).await;
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("arena Context cleanup did not settle: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("arena native cleanup did not settle: {error:?}");
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
) -> Result<Report> {
    let [device] = context.devices() else {
        return Err("arena expected one device".into());
    };
    if device.target() != "gfx942:xnack-" {
        return Err("arena rejects non-gfx942 target".into());
    }
    let device = device.id();
    let original = checked(context.with_gfx942_preparation_device_v1(device, |owner| {
        Ok::<_, String>(owner.observation().unique_id())
    }))?;
    if *original.value() != selected {
        return Err("arena GPU identity differs".into());
    }
    drop(original);
    let stream = checked(context.create_stream(device))?;
    // Finite, explicit metadata/result domains, not replacement canonical proof
    // accounts. Every table is charged before the first driver/native step.
    let metadata = checked(ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, case::METADATA_BYTES),
        case::METADATA_RECORDS,
    ))?;
    let results = checked(GeneratedRuntimeResultBudgetV1::new(
        case::RESULT_BYTES,
        case::RESULT_RECORDS,
    ))?;
    let mut observers = checked(HostMetadataTableV1::try_new(
        case::MEMBERS,
        Some(&metadata),
        || None,
    ))?;
    let report = checked(application.with_native_invocation_scope_async_v1(budget, deadline, async |native| {
        context.with_generated_gfx942_independent_arena2048_scope_async_v1(
            metadata.clone(), deadline, caller_wake, async |scope| {
                let mut tickets = HostMetadataTableV1::try_new(case::MEMBERS, Some(&metadata), || None).map_err(refused)?;
                let mut futures = HostMetadataTableV1::try_new(case::MEMBERS, Some(&metadata), || None).map_err(refused)?;
                let mut next = 0;
                scope.try_submit_v1(device, stream, |checked_device| {
                    native.prepare_generated_independent_arena2048::<fill_write_only_gpu::Marker, _>(
                        &metadata, checked_device, &mut |member| {
                            if member != next { return Err(refused("original argument occurrence differs")); }
                            let count = case::count(member).map_err(refused)?;
                            let mut seed = Vec::new();
                            seed.try_reserve_exact(count).map_err(refused)?;
                            seed.resize(count, u32::MAX);
                            let (input, observer) = GeneratedRuntimeWriteSlice::new_charged(seed.into_boxed_slice());
                            observers[member] = Some(observer);
                            next += 1;
                            let geometry = AqlDispatchGeometryV1::new([count as u32, 1, 1], [64, 1, 1]).map_err(refused)?;
                            Ok((fill_write_only_gpu::RuntimeArguments::new(input), geometry))
                        }, 30_000,
                        GeneratedRuntimeArgumentLimitsV1::new(8 * case::LONG + 8192, 8 * case::LONG, 1),
                        &results,
                    )
                }).map_err(refused)?;
                if next != case::MEMBERS { return Err(refused("arena original source roster incomplete")); }
                for member in 0..case::MEMBERS {
                    let ticket = scope.ticket_v1(member).map_err(refused)?;
                    futures[member] = Some(scope.result_future_v1(&ticket).map_err(refused)?);
                    tickets[member] = Some(ticket);
                }
                // Snapshot actual prepaid logical charges before driver polling.
                // This is neither total RSS nor native GPU-memory telemetry.
                let before = metadata.usage();
                let result_before = results.usage();
                if scope.receipt_observations_v1().is_none_or(|r| r.events != 0)
                    || result_before.reserved_peak_bytes != 3 * case::SOURCE_BYTES
                    || result_before.retained_members + result_before.unissued_members != 2 * case::MEMBERS
                    || result_before.poisoned || result_before.quarantined_members != 0
                    || before.poisoned || before.reserved_records != 0 || before.quarantined_records != 0
                { return Err(refused("prelaunch original charges or receipt state differ")); }
                case::observation::all_copied(scope.drive_with_wake_v1(caller_wake), &mut futures).await.map_err(refused)?;
                if scope.pending_v1() != 1 { return Err(refused("common arena closed before all results")); }
                let receipt = scope.receipt_observations_v1().ok_or_else(|| refused("missing original receipt observations"))?;
                for member in 0..case::MEMBERS {
                    let seen = scope.member_receipt_observation_v1(member).ok_or_else(|| refused("missing original member"))?;
                    if seen.published == 0 || seen.completed <= seen.published
                        || (seen.last_pending != 0 && (seen.last_pending <= seen.published || seen.last_pending >= seen.completed))
                    { return Err(refused("member original receipt chronology differs")); }
                    let typed = observers[member].as_mut().ok_or_else(|| refused("missing original result observer"))?
                        .take_independent_arena2048_result_v1(scope, tickets[member].as_ref().ok_or_else(|| refused("missing original ticket"))?)
                        .map_err(refused)?.ok_or_else(|| refused("arena result contention"))?;
                    case::check_output(member, typed.as_slice()).map_err(refused)?;
                    drop(typed);
                }
                let residual = results.usage();
                let tables = metadata.usage();
                if scope.pending_v1() != 1 || residual.unissued_members != 0 || residual.quarantined_members != 0
                    || residual.poisoned || tables.reserved_records != 0 || tables.quarantined_records != 0 || tables.poisoned
                { return Err(refused("early result disposal lost common custody")); }
                let report = Report {
                    events: receipt.events, publications: receipt.publications, completions: receipt.completions,
                    pending: receipt.pending_observations, maximum_unobserved: receipt.maximum_published_without_observed_completion,
                    pre_metadata_bytes: before.used.get(ResourceKindV1::ControlResidentBytes), pre_metadata_records: before.retained_records,
                    residual_metadata_bytes: tables.used.get(ResourceKindV1::ControlResidentBytes), residual_metadata_records: tables.retained_records,
                    pre_result_bytes: result_before.reserved_peak_bytes, pre_result_records: result_before.retained_members + result_before.unissued_members,
                    residual_source_bytes: residual.reserved_peak_bytes, residual_source_records: residual.retained_members,
                    witness: receipt.out_of_order.map(|w| Witness { earlier: w.earlier_member, later: w.later_member,
                        earlier_published: w.earlier_published, later_published: w.later_published,
                        later_completed: w.later_completed, earlier_pending: w.earlier_pending }),
                };
                report.validate().map_err(refused)?;
                // All source residuals still exist. Resume this same driver for
                // real common queue destruction and exact Context settlement.
                scope.drive_with_wake_v1(caller_wake).await.map_err(refused)?;
                if scope.pending_v1() != 0 { return Err(refused("common arena destruction incomplete")); }
                Ok(report)
            }).await.map_err(refused)?
    }).await)?;
    drop(observers);
    checked(context.destroy_stream(stream))?;
    let output = results.usage();
    let tables = metadata.usage();
    if output.reserved_peak_bytes != 0
        || output.retained_members != 0
        || output.unissued_members != 0
        || output.quarantined_members != 0
        || output.poisoned
        || tables.used != ResourceVectorV1::ZERO
        || tables.retained_records != 0
        || tables.reserved_records != 0
        || tables.quarantined_records != 0
        || tables.poisoned
    {
        return Err(format!(
            "arena original credits retained after disposal: {output:?}, {tables:?}"
        ));
    }
    Ok(report)
}

fn refused(error: impl std::fmt::Debug) -> IntakeError {
    IntakeError::Rejected(format!("native independent Arena2048: {error:?}"))
}
fn caller_wake(deadline: Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(
        std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
    )
}
