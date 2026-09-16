type SourceOutputFullAddressSiteV1 = (u32, Option<u32>, u32);
type SourceOutputFullAddressEventKeyV1 = (u32, u32, u32, u32, u32, u32);

#[derive(Clone, Copy, Eq, PartialEq)]
struct SourceOutputFullAddressAnchorV1 {
    local: SemanticLocalIdV1,
    component: ProductionProjectionArgumentComponentV1,
    scalar: ProductionSemanticScalarTypeV2,
    original: SourceOutputAddressDefV1,
    output: Option<SourceOutputAddressDefV1>,
    ssa: SsaValueV1,
    bits: Option<u64>,
    invocation: bool,
}

struct SourceOutputFullAddressClaimV1 {
    claim: ProductionProjectionArgumentCandidateV1,
    anchor: Option<SourceOutputFullAddressAnchorV1>,
}

#[derive(Default)]
struct SourceOutputFullAddressWorkspaceV1 {
    full: SourceOutputAddressWorkspaceV1,
    memory: SourceOutputAddressWorkspaceV1,
    sites: Vec<(SourceOutputFullAddressSiteV1, usize)>,
    events: Vec<(SourceOutputFullAddressEventKeyV1, usize)>,
    entries: Vec<(u32, usize)>,
    claims: Vec<SourceOutputFullAddressClaimV1>,
}

fn source_output_full_address_workspace_v1(
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputFullAddressWorkspaceV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    budget
        .reserve_storage(std::mem::size_of::<SourceOutputFullAddressWorkspaceV1>())
        .map_err(Error::Resource)?;
    Ok(SourceOutputFullAddressWorkspaceV1::default())
}

fn source_output_full_address_bind_v1(
    row: &mut SourceOutputFullAddressClaimV1,
    anchor: SourceOutputFullAddressAnchorV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(12).map_err(Error::Resource)?;
    if row.claim.source_local != anchor.local
        || row.claim.component != anchor.component
        || row.anchor.is_some_and(|prior| prior != anchor)
    {
        return Err(Error::Invalid("full address leaf source mapping differs"));
    }
    row.anchor = Some(anchor);
    Ok(())
}

fn source_output_full_address_site_v1(
    source: ProductionRankedAccessSourceV1,
) -> SourceOutputFullAddressSiteV1 {
    (
        source.semantic_block(),
        source.semantic_statement(),
        source.semantic_access_ordinal(),
    )
}

fn source_output_full_address_event_key_v1(
    site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
    operand: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    role: fe2o3_pliron::ProductionSemanticSsaEventRoleV1,
) -> Option<SourceOutputFullAddressEventKeyV1> {
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
        ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    let Site::Statement { block, statement } = site else {
        return None;
    };
    let (operand, argument) = match operand {
        Operand::Destination => (0, 0),
        Operand::StoreDestination => (1, 0),
        Operand::RvaluePlace => (2, 0),
        Operand::RvalueOperand(argument) => (3, argument),
        _ => return None,
    };
    let (role, projection) = match role {
        Role::BaseUse => (0, 0),
        Role::ProjectionIndexUse(projection) => (1, projection),
        Role::DestinationDefine => (2, 0),
        _ => return None,
    };
    Some((block.get(), statement, operand, argument, role, projection))
}

fn source_output_full_address_events_v1(
    work: &mut SourceOutputFullAddressWorkspaceV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for (ordinal, event) in captured.events().iter().enumerate() {
        budget.charge_work(5).map_err(Error::Resource)?;
        if let Some(key) =
            source_output_full_address_event_key_v1(event.site(), event.operand(), event.role())
        {
            assert_origin_push_v1(&mut work.events, (key, ordinal), budget)
                .map_err(Error::SourceOrigin)?;
        }
    }
    // These keys contain six numeric fields, unlike the smaller shared indexes.
    assert_origin_sort_v1(&mut work.events, budget, |a, b, budget| {
        budget.charge_work(6)?;
        Ok(a.0.cmp(&b.0))
    })
    .map_err(Error::SourceOrigin)?;
    for pair in work.events.windows(2) {
        budget.charge_work(6).map_err(Error::Resource)?;
        if pair[0].0 == pair[1].0 {
            return Err(Error::Invalid(
                "full address captured event key is ambiguous",
            ));
        }
    }
    for (ordinal, entry) in captured.entry_definitions().iter().enumerate() {
        budget.charge_work(2).map_err(Error::Resource)?;
        assert_origin_push_v1(&mut work.entries, (entry.variable().get(), ordinal), budget)
            .map_err(Error::SourceOrigin)?;
    }
    source_output_ranked_sort_unique_v1(&mut work.entries, budget, |a, b| a.0.cmp(&b.0))
}

