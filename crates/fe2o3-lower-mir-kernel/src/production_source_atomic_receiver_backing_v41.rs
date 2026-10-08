// Existing representation of one original atomic receiver, never a general
// aggregate/pointer allocation exemption or an ordinary memory permission.
fn source_atomic_receiver_root_v41(
    plan: &SourceReferencePlanV29<'_, '_>,
    capture: &SourceAtomicCaptureV41,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    source_reference_emission_prepay_v29::<bool>(budget)?;
    source_reference_emission_prepay_v29::<(
        &SourceAtomicPointerCustodyV41,
        &SourceReferenceNodeV29,
        &SourceReferenceLoanV29,
        &SourceReferenceOriginV29,
        &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
        &[SemanticTypeDeclV1],
        &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
        Option<Type>,
    )>(budget)?;
    budget.charge_work(24)?;
    if plan.instances.owner().source_semantic().wire_version()
        != fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
    {
        return Ok(false);
    }
    let Some(fact) = plan.atomic_custody.get(capture.custody) else {
        return Ok(false);
    };
    let Some(node) = plan.nodes.get(capture.node) else {
        return Ok(false);
    };
    let Some(loan) = plan.loans.get(fact.parent) else {
        return Ok(false);
    };
    let Some(origin) = plan.origins.get(loan.origin) else {
        return Ok(false);
    };
    let Some(root) = plan.instances.instance(plan.root) else {
        return Ok(false);
    };
    let Some(local) = root.declaration().locals().get(fact.local.index() as usize) else {
        return Ok(false);
    };
    if fact.instance != plan.root
        || fact.generation != 0
        || fact.view.is_some()
        || node.atomic_custody != Some(capture.custody)
        || origin.instance != fact.instance
        || origin.local != fact.local
        || origin.generation != fact.generation
        || origin.anchor != Some(fact.anchor)
        || origin.ty != fact.anchor.ty
        || !origin.projections.is_empty()
        || local.ty() != fact.anchor.ty
        || local.role() != SemanticLocalRoleV1::Argument(fact.anchor.argument)
        || loan.kind != SemanticBorrowKindV1::Shared
        || loan.effects.referent_writes != 0
        || loan.effects.address_observations != 0
        || loan.effects.payload_reads != 0
        || loan.effects.payload_writes != 0
        || !matches!(loan.representation,
            SourceReferenceRepresentationV29::ExistingAllocationBinding(anchor)
                if anchor == fact.anchor)
    {
        return Ok(false);
    }
    let types = plan.instances.owner().source_semantic().types();
    let Some(pointer) = source_atomic_view_pointer_v41(types, node.ty) else {
        return Ok(false);
    };
    let Some(SemanticTypeShapeV1::Aggregate(fields)) = types
        .get(fact.anchor.ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(false);
    };
    if pointer.kind() != SemanticPointerKindV1::Raw
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.pointee() != fact.scalar
        || fields.fields().get(fact.field as usize).copied() != Some(node.ty)
    {
        return Ok(false);
    }
    // Reconstruct the exact authenticated root ABI and original capture node,
    // rather than accepting a same-shaped aggregate or a scalar pointer.
    let physical = source_atomic_root_type_v41(plan, fact.parent, budget)?;
    Ok(matches!(physical, Some(Type::Pointer(pointer))
        if pointer.address_space == AddressSpace::Global
            && pointer.access == AccessMode::ReadWrite
            && matches!(pointer.pointee.as_ref(), Type::Scalar(ScalarType::I32 | ScalarType::U32))))
}

