fn source_write_replay_headers_v86() -> Result<usize, ArgumentResourceV1> {
    type Coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
    type Frame<'a> = (
        [&'a (); 20],
        [usize; 8],
        std::slice::Iter<'a, PendingSourceIssuedSiteV29>,
        &'a PendingSourceWriteV86,
        [Option<Coordinate>; 7],
        [Coordinate; 7],
        [&'a Operation; 7],
        SourceOwnedResultV18<&'a [PendingSourceWriteV86]>,
        SourceOwnedResultV18<[Coordinate; 7]>,
        SourceOwnedResultV18<()>,
        Result<Type, ProductionSemanticKirErrorV1>,
        [Type; 2],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        checked_write_tail_headers_v85()?,
    ])
}

pub(super) fn checked_source_writes_v87<'view>(
    original: &'view ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'view [PendingSourceWriteV86]> {
    original.query(budget)?;
    original.retain_query((|| {
        let owner = original.source.root_row(root)?;
        let Some(pending) = owner.source_slots.pending_memory.as_ref() else {
            return Ok(&[][..]);
        };
        check_immutable_issued_roles_v18(original, root, &pending.issued, budget)?;
        Ok(pending.issued.writes.as_slice())
    })())
}

pub(super) fn source_write_tail_locations_v87(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    row: &PendingSourceWriteV86,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<[fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1; 7]> {
    let mut tail = [None; 7];
    for source in
        original.source_operation_rows(root, row.instance.index(), row.block, None, budget)?
    {
        budget.charge_work(8)?;
        let ProductionSourceOperationV18::Operation(operation) =
            original.mapped_source_operation(source.location, budget)?
        else {
            return original
                .source
                .missing("source write original operation is absent");
        };
        tail.rotate_left(1);
        tail[6] = Some(operation);
    }
    let [
        Some(length),
        Some(extent),
        Some(zero),
        Some(offset),
        Some(data),
        Some(pointer),
        Some(store),
    ] = tail
    else {
        return original
            .source
            .missing("source write original suffix is incomplete");
    };
    Ok([length, extent, zero, offset, data, pointer, store])
}

fn check_immutable_source_writes_v86(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    rows: &PendingSourceIssuedRolesV29,
    function: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let semantic = original.source.source_semantic(budget)?;
    let body = function
        .body
        .as_ref()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source write original body is absent",
        ))?;
    let mut count = 0usize;
    for site in &rows.sources {
        let declaration = original
            .source
            .instance(root, site.instance.index(), budget)?
            .0;
        let declaration = semantic
            .functions()
            .get(declaration.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source write function is absent",
            ))?;
        let call = source_issued_call_v29(semantic, declaration, site.block, budget)
            .map_err(immutable_memory_error_v29)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source write call is absent",
            ))?;
        budget.charge_work(3)?;
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                    element,
                    raw_index,
                    index_space,
                    kind,
                    ..
                },
            ..
        }) = semantic.callables().get(call.callee().index() as usize)
        else {
            continue;
        };
        let row = rows
            .writes
            .get(count)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source write call lacks its retained receipt",
            ))?;
        budget.charge_work(14)?;
        if row.instance != site.instance
            || row.block != site.block
            || !matches!(kind, SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint } if *disjoint == row.disjoint)
            || *index_space != row.index_space
            || lower_scalar_type(semantic.types(), *element).map_err(immutable_memory_error_v29)?
                != Type::Scalar(row.element)
            || lower_scalar_type(semantic.types(), *raw_index)
                .map_err(immutable_memory_error_v29)?
                != Type::Scalar(ScalarType::U64)
            || body.parameters.get(row.root_parameter) != Some(&row.root_input)
            || !matches!(function.signature.parameters.get(row.root_parameter), Some(Type::Slice(slice))
                if *slice.element == Type::Scalar(row.element) && slice.address_space == AddressSpace::Global
                    && slice.access == AccessMode::WriteOnly)
        {
            return original
                .source
                .missing("source write retained call or root differs");
        }
        check_issued_original_definition_v18(
            original,
            root,
            row.instance,
            row.block,
            row.definition,
            budget,
        )?;
        let coordinates = source_write_tail_locations_v87(original, root, row, budget)?;
        let [length, _, _, _, _, _, store] = coordinates;
        let first = source_operation_row_v18(original.inventory, length, budget)?.operation;
        let mut operations = [first; 7];
        for (destination, coordinate) in operations.iter_mut().zip(coordinates).skip(1) {
            budget.charge_work(1)?;
            *destination =
                source_operation_row_v18(original.inventory, coordinate, budget)?.operation;
        }
        let checked = check_checked_write_tail_v85(
            CheckedWriteInputsV85 {
                slice: row.receiver,
                index: row.index,
                precondition: None,
                value: row.value,
                element: row.element,
            },
            &operations,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        budget.charge_work(7)?;
        if checked != row.tail {
            return original
                .source
                .missing("source write original suffix differs");
        }
        let Some(access) =
            original.retained_memory_access(root, store, row.tail.pointer, budget)?
        else {
            return original
                .source
                .missing("source write original anchor is absent");
        };
        budget.charge_work(2)?;
        if access.instance != row.instance.index() || access.row != row.anchor {
            return original
                .source
                .missing("source write changed source occurrence");
        }
        original
            .retained_scalar_payload_v18(root, store, &access, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source write scalar payload is absent",
            ))?;
        check_issued_original_effect_v18(
            original,
            root,
            row.instance.index(),
            row.anchor,
            operations[6],
            budget,
        )?;
        count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    if count != rows.writes.len() {
        return original
            .source
            .missing("source write receipt census differs");
    }
    Ok(())
}