fn source_output_full_address_event_v1<'a>(
    work: &SourceOutputFullAddressWorkspaceV1,
    captured: &'a fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    key: SourceOutputFullAddressEventKeyV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<&'a fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1, ProductionSourceOutputErrorV1>
{
    use ProductionSourceOutputErrorV1 as Error;
    let index = assert_origin_find_v1(&work.events, budget, |row, budget| {
        budget.charge_work(6)?;
        Ok(row.0.cmp(&key))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("full address exact source event absent"))?;
    budget.charge_work(5).map_err(Error::Resource)?;
    let event = captured
        .events()
        .get(work.events[index].1)
        .ok_or(Error::Invalid("full address source event index differs"))?;
    if source_output_full_address_event_key_v1(event.site(), event.operand(), event.role())
        != Some(key)
        || !event.is_reachable()
        || !event.is_promoted()
    {
        return Err(Error::Invalid(
            "full address source event is not exact reachable promoted occurrence",
        ));
    }
    // ordinal() belongs to this borrowed, replay-captured row. It is never
    // inferred from local identity or substituted with a ranked/O coordinate.
    let _original_block_event_ordinal = event.ordinal();
    Ok(event)
}

fn source_output_full_address_use_v1(
    work: &SourceOutputFullAddressWorkspaceV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    key: SourceOutputFullAddressEventKeyV1,
    local: SemanticLocalIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SsaValueV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let event = source_output_full_address_event_v1(work, captured, key, budget)?;
    budget.charge_work(3).map_err(Error::Resource)?;
    match (event.event(), event.resolved()) {
        (
            fe2o3_mir_model::SsaEventV1::Use(original),
            Some(SsaResolvedEventV1::Use { variable, value }),
        ) if original == variable && variable.get() == local.index() => Ok(value),
        _ => Err(Error::Invalid("full address captured source use differs")),
    }
}

fn source_output_full_address_entry_v1(
    work: &SourceOutputFullAddressWorkspaceV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    function: &SemanticFunctionDeclV1,
    local: SemanticLocalIdV1,
    value: SsaValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let row = assert_origin_find_v1(&work.entries, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.0.cmp(&local.index()))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("full address source entry absent"))?;
    budget.charge_work(5).map_err(Error::Resource)?;
    let entry = &captured.entry_definitions()[work.entries[row].1];
    let declaration = function
        .locals()
        .get(local.index() as usize)
        .ok_or(Error::Invalid("full address source local absent"))?;
    let SemanticLocalRoleV1::Argument(argument) = declaration.role() else {
        return Err(Error::Invalid(
            "full address source base or formal is not an argument",
        ));
    };
    if entry.variable().get() != local.index()
        || entry.origin() != fe2o3_pliron::ProductionSemanticSsaEntryOriginV1::Argument(argument)
        || entry.value() != Some(value)
    {
        return Err(Error::Invalid(
            "full address captured source entry value differs",
        ));
    }
    Ok(())
}

