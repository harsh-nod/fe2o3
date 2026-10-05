// Common immutable physical replay preserves the conditional source program.
// It does not install singleton descriptor roles or grant final memory safety.
fn selected_replay_error_v30() -> ProductionSourceOwnedViewErrorV18 {
    immutable_memory_error_v29(source_reference_selection_memory_error_v30())
}

fn source_selected_replay_headers_v30() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_reference_selection_memory_headers_v30()?,
        source_reference_selection_actual_headers_v30()?,
        source_selected_replay_frame_headers_v30()?,
    ])
}

pub(super) fn source_selected_replay_frame_headers_v30() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(2, std::mem::size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SourcePhysicalAccessV18<'_>>()?,
        h::<SourcePhysicalPayloadV18<'_>>()?,
        h::<Option<SourcePhysicalPayloadV18<'_>>>()?,
        h::<&PendingSourceSelectedAccessV30>()?,
        h::<&PendingSourceSelectedLeafV30>()?,
        h::<&PendingSourceSelectedGuardV30>()?,
        h::<&[SourceIssuedGuardV29]>()?,
        h::<&SemanticPlaceV1>()?,
        h::<&SemanticTypeDeclV1>()?,
        h::<&ScopedMemoryAnchorsV29>()?,
        h::<&ScopedMemoryAnchorV29>()?,
        h::<&SourceReferenceSelectionActualNodeV30>()?,
        h::<SourceReferenceSelectionSubjectV29>()?,
        h::<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>()?,
        h::<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>()?,
        h::<(usize, usize)>()?,
        h::<Option<(usize, usize)>>()?,
        h::<usize>()?,
        h::<()>()?,
    ])
}

fn check_selected_descriptor_receipt_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    actual: &SourceIssuedActualV29<'_>,
    descriptor: PendingSourceSelectedDescriptorV30,
    guards: &[PendingSourceSelectedGuardV30],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let slice = actual
        .value(descriptor.slice, budget)
        .map_err(immutable_memory_error_v29)?;
    let data = actual
        .value(descriptor.data, budget)
        .map_err(immutable_memory_error_v29)?;
    let address = actual
        .value(descriptor.pointer, budget)
        .map_err(immutable_memory_error_v29)?;
    let location = address.location.ok_or_else(selected_replay_error_v30)?;
    budget.charge_work(13)?;
    if !matches!(slice.ty, Type::Slice(ty) if *ty.element == Type::Scalar(descriptor.element)
        && ty.address_space == descriptor.space && ty.access == descriptor.access)
        || !matches!(
            descriptor.space,
            AddressSpace::Global | AddressSpace::Generic
        )
        || descriptor.access != AccessMode::ReadOnly
        || source_issued_pointer_shape_v26(address.ty)
            != Some((descriptor.element, descriptor.space, descriptor.access))
        || data.ty != address.ty
        || data
            .location
            .is_none_or(|at| at.0 != location.0 || at.1 >= location.1)
        || descriptor.formation
            != (
                location.0,
                u32::try_from(location.1).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            )
        || !matches!(data.operation.map(|row| &row.kind),
            Some(OperationKind::SliceData { slice }) if *slice == descriptor.slice)
        || !matches!(address.operation.map(|row| (&row.kind, row.results.as_slice())),
            Some((OperationKind::GetElementPointer { base, offset }, [result]))
                if *base == descriptor.data && *offset == descriptor.index
                    && result.id == descriptor.pointer && result.ty == *address.ty)
    {
        return Err(selected_replay_error_v30());
    }
    actual
        .selected_index_normalization_v30(
            descriptor.original_index,
            descriptor.original_scalar,
            descriptor.index,
            location,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
    for guard in guards {
        budget.charge_work(8)?;
        if guard.original_descriptor_guard.is_none() || guard.edge != 0 {
            return Err(selected_replay_error_v30());
        }
        let comparison = actual
            .value(guard.condition, budget)
            .map_err(immutable_memory_error_v29)?;
        let at = comparison.location.ok_or_else(selected_replay_error_v30)?;
        let Some(OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        }) = comparison.operation.map(|row| &row.kind)
        else {
            return Err(selected_replay_error_v30());
        };
        if *comparison.ty != Type::BOOL {
            return Err(selected_replay_error_v30());
        }
        let lhs_row = actual
            .value(*lhs, budget)
            .map_err(immutable_memory_error_v29)?;
        if *lhs_row.ty == Type::Scalar(ScalarType::U64) {
            if descriptor.original_scalar != ScalarType::U64 || *lhs != descriptor.original_index {
                return Err(selected_replay_error_v30());
            }
        } else {
            actual
                .selected_index_normalization_v30(
                    descriptor.original_index,
                    descriptor.original_scalar,
                    *lhs,
                    at,
                    budget,
                )
                .map_err(immutable_memory_error_v29)?;
        }
        let mut length = actual
            .value(*rhs, budget)
            .map_err(immutable_memory_error_v29)?;
        if *length.ty != *lhs_row.ty {
            return Err(selected_replay_error_v30());
        }
        if *length.ty == Type::Scalar(ScalarType::U64) {
            let Some(OperationKind::Cast {
                kind: CastKind::Bitcast,
                value,
                to: Type::Scalar(ScalarType::U64),
            }) = length.operation.map(|row| &row.kind)
            else {
                return Err(selected_replay_error_v30());
            };
            length = actual
                .value(*value, budget)
                .map_err(immutable_memory_error_v29)?;
        }
        if *length.ty != Type::INDEX
            || !matches!(length.operation.map(|row| &row.kind),
            Some(OperationKind::SliceLength { slice }) if *slice == descriptor.slice)
        {
            return Err(selected_replay_error_v30());
        }
        let block = original
            .inventory
            .block_for_id(function, guard.block, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or_else(selected_replay_error_v30)?;
        if !matches!(block.block.terminator.as_ref(),
            Some(Terminator::ConditionalBranch { condition, then_target, else_target, .. })
                if *condition == guard.condition && then_target != else_target)
        {
            return Err(selected_replay_error_v30());
        }
    }
    Ok(())
}

