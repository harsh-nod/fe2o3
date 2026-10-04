// An optimized write keeps its original call identity and explicit predicate.
// These facts do not grant address-formation, initializedness, or native authority.
#[cfg(test)]
include!("production_optimized_source_writes_v87_tests.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OptimizedSourceWriteV87 {
    source: PendingSourceWriteV86,
    input: [SliceOperation; 7],
    output: [SliceOperation; 7],
    tail: CheckedWriteTailV85,
    root_input: ValueId,
    receiver: ValueId,
    index: ValueId,
    value: ValueId,
}

fn optimized_write_headers_v87() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a (); 32],
        [usize; 12],
        [SliceOperation; 21],
        [SliceDefinition; 8],
        [ValueId; 14],
        [&'a Operation; 14],
        &'a [PendingSourceWriteV86],
        Vec<Option<OptimizedSourceWriteV87>>,
        OptimizedSourceWriteV87,
        Option<OptimizedSourceWriteV87>,
        SourceOwnedResultV18<Option<OptimizedSourceWriteV87>>,
        SourceOwnedResultV18<&'a [PendingSourceWriteV86]>,
        SourceOwnedResultV18<Vec<Option<OptimizedSourceWriteV87>>>,
        SourceOwnedResultV18<SliceOperation>,
        SourceOwnedResultV18<&'a Operation>,
        SourceOwnedResultV18<()>,
        Option<ProductionOptimizedSourceMemoryAccessV18<'a>>,
        SourceOwnedResultV18<Option<ProductionOptimizedSourceMemoryAccessV18<'a>>>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        issued_output_headers_v18()?,
    ])
}

// The caller owns the returned storage and prepays optimized_write_headers_v87.
// Each source call is replayed once; all producer lookups use indexed inventory
// queries. No shape search or whole-output scan is used to select a CSE producer.
fn optimized_source_writes_v87(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<Option<OptimizedSourceWriteV87>>> {
    optimized.check_exact_original_v18(original, budget)?;
    optimized_source_endpoints_v18(original, optimized, budget)?;
    original.retain_query((|| {
        optimized.replay_selected_transport_v30(root, budget)?;
        let rows = scoped_raw_admission_v29::checked_source_writes_v87(original, root, budget)?;
        let mut facts = emission_vec_v1(rows.len(), budget).map_err(source_emission_error_v18)?;
        if rows.is_empty() {
            return Ok(facts);
        }
        let function = optimized_source_root_function_v18(original, optimized, root, budget)?;
        let actual = SourceIssuedActualV29::from_function(function.function, budget)
            .map_err(source_emission_error_v18)?;
        for row in rows {
            let fact =
                optimized_source_write_v87(original, optimized, root, row, function, budget)?;
            budget.charge_work(2)?;
            if let Some(fact) = &fact {
                if *actual
                    .value(fact.index, budget)
                    .map_err(source_emission_error_v18)?
                    .ty
                    != Type::INDEX
                    || *actual
                        .value(fact.value, budget)
                        .map_err(source_emission_error_v18)?
                        .ty
                        != Type::Scalar(row.element)
                {
                    return original
                        .source
                        .missing("write output index or scalar value type differs");
                }
                source_issued_root_input_transport_v29(
                    &actual,
                    row.root_parameter,
                    fact.root_input,
                    fact.receiver,
                    budget,
                )
                .map_err(source_emission_error_v18)?;
            }
            facts.push(fact);
        }
        let mut valid = true;
        budget.charge_work(facts.len())?;
        fe2o3_kernel_ir::with_function_control_flow_v1(
            function.function,
            Default::default(),
            budget,
            |view| {
                for fact in facts.iter().flatten() {
                    if fact.receiver != fact.root_input
                        && view.unique_value_origin(fact.receiver)? != Some(fact.root_input)
                    {
                        valid = false;
                    }
                }
                Ok(())
            },
        )
        .map_err(|error| match error {
            fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
            _ => ProductionSourceOwnedViewErrorV18::Binding("write output CFG query refused"),
        })?;
        if !valid {
            return original
                .source
                .missing("write output root transport differs");
        }
        Ok(facts)
    })())
}

fn optimized_write_value_descendant_v87(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    input: ValueId,
    output: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let source = original
        .inventory
        .definition_for_value(input_function, input, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "write original operand definition",
        ))?;
    let target = optimized
        .output_inventory(budget)?
        .definition_for_value(output_function, output, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "write output operand definition",
        ))?;
    issued_metadata_descendant_v18(
        original,
        optimized,
        source.coordinate,
        target.coordinate,
        None,
        budget,
    )
}

// Select a pure producer using an actual checked consumer value. CSE may remove
// the input operation; its result must still have this exact checked descendant.
fn optimized_write_producer_v87(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: SliceOperation,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SliceOperation> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    let inventory = optimized.output_inventory(budget)?;
    let target = inventory
        .definition_for_value(output_function, value, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "write selected producer is absent",
        ))?;
    let SliceDefinition::Result {
        operation: output,
        result: 0,
    } = target.coordinate
    else {
        return original
            .source
            .missing("write selected producer is not a single result");
    };
    budget.charge_work(5)?;
    let kind = match optimized.operation(input, budget)? {
        ProductionOptimizedSourceOperationV18::Retained {
            output: retained, ..
        } if retained == output => Kind::Retained,
        ProductionOptimizedSourceOperationV18::Rewritten { .. } => Kind::Substituted,
        _ => {
            return original
                .source
                .missing("write selected producer disposition differs");
        }
    };
    issued_metadata_descendant_v18(
        original,
        optimized,
        SliceDefinition::Result {
            operation: input,
            result: 0,
        },
        target.coordinate,
        Some(kind),
        budget,
    )?;
    Ok(output)
}