fn source_output_full_address_place_v1<'a>(
    function: &'a SemanticFunctionDeclV1,
    source: ProductionRankedAccessSourceV1,
    write: bool,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<
    (
        &'a SemanticPlaceV1,
        fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        SemanticLocalIdV1,
    ),
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Operand;
    budget.charge_work(9).map_err(Error::Resource)?;
    let statement = source
        .semantic_statement()
        .ok_or(Error::Invalid("full address source terminator unsupported"))?;
    let statement = function
        .blocks()
        .get(source.semantic_block() as usize)
        .and_then(|block| block.statements().get(statement as usize))
        .ok_or(Error::Invalid("full address source statement absent"))?;
    let (place, role) = match (write, statement.kind()) {
        (true, SemanticStatementKindV1::Assign(assignment)) => {
            (assignment.destination(), Operand::Destination)
        }
        (true, SemanticStatementKindV1::Store(store)) => {
            (store.destination(), Operand::StoreDestination)
        }
        (false, SemanticStatementKindV1::Assign(assignment)) => match assignment.value().kind() {
            SemanticRvalueKindV1::Load(load) => (load.source(), Operand::RvaluePlace),
            SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            ) => (place, Operand::RvalueOperand(0)),
            _ => {
                return Err(Error::Invalid(
                    "full address source read grammar unsupported",
                ));
            }
        },
        _ => {
            return Err(Error::Invalid(
                "full address source place grammar unsupported",
            ));
        }
    };
    let [dereference, index] = place.projections() else {
        return Err(Error::Invalid(
            "full address requires direct Slice index place",
        ));
    };
    let SemanticProjectionKindV1::Index(index) = index.kind() else {
        return Err(Error::Invalid("full address source index is not a local"));
    };
    if dereference.kind() != SemanticProjectionKindV1::Dereference {
        return Err(Error::Invalid(
            "full address source base is not dereferenced",
        ));
    }
    Ok((place, role, index))
}

fn source_output_full_address_memory_v1(
    lowering: &ProductionRankedKernelLoweringInputV1,
    source: ProductionRankedAccessSourceV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(bool, ProductionRankedValueV1, ProductionRankedValueV1), ProductionSourceOutputErrorV1>
{
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(5).map_err(Error::Resource)?;
    let operation = lowering
        .kernel()
        .blocks()
        .get(source.ranked_block() as usize)
        .and_then(|block| block.operations().get(source.ranked_operation() as usize))
        .ok_or(Error::Invalid("full address ranked use absent"))?;
    let (kind, view, indices) = match operation {
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        }
        | ProductionRankedOperationV1::ValueAccess {
            kind,
            view,
            indices,
            ..
        } => (*kind, *view, indices),
        _ => {
            return Err(Error::Invalid(
                "full address ranked access grammar unsupported",
            ));
        }
    };
    let [index] = indices.as_slice() else {
        return Err(Error::Invalid("full address ranked access rank differs"));
    };
    let write = match kind {
        dialect_kernel::AccessKindAttr::Read => false,
        dialect_kernel::AccessKindAttr::Write => true,
        _ => {
            return Err(Error::Invalid(
                "full address ranked access kind unsupported",
            ));
        }
    };
    Ok((write, view, *index))
}

fn source_output_full_address_anchor_v1<'a>(
    analysis: &'a ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
    work: &SourceOutputFullAddressWorkspaceV1,
    mut value: ProductionRankedValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputAddressLeafV1<'a>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        budget.charge_work(2).map_err(Error::Resource)?;
        if let Some(leaf) = analysis.control.address_leaf_v1(
            analysis.view,
            analysis.candidate,
            analysis.control_ordinal,
            value,
            budget,
        )? {
            return Ok(leaf);
        }
        match source_output_address_ranked_operation_v1(
            &work.memory.ranked,
            analysis.candidate.lowering,
            value,
            budget,
        )? {
            ProductionRankedOperationV1::IndexUnsignedCast {
                source,
                bit_width: 64,
                ..
            } => value = *source,
            _ => {
                return Err(Error::Invalid(
                    "full address D leaf lacks a checked source anchor",
                ));
            }
        }
    }
    Err(Error::Invalid("full address D index depth exceeded"))
}