fn check_immutable_selected_rows_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    rows: &PendingSourceIssuedRolesV29,
    function: &Function,
    actual: &SourceIssuedActualV29<'_>,
    issued_guards: &[SourceIssuedGuardV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    if rows.selected.is_empty() {
        return Ok(());
    }
    budget.reserve_storage(source_selected_replay_headers_v30()?)?;
    let owner = original.source.root_row(root)?;
    let coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
        u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    );
    let semantic = original.source.source_semantic(budget)?;
    let mut previous = None;
    for row in &rows.selected {
        budget.charge_work(8)?;
        let key = (row.instance.index(), row.anchor);
        if previous.replace(key).is_some_and(|prior| prior >= key)
            || row.subject.instance != row.instance
        {
            return Err(selected_replay_error_v30());
        }
        let sidecar = original
            .source
            .sidecar(root, row.instance.index(), budget)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(selected_replay_error_v30)?;
        let anchor = anchors
            .rows
            .get(row.anchor)
            .ok_or_else(selected_replay_error_v30)?;
        let function_id = original
            .source
            .instance(root, row.instance.index(), budget)?
            .0;
        let source = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or_else(selected_replay_error_v30)?;
        let frame = anchor.source.ok_or_else(selected_replay_error_v30)?;
        let place = scoped_payload_place_v29(source, row.subject.site, row.subject.role)
            .ok_or_else(selected_replay_error_v30)?;
        if anchors.subject.instance != row.instance
            || anchors.subject.function != function_id
            || anchors.subject.ledger != original.ledger
            || frame.site != row.subject.site
            || frame.role != Some(ScopedMemoryRoleV29::Operand(row.subject.role))
            || row.subject.source != place as *const SemanticPlaceV1 as usize
        {
            return Err(selected_replay_error_v30());
        }
        let key = TileAttachmentKeyV29 {
            root,
            family: TileAttachmentFamilyV29::MemoryAnchor,
            instance: row.instance.index(),
            row: row.anchor,
            field: TileAttachmentFieldV29::MemoryPosition,
            component: 0,
            part: 0,
        };
        let [position] = original.attachment_range(key, budget)? else {
            return Err(selected_replay_error_v30());
        };
        let ProductionSourceOperationV18::Operation(operation) =
            original.mapped_source_operation(position.location, budget)?
        else {
            return Err(selected_replay_error_v30());
        };
        let block = original
            .inventory
            .block_for_id(coordinate, row.operation.0, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .ok_or_else(selected_replay_error_v30)?;
        if operation.block != block.coordinate || operation.operation != row.operation.1 {
            return Err(selected_replay_error_v30());
        }
        let [pointer] = original.attachment_range(
            TileAttachmentKeyV29 {
                field: TileAttachmentFieldV29::MemoryPointer,
                ..key
            },
            budget,
        )?
        else {
            return Err(selected_replay_error_v30());
        };
        if original.attachment_value(root, pointer.location, budget)? != row.pointer {
            return Err(selected_replay_error_v30());
        }
        let access = SourcePhysicalAccessV18 {
            instance: row.instance.index(),
            row: row.anchor,
            anchor,
        };
        let payload = original
            .retained_scalar_payload_v18(root, operation, &access, budget)?
            .ok_or_else(selected_replay_error_v30)?;
        let emitted = source_operation_row_v18(original.inventory, operation, budget)?.operation;
        check_issued_original_effect_v18(
            original,
            root,
            row.instance.index(),
            row.anchor,
            emitted,
            budget,
        )?;
        let memory = source_address_value_access_v29(emitted)
            .map_err(immutable_memory_error_v29)?
            .ok_or_else(selected_replay_error_v30)?;
        let first = row
            .selection
            .nodes
            .first()
            .ok_or_else(selected_replay_error_v30)?;
        let scalar =
            lower_scalar_type(semantic.types(), place.ty()).map_err(immutable_memory_error_v29)?;
        let ty = semantic
            .types()
            .get(place.ty().index() as usize)
            .ok_or_else(selected_replay_error_v30)?;
        if memory.object
            || memory.pointer != row.pointer
            || memory.value != row.value
            || memory.access != row.memory
            || memory.writing != row.writing
            || payload.value != row.value
            || scalar != Type::Scalar(first.element)
            || *actual
                .value(row.value, budget)
                .map_err(immutable_memory_error_v29)?
                .ty
                != scalar
            || row.memory.alignment == 0
            || u64::from(row.memory.alignment) > ty.layout().alignment_bytes()
            || !source_issued_memory_pointer_v26(
                actual,
                row.pointer,
                row.memory,
                row.writing,
                first.element,
                budget,
            )
            .map_err(immutable_memory_error_v29)?
        {
            return Err(selected_replay_error_v30());
        }
        for (leaf_index, leaf) in row.leaves.iter().enumerate() {
            budget.charge_work(6)?;
            let node = row
                .selection
                .nodes
                .get(leaf.node)
                .ok_or_else(selected_replay_error_v30)?;
            let end = argument_sum_v1(&[leaf.first_guard, leaf.guard_count])?;
            let guards = row
                .guards
                .get(leaf.first_guard..end)
                .ok_or_else(selected_replay_error_v30)?;
            match (node.original.step, leaf.origin) {
                (
                    SourceReferenceSelectionStepV29::Leaf(
                        SourceExternalReferenceOriginV29::Issued { instance, recipe },
                    ),
                    PendingSourceSelectedLeafOriginV30::Issued(issuer),
                ) => {
                    charge_execution_cfg_lookup_v29(rows.issuers.len(), budget)
                        .map_err(immutable_memory_error_v29)?;
                    let index = rows
                        .issuers
                        .binary_search_by_key(&(instance.index(), recipe.block.index()), |row| {
                            (row.instance.index(), row.block.index())
                        })
                        .map_err(|_| selected_replay_error_v30())?;
                    if rows.issuers[index] != issuer
                        || recipe.issuer != issuer.definition
                        || leaf.pointer != issuer.pointer
                    {
                        return Err(selected_replay_error_v30());
                    }
                    for guard in guards {
                        budget.charge_work(4)?;
                        if guard.leaf != leaf_index
                            || guard.original_descriptor_guard.is_some()
                            || guard.condition != issuer.present
                        {
                            return Err(selected_replay_error_v30());
                        }
                        charge_execution_cfg_lookup_v29(issued_guards.len(), budget)
                            .map_err(immutable_memory_error_v29)?;
                        if issued_guards
                            .binary_search_by_key(
                                &(issuer.present, guard.block, guard.edge),
                                |row| (row.present, row.block, row.edge),
                            )
                            .is_err()
                        {
                            return Err(selected_replay_error_v30());
                        }
                    }
                }
                (
                    SourceReferenceSelectionStepV29::Leaf(
                        SourceExternalReferenceOriginV29::Descriptor {
                            instance,
                            descriptor,
                        },
                    ),
                    PendingSourceSelectedLeafOriginV30::Descriptor(origin),
                ) => {
                    if (instance, descriptor) != (origin.instance, origin.descriptor)
                        || leaf.pointer != origin.pointer
                    {
                        return Err(selected_replay_error_v30());
                    }
                    check_selected_descriptor_receipt_v30(
                        original, coordinate, actual, origin, guards, budget,
                    )?;
                }
                _ => return Err(selected_replay_error_v30()),
            }
        }
        // Rebuild observed arguments and selected guard obligations from the
        // immutable actual CFG, then compare every retained ordered field.
        let scratch = budget.storage();
        let mut copy = copied_selected_rows_v30(std::slice::from_ref(row), budget)
            .map_err(immutable_memory_error_v29)?;
        let replay = &mut copy[0];
        for node in &mut replay.selection.nodes {
            budget.charge_work(1)?;
            if let Some(invocation) = node.invocation.as_mut() {
                invocation.argument = None;
            }
        }
        for edge in &mut replay.selection.edges {
            budget.charge_work(1)?;
            edge.argument = None;
        }
        for obligation in &mut replay.obligations {
            budget.charge_work(2)?;
            obligation.guard = None;
            obligation.at_access = false;
        }
        source_reference_selection_check_transport_v30(
            function,
            actual,
            &mut replay.selection,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        check_source_reference_selection_guards_v30(function, actual, replay, budget)
            .map_err(immutable_memory_error_v29)?;
        if !row
            .matches(replay, budget)
            .map_err(immutable_memory_error_v29)?
        {
            return Err(selected_replay_error_v30());
        }
        drop(copy);
        let retained = budget
            .storage()
            .checked_sub(scratch)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(retained)?;
    }
    Ok(())
}
