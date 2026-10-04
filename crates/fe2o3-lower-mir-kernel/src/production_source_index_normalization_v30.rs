// The source selector and post-splice memory readers use the same bounded
// normalization rule. Their caller authenticates the exact definition index.
fn check_source_index_normalization_with_v30<'a, F>(
    original_value: ValueId,
    original_scalar: ScalarType,
    fixed_extent: Option<u64>,
    offset: ValueId,
    definition_count: usize,
    block: BlockId,
    operation: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    mut definition: F,
) -> Result<(), ProductionSemanticKirErrorV1>
where
    F: FnMut(
        ValueId,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceSelectorDefinitionV29<'a>, ProductionSemanticKirErrorV1>,
{
    budget.reserve_storage(argument_sum_v1(&[
        source_reference_emission_headers_v29::<F>()?,
        source_reference_emission_headers_v29::<()>()?,
        argument_product_v1(
            3,
            source_reference_emission_headers_v29::<SourceSelectorDefinitionV29<'_>>()?,
        )?,
    ])?)?;
    let original = definition(original_value, budget)?;
    let lowered = definition(offset, budget)?;
    if *lowered.0 != Type::INDEX || *original.0 != Type::Scalar(original_scalar) {
        return Err(execution_availability_error_v29());
    }
    if let Some(constant) = source_selector_unsigned_constant_v29(
        original_value,
        original_scalar,
        definition_count,
        budget,
        |value, budget| definition(value, budget).map(|(ty, operation, _)| (ty, operation)),
    )? {
        budget.charge_work(3)?;
        if fixed_extent.is_some_and(|length| constant >= length) {
            return Err(execution_availability_error_v29());
        }
        if lowered
            .1
            .and_then(|op| source_selector_constant_v29(op, offset, ScalarType::Index))
            == Some(constant)
        {
            return Ok(());
        }
        if fixed_extent.is_some() {
            return Err(execution_availability_error_v29());
        }
    }
    let path = plan_integer_cast_v1(original_scalar, ScalarType::Index)
        .ok_or_else(execution_availability_error_v29)?;
    let mut value = offset;
    for (kind, scalar) in path.into_iter().flatten().rev() {
        let row = definition(value, budget)?;
        if *row.0 != Type::Scalar(scalar)
            || row
                .2
                .is_none_or(|(owner, ordinal)| owner != block || ordinal >= operation)
        {
            return Err(execution_availability_error_v29());
        }
        let Some(OperationKind::Cast {
            kind: actual,
            value: input,
            to,
        }) = row.1.map(|op| &op.kind)
        else {
            return Err(execution_availability_error_v29());
        };
        if *actual != kind || *to != Type::Scalar(scalar) {
            return Err(execution_availability_error_v29());
        }
        value = *input;
    }
    if value != original_value {
        return Err(execution_availability_error_v29());
    }
    Ok(())
}
