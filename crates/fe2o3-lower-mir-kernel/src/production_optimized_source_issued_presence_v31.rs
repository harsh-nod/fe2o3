pub(super) fn optimized_presence_headers_v31() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<OptimizedScalarPreparedV31<'_>>()?,
        size_of::<OptimizedScalarCatchFrameV31<'_>>(),
        std::mem::align_of::<OptimizedScalarCatchFrameV31<'_>>(),
        size_of::<std::panic::AssertUnwindSafe<OptimizedScalarCatchFrameV31<'_>>>(),
        h::<Vec<OptimizedIssuedPresenceV31>>()?,
        h::<OptimizedIssuedPresenceV31>()?,
        h::<Option<&SourceIssuedPresenceV31>>()?,
        h::<&PendingSourceIssuedIssuerV29>()?,
        size_of::<std::slice::Iter<'_, SourceIssuedPresenceV31>>(),
        argument_product_v1(8, size_of::<usize>())?,
        argument_product_v1(8, size_of::<&()>())?,
    ])
}

pub(super) fn optimized_issued_presences_v31(
    source: &SourceScalarLeavesV18<'_, '_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    function: &CanonicalKirFunctionRefV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<OptimizedIssuedPresenceV31>> {
    source.query(budget)?;
    optimized_source_endpoints_v18(source.relation, optimized, budget)?;
    let mut output =
        emission_vec_v1(source.presences.rows.len(), budget).map_err(source_emission_error_v18)?;
    if source.presences.rows.is_empty() {
        return Ok(output);
    }
    budget.reserve_storage(issued_output_headers_v18()?)?;
    let pending = source
        .relation
        .source
        .root_row(source.root)?
        .source_slots
        .pending_memory
        .as_ref()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized issued presence original owner absent",
        ))?;
    for (original, row) in source.presences.rows.iter().enumerate() {
        source.presence_row_v31(row, budget)?;
        let issuer = scoped_raw_admission_v29::retained_source_issuer_v31(pending, row.issuer)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized issued presence original issuer absent",
            ))?;
        let operation = match optimized.operation(row.operation, budget)? {
            ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
            ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => continue,
            // A pure predicate can be dead. Omission creates no output leaf;
            // any retained source selector still has to normalize exactly.
            ProductionOptimizedSourceOperationV18::Rewritten { .. } => continue,
        };
        let input = scoped_raw_admission_v29::source_issued_tail_locations_v18(
            source.relation,
            source.root,
            issuer,
            budget,
        )?;
        let (length, _) = issued_metadata_output_v18(
            source.relation,
            optimized,
            IssuedMetadataKindV18::Length,
            input[0],
            input[1],
            operation,
            budget,
        )?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized issued presence lost exact length",
        ))?;
        let inventory = optimized.output_inventory(budget)?;
        let comparison = source_operation_row_v18(inventory, operation, budget)?.operation;
        let length = source_operation_row_v18(inventory, length, budget)?.operation;
        let (
            [present],
            [length_result],
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: index,
                rhs,
            },
        ) = (
            comparison.results.as_slice(),
            length.results.as_slice(),
            &comparison.kind,
        )
        else {
            return source
                .relation
                .source
                .missing("optimized issued presence predicate shape differs");
        };
        budget.charge_work(3)?;
        if input[1] != row.operation
            || output.len() == output.capacity()
            || operation.block.function != function.coordinate
            || present.ty != Type::BOOL
            || length_result.ty != Type::INDEX
            || *rhs != length_result.id
        {
            return source
                .relation
                .source
                .missing("optimized issued presence input or capacity differs");
        }
        issued_output_definition_v18(
            source.relation,
            optimized,
            SliceDefinition::Result {
                operation: row.operation,
                result: 0,
            },
            SliceDefinition::Result {
                operation,
                result: 0,
            },
            budget,
        )?;
        issued_output_operand_v18(
            source.relation,
            optimized,
            row.operation,
            operation,
            0,
            *index,
            budget,
        )?;
        issued_output_operand_v18(
            source.relation,
            optimized,
            row.operation,
            operation,
            1,
            length_result.id,
            budget,
        )?;
        let original_index = source
            .relation
            .inventory
            .definition_for_value(row.operation.block.function, issuer.index, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued presence original index absent",
            ))?;
        let actual_index = inventory
            .definition_for_value(function.coordinate, *index, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued presence output index absent",
            ))?;
        issued_metadata_descendant_v18(
            source.relation,
            optimized,
            original_index.coordinate,
            actual_index.coordinate,
            None,
            budget,
        )?;
        output.push(OptimizedIssuedPresenceV31 {
            original,
            operation,
            value: present.id,
        });
    }
    private_array_heapsort_v1(
        &mut output,
        |row| [row.value.0 as usize],
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    for pair in output.windows(2) {
        budget.charge_work(1)?;
        if pair[0].value == pair[1].value {
            return source
                .relation
                .source
                .missing("optimized issued presence names are ambiguous");
        }
    }
    // All helper scratch is already owned by the containing scalar scope. Its
    // exact retained credit is released only after these rows and views drop.
    Ok(output)
}
