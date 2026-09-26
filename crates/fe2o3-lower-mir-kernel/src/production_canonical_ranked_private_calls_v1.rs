// Original call/ABI and helper-frame engines are reused for every root alias.
fn cr_private_calls_v1(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CrPolicyResultV1<usize> {
    if !source.calls.belongs_to(source.inventory) {
        return Err(cr_policy_unsupported_v1(
            ProductionCanonicalRankedSourceRequirementV1::GraphIdentity,
            0,
        ));
    }
    budget.charge_work(source.calls.call_count())?;
    for (root, call) in source.calls.sites() {
        source
            .calls
            .with_call_local_frame_v1(root, call, budget, |view, frame| {
                view.callee().visit_nodes(|node| {
                    if node.source().source_ownership()
                        != SemanticSourceArgumentOwnershipV1::ByValue
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    Ok(())
                })?;
                view.visit_result_nodes(|_| Ok(()))?;
                if let Some(frame) = frame {
                    view.entry.budget.charge_work(argument_sum_v1(&[
                        frame.allocations().len(),
                        frame.accesses().len(),
                        frame.control().len(),
                        frame.edge_bindings().len(),
                        2,
                    ])?)?;
                    if !std::ptr::eq(frame.source(), view.entry.association)
                        || !std::ptr::eq(frame.function(), view.entry.data.target)
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
                Ok(())
            })
            .map_err(ProductionCanonicalRankedSourceErrorV1::from)?;
    }
    // The retained metadata already covers every association, even helpers with
    // no incoming call in another root. Inspect every actual frame as well.
    for (association, frame) in source.arguments.frames.iter().enumerate() {
        budget.charge_work(1)?;
        if let Some(frame) = frame {
            let actual = &source.calls.groups[association].function;
            budget.charge_work(argument_sum_v1(&[
                frame.allocations().len(),
                frame.accesses().len(),
                2,
            ])?)?;
            if !std::ptr::eq(frame.source(), actual.source())
                || !std::ptr::eq(frame.function(), actual.canonical().function)
            {
                return Err(cr_policy_unsupported_v1(
                    ProductionCanonicalRankedSourceRequirementV1::LocalMemory,
                    association,
                ));
            }
        }
    }
    Ok(source.calls.call_count())
}
