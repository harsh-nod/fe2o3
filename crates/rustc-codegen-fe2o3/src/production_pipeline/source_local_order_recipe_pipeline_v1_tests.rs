//! Private inert recipe intent is rebound to live HIR and genuine V12 owners.
use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::local_order::recipes::{
    BYTE_CAP, InstanceBinding, Order, Origin, Recipe,
};

fn preference(order: Order) -> Preference {
    match order {
        Order::SourceOrder => Preference::SourceOrder,
        Order::ReverseReady => Preference::ReverseReady,
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn observe_source_local_order_recipe(
        self,
        mut input: RetainedInput,
        recipe: Option<&Recipe>,
    ) -> Result<(Value, Option<[Vec<u8>; 2]>), String> {
        let tcx = self.stage.tcx;
        let mut captured = capture_local_order(tcx, &self.stage.closure, &input)?;
        captured
            .meter
            .storage(std::mem::size_of::<InstanceBinding>())?;
        captured.meter.scan(1024)?;
        let measured = InstanceBinding {
            function: *captured.identities.function().as_bytes(),
            item: *captured.identities.item_definition().as_bytes(),
            monomorphization: *captured.identities.monomorphization().as_bytes(),
            generic_types: *captured.identities.generic_type_arguments().as_bytes(),
            const_arguments: *captured.identities.const_generic_arguments().as_bytes(),
        };
        let requested = recipe.map(|recipe| recipe.bind(measured)).transpose()?;
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
        let origin = Origin {
            source: captured.original_sha256,
            semantic: *neutral
                .lowered
                .semantic()
                .semantic()
                .semantic_sha256()
                .as_bytes(),
            bound: *bound.canonical().identity().digest(),
        };
        let selected = requested.unwrap_or(Order::SourceOrder);
        let other = requested.unwrap_or(Order::ReverseReady);
        let floor = budget.storage();
        let first = schedule(&bound, region, preference(selected), &mut budget)
            .map_err(|e| e.to_string())?;
        budget
            .reserve_storage(first.retained_storage())
            .map_err(|e| e.to_string())?;
        first.replay(&mut budget).map_err(|e| e.to_string())?;
        let second =
            schedule(&bound, region, preference(other), &mut budget).map_err(|e| e.to_string())?;
        budget
            .reserve_storage(second.retained_storage())
            .map_err(|e| e.to_string())?;
        second.replay(&mut budget).map_err(|e| e.to_string())?;
        let expected = |order| match order {
            Order::SourceOrder => joined.results,
            Order::ReverseReady => [joined.results[1], joined.results[0], joined.results[2]],
        };
        captured
            .meter
            .scan(first.output().canonical().canonical_bytes().len())?;
        captured
            .meter
            .scan(second.output().canonical().canonical_bytes().len())?;
        captured
            .meter
            .scan(bound.canonical().canonical_bytes().len())?;
        captured.meter.rows(6)?;
        if joined.result_order(first.output(), region)? != expected(selected)
            || joined.result_order(second.output(), region)? != expected(other)
            || budget.storage() != floor + first.retained_storage() + second.retained_storage()
        {
            return Err("local-order recipe checked order or accounting mismatch".into());
        }
        if requested.is_some() {
            if first.output().canonical().canonical_bytes()
                != second.output().canonical().canonical_bytes()
            {
                return Err("local-order recipe nondeterministic current-owner replay".into());
            }
        } else if first.output().canonical().canonical_bytes()
            != bound.canonical().canonical_bytes()
            || first.output().canonical().identity() == second.output().canonical().identity()
        {
            return Err(
                "local-order recipe two preferences did not produce distinct orders".into(),
            );
        }
        let mut oracle = simulation::OracleLedger::default();
        let simulation = if requested.is_some() {
            Some(simulation::observe(first.output(), &mut oracle)?)
        } else {
            None
        };
        // Encoding is inert. Charge fixed caps before its allocations, including
        // shared-codec readback; neither returned byte vector contains an owner.
        captured.meter.storage(4 * BYTE_CAP)?;
        captured.meter.scan(4 * BYTE_CAP)?;
        let generated = if requested.is_none() {
            let pair = [
                Recipe::new(measured, origin, Order::SourceOrder).encode()?,
                Recipe::new(measured, origin, Order::ReverseReady).encode()?,
            ];
            for bytes in &pair {
                Recipe::decode(bytes)?.bind(measured)?;
            }
            Some(pair)
        } else {
            None
        };
        captured.recheck(tcx, &mut input)?;
        captured.meter.storage(64 * 1024)?;
        captured.meter.scan(64 * 1024)?;
        let report = json!({
            "stage":"actual_source_local_order_private_recipe",
            "mode":if requested.is_some() {"replay"} else {"generate"},
            "current_binding":measured,"current_origin":origin,
            "source_initializer":captured.initializer,
            "parameter_ordinals":captured.parameters.each_ref().map(|p| p.ordinal),
            "preference":selected,
            "source_unchanged_from_origin":recipe.map(|r| r.origin().source == origin.source),
            "previous_origin":recipe.map(Recipe::origin),
            "bound_version":"V12","scheduled_version":"V12",
            "selected_identity":first.output().canonical().identity().digest(),
            "second_identity":second.output().canonical().identity().digest(),
            "selected_order":expected(selected).map(|id| id.0),
            "second_order":expected(other).map(|id| id.0),
            "simulation":simulation,"oracle_accounting":oracle,
            "independent_transition_replays":2,
            "canonical_work":budget.work(),"canonical_peak_storage":budget.peak_storage(),
            "canonical_work_limit":CANONICAL_WORK,"canonical_storage_limit":CANONICAL_STORAGE,
            "source_scan_accounting":captured.meter,"source_io_accounting":input.io,
            "previous_evidence_reused":false,"serialized_owner_read":false,
            "stored_origin_is_untrusted_metadata":true,
            "fixed_production_policy_modified":false,"public_recipe_admitted":false,
            "final_source_output_admitted":false,"native_emitted":false,
            "grants_artifact_or_launch_authority":false,
        });
        Ok((report, generated))
    }
}