#[allow(clippy::too_many_arguments)]
fn source_output_full_address_check_anchor_v1(
    analysis: &ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
    work: &SourceOutputFullAddressWorkspaceV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    function: &SemanticFunctionDeclV1,
    leaf: &SourceOutputAddressLeafV1<'_>,
    local: SemanticLocalIdV1,
    component: ProductionProjectionArgumentComponentV1,
    used: SsaValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputFullAddressAnchorV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use SourceOutputAddressLeafV1 as Leaf;
    budget.charge_work(6).map_err(Error::Resource)?;
    let row = match leaf {
        Leaf::Formal(formal) => formal.row,
        Leaf::Literal(literal) => literal.row,
        Leaf::Invocation(row, _) => row,
    };
    if row.source_local != local
        || row.component != component
        || row.scalar != source_output_address_u64_v1()
    {
        return Err(Error::Invalid(
            "full address source anchor local component or type differs",
        ));
    }
    let (output, bits, ssa, invocation) = match row.origin {
        SourceOutputProjectionLeafOriginV1::Formal(formal) => {
            source_output_full_address_entry_v1(work, captured, function, local, used, budget)?;
            (Some(formal.output), None, used, false)
        }
        SourceOutputProjectionLeafOriginV1::Literal(literal) => {
            if component != ProductionProjectionArgumentComponentV1::Scalar || literal.ssa != used {
                return Err(Error::Invalid(
                    "full address literal use resolves to another definition",
                ));
            }
            let key = (literal.block.index(), literal.statement, 0, 0, 2, 0);
            let event = source_output_full_address_event_v1(work, captured, key, budget)?;
            budget.charge_work(3).map_err(Error::Resource)?;
            if !matches!((event.event(), event.resolved()),
                (fe2o3_mir_model::SsaEventV1::Define(original), Some(SsaResolvedEventV1::Define { variable, value }))
                if original == variable && variable.get() == local.index() && value == used)
            {
                return Err(Error::Invalid(
                    "full address literal captured definition differs",
                ));
            }
            (None, Some(literal.bits), used, false)
        }
        SourceOutputProjectionLeafOriginV1::Invocation(invocation) => {
            budget.charge_work(6).map_err(Error::Resource)?;
            let index = analysis
                .control
                .invocation_sources
                .get(invocation.source_index)
                .ok_or(Error::Invalid(
                    "full address invocation source index absent",
                ))?;
            let actual = source_output_invocation_source_anchor_v1(
                analysis.view,
                index,
                local,
                used,
                budget,
            )?;
            if actual != invocation.source
                || component != ProductionProjectionArgumentComponentV1::Scalar
            {
                return Err(Error::Invalid("full address own invocation use differs"));
            }
            (Some(invocation.output), None, actual.raw, true)
        }
    };
    Ok(SourceOutputFullAddressAnchorV1 {
        local,
        component,
        scalar: row.scalar,
        original: row.original(),
        output,
        ssa,
        bits,
        invocation,
    })
}

fn source_output_full_address_leaf_v1(
    work: &mut SourceOutputFullAddressWorkspaceV1,
    lowering: &ProductionRankedKernelLoweringInputV1,
    mut value: ProductionRankedValueV1,
    anchor: SourceOutputFullAddressAnchorV1,
    expected: &NormalizedScalarNodeV1<usize>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        budget.charge_work(3).map_err(Error::Resource)?;
        match value {
            ProductionRankedValueV1::Argument(argument)
                if !anchor.invocation
                    && (argument as usize) < lowering.kernel().argument_count() => {}
            ProductionRankedValueV1::Local(_) => match source_output_address_ranked_operation_v1(
                &work.full.ranked,
                lowering,
                value,
                budget,
            )? {
                ProductionRankedOperationV1::IndexUnknown { .. } if !anchor.invocation => {}
                ProductionRankedOperationV1::InvocationIndex {
                    dimension: 0,
                    launch_extent: 0,
                    ..
                } if anchor.invocation => {}
                ProductionRankedOperationV1::IndexUnsignedCast {
                    source,
                    bit_width: 64,
                    ..
                } => {
                    value = *source;
                    continue;
                }
                ProductionRankedOperationV1::IndexConstant { value, .. } => {
                    budget.charge_work(3).map_err(Error::Resource)?;
                    let actual = NormalizedScalarNodeV1::Constant {
                        scalar: source_output_address_u64_v1(),
                        bits: *value,
                    };
                    return if &actual == expected {
                        Ok(())
                    } else {
                        Err(Error::Invalid("full address ranked constant differs"))
                    };
                }
                _ => {
                    return Err(Error::Invalid(
                        "full address ranked leaf grammar unsupported",
                    ));
                }
            },
            _ => {
                return Err(Error::Invalid(
                    "full address ranked leaf is not a supported definition",
                ));
            }
        }
        let index = assert_origin_find_v1(&work.claims, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.claim.ranked_value.cmp(&value))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("full address source claim absent"))?;
        let row = &mut work.claims[index];
        // The checked source occurrence fixes this unknown's interpretation;
        // it is not inferred from equality with a D-local numeric identifier.
        return source_output_full_address_bind_v1(row, anchor, budget);
    }
    Err(Error::Invalid("full address ranked index depth exceeded"))
}

