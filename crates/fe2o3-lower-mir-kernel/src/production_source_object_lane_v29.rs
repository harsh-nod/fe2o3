// Lane selection uses selected original backing, never an emitted operation or
// candidate allocation census. The existing scalar checker remains closed to
// typed objects; the consuming physical transaction proves all of their uses.
fn source_physical_object_count_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    if plan.cells.rows.is_empty() {
        return Ok(0);
    }
    let mut count = 0usize;
    with_canonical_call_scratch_v1(budget, |budget| {
        let arrays = source_array_eligibility_v29(plan, budget)?;
        if arrays.len() != plan.cells.rows.len() {
            return Err(scoped_object_allocation_error_v29());
        }
        for (cell, (row, array)) in plan.cells.rows.iter().zip(&arrays).enumerate() {
            plan.charge(3, budget)?;
            let SourceBackingKindV29::Object(schema) = row.kind else {
                if array.is_some() {
                    return Err(scoped_object_allocation_error_v29());
                }
                continue;
            };
            // Query each original generation/schema even when this cell can
            // use the independently authenticated legacy array representation.
            source_object_storage_v29(
                plan,
                cell,
                row.instance,
                row.local,
                row.generation,
                schema,
                budget,
            )?;
            if let Some(array) = array {
                if *array != schema {
                    return Err(scoped_object_allocation_error_v29());
                }
            } else {
                count = argument_sum_v1(&[count, 1])?;
            }
        }
        Ok(())
    })
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    Ok(count)
}
