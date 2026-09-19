// These attachments retain call-instance identity and full-module coordinates.
// They remain pending source-value equivalence, elision checks and discharge.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source replay remains gated")
)]
fn collect_scoped_module_assertions_v29(
    module: &Module,
    roots: &[ScopedModuleRootV29],
    source: &ExecutionLifecycleSourceV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ReplayedInstanceAssertV1>, ProductionSemanticKirErrorV1> {
    if source.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    scoped_slot_attempt_v29(budget, |budget| {
        budget.charge_work(2)?;
        if roots.len() != source.launch.roots().len() || roots.len() != module.kernels.len() {
            return Err(scoped_module_error_v29());
        }
        let graph = AssertGraphIndexV1::build_functions(&module.functions, true, budget)?;
        let mut assertions = Vec::new();
        for (ordinal, root) in roots.iter().enumerate() {
            budget.charge_work(2)?;
            if root.function_ordinal != ordinal
                || root.coordinates.root != source.launch.roots()[ordinal].selected_root()
            {
                return Err(scoped_module_error_v29());
            }
            production_call_instances_v1::with_production_call_instances_v1(
                source.owner,
                source.launch.roots()[ordinal].selected_root(),
                budget,
                |instances, budget| {
                    Ok(replay_instance_asserts_in_functions_v1(
                        InstanceAssertReplaySubjectV1 {
                            functions: &module.functions,
                            function_ordinal: root.function_ordinal,
                            sidecars: &root.sidecars,
                            coordinates: &root.coordinates,
                            slot_relocation: root.slot_relocation.as_ref(),
                            insertions: &root.insertions,
                        },
                        instances,
                        &graph,
                        budget,
                        &mut |row, budget| {
                            assert_origin_push_v1(&mut assertions, row, budget)?;
                            Ok(())
                        },
                    ))
                },
            )
            .map_err(scoped_root_instance_error_v29)??;
        }
        graph.release(budget)?;
        Ok(assertions)
    })
}
