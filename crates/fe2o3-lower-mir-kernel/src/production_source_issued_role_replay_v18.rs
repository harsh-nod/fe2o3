// Immutable replay of private retained recipes, not a native memory or
// initializedness certificate. The mandatory physical owner caller authenticates
// the exact original graph/root first; no alternate memory profile is selected.

pub(super) fn source_issued_tail_locations_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    row: &PendingSourceIssuedIssuerV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<[fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1; 4]> {
    let mut tail = [None; 4];
    for source in
        original.source_operation_rows(root, row.instance.index(), row.block, None, budget)?
    {
        budget.charge_work(5)?;
        let ProductionSourceOperationV18::Operation(operation) =
            original.mapped_source_operation(source.location, budget)?
        else {
            return Err(immutable_memory_error_v29(source_issued_error_v29()));
        };
        tail.rotate_left(1);
        tail[3] = Some(operation);
    }
    let [Some(length), Some(compare), Some(data), Some(address)] = tail else {
        return Err(immutable_memory_error_v29(source_issued_error_v29()));
    };
    Ok([length, compare, data, address])
}

// Borrowed owner recipes only. Optimized roles must additionally recheck their
// actual output operands/results and current success edges before publication.
// The closed caller prepays this query's fixed borrowed return envelopes.
pub(super) fn checked_issued_source_rows_v18<'a>(
    original: &'a ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<&'a PendingSourceIssuedRolesV29>> {
    original.query(budget)?;
    original.retain_query((|| {
        let owner = original.source.root_row(root)?;
        let Some(pending) = owner.source_slots.pending_memory.as_ref() else {
            return Ok(None);
        };
        check_immutable_issued_roles_v18(original, root, &pending.issued, budget)?;
        Ok(Some(&pending.issued))
    })())
}

fn check_issued_original_definition_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    row: &PendingSourceIssuedIssuerV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let source = original.source.source_semantic(budget)?;
    let function_id = original
        .source
        .instance(root, row.instance.index(), budget)?
        .0;
    let function = source.functions().get(function_id.index() as usize).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("issued original definition function"),
    )?;
    let call = source_issued_call_v29(source, function, row.block, budget)
        .map_err(immutable_memory_error_v29)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued original definition call",
        ))?;
    let destination = call
        .destination()
        .map(|destination| destination.place())
        .filter(|destination| destination.projections().is_empty())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued original definition destination",
        ))?;
    let ssa = original.source.source_ssa(budget)?;
    let occurrences = ssa
        .occurrences_v1()
        .and_then(|rows| rows.function(function_id))
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued original definition occurrences",
        ))?;
    let mut definition = None;
    for edge in occurrences.edge_definitions() {
        budget.charge_work(6)?;
        if edge.is_reachable()
            && edge.is_promoted()
            && edge.edge().source().get() == row.block.index()
            && edge.edge().ordinal() == 0
            && edge.ordinal() == 0
            && edge.variable().get() == destination.local().index()
        {
            if definition
                .replace(
                    edge.value()
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "issued original definition value",
                        ))?,
                )
                .is_some()
            {
                return original
                    .source
                    .missing("issued original definition is ambiguous");
            }
        }
    }
    budget.charge_work(1)?;
    if definition != Some(row.definition) {
        return original
            .source
            .missing("issued original definition differs");
    }
    Ok(())
}

// Both immutable and optimized access replay call this against the authenticated
// original statement. The retained MemoryAccess bit is never effect authority.
pub(super) fn check_issued_original_effect_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    instance: usize,
    anchor: usize,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.query(budget)?;
    original.retain_query((|| {
        let semantic = original.source.source_semantic(budget)?;
        let function_id = original.source.instance(root, instance, budget)?.0;
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued original effect function",
            ))?;
        let anchors = original
            .source
            .sidecar(root, instance, budget)?
            .scoped_memory_anchors
            .as_ref()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued original effect anchors",
            ))?;
        let row = anchors
            .rows
            .get(anchor)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued original effect anchor",
            ))?;
        check_scoped_payload_effect_v29(function, row, operation, budget)
            .map_err(immutable_memory_error_v29)
    })())
}