fn source_atomic_receiver_capture_read_v41(
    plan: &SourceReferencePlanV29<'_, '_>,
    access: &SourceReferenceAccessRecordV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    source_reference_emission_prepay_v29::<bool>(budget)?;
    source_reference_emission_prepay_v29::<(
        &SourceReferenceAccessRecordV29,
        &SourceAtomicCaptureV41,
        &SourceAtomicPointerCustodyV41,
        &SourceReferenceNodeV29,
        &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
        &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        &SemanticPlaceV1,
        &[usize],
        &SemanticProjectionV1,
        bool,
    )>(budget)?;
    budget.charge_work(12)?;
    if access.key.access != SourceReferenceAccessV29::Read
        || access.checked_enum_read.is_some()
        || !access.shared_path
        || access.generation != 0
    {
        return Ok(false);
    }
    let Some([projection]) = plan.projections.get(access.projections.clone()) else {
        return Ok(false);
    };
    let Some(traversed) = plan.access_loans.get(access.traversed.clone()) else {
        return Ok(false);
    };
    let mut found = false;
    for capture in &plan.atomic_captures {
        budget.charge_work(14)?;
        if capture.site != access.key.site || capture.source != access.key.source {
            continue;
        }
        if found || !source_atomic_receiver_root_v41(plan, capture, budget)? {
            return Ok(false);
        }
        let fact = &plan.atomic_custody[capture.custody];
        let node = &plan.nodes[capture.node];
        if access.instance != fact.instance
            || access.local != fact.local
            || access.generation != fact.generation
            || access.loan != Some(fact.parent)
            || traversed != [fact.parent]
            || projection.kind() != SemanticProjectionKindV1::Field(fact.field)
            || projection.result_type() != node.ty
            || access.ty != node.ty
        {
            return Ok(false);
        }
        // No raw locator is dereferenced. Recover the place from its actual
        // original assignment; only a Copy of the selected pointer field is
        // representation-only. A projected Move retains its storage obligation.
        let Some(function) = plan.instances.instance(capture.site.instance) else {
            return Ok(false);
        };
        let Some(statement) = capture.site.statement.and_then(|ordinal| {
            function
                .declaration()
                .blocks()
                .get(capture.site.block.index() as usize)
                .and_then(|block| block.statements().get(ordinal))
        }) else {
            return Ok(false);
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(false);
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return Ok(false);
        };
        if place as *const SemanticPlaceV1 as usize != capture.source
            || place.local() != access.source_local
            || place.ty() != access.ty
            || place.projections().is_empty()
        {
            return Ok(false);
        }
        found = true;
    }
    Ok(found)
}

fn source_atomic_receiver_value_v41(
    plan: &SourceReferencePlanV29<'_, '_>,
    source: &SourceReferenceRepresentationDemandV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    source_reference_emission_prepay_v29::<bool>(budget)?;
    source_reference_emission_prepay_v29::<(
        &SourceReferenceRepresentationDemandV29,
        &SourceReferenceNodeV29,
        SourceReferenceAnchorV29,
        &SourceAtomicCaptureV41,
        &SourceAtomicPointerCustodyV41,
        bool,
    )>(budget)?;
    budget.charge_work(10)?;
    if source.instance != plan.root
        || source.generation != 0
        || !source.projections.is_empty()
        || source.selector_source.is_some()
    {
        return Ok(false);
    }
    let Some(node) = plan.nodes.get(source.node) else {
        return Ok(false);
    };
    let SourceReferenceNodeKindV29::Plain(Some(anchor)) = node.kind else {
        return Ok(false);
    };
    if node.ty != anchor.ty || node.atomic_custody.is_some() || node.descriptor.is_some() {
        return Ok(false);
    }
    let mut found = false;
    for capture in &plan.atomic_captures {
        budget.charge_work(6)?;
        let Some(fact) = plan.atomic_custody.get(capture.custody) else {
            return Ok(false);
        };
        if fact.instance != source.instance || fact.local != source.local {
            continue;
        }
        if fact.generation != source.generation
            || fact.anchor != anchor
            || !source_atomic_receiver_root_v41(plan, capture, budget)?
        {
            return Ok(false);
        }
        found = true;
    }
    Ok(found)
}
