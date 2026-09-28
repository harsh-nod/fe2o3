// Constant facts only: source identity, availability, and memory admission stay
// with the caller. The lookup must authenticate the exact value definition.
fn source_selector_unsigned_constant_v29<'ir>(
    mut value: ValueId,
    scalar: ScalarType,
    definitions: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    mut lookup: impl FnMut(
        ValueId,
        &mut ArgumentBudgetV1<'_>,
    )
        -> Result<(&'ir Type, Option<&'ir Operation>), ProductionSemanticKirErrorV1>,
) -> Result<Option<u64>, ProductionSemanticKirErrorV1> {
    let mut expected = Some(scalar);
    let mut cast = None;
    let mut mask = u64::MAX;
    for _ in 0..definitions {
        budget.charge_work(12)?;
        let (ty, operation) = lookup(value, budget)?;
        let Type::Scalar(actual) = *ty else {
            return Ok(None);
        };
        if expected.is_some_and(|expected| expected != actual) {
            return Err(execution_availability_error_v29());
        }
        let Some(width_mask) = truncate_unsigned_constant_v1(u64::MAX, ty) else {
            return Ok(None);
        };
        if let Some((kind, output)) = cast {
            if plan_integer_cast_v1(actual, output) != Some([Some((kind, output)), None]) {
                return Err(execution_availability_error_v29());
            }
        }
        mask &= width_mask;
        let Some(operation) = operation else {
            return Ok(None);
        };
        if !matches!(
            operation.kind,
            OperationKind::Constant(_) | OperationKind::Cast { .. }
        ) {
            return Ok(None);
        }
        if operation.results.len() != 1
            || operation.results[0].id != value
            || &operation.results[0].ty != ty
        {
            return Err(execution_availability_error_v29());
        }
        if let Some(constant) = source_selector_constant_v29(operation, value, actual) {
            return Ok(Some(constant & mask));
        }
        let OperationKind::Cast {
            kind,
            value: input,
            ref to,
        } = operation.kind
        else {
            return Ok(None);
        };
        if to != ty {
            return Err(execution_availability_error_v29());
        }
        if !matches!(
            kind,
            CastKind::Truncate | CastKind::ZeroExtend | CastKind::Bitcast
        ) {
            return Ok(None);
        }
        // Walking backwards, every unsigned narrowing discards high bits;
        // later widening cannot restore them. No traversal stack is needed.
        cast = Some((kind, actual));
        expected = None;
        value = input;
    }
    Err(execution_availability_error_v29())
}

#[cfg(test)]
#[path = "production_source_selector_constants_v29_tests.rs"]
mod source_selector_constants_tests_v29;
