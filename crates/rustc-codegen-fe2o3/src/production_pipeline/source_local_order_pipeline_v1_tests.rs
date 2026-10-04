//! Genuine source-owned connected V12 -> target-bound V12 -> diagnostic order.
//! Intended child of source_bitselect_candidate_v1_tests; no policy hook.
use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::local_order::capture_local_order;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_kernel_opt::{
    CheckedU32LocalOrderErrorV1, U32LocalOrderPreferenceV1 as Preference,
    U32LocalOrderRegionV1 as Region, schedule_checked_u32_local_order_v1 as schedule,
};

use super::local_order_join as join;
#[path = "source_local_order_simulation_v1_tests.rs"]
mod simulation;

#[path = "source_local_order_recipe_pipeline_v1_tests.rs"]
mod recipes;

#[path = "source_local_order_continuation_v1_tests.rs"]
mod continuation;

const CANONICAL_WORK: usize = 20_000_000;
const CANONICAL_STORAGE: usize = 32 * 1024 * 1024;

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_source_local_order_feasibility(
        self,
        mut input: RetainedInput,
    ) -> Result<Value, String> {
        let tcx = self.stage.tcx;
        let mut captured = capture_local_order(tcx, &self.stage.closure, &input)?;
        let imported = self
            .import_semantic_mir()
            .map_err(|e| format!("local-order import: {e}"))?;
        let middle = imported
            .construct_semantic_middle_end()
            .map_err(|e| format!("local-order middle: {e}"))?;
        let ssa = middle
            .construct_semantic_ssa()
            .map_err(|e| format!("local-order SSA: {e}"))?;
        let materialized = ssa
            .materialize_target_neutral()
            .map_err(|e| format!("local-order materialization: {e}"))?;
        let source_floor = materialized
            .materialized
            .unit_local_source_storage_floor_v1()
            .map_err(|e| e.to_string())?;
        let ranked = materialized
            .verify_general_kernel_checks()
            .map_err(|e| format!("local-order ranked: {e}"))?;
        let neutral = ranked
            .attach_target_neutral_checks()
            .map_err(|e| format!("local-order attachment: {e}"))?;
        captured.recheck(tcx, &mut input)?;
        let joined = join::exact_join(&mut captured, &neutral.lowered)?;
        let source = neutral
            .lowered
            .pre_ranked_executable()
            .ok_or("local-order genuine connected V12 absent")?;
        if !std::ptr::eq(source.module(), neutral.lowered.module()) {
            return Err(
                "local-order correspondence and connected V12 are not the same live graph".into(),
            );
        }
        let mut work = Work::new(CANONICAL_WORK);
        let mut budget = Budget::new(&mut work, CANONICAL_STORAGE);
        budget
            .reserve_storage(source_floor)
            .map_err(|e| e.to_string())?;
        // Exactly the existing target-owner API over the actual connected N.
        // Its inherited bounded native-binding allocation domain is unchanged;
        // this ledger covers V12 admission, ordering and replay, not all RSS.
        let binding = dialect_amdgcn::bind_production_target_v1(
            source.module(),
            neutral.bindings.rustc_target.profile(),
        )
        .map_err(|e| format!("local-order actual target binding: {e}"))?;
        let (bound, bound_storage) =
            Owner::from_module_ref_with_verification_budget_v12(binding.module(), &mut budget)
                .map_err(|e| format!("local-order bound V12 admission: {e}"))?;
        budget
            .reserve_storage(bound_storage.retained_storage())
            .map_err(|e| e.to_string())?;
        drop(binding);
        let region = joined.bound_region(&bound, &mut captured.meter)?;
        let floor = budget.storage();
        let mut malformed = region;
        malformed.operation_count = 1;
        if !matches!(
            schedule(&bound, malformed, Preference::ReverseReady, &mut budget),
            Err(CheckedU32LocalOrderErrorV1::RegionBounds)
        ) || budget.storage() != floor
        {
            return Err("local-order malformed-region refusal or storage floor changed".into());
        }
        let after_refusal = budget.work();
        let first = schedule(&bound, region, Preference::SourceOrder, &mut budget)
            .map_err(|e| e.to_string())?;
        budget
            .reserve_storage(first.retained_storage())
            .map_err(|e| e.to_string())?;
        first.replay(&mut budget).map_err(|e| e.to_string())?;
        let second = schedule(&bound, region, Preference::ReverseReady, &mut budget)
            .map_err(|e| e.to_string())?;
        budget
            .reserve_storage(second.retained_storage())
            .map_err(|e| e.to_string())?;
        second.replay(&mut budget).map_err(|e| e.to_string())?;
        let repeated = schedule(&bound, region, Preference::ReverseReady, &mut budget)
            .map_err(|e| e.to_string())?;
        budget
            .reserve_storage(repeated.retained_storage())
            .map_err(|e| e.to_string())?;
        repeated.replay(&mut budget).map_err(|e| e.to_string())?;
        captured
            .meter
            .scan(bound.canonical().canonical_bytes().len())?;
        captured
            .meter
            .scan(second.output().canonical().canonical_bytes().len())?;
        if first.output().canonical().canonical_bytes() != bound.canonical().canonical_bytes()
            || second.output().canonical().identity() == first.output().canonical().identity()
            || repeated.output().canonical().canonical_bytes()
                != second.output().canonical().canonical_bytes()
            || joined.result_order(first.output(), region)? != joined.results
            || joined.result_order(second.output(), region)?
                != [joined.results[1], joined.results[0], joined.results[2]]
            || budget.work() <= after_refusal
            || budget.storage()
                != floor
                    + first.retained_storage()
                    + second.retained_storage()
                    + repeated.retained_storage()
        {
            return Err("local-order actual order, replay, identity or accounting mismatch".into());
        }
        let mut oracle_ledger = simulation::OracleLedger::default();
        let first_sim = simulation::observe(first.output(), &mut oracle_ledger)?;
        let second_sim = simulation::observe(second.output(), &mut oracle_ledger)?;
        captured.recheck(tcx, &mut input)?;
        captured.meter.storage(64 * 1024)?;
        captured.meter.scan(64 * 1024)?;
        let report = json!({
            "stage":"actual_source_bound_v12_local_order_diagnostic",
            "source_sha256":captured.original_sha256,
            "source_semantic_identity":neutral.lowered.semantic().semantic().semantic_sha256().as_bytes(),
            "connected_version":"V12","bound_version":"V12","scheduled_version":"V12",
            "connected_identity":source.canonical().identity().digest(),
            "bound_identity":bound.canonical().identity().digest(),
            "source_order_identity":first.output().canonical().identity().digest(),
            "reverse_ready_identity":second.output().canonical().identity().digest(),
            "parameter_ordinals":captured.parameters.each_ref().map(|p| p.ordinal),
            "source_initializer":captured.initializer,
            "region":{"function":region.block.function.0,"block":region.block.block,"first":region.first_operation,"count":region.operation_count},
            "source_order":joined.results.map(|id| id.0),
            "reverse_ready":[joined.results[1].0,joined.results[0].0,joined.results[2].0],
            "source_order_simulation":first_sim,"reverse_ready_simulation":second_sim,
            "oracle_accounting":oracle_ledger,
            "independent_transition_replays":3,"malformed_region_refusals":1,
            "canonical_work":budget.work(),"canonical_peak_storage":budget.peak_storage(),
            "canonical_work_limit":CANONICAL_WORK,"canonical_storage_limit":CANONICAL_STORAGE,
            "source_scan_accounting":captured.meter,"source_io_accounting":input.io,
            "fixed_production_policy_modified":false,"persistent_recipe_admitted":false,
            "final_source_output_admitted":false,"native_emitted":false,"hardware_observed":false,
            "grants_artifact_or_launch_authority":false,
        });
        // Owners remain on this stack through source currentness and reporting.
        // No result borrows an owner that is moved into the same struct.
        Ok(report)
    }
}
