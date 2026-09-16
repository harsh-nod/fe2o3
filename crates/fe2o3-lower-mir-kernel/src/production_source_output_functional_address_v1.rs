type SourceOutputFullAddressSiteV1 = (u32, Option<u32>, u32);
type SourceOutputFullAddressEventKeyV1 = (u32, u32, u32, u32, u32, u32);

#[cfg(test)]
mod full_identity_selected_context_components_v1 {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn full_identity_selected_context_prepays_and_rejects_every_seal_axis() {
        // Inert data, not full ranked/source/control capabilities.
        let original = identity_address_transfer_components_v1::descriptor()
            .selected_offset
            .unwrap();
        for case in 0..7 {
            let mut actual = original;
            let mut physical = original.definition;
            let foreign = SourceOutputAddressDefV1::FunctionArgument {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(99),
                argument: 0,
            };
            match case {
                0 => actual.definition = foreign,
                1 => actual.value = ValueId(99),
                2 => actual.condition_definition = foreign,
                3 => actual.condition = ValueId(99),
                4 => actual.index_definition = foreign,
                5 => actual.index = ValueId(99),
                6 => physical = foreign,
                _ => unreachable!(),
            }
            let mut work = Work::new(10);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
            assert!(matches!(
                source_output_full_identity_selected_v1(original, actual, physical, &mut budget),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "full identity selected offset context differs"
                ))
            ));
            assert_eq!(budget.work(), 10);
        }
        for limit in [9, 10] {
            let mut work = Work::new(limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
            let result = source_output_full_identity_selected_v1(
                original,
                original,
                original.definition,
                &mut budget,
            );
            if limit == 10 {
                result.unwrap();
            } else {
                assert!(
                    matches!(result, Err(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Work(error))) if error.actual() == 10 && error.limit() == 9)
                );
                assert_eq!(budget.work(), 0);
            }
            assert_eq!(budget.storage(), 0);
        }
    }
}

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

#[derive(Clone, Copy)]
struct SourceOutputFullIdentityAnchorsV1 {
    index: SourceOutputFullAddressAnchorV1,
    extent: SourceOutputFullAddressAnchorV1,
    index_symbol: u32,
    extent_symbol: u32,
    selected_offset: SourceOutputIdentitySelectedOffsetV1,
}

fn source_output_full_identity_selected_v1(
    prepared: SourceOutputIdentitySelectedOffsetV1,
    current: SourceOutputIdentitySelectedOffsetV1,
    physical: SourceOutputAddressDefV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    // Six retained fields plus the actual P offset and fixed association work.
    budget.charge_work(10).map_err(Error::Resource)?;
    if prepared != current || physical != prepared.definition {
        return Err(Error::Invalid(
            "full identity selected offset context differs",
        ));
    }
    Ok(())
}

