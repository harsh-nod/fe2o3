pub(super) fn checked_source_lengths_v76<'view>(
    original: &'view ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'view [PendingSourceLengthV76]> {
    original.query(budget)?;
    original.retain_query((|| {
        let owner = original.source.root_row(root)?;
        let Some(pending) = owner.source_slots.pending_memory.as_ref() else {
            return Ok(&[][..]);
        };
        check_immutable_issued_roles_v18(original, root, &pending.issued, budget)?;
        Ok(pending.issued.lengths.as_slice())
    })())
}

pub(super) fn source_length_replay_headers_v76() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a (); 12],
        [usize; 5],
        std::slice::Iter<'a, PendingSourceIssuedSiteV29>,
        &'a PendingSourceLengthV76,
        Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
        SourceOwnedResultV18<&'a [PendingSourceLengthV76]>,
        SourceOwnedResultV18<()>,
        Option<(SemanticTypeIdV1, SemanticTypeIdV1, SemanticTypeIdV1)>,
        Result<Type, ProductionSemanticKirErrorV1>,
        [Type; 2],
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

fn check_immutable_source_lengths_v76(
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
            "source length original body is absent",
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
                "source length function is absent",
            ))?;
        let call = source_issued_call_v29(semantic, declaration, site.block, budget)
            .map_err(immutable_memory_error_v29)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source length call is absent",
            ))?;
        let Some((_, element, raw_index)) = source_length_call_v76(semantic, call) else {
            continue;
        };
        budget.charge_work(12)?;
        let row = rows
            .lengths
            .get(count)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source length call lacks its retained receipt",
            ))?;
        if row.instance != site.instance
            || row.block != site.block
            || lower_scalar_type(semantic.types(), element).map_err(immutable_memory_error_v29)?
                != Type::Scalar(row.element)
            || lower_scalar_type(semantic.types(), raw_index).map_err(immutable_memory_error_v29)?
                != Type::Scalar(ScalarType::U64)
            || body.parameters.get(row.root_parameter) != Some(&row.root_input)
            || !matches!(function.signature.parameters.get(row.root_parameter), Some(Type::Slice(slice))
                if *slice.element == Type::Scalar(row.element) && slice.address_space == AddressSpace::Global
                    && slice.access == row.access)
        {
            return original
                .source
                .missing("source length retained call or root differs");
        }
        check_issued_original_definition_v18(
            original,
            root,
            row.instance,
            row.block,
            row.definition,
            budget,
        )?;
        let mut last = None;
        for source in
            original.source_operation_rows(root, row.instance.index(), row.block, None, budget)?
        {
            budget.charge_work(2)?;
            let ProductionSourceOperationV18::Operation(operation) =
                original.mapped_source_operation(source.location, budget)?
            else {
                return original
                    .source
                    .missing("source length original operation is absent");
            };
            last = Some(operation);
        }
        let last = last.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source length original span is empty",
        ))?;
        let operation = source_operation_row_v18(original.inventory, last, budget)?.operation;
        if !matches!(operation.kind, OperationKind::SliceLength { slice } if slice == row.receiver)
            || !matches!(operation.results.as_slice(), [result] if result.id == row.length && result.ty == Type::INDEX)
        {
            return original
                .source
                .missing("source length original receiver or result differs");
        }
        count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    if count != rows.lengths.len() {
        return original
            .source
            .missing("source length receipt census differs");
    }
    Ok(())
}