fn optimized_source_write_v87(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    row: &PendingSourceWriteV86,
    function: &CanonicalKirFunctionRefV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<OptimizedSourceWriteV87>> {
    let input =
        scoped_raw_admission_v29::source_write_tail_locations_v87(original, root, row, budget)?;
    let access = optimized
        .scalar_memory_access_at_v25(root, input[6], row.instance.index(), row.anchor, budget)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "write original scalar payload is absent",
        ))?;
    let Some(store) = access.output() else {
        return Ok(None);
    };
    budget.charge_work(12)?;
    if access.input() != input[6]
        || access.instance() != row.instance.index()
        || store.block.function != function.coordinate
        || !matches!(
            access.payload(),
            Some(ProductionOptimizedSourcePayloadV18::Store { .. })
        )
    {
        return original
            .source
            .missing("write actual source occurrence or payload differs");
    }
    let inventory = optimized.output_inventory(budget)?;
    let store_operation = source_operation_row_v18(inventory, store, budget)?.operation;
    let OperationKind::GuardedStore {
        pointer,
        predicate,
        value,
        ..
    } = store_operation.kind
    else {
        return original
            .source
            .missing("write actual effect is not guarded");
    };
    let address = optimized_write_producer_v87(
        original,
        optimized,
        input[5],
        function.coordinate,
        pointer,
        budget,
    )?;
    let OperationKind::GetElementPointer { base, offset } =
        source_operation_row_v18(inventory, address, budget)?
            .operation
            .kind
    else {
        return original
            .source
            .missing("write actual pointer producer differs");
    };
    let data = optimized_write_producer_v87(
        original,
        optimized,
        input[4],
        function.coordinate,
        base,
        budget,
    )?;
    let select = optimized_write_producer_v87(
        original,
        optimized,
        input[3],
        function.coordinate,
        offset,
        budget,
    )?;
    let OperationKind::Select {
        true_value: index,
        false_value: zero_value,
        ..
    } = source_operation_row_v18(inventory, select, budget)?
        .operation
        .kind
    else {
        return original
            .source
            .missing("write actual selected offset differs");
    };
    let zero = optimized_write_producer_v87(
        original,
        optimized,
        input[2],
        function.coordinate,
        zero_value,
        budget,
    )?;
    let extent = optimized_write_producer_v87(
        original,
        optimized,
        input[1],
        function.coordinate,
        predicate,
        budget,
    )?;
    let OperationKind::Compare {
        rhs: length_value, ..
    } = source_operation_row_v18(inventory, extent, budget)?
        .operation
        .kind
    else {
        return original
            .source
            .missing("write actual extent comparison differs");
    };
    let length = optimized_write_producer_v87(
        original,
        optimized,
        input[0],
        function.coordinate,
        length_value,
        budget,
    )?;
    let OperationKind::SliceLength { slice: receiver } =
        source_operation_row_v18(inventory, length, budget)?
            .operation
            .kind
    else {
        return original
            .source
            .missing("write actual length producer differs");
    };
    let output = [length, extent, zero, select, data, address, store];
    let mut operations = [store_operation; 7];
    for (target, coordinate) in operations.iter_mut().zip(output) {
        budget.charge_work(1)?;
        *target = source_operation_row_v18(inventory, coordinate, budget)?.operation;
    }
    let tail = check_checked_write_tail_v85(
        CheckedWriteInputsV85 {
            slice: receiver,
            index,
            precondition: None,
            value,
            element: row.element,
        },
        &operations,
        budget,
    )
    .map_err(source_emission_error_v18)?;
    scoped_raw_admission_v29::check_issued_original_effect_v18(
        original,
        root,
        row.instance.index(),
        row.anchor,
        store_operation,
        budget,
    )?;
    let body =
        function
            .function
            .body
            .as_ref()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "write output body is absent",
            ))?;
    let root_input = *body.parameters.get(row.root_parameter).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("write output root parameter is absent"),
    )?;
    budget.charge_work(5)?;
    if !matches!(function.function.signature.parameters.get(row.root_parameter), Some(Type::Slice(slice))
        if *slice.element == Type::Scalar(row.element) && slice.address_space == AddressSpace::Global && slice.access == AccessMode::WriteOnly)
    {
        return original.source.missing("write output root type differs");
    }
    for (before, after) in [
        (row.root_input, root_input),
        (row.receiver, receiver),
        (row.index, index),
        (row.value, value),
    ] {
        budget.charge_work(1)?;
        optimized_write_value_descendant_v87(
            original,
            optimized,
            input[6].block.function,
            function.coordinate,
            before,
            after,
            budget,
        )?;
    }
    Ok(Some(OptimizedSourceWriteV87 {
        source: *row,
        input,
        output,
        tail,
        root_input,
        receiver,
        index,
        value,
    }))
}
