// Both query paths supply already checked root-qualified call/return groups.
// This assembler owns only selected-call scratch, never shared session indexes.
#[derive(Clone, Copy)]
struct CheckedCallSiteV1<'a> {
    caller: &'a SemanticKirFunctionCorrespondenceV1,
    source: &'a SemanticDirectCallV1,
    callee: &'a SemanticKirFunctionCorrespondenceV1,
    callee_target: &'a Function,
    block: &'a BasicBlock,
    span: &'a SemanticKirTerminatorOperationSpanV1,
    anchor: &'a SemanticKirCallReturnV1,
    returns: &'a [SemanticKirCallReturnV1],
}

fn with_checked_call_site_v1<'w, R>(
    owner: &ProductionSemanticSsaOwnerV1,
    components: &[CallResultComponentV1],
    site: CheckedCallSiteV1<'_>,
    parameters: ArgumentTraceV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    use_view: impl for<'s> FnOnce(
        &mut ProductionCallViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let SemanticKirCallReturnKindV1::Call {
        arguments_first,
        call_operation,
        destination_end,
        destination,
        transport,
    } = site.anchor.kind
    else {
        return Err(mismatch());
    };
    let block = site.block;
    let operation = &block.operations[call_operation as usize];
    let destination = match destination {
        SemanticKirCallDestinationV1::Local => ProductionCallDestinationV1::Local,
        SemanticKirCallDestinationV1::Retained { .. } => {
            ProductionCallDestinationV1::Retained(&block.operations[destination_end as usize - 1])
        }
        SemanticKirCallDestinationV1::Projected { .. } => ProductionCallDestinationV1::Projected {
            preparation: &block.operations
                [site.span.first_operation_ordinal as usize..arguments_first as usize],
            store: &block.operations[destination_end as usize - 1],
        },
    };
    let transport = call_components_v1(components, transport)?;
    let plan = owner
        .plan_for_function(site.caller.semantic_function)
        .ok_or_else(mismatch)?
        .plan();
    let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(site.anchor.semantic_block.index()), 0);
    let definitions = plan.edge_definitions(edge).ok_or_else(mismatch)?;
    let arguments = plan.edge_arguments(edge).ok_or_else(mismatch)?;
    let semantic = owner.source_semantic();
    let callee = site.callee.semantic_function;
    let result_function = &semantic.functions()[callee.index() as usize];
    budget.charge_work(result_function.locals().len())?;
    prepay_typed_shape_v1(
        semantic.types(),
        result_function.abi().source_output_type(),
        0,
        budget,
    )?;
    let result_shape = helper_result_components_v1(semantic.types(), result_function, callee)?;
    with_parameter_correspondence_v1(
        semantic,
        site.callee,
        site.callee_target,
        parameters,
        budget,
        |entry| {
            let mut view = ProductionCallViewV1 {
                entry: ProductionArgumentViewV1 {
                    data: entry.data.reborrow_v1(),
                    budget: &mut *entry.budget,
                },
                caller: site.caller,
                source: site.source,
                block,
                operation,
                destination,
                transport,
                result_shape: &result_shape,
                definitions,
                arguments,
                returns: site.returns,
                components,
            };
            use_view(&mut view)
        },
    )
}