fn check_immutable_issued_roles_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    rows: &PendingSourceIssuedRolesV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.query(budget)?;
    let floor = budget.storage();
    let retained = scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        let floor = budget.storage();
        original.source.retain_construction(|| {
            budget.reserve_storage(source_issued_replay_headers_v18()?)?;
            check_immutable_issued_roles_inner_v18(original, root, rows, budget)?;
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting.into())
        })
    })?;
    original.retain_query(budget.release_storage(retained).map_err(Into::into))
}

fn check_immutable_issued_roles_inner_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    rows: &PendingSourceIssuedRolesV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let owner = original.source.root_row(root)?;
    let expected = &owner
        .source_slots
        .pending_memory
        .as_ref()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued immutable memory owner is absent",
        ))?
        .issued;
    budget.charge_work(1)?;
    if !std::ptr::eq(rows, expected)
        && !pending_issued_roles_match_v29(rows, expected, budget)
            .map_err(immutable_memory_error_v29)?
    {
        return original
            .source
            .missing("issued immutable receipt differs from its original owner");
    }
    let semantic = original.source.source_semantic(budget)?;
    let mut source_count = 0usize;
    for instance in &owner.coordinates.sources.rows {
        if original
            .source
            .optional_sidecar(root, instance.instance.index(), budget)?
            .is_none()
        {
            continue;
        }
        let source = original
            .source
            .instance(root, instance.instance.index(), budget)?
            .0;
        let function = semantic.functions().get(source.index() as usize).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("issued source function"),
        )?;
        for block in 0..function.blocks().len() {
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if source_issued_call_v29(semantic, function, block, budget)
                .map_err(immutable_memory_error_v29)?
                .is_none()
                || original
                    .source_block_entry(root, instance.instance.index(), block, budget)?
                    .is_none()
            {
                continue;
            }
            budget.charge_work(2)?;
            if rows.sources.get(source_count)
                != Some(&PendingSourceIssuedSiteV29 {
                    instance: instance.instance,
                    block,
                })
            {
                return original
                    .source
                    .missing("issued source issuer census differs");
            }
            source_count = source_count
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
    }
    if source_count != rows.sources.len() {
        return original
            .source
            .missing("issued source issuer census differs");
    }
    if source_count == 0 {
        if !rows.issuers.is_empty() || !rows.accesses.is_empty() {
            return original
                .source
                .missing("issued evidence lacks original issuer");
        }
        return Ok(());
    }
    let function = original
        .inventory
        .functions()
        .get(owner.function_ordinal)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued original root function",
        ))?
        .function;
    let body = function
        .body
        .as_ref()
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued original root body",
        ))?;
    let actual = SourceIssuedActualV29::from_function(function, budget)
        .map_err(immutable_memory_error_v29)?;
    let mut guards =
        source_issued_guards_v29(function, &actual, budget).map_err(immutable_memory_error_v29)?;
    call_splice_sort_work_v1(guards.len(), budget)
        .map_err(source_address_call_error_v29)
        .map_err(immutable_memory_error_v29)?;
    guards.sort_unstable_by_key(|row| (row.present, row.block, row.edge));
    let mut issuers =
        emission_vec_v1(rows.issuers.len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(rows.issuers.len())?;
    issuers.extend(0..rows.issuers.len());
    call_splice_sort_work_v1(issuers.len(), budget)
        .map_err(source_address_call_error_v29)
        .map_err(immutable_memory_error_v29)?;
    issuers.sort_unstable_by_key(|&index| {
        (
            rows.issuers[index].instance.index(),
            rows.issuers[index].definition,
        )
    });
    for pair in issuers.windows(2) {
        budget.charge_work(2)?;
        let left = &rows.issuers[pair[0]];
        let right = &rows.issuers[pair[1]];
        if (left.instance, left.definition) == (right.instance, right.definition) {
            return original
                .source
                .missing("issued immutable definition is ambiguous");
        }
    }
    let mut accesses =
        emission_vec_v1(rows.accesses.len(), budget).map_err(immutable_memory_error_v29)?;
    let mut previous = None;
    for row in &rows.issuers {
        let key = (row.instance.index(), row.block.index());
        budget.charge_work(2)?;
        if previous
            .replace(key)
            .is_some_and(|previous| previous >= key)
        {
            return original
                .source
                .missing("issued source duplicate or unordered issuer");
        }
        charge_execution_cfg_lookup_v29(rows.sources.len(), budget)
            .map_err(immutable_memory_error_v29)?;
        if rows
            .sources
            .binary_search_by_key(&key, |row| (row.instance.index(), row.block.index()))
            .is_err()
        {
            return original.source.missing("issued source unclaimed issuer");
        }
        check_issued_original_definition_v18(original, root, row, budget)?;
        let [length, compare, data, address] =
            source_issued_tail_locations_v18(original, root, row, budget)?;
        let length = source_operation_row_v18(original.inventory, length, budget)?.operation;
        let compare = source_operation_row_v18(original.inventory, compare, budget)?.operation;
        let data = source_operation_row_v18(original.inventory, data, budget)?.operation;
        let address = source_operation_row_v18(original.inventory, address, budget)?.operation;
        check_source_issued_tail_v29(
            SourceIssuedPhysicalV29 {
                present: row.present,
                pointer: row.pointer,
                element: row.element,
                access: row.access,
            },
            row.receiver,
            row.index,
            length,
            compare,
            data,
            address,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        budget.charge_work(7)?;
        if !matches!(length.results.as_slice(), [value] if value.id == row.length)
            || !matches!(data.results.as_slice(), [value] if value.id == row.data)
            || body.parameters.get(row.root_parameter) != Some(&row.root_input)
            || !matches!(function.signature.parameters.get(row.root_parameter), Some(Type::Slice(slice))
                if *slice.element == Type::Scalar(row.element) && slice.address_space == AddressSpace::Global
                    && slice.access == row.access)
        {
            return Err(immutable_memory_error_v29(source_issued_error_v29()));
        }
    }
    let mut previous_access = None;
    for row in &rows.accesses {
        budget.charge_work(2)?;
        let key = (row.instance.index(), row.anchor);
        if previous_access
            .replace(key)
            .is_some_and(|previous| previous >= key)
        {
            return original
                .source
                .missing("issued source duplicate or unordered access");
        }
        charge_execution_cfg_lookup_v29(issuers.len(), budget)
            .map_err(immutable_memory_error_v29)?;
        let issuer = issuers
            .binary_search_by_key(&(row.instance.index(), row.issuer), |&index| {
                (
                    rows.issuers[index].instance.index(),
                    rows.issuers[index].definition,
                )
            })
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding("issued access original issuer")
            })?;
        let issuer = &rows.issuers[issuers[issuer]];
        let [position] = original.attachment_range(
            TileAttachmentKeyV29 {
                root,
                family: TileAttachmentFamilyV29::MemoryAnchor,
                instance: row.instance.index(),
                row: row.anchor,
                field: TileAttachmentFieldV29::MemoryPosition,
                component: 0,
                part: 0,
            },
            budget,
        )?
        else {
            return original.source.missing("issued access position census");
        };
        let ProductionSourceOperationV18::Operation(coordinate) =
            original.mapped_source_operation(position.location, budget)?
        else {
            return original.source.missing("issued access actual position");
        };
        let Some(access) =
            original.retained_memory_access(root, coordinate, issuer.pointer, budget)?
        else {
            return original
                .source
                .missing("issued access actual source receipt");
        };
        if access.instance != row.instance.index() || access.row != row.anchor {
            return original
                .source
                .missing("issued access changed source occurrence");
        }
        original
            .retained_scalar_payload_v18(root, coordinate, &access, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued access scalar payload",
            ))?;
        let operation = source_operation_row_v18(original.inventory, coordinate, budget)?.operation;
        check_issued_original_effect_v18(
            original,
            root,
            row.instance.index(),
            row.anchor,
            operation,
            budget,
        )?;
        let value = source_address_value_access_v29(operation)
            .map_err(immutable_memory_error_v29)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued access opcode",
            ))?;
        budget.charge_work(8)?;
        if value.object
            || value.pointer != issuer.pointer
            || value.access.address_space != AddressSpace::Global
            || (value.writing && issuer.access != AccessMode::ReadWrite)
            || value.access != row.access
            || value.writing != row.writing
            || *actual
                .value(value.value, budget)
                .map_err(immutable_memory_error_v29)?
                .ty
                != Type::Scalar(issuer.element)
        {
            return Err(immutable_memory_error_v29(source_issued_error_v29()));
        }
        charge_execution_cfg_lookup_v29(guards.len(), budget)
            .map_err(immutable_memory_error_v29)?;
        if guards
            .binary_search_by_key(
                &(issuer.present, row.guard_block, row.guard_edge),
                |guard| (guard.present, guard.block, guard.edge),
            )
            .is_err()
        {
            return Err(immutable_memory_error_v29(source_issued_error_v29()));
        }
        if coordinate.block.function.0 as usize != owner.function_ordinal {
            return original
                .source
                .missing("issued access changed physical root");
        }
        let block = body
            .blocks
            .get(coordinate.block.block as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "issued access block",
            ))?
            .id;
        accesses.push((row.guard_block, row.guard_edge, block));
    }
    budget.charge_work(argument_product_v1(
        2,
        argument_sum_v1(&[rows.issuers.len(), accesses.len()])?,
    )?)?;
    let mut valid = true;
    fe2o3_kernel_ir::with_function_control_flow_v1(function, Default::default(), budget, |view| {
        for issuer in &rows.issuers {
            if issuer.receiver != issuer.root_input
                && view.unique_value_origin(issuer.receiver)? != Some(issuer.root_input)
            {
                valid = false;
            }
        }
        for &(guard, edge, access) in &accesses {
            if !view.success_edge_dominates(guard, edge, access)? {
                valid = false;
            }
        }
        Ok(())
    })
    .map_err(|error| match error {
        fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
        _ => immutable_memory_error_v29(source_issued_error_v29()),
    })?;
    if !valid {
        return Err(immutable_memory_error_v29(source_issued_error_v29()));
    }
    Ok(())
}

