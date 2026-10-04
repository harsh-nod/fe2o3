// A whole-source census predicate only. The caller still replays its actual
// source/SSA custody, and native definedness/value correspondence are separate.
fn masked_assertion_query_error_v1(error: crate::ProductionSemanticMaskedShiftQueryErrorV1) -> E {
    match error {
        crate::ProductionSemanticMaskedShiftQueryErrorV1::Resource(error) => E::Resource(error),
        error => E::Source(ProductionSemanticKirErrorV1::MaskedAssertionQuery(error)),
    }
}

fn masked_assertion_success_shift_v1(
    source: &AdmittedInertSemanticMirV1,
    function: usize,
    block: usize,
    statement: usize,
    query: Option<&mut crate::ProductionSemanticMaskedShiftQueryV1<'_, '_>>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    let Some(query) = query else {
        return Ok(false);
    };
    let function =
        SemanticFunctionIdV1::from_index(u32::try_from(function).map_err(|_| arithmetic())?);
    let block = SemanticBlockIdV1::from_index(u32::try_from(block).map_err(|_| arithmetic())?);
    let statement = u32::try_from(statement).map_err(|_| arithmetic())?;
    let Some(fact) = query
        .shift(block, statement, budget)
        .map_err(masked_assertion_query_error_v1)?
    else {
        return Ok(false);
    };
    // The scoped lookup checks budget slot/ledger before this binding work.
    charge(budget, 8)?;
    if !std::ptr::eq(fact.owner(), source)
        || fact.function() != function
        || fact.successor_block() != block
        || fact.shift_statement() != statement
    {
        return Err(E::Source(
            ProductionSemanticKirErrorV1::CorrespondenceMismatch,
        ));
    }
    Ok(true)
}

#[cfg(test)]
#[path = "production_checked_output_masked_assert_success_v1_tests.rs"]
mod masked_assert_success_tests;
