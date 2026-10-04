// Shared descriptor transport grammar. Whole-value phi propagation remains in
// the existing paid solver; only its exact SliceToGeneric wrapper is peeled.
enum DescriptorOriginV30 {
    Exact(SliceDefinition),
    Unknown,
    Cyclic,
}

fn source_descriptor_origin_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a (); 6],
        [usize; 4],
        ValueId,
        DescriptorOriginV30,
        SliceDefinition,
        SliceOperation,
        Option<SliceDefinition>,
        &'a CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>,
        SliceResult<Option<SliceDefinition>>,
        SliceResult<DescriptorOriginV30>,
        Result<Option<SliceDefinition>, CanonicalKirInventoryErrorV1>,
        Result<
            Option<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>>,
            CanonicalKirInventoryErrorV1,
        >,
        Option<&'a CanonicalKirOperationRefV1<'a>>,
        std::ops::Range<usize>,
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

fn source_descriptor_origin_v30<O>(
    inventory: &CanonicalKirInventoryV1<'_, O>,
    function: &CanonicalKirFunctionRefV1<'_>,
    origins: &super::value_origin_v1::WholeValueOriginsV1<'_, O>,
    mut value: ValueId,
    budget: &mut SliceBudget<'_>,
) -> SliceResult<DescriptorOriginV30> {
    budget.charge_work(2)?;
    if !origins.belongs_to(inventory, function.coordinate)
        || inventory
            .functions()
            .get(function.coordinate.0 as usize)
            .is_none_or(|actual| !std::ptr::eq(actual, function))
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let steps = function
        .definitions
        .len()
        .checked_add(1)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    for _ in 0..steps {
        budget.charge_work(1)?;
        let Some(origin) = origins
            .resolve(value, budget)
            .map_err(slice_inventory_error)?
        else {
            return Ok(DescriptorOriginV30::Unknown);
        };
        if let SliceDefinition::Result {
            operation,
            result: 0,
        } = origin
        {
            budget.charge_work(4)?;
            let row = function
                .blocks
                .start
                .checked_add(operation.block.block as usize)
                .filter(|index| *index < function.blocks.end)
                .and_then(|index| inventory.blocks().get(index))
                .filter(|row| {
                    row.coordinate == operation.block
                        && operation.block.function == function.coordinate
                })
                .and_then(|block| {
                    block
                        .operations
                        .start
                        .checked_add(operation.operation as usize)
                        .filter(|index| *index < block.operations.end)
                })
                .and_then(|index| inventory.operations().get(index))
                .filter(|row| row.coordinate == operation)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if let OperationKind::Cast {
                kind: CastKind::SliceToGeneric,
                value: input,
                to,
            } = &row.operation.kind
            {
                let definition = inventory
                    .definition_for_value(function.coordinate, *input, budget)
                    .map_err(slice_inventory_error)?
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if !source_descriptor_widening_v29(definition.ty, to)
                    || row.operation.results.len() != 1
                    || row.operation.results[0].ty != *to
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                value = *input;
                continue;
            }
        }
        return Ok(DescriptorOriginV30::Exact(origin));
    }
    Ok(DescriptorOriginV30::Cyclic)
}
