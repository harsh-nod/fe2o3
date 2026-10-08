// Exact original external AtomicRmw accesses. These are not private-slot
// stores, and a Generic address or an empty private-origin set is not a permit.
// All rows are checked again against the complete actual expanded function.
#[derive(Clone, Copy)]
struct SourceAtomicExternalRowV41 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    original: usize,
    block: BlockId,
    operation: usize,
    pointer: ValueId,
}

fn source_atomic_external_error_v41() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "atomic external access changed its original custody, payload or relocated graph",
    )
}

fn source_atomic_execution_site_v41(
    site: SourceReferenceSiteV29,
) -> Result<ExecutionSiteV29, ProductionSemanticKirErrorV1> {
    Ok(execution_site_v29(
        site.block,
        Some(
            u32::try_from(
                site.statement
                    .ok_or_else(source_atomic_external_error_v41)?,
            )
            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        ),
    ))
}

// Classification is only a proposal. The mandatory whole-root check below
// proves the actual root transport, every receipt and the complete effect census.
fn source_atomic_external_role_v41(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    row: &ScopedMemoryAnchorV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, SourceAtomicUseV41), ProductionSemanticKirErrorV1> {
    references.plan.check_owner(instances, budget)?;
    source_reference_emission_prepay_v29::<(usize, SourceAtomicUseV41)>(budget)?;
    budget.charge_work(8)?;
    let Some(frame) = row.source else {
        return Err(source_atomic_external_error_v41());
    };
    if frame.role
        != Some(ScopedMemoryRoleV29::Operand(
            ExecutionOperandV29::AtomicAddress,
        ))
        || !matches!(
            row.kind,
            ScopedMemoryAnchorKindV29::Access {
                payload: Some(ScopedMemoryPayloadV29::AtomicRmw { .. }),
                ..
            }
        )
    {
        return Err(source_atomic_external_error_v41());
    }
    let function = instances
        .instance(instance)
        .ok_or_else(source_atomic_external_error_v41)?
        .declaration();
    let Some(SemanticStatementKindV1::AtomicRmw(atomic)) =
        scoped_source_statement_v29(function, frame.site)
    else {
        return Err(source_atomic_external_error_v41());
    };
    let (block, statement) = scoped_memory_site_key_v29(frame.site);
    let site = SourceReferenceSiteV29 {
        instance,
        block: SemanticBlockIdV1::from_index(block),
        statement: statement.map(|n| n as usize),
    };
    references
        .atomic_use_v41(site, atomic, budget)?
        .ok_or_else(source_atomic_external_error_v41)
}

fn source_atomic_external_claim_v41(
    references: &SourceReferenceEmissionV29<'_, '_>,
    index: usize,
    source: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceAtomicPhysicalReceiptV41, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    source_reference_emission_prepay_v29::<SourceAtomicPhysicalReceiptV41>(budget)?;
    budget.charge_work(4)?;
    let claim = references
        .atomic_receipts
        .get(index)
        .and_then(|row| row.get())
        .ok_or_else(source_atomic_external_error_v41)?;
    if claim.source != source || claim.first > claim.end {
        return Err(source_atomic_external_error_v41());
    }
    Ok(claim)
}

fn source_atomic_external_root_v41(
    references: &SourceReferenceEmissionV29<'_, '_>,
    actual: &SourceIssuedActualV29<'_>,
    custody: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ValueId, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    source_reference_emission_prepay_v29::<Type>(budget)?;
    budget.charge_work(12)?;
    let plan = references.plan;
    let fact = plan
        .atomic_custody
        .get(custody)
        .ok_or_else(source_atomic_external_error_v41)?;
    let parent = plan
        .loans
        .get(fact.parent)
        .ok_or_else(source_atomic_external_error_v41)?;
    let origin = plan
        .origins
        .get(parent.origin)
        .ok_or_else(source_atomic_external_error_v41)?;
    if origin.instance != fact.instance
        || origin.local != fact.local
        || origin.generation != fact.generation
        || origin.anchor != Some(fact.anchor)
        || origin.ty != fact.anchor.ty
        || !origin.projections.is_empty()
        || !matches!(parent.representation, SourceReferenceRepresentationV29::ExistingAllocationBinding(anchor)
            if anchor == fact.anchor)
    {
        return Err(source_atomic_external_error_v41());
    }
    let expected = source_atomic_root_type_v41(plan, fact.parent, budget)?
        .ok_or_else(source_atomic_external_error_v41)?;
    let root = actual
        .root_arguments
        .get(fact.anchor.argument as usize)
        .ok_or_else(source_atomic_external_error_v41)?;
    let input = root.input.ok_or_else(source_atomic_external_error_v41)?;
    let physical = root.physical.ok_or_else(source_atomic_external_error_v41)?;
    if root.argument != fact.anchor.argument
        || root.ty != fact.anchor.ty
        || !root.entry
        || actual.value(input, budget)?.input != Some(physical)
        || !invocation_equal_types_v1(actual.value(input, budget)?.ty, &expected, budget)?
        || !matches!(expected, Type::Pointer(pointer)
            if pointer.address_space == AddressSpace::Global && pointer.access == AccessMode::ReadWrite)
    {
        return Err(source_atomic_external_error_v41());
    }
    Ok(input)
}

