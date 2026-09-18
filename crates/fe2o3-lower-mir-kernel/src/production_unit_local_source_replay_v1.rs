// Fresh original source/N reconstruction for the additive erased route.
// The legacy source/output replay remains raw-empty and is not called here.

/// Replays the complete original UnitLocal materialization, not its erased E.
/// Pending lowering, SSA replay, transient Module/correspondence allocation and
/// complete structural comparison retain their existing stored-limit domain.
/// This ledger additionally covers every new assertion emission/sealing payload
/// and header while original N/origins/source capture remain caller-reserved.
/// No second canonical graph is admitted or retained. Sealing against original
/// N follows complete fresh Module equality; stored origins are never inputs to
/// their fresh derivation. All new canonical scratch drops before floor restore.
fn source_output_unit_local_replay_v1(
    source: &ProductionPreRankedKirOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        budget.charge_work(8)?;
        if budget.storage() < source.unit_local_source_storage_floor_v1()? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if source.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::UnitLocal {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        source
            .semantic_ssa
            .verify_replay()
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let semantic = source.semantic_ssa.source_semantic();
        budget.charge_work(35)?;
        if source.source_launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
            || source.source_launch.roots().len() != semantic.roots().len()
            || source.launch_roots.len() != semantic.roots().len()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        for ((row, root), retained) in source
            .source_launch
            .roots()
            .iter()
            .zip(semantic.roots())
            .zip(source.launch_roots.iter())
        {
            // Fixed root identity/binding comparisons and at most three axes
            // of the existing scalar layout derivation, including grid ID.
            budget.charge_work(160)?;
            let function = semantic
                .functions()
                .get(root.index() as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let entry = function
                .kernel_entry()
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let layout = crate::ProductionSourceExecutionLayoutV1::try_from_source(
                semantic.target().architecture(),
                function,
                row.source_launch(),
            )
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let fresh = RetainedRankedLaunchRootV1 {
                selected_root: row.selected_root(),
                launch_rank: row.source_rank(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            };
            if row.selected_root() != *root
                || fresh != *retained
                || row.semantic_root_identity() != function.identity()
                || row.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
                || layout != row.layout()
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<AssertOriginEmissionV1<'_, '_>>(),
            std::mem::size_of::<SealedAssertOriginsV1>(),
        ])?)?;
        let mut emission = AssertOriginEmissionV1::new(budget);
        let PendingHelperSourceLoweringV1 {
            module,
            correspondence,
            requires_source,
        } = lower_pending_module_with_assert_origins_v1(
            &source.semantic_ssa,
            source.limits,
            &source.launch_roots,
            &mut emission,
        )?;
        if !requires_source
            || source.executable().module() != &module
            || source.correspondence != correspondence
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let origins = emission.seal(&source.semantic_ssa, &correspondence, source.executable())?;
        compare_unit_local_origin_rows_v1(&source.assert_origins, &origins, budget)?;
        drop(origins);
        drop(correspondence);
        drop(module);
        Ok(())
    })
}

fn compare_unit_local_origin_rows_v1(
    original: &SealedAssertOriginsV1,
    fresh: &SealedAssertOriginsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    if original.aliases.len() != fresh.aliases.len()
        || original.functions.len() != fresh.functions.len()
        || original.bindings.len() != fresh.bindings.len()
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    for (old, new) in original.aliases.iter().zip(&fresh.aliases) {
        budget.charge_work(4)?;
        if old.site != new.site || old.binding != new.binding {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    for (old, new) in original.functions.iter().zip(&fresh.functions) {
        budget.charge_work(4)?;
        if (old.owner, old.function, old.canonical, old.reachable_blocks)
            != (new.owner, new.function, new.canonical, new.reachable_blocks)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    budget.charge_work(argument_product_v1(
        fresh.bindings.len(),
        std::mem::size_of::<SemanticKirAssertConditionBindingV1>(),
    )?)?;
    if original.bindings != fresh.bindings {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}
