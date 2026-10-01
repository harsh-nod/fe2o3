fn source_node_contains_enum_v55(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    nodes: &mut usize,
    depth: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.source_reference_charge_v29(plan, 3)?;
    *nodes = argument_sum_v1(&[*nodes, 1])?;
    if *nodes > MAX_SSA_VALUE_COMPONENTS_V1 || depth >= 256 {
        return Err(execution_call_error_v29());
    }
    let row = plan.nodes.get(node).ok_or_else(execution_call_error_v29)?;
    match row.kind {
        SourceReferenceNodeKindV29::Enum { .. } | SourceReferenceNodeKindV29::EnumView(_) => {
            Ok(true)
        }
        SourceReferenceNodeKindV29::Aggregate { first, count } => {
            for offset in 0..count {
                budget.source_reference_charge_v29(plan, 2)?;
                let child = *plan
                    .children
                    .get(argument_sum_v1(&[first, offset])?)
                    .ok_or_else(execution_call_error_v29)?;
                if child >= node {
                    return Err(execution_call_error_v29());
                }
                if source_node_contains_enum_v55(plan, child, nodes, depth + 1, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn check_source_enum_helper_parameter_v55(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    mapped: fe2o3_mir_model::SemanticAdjustedArgumentV1<'_>,
    node: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    if !source_node_contains_enum_v55(plan, node, &mut 0, 0, budget)? {
        return Ok(false);
    }
    budget.source_reference_charge_v29(plan, 12)?;
    let row = plan
        .instances
        .instance(instance)
        .ok_or_else(execution_call_error_v29)?;
    let incoming = plan
        .instances
        .incoming(instance)
        .ok_or_else(execution_call_error_v29)?;
    let function = row.declaration();
    let argument = mapped.abi();
    if instance == plan.instances.root()
        || incoming.child() != Some(instance)
        || function.role() != SemanticFunctionRoleV1::InternalHelper
        || function.export().is_some()
        || !function
            .abi()
            .adjusted_arguments()
            .get(mapped.ordinal() as usize)
            .is_some_and(|original| std::ptr::eq(original, argument))
        || mapped.source_ownership() != SemanticSourceArgumentOwnershipV1::ByValue
        || argument.value().adjusted().is_some()
        || argument.value().pointee_override().is_some()
        || argument.ty() != plan.nodes[node].ty
    {
        return Err(execution_call_error_v29());
    }
    // MIR admission already checked the exact source ABI mode and attributes.
    // Expanded helper calls transport logical values, never packed enum bytes.
    // Validate the entire owned carrier, including nested loans/alternatives;
    // ordinary helper reconstruction below uses that same entry node.
    let floor = budget.storage();
    let result = source_reference_node_types_v29(plan, node, budget).map(drop);
    budget.release_storage(budget.storage() - floor)?;
    result?;
    Ok(true)
}