// A field-chain view may preserve the physical address, but its exact original
// BaseUse and original SSA definition must still name that same physical value.
// This is not ordinary dereference permission and does not load projected bytes.
fn source_atomic_external_base_v41(
    instances: &ExecutionInstancesV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    place: &SemanticPlaceV1,
    value: ValueId,
    ty: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Option<SsaValueV1>>(budget)?;
    let function = instances
        .instance(instance)
        .ok_or_else(source_atomic_external_error_v41)?
        .declaration();
    if !scoped_payload_place_v29(function, site, role).is_some_and(|p| std::ptr::eq(p, place)) {
        return Err(source_atomic_external_error_v41());
    }
    let occurrences = instances
        .occurrences(instance)
        .ok_or_else(source_atomic_external_error_v41)?;
    let mut definition = None;
    for event in occurrences.events() {
        budget.charge_work(6)?;
        if event.site() != site
            || event.operand() != role
            || event.role() != ExecutionEventV29::BaseUse
            || event.event().variable().get() != place.local().index()
        {
            continue;
        }
        let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
            return Err(source_atomic_external_error_v41());
        };
        if !event.is_reachable()
            || !event.is_promoted()
            || variable.get() != place.local().index()
            || definition.replace(value).is_some()
        {
            return Err(source_atomic_external_error_v41());
        }
    }
    let archive = source_index
        .sidecar(instance, budget)?
        .execution_observation
        .as_ref()
        .ok_or_else(source_atomic_external_error_v41)?;
    archive.check_original_v29(instances, instance, budget)?;
    let SemanticValueBindingV1::Value {
        id,
        ty: original_type,
    } = archive.lookup_original_v29(
        instances,
        instance,
        definition.ok_or_else(source_atomic_external_error_v41)?,
        budget,
    )?
    else {
        return Err(source_atomic_external_error_v41());
    };
    if *id != value || !invocation_equal_types_v1(original_type, ty, budget)? {
        return Err(source_atomic_external_error_v41());
    }
    Ok(())
}

fn source_atomic_external_result_v41(
    instances: &ExecutionInstancesV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    row: SourceAtomicUseV41,
    result: ValueId,
    ty: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Option<SsaValueV1>>(budget)?;
    let site = source_atomic_execution_site_v41(row.site)?;
    let function = instances
        .instance(row.site.instance)
        .ok_or_else(source_atomic_external_error_v41)?
        .declaration();
    let Some(SemanticStatementKindV1::AtomicRmw(atomic)) =
        scoped_source_statement_v29(function, site)
    else {
        return Err(source_atomic_external_error_v41());
    };
    if atomic.destination() as *const SemanticPlaceV1 as usize != row.destination
        || !atomic.destination().projections().is_empty()
    {
        return Err(source_atomic_external_error_v41());
    }
    let occurrences = instances
        .occurrences(row.site.instance)
        .ok_or_else(source_atomic_external_error_v41)?;
    let mut definition = None;
    for event in occurrences.events() {
        budget.charge_work(5)?;
        if event.site() != site
            || event.role() != ExecutionEventV29::DestinationDefine
            || event.event().variable().get() != atomic.destination().local().index()
        {
            continue;
        }
        let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved() else {
            return Err(source_atomic_external_error_v41());
        };
        if !event.is_reachable()
            || !event.is_promoted()
            || variable.get() != atomic.destination().local().index()
            || definition.replace(value).is_some()
        {
            return Err(source_atomic_external_error_v41());
        }
    }
    let archive = source_index
        .sidecar(row.site.instance, budget)?
        .execution_observation
        .as_ref()
        .ok_or_else(source_atomic_external_error_v41)?;
    archive.check_original_v29(instances, row.site.instance, budget)?;
    let SemanticValueBindingV1::Value {
        id,
        ty: original_type,
    } = archive.lookup_original_v29(
        instances,
        row.site.instance,
        definition.ok_or_else(source_atomic_external_error_v41)?,
        budget,
    )?
    else {
        return Err(source_atomic_external_error_v41());
    };
    if *id != result || !invocation_equal_types_v1(original_type, ty, budget)? {
        return Err(source_atomic_external_error_v41());
    }
    Ok(())
}