pub(super) fn source_issued_replay_headers_v18() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            argument_product_v1(2, std::mem::size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        source_issued_census_query_headers_v29()?,
        h::<usize>()?,
        h::<bool>()?,
        h::<()>()?,
        h::<Option<usize>>()?,
        h::<&ScopedMemoryAnchorV29>()?,
        h::<Option<&ScopedMemoryAnchorV29>>()?,
        h::<&ScopedMemoryAnchorsV29>()?,
        h::<Option<&ScopedMemoryAnchorsV29>>()?,
        h::<ScopedMemoryStoreSourceV29>()?,
        h::<ScopedMemoryReadV29>()?,
        h::<Option<&SemanticStatementKindV1>>()?,
        h::<&fe2o3_mir_model::semantic_mir_v1::SemanticMemoryStoreV1>()?,
        std::mem::size_of::<(
            &ProductionSourceCorrespondenceV18<'_>,
            usize,
            usize,
            usize,
            &Operation,
        )>(),
        std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        h::<&fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1>()?,
        h::<&ProductionSemanticSsaOwnerV1>()?,
        h::<fe2o3_pliron::ProductionSemanticSsaOccurrenceViewV1<'_>>()?,
        h::<Option<fe2o3_pliron::ProductionSemanticSsaOccurrenceViewV1<'_>>>()?,
        h::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>()?,
        h::<Option<ProductionSemanticSsaFunctionOccurrencesV1<'_>>>()?,
        h::<&ScopedModuleRootV29>()?,
        h::<&Function>()?,
        h::<&PendingSourceIssuedRolesV29>()?,
        h::<&PendingSourceMemoryV29>()?,
        h::<Option<&PendingSourceMemoryV29>>()?,
        h::<Option<&PendingInstanceSidecarsV29>>()?,
        h::<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>()?,
        h::<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>>()?,
        h::<&fe2o3_kernel_ir::FunctionBody>()?,
        h::<Option<&fe2o3_kernel_ir::FunctionBody>>()?,
        h::<Option<&fe2o3_kernel_ir::BasicBlock>>()?,
        h::<&fe2o3_kernel_ir::BasicBlock>()?,
        h::<Option<&ValueId>>()?,
        h::<Option<&Type>>()?,
        h::<&Type>()?,
        h::<&fe2o3_kernel_ir::PointerType>()?,
        h::<Type>()?,
        argument_product_v1(4, h::<&fe2o3_kernel_ir::ValueDef>()?)?,
        h::<&fe2o3_kernel_ir::SliceType>()?,
        h::<&SemanticFunctionDeclV1>()?,
        h::<Option<&SemanticFunctionDeclV1>>()?,
        h::<PendingSourceIssuedSiteV29>()?,
        h::<Option<&PendingSourceIssuedSiteV29>>()?,
        h::<&PendingSourceIssuedSiteV29>()?,
        h::<&PendingSourceIssuedIssuerV29>()?,
        h::<Option<&PendingSourceIssuedIssuerV29>>()?,
        h::<(usize, usize)>()?,
        h::<Option<(usize, usize)>>()?,
        h::<(usize, u32)>()?,
        h::<Option<(usize, u32)>>()?,
        h::<[fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1; 4]>()?,
        h::<[Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>; 4]>()?,
        h::<&[SourceAttachmentV18]>()?,
        h::<TileAttachmentKeyV29>()?,
        h::<&SourceAttachmentV18>()?,
        h::<Option<&SourceAttachmentV18>>()?,
        h::<ProductionSourceOperationV18>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>()?,
        h::<Option<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>>()?,
        argument_product_v1(4, h::<&Operation>()?)?,
        argument_product_v1(
            4,
            h::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()?,
        )?,
        h::<SourceIssuedPhysicalV29>()?,
        h::<SourceIssuedActualV29<'_>>()?,
        h::<Vec<SourceIssuedGuardV29>>()?,
        h::<&SourceIssuedActualValueV29<'_>>()?,
        h::<Option<&SourceIssuedActualValueV29<'_>>>()?,
        // present() can retain one value row while querying another row.
        argument_product_v1(2, h::<SourceIssuedActualValueV29<'_>>()?)?,
        argument_product_v1(
            2,
            std::mem::size_of::<Result<SourceIssuedActualValueV29<'_>, ProductionSemanticKirErrorV1>>(
            ),
        )?,
        h::<Option<ValueId>>()?,
        std::mem::size_of::<Result<Option<ValueId>, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<Result<Vec<SourceIssuedGuardV29>, ProductionSemanticKirErrorV1>>(),
        h::<Vec<(BlockId, usize, BlockId)>>()?,
        h::<Vec<usize>>()?,
        std::mem::size_of::<Result<Vec<usize>, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<Result<Vec<(BlockId, usize, BlockId)>, ProductionSemanticKirErrorV1>>(),
        h::<Option<SourcePhysicalAccessV18<'_>>>()?,
        h::<SourcePhysicalAccessV18<'_>>()?,
        h::<Option<SourcePhysicalPayloadV18<'_>>>()?,
        h::<SourcePhysicalPayloadV18<'_>>()?,
        h::<Option<SourceAddressValueAccessV29>>()?,
        h::<SourceAddressValueAccessV29>()?,
        std::mem::size_of::<
            Result<Option<SourceAddressValueAccessV29>, ProductionSemanticKirErrorV1>,
        >(),
        h::<std::slice::Iter<'_, PendingSourceIssuedIssuerV29>>()?,
        h::<std::slice::Iter<'_, PendingSourceIssuedAccessV29>>()?,
        h::<&PendingSourceIssuedAccessV29>()?,
        h::<Option<&PendingSourceIssuedAccessV29>>()?,
        h::<std::slice::Iter<'_, OwnedInstanceSourceV1>>()?,
        h::<&OwnedInstanceSourceV1>()?,
        h::<Option<&OwnedInstanceSourceV1>>()?,
        h::<&SourceIssuedGuardV29>()?,
        h::<Option<&SourceIssuedGuardV29>>()?,
        h::<std::slice::Iter<'_, (BlockId, usize, BlockId)>>()?,
        h::<&(BlockId, usize, BlockId)>()?,
        h::<Option<&(BlockId, usize, BlockId)>>()?,
        h::<(BlockId, usize, BlockId)>()?,
        h::<std::slice::Windows<'_, usize>>()?,
        h::<&[usize]>()?,
        h::<Option<&[usize]>>()?,
        h::<&usize>()?,
        h::<Result<usize, usize>>()?,
        h::<(usize, SsaValueV1)>()?,
        h::<(ValueId, BlockId, usize)>()?,
        h::<std::slice::Iter<'_, SourceAttachmentV18>>()?,
        // Scoped-attempt/construction/CFG and sequential comparator/tail
        // captures have bounded borrowed layouts, including alignment space.
        argument_product_v1(4 + 5 + 3 + 6 + 2, std::mem::size_of::<usize>())?,
        std::mem::size_of::<Result<(), fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1>>(),
        std::mem::size_of::<std::thread::Result<SourceOwnedResultV18<usize>>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}
