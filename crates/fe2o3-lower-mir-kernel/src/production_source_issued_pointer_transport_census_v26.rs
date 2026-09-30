// Inert exact-issuer cast census for original/optimized source completion.
// Callers must still join each result to authentic occurrence/definition rows.

fn source_issued_actual_function_v26(
    function: &Function,
    actual: &SourceIssuedActualV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let body = function.body.as_ref().ok_or_else(source_issued_error_v29)?;
    if body.parameters.len() != function.signature.parameters.len() {
        return Err(source_issued_error_v29());
    }
    let mut count = 0;
    for (ordinal, (id, ty)) in body
        .parameters
        .iter()
        .zip(&function.signature.parameters)
        .enumerate()
    {
        budget.charge_work(5)?;
        let row = actual.value(*id, budget)?;
        if !std::ptr::eq(row.ty, ty) || row.operation.is_some() || row.input != Some(ordinal) {
            return Err(source_issued_error_v29());
        }
        count = argument_sum_v1(&[count, 1])?;
    }
    for block in &body.blocks {
        budget.charge_work(1)?;
        for value in &block.parameters {
            budget.charge_work(5)?;
            let row = actual.value(value.id, budget)?;
            if !std::ptr::eq(row.ty, &value.ty) || row.operation.is_some() || row.input.is_some() {
                return Err(source_issued_error_v29());
            }
            count = argument_sum_v1(&[count, 1])?;
        }
        for operation in &block.operations {
            budget.charge_work(1)?;
            for value in &operation.results {
                budget.charge_work(6)?;
                let row = actual.value(value.id, budget)?;
                if !std::ptr::eq(row.ty, &value.ty)
                    || !row
                        .operation
                        .is_some_and(|actual| std::ptr::eq(actual, operation))
                    || row.input.is_some()
                {
                    return Err(source_issued_error_v29());
                }
                count = argument_sum_v1(&[count, 1])?;
            }
        }
    }
    budget.charge_work(1)?;
    if count != actual.values.len() {
        return Err(source_issued_error_v29());
    }
    Ok(())
}

/// Returns sorted, unique cast-result IDs only after every complete exact-issuer
/// path succeeds. Partial paths never escape. The caller owns all construction
/// growth, including the returned Vec header/backing; CFG and bitmap scratch
/// settle here. This follows the existing root-origin batch ownership contract.
fn source_issued_pointer_transport_census_v26(
    function: &Function,
    actual: &SourceIssuedActualV29<'_>,
    rows: &[SourceIssuedPointerTransportV26],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ValueId>, ProductionSemanticKirErrorV1> {
    budget.check_prior_denials_v1()?;
    let header = argument_sum_v1(&[
        std::mem::size_of::<Vec<ValueId>>(),
        std::mem::align_of::<Vec<ValueId>>(),
        std::mem::size_of::<Result<Vec<ValueId>, ProductionSemanticKirErrorV1>>(),
    ])?;
    budget.reserve_storage(header)?;
    let mut census = emission_vec_v1(
        if rows.is_empty() {
            0
        } else {
            actual.values.len()
        },
        budget,
    )?;
    let retained_floor = budget.storage();
    with_canonical_call_scratch_v1(budget, |budget| {
        type Frame<'a, 'work> = (
            Vec<u8>,
            &'a mut Vec<u8>,
            &'a SourceIssuedActualV29<'a>,
            &'a [SourceIssuedPointerTransportV26],
            &'a mut Vec<ValueId>,
            &'a Function,
            &'a mut ArgumentBudgetV1<'work>,
            [usize; 8],
            bool,
            Result<(), ProductionSemanticKirErrorV1>,
            std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>,
        );
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Frame<'_, '_>>(),
            std::mem::align_of::<Frame<'_, '_>>(),
        ])?)?;
        source_issued_actual_function_v26(function, actual, budget)?;
        if rows.is_empty() {
            return Ok(());
        }
        let mut selected = emission_vec_v1::<u8>(actual.values.len(), budget)?;
        budget.charge_work(actual.values.len())?;
        selected.resize(actual.values.len(), 0);
        source_issued_pointer_walk_quote_v26(actual.values.len(), rows.len(), budget)?;
        // The common walker already prepays lookup/type work. This additional
        // debit covers the private bitmap visit on every possible cast step.
        budget.charge_work(argument_product_v1(
            rows.len(),
            argument_sum_v1(&[actual.values.len(), 1])?,
        )?)?;
        for row in rows {
            budget.charge_work(6)?;
            let Some((element, AddressSpace::Global, access)) =
                source_issued_pointer_shape_v26(actual.value(row.issuer, budget)?.ty)
            else {
                return Err(source_issued_error_v29());
            };
            let Some((actual_element, space, actual_access)) =
                source_issued_pointer_shape_v26(actual.value(row.pointer, budget)?.ty)
            else {
                return Err(source_issued_error_v29());
            };
            if element != actual_element
                || !matches!(space, AddressSpace::Global | AddressSpace::Generic)
                || !(access == actual_access
                    || (access == AccessMode::ReadWrite && actual_access == AccessMode::ReadOnly))
            {
                return Err(source_issued_error_v29());
            }
        }
        let mut valid = true;
        fe2o3_kernel_ir::with_function_control_flow_v1(
            function,
            Default::default(),
            budget,
            |view| {
                for row in rows {
                    if source_issued_pointer_walk_visit_v26(
                        actual,
                        row.pointer,
                        Some(row.issuer),
                        view,
                        |index| selected[index] = 1,
                    )? != Some(row.issuer)
                    {
                        valid = false;
                        break;
                    }
                }
                Ok(())
            },
        )
        .map_err(|error| match error {
            fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
            _ => source_issued_error_v29(),
        })?;
        if !valid {
            return Err(source_issued_error_v29());
        }
        for (index, value) in actual.values.iter().enumerate() {
            budget.charge_work(2)?;
            if selected[index] != 0 {
                census.push(value.id);
            }
        }
        drop(selected);
        Ok(())
    })?;
    if budget.storage() != retained_floor {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(census)
}

#[cfg(test)]
mod source_issued_pointer_transport_census_tests_v26 {
    use super::source_issued_call_provenance_tests_v26::transport_graph;
    use super::*;
    include!("production_source_issued_pointer_transport_census_v26_tests.rs");
}