fn source_atomic_external_rhs_v41(
    instances: &ExecutionInstancesV29<'_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    actual: &SourceIssuedActualV29<'_>,
    instance: ProductionCallInstanceIdV1,
    source: ScopedMemoryStoreSourceV29,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<Constant>(budget)?;
    source_reference_emission_prepay_v29::<Type>(budget)?;
    budget.charge_work(7)?;
    let ScopedMemoryStoreSourceV29::Operand {
        site,
        role: ExecutionOperandV29::AtomicValue,
        ty,
        source,
    } = source
    else {
        return Err(source_atomic_external_error_v41());
    };
    let function = instances
        .instance(instance)
        .ok_or_else(source_atomic_external_error_v41)?
        .declaration();
    match (
        source,
        scoped_source_operand_v29(function, site, ExecutionOperandV29::AtomicValue),
    ) {
        (
            ScopedMemoryOperandSourceV29::Place(
                occurrence @ ScopedMemoryOccurrenceV29::Promoted { .. },
            ),
            Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)),
        ) => {
            if place.ty() != ty {
                return Err(source_atomic_external_error_v41());
            }
            let archive = source_index
                .sidecar(instance, budget)?
                .execution_observation
                .as_ref()
                .ok_or_else(source_atomic_external_error_v41)?;
            archive.check_original_v29(instances, instance, budget)?;
            check_scoped_payload_archive_v29(&archive.bindings, place, occurrence, value, budget)
        }
        (ScopedMemoryOperandSourceV29::Constant, Some(SemanticOperandV1::Constant(source))) => {
            if source.ty() != ty {
                return Err(source_atomic_external_error_v41());
            }
            let SemanticConstantValueV1::Scalar(bits) = source.value() else {
                return Err(source_atomic_external_error_v41());
            };
            let ty = lower_scalar_type(instances.owner().source_semantic().types(), ty)?;
            let expected = lower_constant(ty, *bits)?;
            let Some(Operation {
                kind: OperationKind::Constant(observed),
                results,
            }) = actual.value(value, budget)?.operation
            else {
                return Err(source_atomic_external_error_v41());
            };
            if !matches!(results.as_slice(), [result] if result.id == value)
                || *observed != expected
            {
                return Err(source_atomic_external_error_v41());
            }
            Ok(())
        }
        // Retained-memory RHS needs its own exact earlier-load/history relation;
        // unsupported transport remains a refusal, never a guessed scalar.
        _ => Err(source_atomic_external_error_v41()),
    }
}