#[allow(clippy::too_many_arguments)]
fn source_output_full_address_access_v1(
    relation: &ProductionPhysicalAddressRelationV1<'_, '_, '_>,
    root: &ProductionRankedSemanticProjectionRootV1,
    original_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    function: &SemanticFunctionDeclV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    physical: &SourceOutputPhysicalAddressRowV1,
    work: &mut SourceOutputFullAddressWorkspaceV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
    };
    let analysis = relation.analysis;
    budget.charge_work(4).map_err(Error::Resource)?;
    let access = analysis
        .accesses
        .get(physical.access)
        .ok_or(Error::Invalid("full address D access absent"))?;
    if access.operation != physical.operation {
        return Err(Error::Invalid("full address own O operation differs"));
    }
    let key = source_output_full_address_site_v1(access.source);
    let site = assert_origin_find_v1(&work.sites, budget, |row, budget| {
        budget.charge_work(3)?;
        Ok(row.0.cmp(&key))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("full address exact source access absent"))?;
    budget.charge_work(2).map_err(Error::Resource)?;
    let full_source = root.access_sources[work.sites[site].1];
    let ProductionSourceOutputGlobalAccessV1::Retained {
        original,
        operation,
        parameter,
        ..
    } = analysis.view.global_access(
        analysis.candidate.selected_root,
        analysis.candidate.selected_function,
        key.0,
        key.1,
        key.2,
        budget,
    )?
    else {
        return Err(Error::Invalid(
            "full address source N/O access is not retained",
        ));
    };
    budget.charge_work(4).map_err(Error::Resource)?;
    if operation != physical.operation
        || original.block.function != original_function
        || parameter != physical.allocation
    {
        return Err(Error::Invalid(
            "full address source N/O allocation or operation differs",
        ));
    }
    // R1 checked this full source-site against exact original N. The D row and
    // source-owned query independently identify this very retained O use.
    let (write, full_view, full_index) =
        source_output_full_address_memory_v1(&root.lowering, full_source, budget)?;
    let (memory_write, memory_view, memory_index) =
        source_output_full_address_memory_v1(analysis.candidate.lowering, access.source, budget)?;
    budget.charge_work(1).map_err(Error::Resource)?;
    if write != memory_write {
        return Err(Error::Invalid("full address access kind differs"));
    }
    let full = source_output_address_ranked_operation_v1(
        &work.full.ranked,
        &root.lowering,
        full_view,
        budget,
    )?;
    let memory = source_output_address_ranked_operation_v1(
        &work.memory.ranked,
        analysis.candidate.lowering,
        memory_view,
        budget,
    )?;
    let (
        ProductionRankedOperationV1::ViewInSpace {
            element_width: fw,
            writable: fp,
            shape: fs,
            dynamic_extents: fe,
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            allocation_origin: fo,
            noalias_class: fc,
            ..
        },
        ProductionRankedOperationV1::ViewInSpace {
            element_width: mw,
            writable: mp,
            shape: ms,
            dynamic_extents: me,
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
            allocation_origin: mo,
            noalias_class: mc,
            ..
        },
    ) = (full, memory)
    else {
        return Err(Error::Invalid("full address Global views absent"));
    };
    let full_extent = source_output_address_shape_v1(fs, fe, *fw, physical.element_bytes, budget)?;
    let memory_extent =
        source_output_address_shape_v1(ms, me, *mw, physical.element_bytes, budget)?;
    budget.charge_work(5).map_err(Error::Resource)?;
    if fp != mp || fo != mo || fc != mc || memory_extent != physical.extent {
        return Err(Error::Invalid(
            "full address view permission allocation or extent differs",
        ));
    }
    let (place, operand, index_local) =
        source_output_full_address_place_v1(function, access.source, write, budget)?;
    let statement = key
        .1
        .ok_or(Error::Invalid("full address source statement absent"))?;
    let site = Site::Statement {
        block: SsaBlockIdV1::new(key.0),
        statement,
    };
    budget.charge_work(2).map_err(Error::Resource)?;
    let base_key = source_output_full_address_event_key_v1(site, operand, Role::BaseUse)
        .ok_or(Error::Invalid("full address source base role unsupported"))?;
    let index_key =
        source_output_full_address_event_key_v1(site, operand, Role::ProjectionIndexUse(1))
            .ok_or(Error::Invalid("full address source index role unsupported"))?;
    let base = source_output_full_address_use_v1(work, captured, base_key, place.local(), budget)?;
    let index = source_output_full_address_use_v1(work, captured, index_key, index_local, budget)?;
    let extent_leaf = source_output_full_address_anchor_v1(analysis, work, memory_extent, budget)?;
    let extent_anchor = source_output_full_address_check_anchor_v1(
        analysis,
        work,
        captured,
        function,
        &extent_leaf,
        place.local(),
        ProductionProjectionArgumentComponentV1::SliceLength,
        base,
        budget,
    )?;
    let expected_extent = source_output_address_leaf_node_v1(extent_leaf, |units| {
        budget.charge_work(units).map_err(Error::Resource)
    })?;
    source_output_full_address_leaf_v1(
        work,
        &root.lowering,
        full_extent,
        extent_anchor,
        &expected_extent,
        budget,
    )?;
    let index_leaf = source_output_full_address_anchor_v1(analysis, work, memory_index, budget)?;
    let index_anchor = source_output_full_address_check_anchor_v1(
        analysis,
        work,
        captured,
        function,
        &index_leaf,
        index_local,
        ProductionProjectionArgumentComponentV1::Scalar,
        index,
        budget,
    )?;
    let expected_index = source_output_address_leaf_node_v1(index_leaf, |units| {
        budget.charge_work(units).map_err(Error::Resource)
    })?;
    source_output_full_address_leaf_v1(
        work,
        &root.lowering,
        full_index,
        index_anchor,
        &expected_index,
        budget,
    )
}