#[derive(Default)]
struct SourceOutputFullAddressWorkspaceV1 {
    full: SourceOutputAddressWorkspaceV1,
    memory: SourceOutputAddressWorkspaceV1,
    sites: Vec<(SourceOutputFullAddressSiteV1, usize)>,
    events: Vec<(SourceOutputFullAddressEventKeyV1, usize)>,
    entries: Vec<(u32, usize)>,
    claims: Vec<SourceOutputFullAddressClaimV1>,
    identity: Option<SourceOutputFullIdentityAnchorsV1>,
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

fn source_output_full_address_define_v1(
    work: &SourceOutputFullAddressWorkspaceV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    key: SourceOutputFullAddressEventKeyV1,
    local: SemanticLocalIdV1,
    expected: SsaValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let event = source_output_full_address_event_v1(work, captured, key, budget)?;
    budget.charge_work(3).map_err(Error::Resource)?;
    if !matches!(expected, SsaValueV1::Definition(_))
        || !matches!((event.event(), event.resolved()),
            (fe2o3_mir_model::SsaEventV1::Define(original), Some(SsaResolvedEventV1::Define { variable, value }))
            if original == variable && variable.get() == local.index() && value == expected)
    {
        return Err(Error::Invalid("full identity captured definition differs"));
    }
    Ok(())
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

fn source_output_full_identity_anchors_v1(
    analysis: &ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
    original_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    work: &SourceOutputFullAddressWorkspaceV1,
    captured: &fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    function: &SemanticFunctionDeclV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Option<SourceOutputFullIdentityAnchorsV1>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_pliron::{
        ProductionSemanticSsaEventRoleV1 as Role, ProductionSemanticSsaOccurrenceSiteV1 as Site,
        ProductionSemanticSsaOperandRoleV1 as Operand,
    };
    let Some(identity) = &analysis.control.candidates[analysis.control_ordinal].identity_address
    else {
        return Ok(None);
    };
    // Existing32 plus twelve for the six-field selected-offset copy retained
    // in this query's already size_of-charged workspace header.
    budget.charge_work(44).map_err(Error::Resource)?;
    analysis.control.require_candidate_at_v1(
        analysis.view,
        analysis.candidate,
        analysis.control_ordinal,
        budget,
    )?;
    let invocation = identity.invocation;
    let index = analysis
        .control
        .invocation_sources
        .get(invocation.source_index)
        .ok_or(Error::Invalid("full identity source index absent"))?;
    let original_owner = match invocation.original {
        SourceOutputAddressDefV1::FunctionArgument { function, .. } => function,
        SourceOutputAddressDefV1::BlockArgument { block, .. } => block.function,
        SourceOutputAddressDefV1::Result { operation, .. } => operation.block.function,
    };
    if !invocation.identity_getter
        || index.source != std::ptr::from_ref(analysis.view.source).cast()
        || index.function != analysis.candidate.selected_function
        || index.canonical != original_function
        || original_owner != original_function
        || !std::ptr::eq(captured.owner(), analysis.view.source.semantic_ssa())
        || analysis
            .view
            .source
            .semantic_ssa()
            .source_semantic()
            .functions()
            .get(index.function.index() as usize)
            .is_none_or(|actual| !std::ptr::eq(actual, function))
    {
        return Err(Error::Invalid(
            "full identity original source association differs",
        ));
    }
    let facts = identity.source_facts;
    // Identity raw is the Option CallReturn definition, not an integer index.
    let getter_definition =
        source_output_identity_direct_definition_v1(index, invocation.source.raw, budget)?;
    let (getter, _) = source_output_invocation_call_v1(
        function,
        index,
        captured,
        facts.option,
        invocation.source.raw,
        getter_definition,
        budget,
    )?;
    let producer_definition =
        source_output_identity_direct_definition_v1(index, invocation.source.witness, budget)?;
    let (producer, _) = source_output_invocation_call_v1(
        function,
        index,
        captured,
        identity.witness,
        invocation.source.witness,
        producer_definition,
        budget,
    )?;
    if getter != invocation.source.get || producer != invocation.source.producer {
        return Err(Error::Invalid("full identity exact CallReturn differs"));
    }
    // Terminator operands require Control's seven-field index; the full
    // statement index below has six fields and different operand tags.
    let receiver = source_output_invocation_use_v1(
        index,
        captured,
        (1, getter.source().get(), 0, 3, 0, 0, 0),
        facts.receiver,
        budget,
    )?;
    let witness = source_output_invocation_use_v1(
        index,
        captured,
        (1, getter.source().get(), 0, 3, 1, 0, 0),
        identity.witness,
        budget,
    )?;
    if receiver != facts.receiver_value || witness != invocation.source.witness {
        return Err(Error::Invalid("full identity getter argument use differs"));
    }
    let receiver_site = Site::Statement {
        block: SsaBlockIdV1::new(facts.receiver_site.0),
        statement: facts.receiver_site.1,
    };
    let payload_site = Site::Statement {
        block: SsaBlockIdV1::new(facts.payload_site.0),
        statement: facts.payload_site.1,
    };
    let receiver_use =
        source_output_full_address_event_key_v1(receiver_site, Operand::RvaluePlace, Role::BaseUse)
            .ok_or(Error::Invalid("full identity receiver use key absent"))?;
    let receiver_define = source_output_full_address_event_key_v1(
        receiver_site,
        Operand::Destination,
        Role::DestinationDefine,
    )
    .ok_or(Error::Invalid(
        "full identity receiver definition key absent",
    ))?;
    let payload_use = source_output_full_address_event_key_v1(
        payload_site,
        Operand::RvalueOperand(0),
        Role::BaseUse,
    )
    .ok_or(Error::Invalid("full identity payload use key absent"))?;
    let payload_define = source_output_full_address_event_key_v1(
        payload_site,
        Operand::Destination,
        Role::DestinationDefine,
    )
    .ok_or(Error::Invalid(
        "full identity payload definition key absent",
    ))?;
    if source_output_full_address_use_v1(work, captured, receiver_use, identity.slice, budget)?
        != facts.slice_entry
    {
        return Err(Error::Invalid(
            "full identity receiver source entry differs",
        ));
    }
    source_output_full_address_define_v1(
        work,
        captured,
        receiver_define,
        facts.receiver,
        facts.receiver_value,
        budget,
    )?;
    if source_output_full_address_use_v1(work, captured, payload_use, facts.option, budget)?
        != invocation.source.raw
    {
        return Err(Error::Invalid("full identity payload Option use differs"));
    }
    source_output_full_address_define_v1(
        work,
        captured,
        payload_define,
        facts.payload,
        facts.payload_value,
        budget,
    )?;
    let index_leaf =
        source_output_full_address_anchor_v1(analysis, work, identity.ranked_index, budget)?;
    let SourceOutputAddressLeafV1::Invocation(row, checked) = &index_leaf else {
        return Err(Error::Invalid("full identity index leaf is not Invocation"));
    };
    if **checked != invocation
        || row.source_local != identity.witness
        || row.component != ProductionProjectionArgumentComponentV1::Scalar
        || row.scalar != source_output_address_u64_v1()
    {
        return Err(Error::Invalid("full identity sealed index differs"));
    }
    let index_anchor = SourceOutputFullAddressAnchorV1 {
        local: identity.witness,
        component: ProductionProjectionArgumentComponentV1::Scalar,
        scalar: row.scalar,
        original: invocation.original,
        output: Some(invocation.output),
        ssa: invocation.source.witness,
        bits: None,
        invocation: true,
    };
    let extent_leaf =
        source_output_full_address_anchor_v1(analysis, work, identity.ranked_extent, budget)?;
    let extent_anchor = source_output_full_address_check_anchor_v1(
        analysis,
        work,
        captured,
        function,
        &extent_leaf,
        identity.slice,
        ProductionProjectionArgumentComponentV1::SliceLength,
        facts.slice_entry,
        budget,
    )?;
    let index_node = source_output_address_leaf_node_v1(index_leaf, |units| {
        budget.charge_work(units).map_err(Error::Resource)
    })?;
    let extent_node = source_output_address_leaf_node_v1(extent_leaf, |units| {
        budget.charge_work(units).map_err(Error::Resource)
    })?;
    let (
        NormalizedScalarNodeV1::Symbol {
            symbol: index_symbol,
            ..
        },
        NormalizedScalarNodeV1::Symbol {
            symbol: extent_symbol,
            ..
        },
    ) = (index_node, extent_node)
    else {
        return Err(Error::Invalid("full identity anchors are not Symbols"));
    };
    Ok(Some(SourceOutputFullIdentityAnchorsV1 {
        index: index_anchor,
        extent: extent_anchor,
        index_symbol,
        extent_symbol,
        selected_offset: identity.selected_offset,
    }))
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
    if let Some(prepared) = work.identity {
        use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Operand;
        budget.charge_work(24).map_err(Error::Resource)?;
        let (identity, own) = analysis
            .control
            .identity_address_use_v1(
                analysis.view,
                analysis.candidate,
                analysis.control_ordinal,
                access.source,
                physical.operation,
                budget,
            )?
            .ok_or(Error::Invalid("full identity own Store seal absent"))?;
        if !write
            || own.original != original
            || own.output != operation
            || own.pointer.definition != physical.pointer
            || own.gep != physical.gep
            || own.allocation != physical.allocation
            || memory_index != identity.ranked_index
            || memory_extent != identity.ranked_extent
        {
            return Err(Error::Invalid("full identity own physical Store differs"));
        }
        source_output_full_identity_selected_v1(
            prepared.selected_offset,
            identity.selected_offset,
            physical.offset,
            budget,
        )?;
        let statement = key.1.ok_or(Error::Invalid(
            "full identity source Store statement absent",
        ))?;
        let source = function
            .blocks()
            .get(key.0 as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or(Error::Invalid("full identity source Store absent"))?;
        let (place, operand) = match source.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                (assignment.destination(), Operand::Destination)
            }
            SemanticStatementKindV1::Store(store) => {
                (store.destination(), Operand::StoreDestination)
            }
            _ => return Err(Error::Invalid("full identity source Store grammar differs")),
        };
        if place.local() != identity.source_facts.payload
            || !matches!(place.projections(), [projection] if projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Err(Error::Invalid(
                "full identity own payload dereference differs",
            ));
        }
        let site = Site::Statement {
            block: SsaBlockIdV1::new(key.0),
            statement,
        };
        let key = source_output_full_address_event_key_v1(site, operand, Role::BaseUse)
            .ok_or(Error::Invalid("full identity own Store source key absent"))?;
        if source_output_full_address_use_v1(
            work,
            captured,
            key,
            identity.source_facts.payload,
            budget,
        )? != identity.source_facts.payload_value
        {
            return Err(Error::Invalid(
                "full identity own Store payload use differs",
            ));
        }
        let expected_index = NormalizedScalarNodeV1::Symbol {
            symbol: prepared.index_symbol,
            scalar: source_output_address_u64_v1(),
        };
        let expected_extent = NormalizedScalarNodeV1::Symbol {
            symbol: prepared.extent_symbol,
            scalar: source_output_address_u64_v1(),
        };
        source_output_full_address_leaf_v1(
            work,
            &root.lowering,
            full_index,
            prepared.index,
            &expected_index,
            budget,
        )?;
        return source_output_full_address_leaf_v1(
            work,
            &root.lowering,
            full_extent,
            prepared.extent,
            &expected_extent,
            budget,
        );
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
    /// original ledger. Only direct Global Slice accesses and the same privately
    /// checked identity-getter Store chain are supported; absent capture refuses.
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
            work.identity = source_output_full_identity_anchors_v1(
                self.analysis,
                original_function,
                &work,
                &captured,
                function,
                budget,
            )?;
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

    #[test]
    fn functional_identity_statement_keys_do_not_reinterpret_getter_terminator_keys() {
        use fe2o3_pliron::{
            ProductionSemanticSsaEventRoleV1 as Role,
            ProductionSemanticSsaOccurrenceSiteV1 as Site,
            ProductionSemanticSsaOperandRoleV1 as Operand,
        };
        let site = Site::Statement {
            block: SsaBlockIdV1::new(3),
            statement: 7,
        };
        for (operand, role, expected) in [
            (Operand::RvaluePlace, Role::BaseUse, (3, 7, 2, 0, 0, 0)),
            (
                Operand::Destination,
                Role::DestinationDefine,
                (3, 7, 0, 0, 2, 0),
            ),
            (Operand::RvalueOperand(0), Role::BaseUse, (3, 7, 3, 0, 0, 0)),
            (Operand::Destination, Role::BaseUse, (3, 7, 0, 0, 0, 0)),
            (Operand::StoreDestination, Role::BaseUse, (3, 7, 1, 0, 0, 0)),
        ] {
            assert_eq!(
                source_output_full_address_event_key_v1(site, operand, role),
                Some(expected)
            );
            assert_ne!(
                source_output_full_address_event_key_v1(
                    Site::Statement {
                        block: SsaBlockIdV1::new(3),
                        statement: 8
                    },
                    operand,
                    role
                ),
                Some(expected)
            );
            assert_ne!(
                source_output_full_address_event_key_v1(site, operand, Role::ProjectionIndexUse(0)),
                Some(expected)
            );
        }
        for argument in [0, 1] {
            assert_eq!(
                source_output_full_address_event_key_v1(
                    Site::Terminator {
                        block: SsaBlockIdV1::new(3)
                    },
                    Operand::CallArgument(argument),
                    Role::BaseUse
                ),
                None
            );
            assert_eq!(
                source_output_full_address_event_key_v1(
                    site,
                    Operand::CallArgument(argument),
                    Role::BaseUse
                ),
                None
            );
        }
        let exact = ProductionRankedAccessSourceV1::new(3, Some(7), 0, 91, 92);
        assert_eq!(source_output_full_address_site_v1(exact), (3, Some(7), 0));
        for changed in [
            ProductionRankedAccessSourceV1::new(4, Some(7), 0, 91, 92),
            ProductionRankedAccessSourceV1::new(3, Some(8), 0, 91, 92),
            ProductionRankedAccessSourceV1::new(3, None, 0, 91, 92),
            ProductionRankedAccessSourceV1::new(3, Some(7), 1, 91, 92),
        ] {
            assert_ne!(
                source_output_full_address_site_v1(changed),
                source_output_full_address_site_v1(exact)
            );
        }
        // A ranked coordinate is not a source site or a captured event key.
        assert_eq!(
            source_output_full_address_site_v1(ProductionRankedAccessSourceV1::new(
                3,
                Some(7),
                0,
                2,
                4
            )),
            source_output_full_address_site_v1(exact)
        );
    }

    #[test]
    fn functional_identity_global_binding_keeps_witness_ssa_and_both_definition_owners() {
        use fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1 as Function;
        let mut witness = anchor();
        witness.invocation = true;
        witness.ssa = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(5));
        let raw_option = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(4));
        // These private numeric records exercise consistent binding only. They
        // cannot construct a public source, Control, P or R1 capability.
        for case in 0..5 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(36);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 13);
            budget.reserve_storage(13).unwrap();
            let mut claim = SourceOutputFullAddressClaimV1 {
                claim: ProductionProjectionArgumentCandidateV1 {
                    ranked_value: ProductionRankedValueV1::Argument(91),
                    source_local: witness.local,
                    component: witness.component,
                },
                anchor: None,
            };
            source_output_full_address_bind_v1(&mut claim, witness, &mut budget).unwrap();
            let mut changed = witness;
            match case {
                0 => changed.ssa = raw_option,
                1 => {
                    changed.original = SourceOutputAddressDefV1::FunctionArgument {
                        function: Function(1),
                        argument: 2,
                    }
                }
                2 => {
                    changed.output = Some(SourceOutputAddressDefV1::FunctionArgument {
                        function: Function(1),
                        argument: 2,
                    })
                }
                3 => changed.invocation = false,
                4 => changed.bits = Some(0),
                _ => unreachable!(),
            }
            assert!(matches!(
                source_output_full_address_bind_v1(&mut claim, changed, &mut budget),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "full address leaf source mapping differs"
                ))
            ));
            assert!(claim.anchor == Some(witness));
            source_output_full_address_bind_v1(&mut claim, witness, &mut budget).unwrap();
            assert_eq!(budget.work(), 36);
            assert_eq!(budget.storage(), 13);
        }
    }

    #[test]
    fn functional_identity_full_leaf_grammar_and_extent_are_checked_independently() {
        use fe2o3_pliron::{
            ProductionRankedBlockV1 as Block, ProductionRankedKernelV1 as Kernel,
            ProductionRankedTerminatorV1 as Terminator, ProductionRankedValueIdV1 as Id,
        };
        for case in 0..9 {
            let mut expected_anchor = anchor();
            expected_anchor.invocation = case != 6;
            let id = Id::new(0);
            let mut operations = vec![match case {
                1 => ProductionRankedOperationV1::InvocationIndex {
                    result: id,
                    dimension: 1,
                    launch_extent: 0,
                },
                2 => ProductionRankedOperationV1::InvocationIndex {
                    result: id,
                    dimension: 0,
                    launch_extent: 1,
                },
                3 => ProductionRankedOperationV1::IndexUnknown { result: id },
                4 => ProductionRankedOperationV1::IndexConstant {
                    result: id,
                    value: 0,
                },
                _ => ProductionRankedOperationV1::InvocationIndex {
                    result: id,
                    dimension: 0,
                    launch_extent: 0,
                },
            }];
            let mut value = ProductionRankedValueV1::Local(id);
            if case == 5 {
                value = ProductionRankedValueV1::Argument(0);
            }
            if matches!(case, 7 | 8) {
                operations.push(ProductionRankedOperationV1::IndexUnsignedCast {
                    result: Id::new(1),
                    source: value,
                    bit_width: if case == 7 { 32 } else { 64 },
                });
                value = ProductionRankedValueV1::Local(Id::new(1));
            }
            let kernel = Kernel::new(
                "identity_leaf_component",
                1,
                vec![Block::new(operations, Terminator::Return)],
            )
            .unwrap();
            let lowering = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
                fe2o3_pliron::ProductionConstructionV1::ranked_kernel(
                    "identity_leaf_component",
                    kernel,
                )
                .unwrap(),
                fe2o3_pliron::ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            let mut meter = CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = AssertOriginBudgetV1::new(&mut meter, 1_000_000);
            budget.reserve_storage(13).unwrap();
            source_output_global_scratch_scope_v1(&mut budget, |budget| {
                let mut work = source_output_full_address_workspace_v1(budget)?;
                source_output_address_ranked_index_v1(&mut work.full, &lowering, budget)?;
                if matches!(case, 1 | 2) {
                    let expected = if case == 1 { (1, 0) } else { (0, 1) };
                    let mut invocations = lowering
                        .kernel()
                        .blocks()
                        .iter()
                        .flat_map(|block| block.operations())
                        .filter_map(|operation| match operation {
                            ProductionRankedOperationV1::InvocationIndex {
                                result,
                                dimension,
                                launch_extent,
                            } if *result == id => Some((*dimension, *launch_extent)),
                            _ => None,
                        });
                    assert_eq!(invocations.next(), Some(expected));
                    assert_eq!(invocations.next(), None);
                    assert!(work.full.ranked.iter().all(|row| row.value != id));
                }
                assert_origin_push_v1(&mut work.claims, SourceOutputFullAddressClaimV1 {
                    claim: ProductionProjectionArgumentCandidateV1 {
                        ranked_value: ProductionRankedValueV1::Local(id), source_local: expected_anchor.local,
                        component: expected_anchor.component,
                    }, anchor: None,
                }, budget).map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                let expected = NormalizedScalarNodeV1::Symbol { symbol: 17, scalar: source_output_address_u64_v1() };
                let result = source_output_full_address_leaf_v1(&mut work, &lowering, value, expected_anchor, &expected, budget);
                let error = match case {
                    0 | 8 => None,
                    1 | 2 => Some("physical ranked definition unsupported or absent"),
                    4 => Some("full address ranked constant differs"),
                    5 => Some("full address ranked leaf is not a supported definition"),
                    _ => Some("full address ranked leaf grammar unsupported"),
                };
                if let Some(expected) = error {
                    assert!(matches!(&result, Err(ProductionSourceOutputErrorV1::Invalid(actual)) if *actual == expected), "case {case}: {result:?}");
                    assert!(work.claims[0].anchor.is_none());
                } else {
                    result?;
                    assert!(work.claims[0].anchor == Some(expected_anchor));
                }
                Ok(())
            }).unwrap();
            assert_eq!(budget.storage(), 13);
        }
    }

    #[test]
    fn functional_identity_claim_push_denial_after_allocation_restores_larger_workspace_floor() {
        let header = std::mem::size_of::<SourceOutputFullAddressWorkspaceV1>();
        let row_bytes = std::mem::size_of::<SourceOutputFullAddressClaimV1>();
        // 7 prefix + 2 workspace + 1 empty-Vec reserve + 1 insertion.
        for limit in [11, 10] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
            budget.reserve_storage(13).unwrap();
            budget.charge_work(7).unwrap();
            let mut allocated = false;
            let result = source_output_global_scratch_scope_v1(&mut budget, |budget| {
                let mut scratch = source_output_full_address_workspace_v1(budget)?;
                assert!(scratch.identity.is_none());
                let result = assert_origin_push_v1(
                    &mut scratch.claims,
                    SourceOutputFullAddressClaimV1 {
                        claim: ProductionProjectionArgumentCandidateV1 {
                            ranked_value: ProductionRankedValueV1::Argument(91),
                            source_local: anchor().local,
                            component: anchor().component,
                        },
                        anchor: None,
                    },
                    budget,
                )
                .map_err(ProductionSourceOutputErrorV1::SourceOrigin);
                assert!(scratch.claims.capacity() >= 4);
                allocated = true;
                assert_eq!(
                    budget.storage(),
                    13 + header + scratch.claims.capacity() * row_bytes
                );
                assert_eq!(scratch.claims.len(), usize::from(limit == 11));
                result
            });
            assert!(allocated);
            if limit == 11 {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::SourceOrigin(
                        SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Work(_))
                    ))
                ));
            }
            assert_eq!(budget.storage(), 13);
            assert_eq!(budget.work(), limit);
            assert_eq!(
                work.failed_work(),
                if limit == 11 { None } else { Some(11) }
            );
        }
    }
}