fn check_source_atomic_external_memory_v41(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    graph: &SourceAddressMemoryV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    references.plan.check_owner(instances, budget)?;
    references.check(budget)?;
    let plan = references.plan;
    let function = &source_index.pending.function;
    let body = function
        .body
        .as_ref()
        .ok_or_else(source_atomic_external_error_v41)?;
    budget.charge_work(body.blocks.len())?;
    if graph.blocks.len() != body.blocks.len() {
        return Err(source_atomic_external_error_v41());
    }
    for block in &body.blocks {
        if !std::ptr::eq(graph.blocks[graph.block(block.id, budget)?].1, block) {
            return Err(source_atomic_external_error_v41());
        }
    }
    // Empty plans still census the complete actual function. Unclaimed Atomic
    // operations cannot hide outside the private-memory load/store equations.
    let mut actual_count = 0;
    for block in &body.blocks {
        budget.charge_work(1)?;
        for operation in &block.operations {
            budget.charge_work(1)?;
            if matches!(operation.kind, OperationKind::Atomic(_)) {
                actual_count = argument_sum_v1(&[actual_count, 1])?;
            }
        }
    }
    if plan.atomic_uses.is_empty()
        && plan.atomic_captures.is_empty()
        && plan.atomic_formations.is_empty()
    {
        return if actual_count == 0 {
            Ok(())
        } else {
            Err(source_atomic_external_error_v41())
        };
    }
    if actual_count != plan.atomic_uses.len()
        || instances.owner().source_semantic().wire_version()
            != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
        || plan.atomic_captures.is_empty()
    {
        return Err(source_atomic_external_error_v41());
    }
    // Fixed query envelopes plus actual Vec capacities are charged on the same
    // original scratch ledger. No row escapes this activation scratch scope.
    source_reference_emission_prepay_v29::<SourceAtomicExternalRowV41>(budget)?;
    source_reference_emission_prepay_v29::<SourceAtomicCaptureV41>(budget)?;
    source_reference_emission_prepay_v29::<SourceAtomicViewFormationV41>(budget)?;
    source_reference_emission_prepay_v29::<SourceAtomicUseV41>(budget)?;
    source_reference_emission_prepay_v29::<SourceIssuedPointerTransportV26>(budget)?;
    source_reference_emission_prepay_v29::<ScopedMemoryPayloadV29>(budget)?;
    source_reference_emission_prepay_v29::<(Option<(BlockId, u32)>, [usize; 16])>(budget)?;
    source_reference_emission_prepay_v29::<Vec<SourceAtomicExternalRowV41>>(budget)?;
    source_reference_emission_prepay_v29::<Vec<bool>>(budget)?;
    source_reference_emission_prepay_v29::<Vec<SourceIssuedPointerTransportV26>>(budget)?;
    let mut actual = SourceIssuedActualV29::from_function(function, budget)?;
    actual.bind_root_arguments(plan, source_index, function, budget)?;
    let capacity = argument_sum_v1(&[
        argument_product_v1(plan.atomic_captures.len(), 2)?,
        plan.atomic_formations.len(),
        plan.atomic_uses.len(),
    ])?;
    let mut transports = emission_vec_v1(capacity, budget)?;
    let mut rows = emission_vec_v1(plan.atomic_uses.len(), budget)?;
    let mut seen = emission_vec_v1(plan.atomic_uses.len(), budget)?;
    budget.charge_work(plan.atomic_uses.len())?;
    seen.resize(plan.atomic_uses.len(), false);
    for (index, capture) in plan.atomic_captures.iter().enumerate() {
        budget.charge_work(10)?;
        let declaration = instances
            .instance(capture.site.instance)
            .ok_or_else(source_atomic_external_error_v41)?
            .declaration();
        let site = source_atomic_execution_site_v41(capture.site)?;
        let Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) =
            scoped_source_operand_v29(declaration, site, ExecutionOperandV29::RvalueOperand(0))
        else {
            return Err(source_atomic_external_error_v41());
        };
        if place as *const SemanticPlaceV1 as usize != capture.source {
            return Err(source_atomic_external_error_v41());
        }
        let claim = source_atomic_external_claim_v41(references, index, capture.source, budget)?;
        let frame =
            ScopedMemoryFrameV29::operand(site, Some(ExecutionOperandV29::RvalueOperand(0)));
        source_index.frame_gap(
            capture.site.instance,
            frame,
            claim.block,
            claim.first,
            budget,
        )?;
        source_index.frame_gap(capture.site.instance, frame, claim.block, claim.end, budget)?;
        if claim.end != argument_sum_v1(&[claim.first, 1])? {
            return Err(source_atomic_external_error_v41());
        }
        let operation = source_index.emitted.operation(
            capture.site.instance,
            claim.block,
            claim.first,
            budget,
        )?;
        let expected = source_atomic_node_type_v41(plan, capture.node, budget)?
            .ok_or_else(source_atomic_external_error_v41)?;
        if !matches!((&operation.kind, operation.results.as_slice()),
            (OperationKind::Cast { kind: CastKind::PointerToGeneric, value, to }, [result])
                if *value == claim.base && result.id == claim.result && *to == expected && result.ty == expected)
        {
            return Err(source_atomic_external_error_v41());
        }
        let issuer = source_atomic_external_root_v41(references, &actual, capture.custody, budget)?;
        if graph.exact(claim.result, budget)?.is_some() {
            return Err(source_atomic_external_error_v41());
        }
        transports.push(SourceIssuedPointerTransportV26 {
            pointer: claim.base,
            issuer,
        });
        transports.push(SourceIssuedPointerTransportV26 {
            pointer: claim.result,
            issuer,
        });
    }
    for (index, formation) in plan.atomic_formations.iter().enumerate() {
        budget.charge_work(8)?;
        let declaration = instances
            .instance(formation.site.instance)
            .ok_or_else(source_atomic_external_error_v41)?
            .declaration();
        let site = source_atomic_execution_site_v41(formation.site)?;
        let Some(SemanticStatementKindV1::Assign(assign)) =
            scoped_source_statement_v29(declaration, site)
        else {
            return Err(source_atomic_external_error_v41());
        };
        if plan.atomic_view_formation_v41(formation.site.instance, site, assign.value(), budget)?
            != Some(*formation)
        {
            return Err(source_atomic_external_error_v41());
        }
        let claim = source_atomic_external_claim_v41(
            references,
            argument_sum_v1(&[plan.atomic_captures.len(), index])?,
            formation.source,
            budget,
        )?;
        let (place, role) = match assign.value().kind() {
            SemanticRvalueKindV1::Cast {
                operand: SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                ..
            } => (place, ExecutionOperandV29::RvalueOperand(0)),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            }
            | SemanticRvalueKindV1::AddressOf { place, .. } => {
                (place, ExecutionOperandV29::RvaluePlace)
            }
            _ => return Err(source_atomic_external_error_v41()),
        };
        let frame = ScopedMemoryFrameV29::operand(site, Some(role));
        source_index.frame_gap(
            formation.site.instance,
            frame,
            claim.block,
            claim.first,
            budget,
        )?;
        source_index.frame_gap(
            formation.site.instance,
            frame,
            claim.block,
            claim.end,
            budget,
        )?;
        let expected =
            source_atomic_custody_type_v41(plan, formation.input, formation.input_type, budget)?;
        let output =
            source_atomic_custody_type_v41(plan, formation.output, formation.output_type, budget)?;
        if claim.base != claim.result
            || claim.first != claim.end
            || !invocation_equal_types_v1(&expected, &output, budget)?
            || !invocation_equal_types_v1(actual.value(claim.result, budget)?.ty, &output, budget)?
        {
            return Err(source_atomic_external_error_v41());
        }
        source_atomic_external_base_v41(
            instances,
            source_index,
            formation.site.instance,
            site,
            role,
            place,
            claim.base,
            &expected,
            budget,
        )?;
        let issuer =
            source_atomic_external_root_v41(references, &actual, formation.output, budget)?;
        if graph.exact(claim.result, budget)?.is_some() {
            return Err(source_atomic_external_error_v41());
        }
        transports.push(SourceIssuedPointerTransportV26 {
            pointer: claim.result,
            issuer,
        });
    }
    for sidecar in &source_index.pending.sidecars.rows {
        budget.charge_work(1)?;
        let instance = sidecar
            .source_call_instance
            .ok_or_else(source_atomic_external_error_v41)?;
        let recorded = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_atomic_external_error_v41)?;
        let declaration = instances
            .instance(instance)
            .ok_or_else(source_atomic_external_error_v41)?
            .declaration();
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(source_atomic_external_error_v41)?;
        for (anchor, row) in recorded.rows.iter().enumerate() {
            budget.charge_work(2)?;
            let ScopedMemoryAnchorKindV29::Access {
                pointer,
                payload:
                    Some(ScopedMemoryPayloadV29::AtomicRmw {
                        result,
                        value,
                        source,
                        ..
                    }),
            } = row.kind
            else {
                continue;
            };
            let (index, original) =
                source_atomic_external_role_v41(instances, references, instance, row, budget)?;
            if std::mem::replace(
                seen.get_mut(index).ok_or(ArgumentResourceV1::Accounting)?,
                true,
            ) {
                return Err(source_atomic_external_error_v41());
            }
            let claim = source_atomic_external_claim_v41(
                references,
                argument_sum_v1(&[
                    plan.atomic_captures.len(),
                    plan.atomic_formations.len(),
                    index,
                ])?,
                original.source,
                budget,
            )?;
            let frame = row.source.ok_or_else(source_atomic_external_error_v41)?;
            source_index.frame_gap(instance, frame, row.block, row.position, budget)?;
            if claim.base != pointer
                || claim.result != result
                || claim.block != row.block
                || claim.first != row.position
                || claim.end != argument_sum_v1(&[row.position, 1])?
            {
                return Err(source_atomic_external_error_v41());
            }
            let operation =
                source_index
                    .emitted
                    .operation(instance, row.block, row.position, budget)?;
            check_scoped_payload_v29(declaration, &occurrences, row, operation, budget)?;
            let Some(SemanticStatementKindV1::AtomicRmw(atomic)) =
                scoped_source_statement_v29(declaration, frame.site)
            else {
                return Err(source_atomic_external_error_v41());
            };
            let expected = source_atomic_node_type_v41(plan, original.node, budget)?
                .ok_or_else(source_atomic_external_error_v41)?;
            source_atomic_external_base_v41(
                instances,
                source_index,
                instance,
                frame.site,
                ExecutionOperandV29::AtomicAddress,
                atomic.address(),
                pointer,
                &expected,
                budget,
            )?;
            source_atomic_external_rhs_v41(
                instances,
                source_index,
                &actual,
                instance,
                source,
                value,
                budget,
            )?;
            let [output] = operation.results.as_slice() else {
                return Err(source_atomic_external_error_v41());
            };
            source_atomic_external_result_v41(
                instances,
                source_index,
                original,
                result,
                &output.ty,
                budget,
            )?;
            let issuer =
                source_atomic_external_root_v41(references, &actual, original.custody, budget)?;
            if graph.exact(pointer, budget)?.is_some() {
                return Err(source_atomic_external_error_v41());
            }
            let (block, position) = source_index
                .emitted
                .point(
                    instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    budget,
                )?
                .ok_or_else(source_atomic_external_error_v41)?;
            if rows.len() == rows.capacity() || transports.len() == transports.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            rows.push(SourceAtomicExternalRowV41 {
                instance,
                anchor,
                original: index,
                block,
                operation: position as usize,
                pointer,
            });
            transports.push(SourceIssuedPointerTransportV26 { pointer, issuer });
        }
    }
    budget.charge_work(seen.len())?;
    if seen.iter().any(|row| !row) || rows.len() != actual_count || transports.len() != capacity {
        return Err(source_atomic_external_error_v41());
    }
    call_splice_sort_work_v1(rows.len(), budget).map_err(source_address_call_error_v29)?;
    rows.sort_unstable_by_key(|row| (row.block, row.operation));
    for pair in rows.windows(2) {
        budget.charge_work(2)?;
        if (pair[0].block, pair[0].operation) == (pair[1].block, pair[1].operation) {
            return Err(source_atomic_external_error_v41());
        }
    }
    for block in &body.blocks {
        budget.charge_work(1)?;
        for (position, operation) in block.operations.iter().enumerate() {
            budget.charge_work(1)?;
            let OperationKind::Atomic(atomic) = &operation.kind else {
                continue;
            };
            charge_execution_cfg_lookup_v29(rows.len(), budget)?;
            let selected = rows
                .binary_search_by_key(&(block.id, position), |row| (row.block, row.operation))
                .map_err(|_| source_atomic_external_error_v41())?;
            let row = rows[selected];
            if row.pointer != atomic.pointer || !seen[row.original] {
                return Err(source_atomic_external_error_v41());
            }
            let original = source_index
                .sidecar(row.instance, budget)?
                .scoped_memory_anchors
                .as_ref()
                .and_then(|recorded| recorded.rows.get(row.anchor))
                .ok_or_else(source_atomic_external_error_v41)?;
            if !std::ptr::eq(
                source_index.emitted.operation(
                    row.instance,
                    original.block,
                    original.position,
                    budget,
                )?,
                operation,
            ) {
                return Err(source_atomic_external_error_v41());
            }
        }
    }
    // Original root arguments were bound by the exact ABI emitter. This
    // existing all-path CFG query follows actual casts/parameter edges back to
    // that same Global input, and refuses offsets, mixed peers and unknowns.
    check_source_issued_pointer_transports_v26(function, &actual, &transports, budget)?;
    Ok(())
}