impl ProductionPhysicalAddressRelationV1<'_, '_, '_> {
    /// Compares a full R1-ranked root's addresses with this exact source/N/O
    /// relation, using pre-captured source SSA occurrences and inert full claims.
    /// This returns no functional, bounds, race, reference or final authority.
    /// The caller keeps the separate SSA capture receipt reserved on the same
    /// original ledger. Absent capture and non-Global-Slice access shapes refuse.
    /// Only a direct root body is supported; wrapper-selected bodies and
    /// helper/phi-derived index leaves are not authorized by this comparison.
    pub fn check_borrowed_ranked_addresses_v1(
        &self,
        original: &ProductionBorrowedRankedCorrespondenceV1<'_>,
        root_ordinal: usize,
        full_claims: &ProductionProjectionControlCandidateV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        self.analysis.require_live_v1(budget)?;
        budget.charge_work(10).map_err(Error::Resource)?;
        if budget.storage() < self.floor {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        if !std::ptr::eq(original.materialized(), self.analysis.view.source()) {
            return Err(Error::Invalid("full address original owner differs"));
        }
        let root = original
            .roots()
            .get(root_ordinal)
            .ok_or(Error::Invalid("full address root absent"))?;
        if root.selected_root != self.analysis.candidate.selected_root {
            return Err(Error::Invalid("full address selected root differs"));
        }
        if root.selected_root != self.analysis.candidate.selected_function {
            return Err(Error::Invalid(
                "full address join requires a direct root body",
            ));
        }
        if self.private_accesses != 0 || self.rows.len() != self.analysis.access_count() {
            return Err(Error::Invalid(
                "full address join currently requires only Global Slice accesses",
            ));
        }
        let ssa = original.materialized().semantic_ssa();
        let receipt = ssa
            .occurrence_storage()
            .ok_or(Error::Invalid("full address source SSA capture absent"))?;
        let captured = ssa
            .occurrences_v1()
            .ok_or(Error::Invalid("full address source SSA capture absent"))?;
        let captured = captured
            .function(self.analysis.candidate.selected_function)
            .ok_or(Error::Invalid("full address captured source body absent"))?;
        budget.charge_work(5).map_err(Error::Resource)?;
        if !std::ptr::eq(captured.owner(), ssa) || budget.storage() < receipt.retained_storage() {
            return Err(Error::Invalid(
                "full address captured source owner or reservation differs",
            ));
        }
        let function = ssa
            .source_semantic()
            .functions()
            .get(self.analysis.candidate.selected_function.index() as usize)
            .ok_or(Error::Invalid("full address source body absent"))?;
        let original_function = source_output_ordinary_function_alias_v1(
            original.materialized(),
            root.selected_root,
            self.analysis.candidate.selected_function,
            budget,
        )?;
        source_output_global_scratch_scope_v1(budget, |budget| {
            let mut work = source_output_full_address_workspace_v1(budget)?;
            source_output_full_address_events_v1(&mut work, &captured, budget)?;
            source_output_address_ranked_index_v1(&mut work.full, &root.lowering, budget)?;
            source_output_address_ranked_index_v1(
                &mut work.memory,
                self.analysis.candidate.lowering,
                budget,
            )?;
            for (ordinal, source) in root.access_sources.iter().enumerate() {
                budget.charge_work(4).map_err(Error::Resource)?;
                assert_origin_push_v1(
                    &mut work.sites,
                    (source_output_full_address_site_v1(*source), ordinal),
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
            source_output_ranked_sort_unique_v1(&mut work.sites, budget, |a, b| a.0.cmp(&b.0))?;
            for claim in &full_claims.arguments {
                budget.charge_work(1).map_err(Error::Resource)?;
                assert_origin_push_v1(
                    &mut work.claims,
                    SourceOutputFullAddressClaimV1 {
                        claim: *claim,
                        anchor: None,
                    },
                    budget,
                )
                .map_err(Error::SourceOrigin)?;
            }
            source_output_ranked_sort_unique_v1(&mut work.claims, budget, |a, b| {
                a.claim.ranked_value.cmp(&b.claim.ranked_value)
            })?;
            budget.charge_work(1).map_err(Error::Resource)?;
            if work.sites.len() != self.analysis.access_count() {
                return Err(Error::Invalid(
                    "full address source access multiplicity differs",
                ));
            }
            for physical in self.rows {
                source_output_full_address_access_v1(
                    self,
                    root,
                    original_function,
                    function,
                    &captured,
                    physical,
                    &mut work,
                    budget,
                )?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod functional_address_component_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

    fn anchor() -> SourceOutputFullAddressAnchorV1 {
        let definition = SourceOutputAddressDefV1::FunctionArgument {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
            argument: 2,
        };
        SourceOutputFullAddressAnchorV1 {
            local: SemanticLocalIdV1::from_index(8),
            component: ProductionProjectionArgumentComponentV1::Scalar,
            scalar: source_output_address_u64_v1(),
            original: definition,
            output: Some(definition),
            ssa: SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(0),
                variable: fe2o3_mir_model::SsaVariableIdV1::new(8),
            },
            bits: None,
            invocation: false,
        }
    }

    #[test]
    fn functional_address_mapping_work_is_prepaid_and_consistent() {
        for limit in [31, 30] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 128);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(13).unwrap();
            let mut row = SourceOutputFullAddressClaimV1 {
                claim: ProductionProjectionArgumentCandidateV1 {
                    ranked_value: ProductionRankedValueV1::Argument(91),
                    source_local: anchor().local,
                    component: anchor().component,
                },
                anchor: None,
            };
            source_output_full_address_bind_v1(&mut row, anchor(), &mut budget).unwrap();
            assert!(row.anchor == Some(anchor()));
            let second = source_output_full_address_bind_v1(&mut row, anchor(), &mut budget);
            assert_eq!(second.is_ok(), limit == 31);
            assert!(row.anchor == Some(anchor()));
            assert_eq!(budget.storage(), 13);
            assert_eq!(budget.work(), if limit == 31 { 31 } else { 19 });
            assert_eq!(
                work.failed_work(),
                if limit == 31 { None } else { Some(31) }
            );
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(24);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 128);
        let mut row = SourceOutputFullAddressClaimV1 {
            claim: ProductionProjectionArgumentCandidateV1 {
                ranked_value: ProductionRankedValueV1::Argument(91),
                source_local: anchor().local,
                component: anchor().component,
            },
            anchor: None,
        };
        source_output_full_address_bind_v1(&mut row, anchor(), &mut budget).unwrap();
        let mut different = anchor();
        different.ssa = SsaValueV1::BlockArgument {
            block: SsaBlockIdV1::new(1),
            variable: fe2o3_mir_model::SsaVariableIdV1::new(8),
        };
        assert!(matches!(
            source_output_full_address_bind_v1(&mut row, different, &mut budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "full address leaf source mapping differs"
            ))
        ));
        assert!(row.anchor == Some(anchor()));
        assert_eq!(budget.work(), 24);
    }

    #[test]
    fn functional_address_workspace_header_has_exact_floor_and_one_short_denial() {
        let header = std::mem::size_of::<SourceOutputFullAddressWorkspaceV1>();
        for storage in [13 + header, 12 + header] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(9);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(13).unwrap();
            let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                let workspace = source_output_full_address_workspace_v1(budget)?;
                assert!(workspace.events.is_empty() && workspace.claims.is_empty());
                assert_eq!(budget.storage(), 13 + header);
                Ok(())
            });
            assert_eq!(result.is_ok(), storage == 13 + header);
            assert_eq!(budget.storage(), 13);
            assert_eq!(budget.work(), 9);
        }
    }

    #[test]
    fn functional_address_event_key_keeps_operand_projection_and_site_distinct() {
        use fe2o3_pliron::{
            ProductionSemanticSsaEventRoleV1 as Role,
            ProductionSemanticSsaOccurrenceSiteV1 as Site,
            ProductionSemanticSsaOperandRoleV1 as Operand,
        };
        let site = Site::Statement {
            block: SsaBlockIdV1::new(2),
            statement: 3,
        };
        let exact = source_output_full_address_event_key_v1(
            site,
            Operand::Destination,
            Role::ProjectionIndexUse(1),
        );
        assert_eq!(exact, Some((2, 3, 0, 0, 1, 1)));
        for changed in [
            source_output_full_address_event_key_v1(
                site,
                Operand::RvalueOperand(0),
                Role::ProjectionIndexUse(1),
            ),
            source_output_full_address_event_key_v1(
                site,
                Operand::Destination,
                Role::ProjectionIndexUse(0),
            ),
            source_output_full_address_event_key_v1(site, Operand::Destination, Role::BaseUse),
            source_output_full_address_event_key_v1(
                Site::Statement {
                    block: SsaBlockIdV1::new(2),
                    statement: 4,
                },
                Operand::Destination,
                Role::ProjectionIndexUse(1),
            ),
        ] {
            assert_ne!(exact, changed);
        }
    }
}