#[cfg(test)]
mod functional_address_captured_index_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        InertSemanticMirRequestV1, SemanticAssignmentV1, SemanticBasicBlockV1, SemanticMirLimitsV1,
        SemanticRvalueV1, SemanticStatementV1,
    };
    use fe2o3_pliron::{
        ProductionSemanticSsaFunctionOccurrencesV1 as Captured, ProductionSemanticSsaLimitsV1,
        ProductionSemanticSsaOwnerV1,
    };

    const PREFIX: usize = 13;
    const FIRST_DEFINE: SourceOutputFullAddressEventKeyV1 = (0, 0, 0, 0, 2, 0);
    const COPY_USE: SourceOutputFullAddressEventKeyV1 = (0, 1, 3, 0, 0, 0);
    const SECOND_DEFINE: SourceOutputFullAddressEventKeyV1 = (0, 1, 0, 0, 2, 0);

    fn with_captured_rows(body: impl FnOnce(&Captured<'_>, &mut AssertOriginBudgetV1<'_>)) {
        let seed = resource_tests::scalar_transmute_semantic_owner();
        let semantic = seed.semantic();
        let function = &semantic.functions()[0];
        let block = &function.blocks()[0];
        let local = SemanticLocalIdV1::from_index(1);
        let ty = function.locals()[1].ty();
        let place = SemanticPlaceV1::new(local, vec![], ty).unwrap();
        let mut statements = block.statements().to_vec();
        assert_eq!(statements.len(), 1);
        statements.push(SemanticStatementV1::new(
            function.source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place.clone(),
                SemanticRvalueV1::new(
                    ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)),
                ),
            )),
        ));
        let changed = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            function.abi().clone(),
            function.locals().to_vec(),
            function.entry(),
            vec![
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    statements,
                    block.terminator().clone(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone());
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            semantic.types().to_vec(),
            semantic.allocations().to_vec(),
            semantic.statics().to_vec(),
            semantic.vtables().to_vec(),
            vec![changed],
            semantic.callables().to_vec(),
            semantic.roots().to_vec(),
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let mut ssa = ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let receipt = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        assert_eq!(budget.storage(), PREFIX);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        {
            let occurrences = ssa.occurrences_v1().unwrap();
            let captured = occurrences
                .function(SemanticFunctionIdV1::from_index(0))
                .unwrap();
            assert!(std::ptr::eq(captured.owner(), &ssa));
            body(&captured, &mut budget);
        }
        assert_eq!(budget.storage(), floor);
        drop(ssa);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        budget.release_storage(PREFIX).unwrap();
        assert_eq!(work.failed_work(), None);
    }

    fn with_index(
        captured: &Captured<'_>,
        budget: &mut AssertOriginBudgetV1<'_>,
        body: impl FnOnce(
            &mut SourceOutputFullAddressWorkspaceV1,
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        source_output_global_scratch_scope_v1(budget, |budget| {
            let mut workspace = source_output_full_address_workspace_v1(budget)?;
            source_output_full_address_events_v1(&mut workspace, captured, budget)?;
            assert_eq!(workspace.events.len(), 3);
            body(&mut workspace, budget)
        })
    }

    #[test]
    fn functional_address_real_capture_rejects_corrupt_event_index_rows() {
        with_captured_rows(|captured, budget| {
            let floor = budget.storage();
            with_index(captured, budget, |workspace, budget| {
                let live = budget.storage();
                let at = workspace
                    .events
                    .iter()
                    .position(|row| row.0 == COPY_USE)
                    .unwrap();
                let original = workspace.events[at];
                let different = workspace
                    .events
                    .iter()
                    .find(|row| row.0 == FIRST_DEFINE)
                    .unwrap()
                    .1;
                assert_ne!(original.1, different);
                let exact = &captured.events()[original.1];
                assert!(exact.is_reachable() && exact.is_promoted());
                assert!(std::ptr::eq(
                    source_output_full_address_event_v1(workspace, captured, COPY_USE, budget)?,
                    exact,
                ));
                let removed = workspace.events.remove(at);
                assert!(matches!(
                    source_output_full_address_event_v1(workspace, captured, COPY_USE, budget),
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "full address exact source event absent"
                    ))
                ));
                workspace.events.insert(at, removed);
                assert_eq!(workspace.events[at], original);
                assert_eq!(budget.storage(), live);
                for (ordinal, expected) in [
                    (
                        captured.events().len(),
                        "full address source event index differs",
                    ),
                    (
                        different,
                        "full address source event is not exact reachable promoted occurrence",
                    ),
                ] {
                    assert!(std::ptr::eq(
                        source_output_full_address_event_v1(workspace, captured, COPY_USE, budget)?,
                        exact,
                    ));
                    workspace.events[at].1 = ordinal;
                    assert!(matches!(
                        source_output_full_address_event_v1(workspace, captured, COPY_USE, budget),
                        Err(ProductionSourceOutputErrorV1::Invalid(actual)) if actual == expected
                    ));
                    assert_eq!(budget.storage(), live);
                    workspace.events[at] = original;
                    assert!(std::ptr::eq(
                        source_output_full_address_event_v1(workspace, captured, COPY_USE, budget)?,
                        exact,
                    ));
                }
                assert_eq!(budget.storage(), live);
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }

    #[test]
    fn functional_address_real_capture_checks_use_and_definition_identity() {
        with_captured_rows(|captured, budget| {
            let floor = budget.storage();
            with_index(captured, budget, |workspace, budget| {
                let live = budget.storage();
                let local = SemanticLocalIdV1::from_index(1);
                let other_local = SemanticLocalIdV1::from_index(0);
                let values = [FIRST_DEFINE, SECOND_DEFINE].map(|key| {
                    let row = source_output_full_address_event_v1(workspace, captured, key, budget)
                        .unwrap();
                    let Some(SsaResolvedEventV1::Define { variable, value }) = row.resolved()
                    else {
                        panic!("actual promoted assignment must have a captured definition");
                    };
                    assert_eq!(variable.get(), local.index());
                    assert!(matches!(value, SsaValueV1::Definition(_)));
                    value
                });
                assert_ne!(values[0], values[1]);
                assert_eq!(
                    source_output_full_address_use_v1(
                        workspace, captured, COPY_USE, local, budget
                    )?,
                    values[0],
                );
                for (key, wrong_local) in [(COPY_USE, other_local), (FIRST_DEFINE, local)] {
                    assert_eq!(
                        source_output_full_address_use_v1(
                            workspace, captured, COPY_USE, local, budget
                        )?,
                        values[0],
                    );
                    assert!(matches!(
                        source_output_full_address_use_v1(
                            workspace,
                            captured,
                            key,
                            wrong_local,
                            budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full address captured source use differs"
                        ))
                    ));
                    assert_eq!(budget.storage(), live);
                    assert_eq!(
                        source_output_full_address_use_v1(
                            workspace, captured, COPY_USE, local, budget
                        )?,
                        values[0],
                    );
                }
                for (key, wrong_local, wrong_value) in [
                    (FIRST_DEFINE, other_local, values[0]),
                    (FIRST_DEFINE, local, values[1]),
                    (COPY_USE, local, values[0]),
                ] {
                    source_output_full_address_define_v1(
                        workspace,
                        captured,
                        FIRST_DEFINE,
                        local,
                        values[0],
                        budget,
                    )?;
                    assert!(matches!(
                        source_output_full_address_define_v1(
                            workspace,
                            captured,
                            key,
                            wrong_local,
                            wrong_value,
                            budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full identity captured definition differs"
                        ))
                    ));
                    assert_eq!(budget.storage(), live);
                    source_output_full_address_define_v1(
                        workspace,
                        captured,
                        FIRST_DEFINE,
                        local,
                        values[0],
                        budget,
                    )?;
                }
                assert_eq!(
                    source_output_full_address_use_v1(
                        workspace, captured, COPY_USE, local, budget
                    )?,
                    values[0],
                );
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }

    #[test]
    fn functional_address_real_capture_index_scope_restores_errors_panics_and_reentry() {
        with_captured_rows(|captured, budget| {
            let floor = budget.storage();
            let before = budget.work();
            let mut error_entered = false;
            let error = with_index(captured, budget, |workspace, budget| {
                source_output_full_address_event_v1(workspace, captured, COPY_USE, budget)?;
                error_entered = true;
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "captured index callback error",
                ))
            });
            assert!(error_entered);
            assert!(matches!(
                error,
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "captured index callback error"
                ))
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before);
            let after_error = budget.work();
            let mut panic_entered = false;
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_index(captured, budget, |workspace, budget| {
                    source_output_full_address_event_v1(workspace, captured, COPY_USE, budget)?;
                    panic_entered = true;
                    panic!("captured index callback panic");
                })
            }))
            .expect_err("the entered captured-index callback must unwind");
            assert!(panic_entered);
            let payload = panic
                .downcast_ref::<&'static str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str));
            assert_eq!(payload, Some("captured index callback panic"));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > after_error);
            let after_panic = budget.work();
            with_index(captured, budget, |workspace, budget| {
                source_output_full_address_event_v1(workspace, captured, COPY_USE, budget)?;
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > after_panic);
        });
    }
}
