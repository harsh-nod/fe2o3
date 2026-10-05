// The existing checked loan representation is reused before candidate entry
// storage is selected. These rows are locators, not allocation exemptions.
fn source_reference_existing_value_loan_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    index: usize,
    declaration: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let loan = plan.loans.get(index).ok_or_else(execution_cfg_error_v29)?;
    let origin = plan
        .origins
        .get(loan.origin)
        .ok_or_else(execution_cfg_error_v29)?;
    if budget
        .source_reference_scalar_cell_v29(plan, index)?
        .is_some()
    {
        return Ok(false);
    }
    let SourceReferenceRepresentationV29::ExistingAllocationBinding(anchor) =
        budget.source_reference_representation_v29(plan, index)?
    else {
        return Ok(false);
    };
    if !origin.projections.is_empty()
        || origin.ty != declaration
        || origin.anchor != Some(anchor)
        || origin.ty != anchor.ty
        || loan.effects.referent_writes != 0
        || loan.effects.address_observations != 0
    {
        return Ok(false);
    }
    let payload = source_reference_payload_types_v29(plan, index, budget)?;
    if payload.len() != 1 || !execution_is_direct_owner_type_v29(&payload[0]) {
        return Err(execution_cfg_error_v29());
    }
    Ok(true)
}

type SourceExistingReceiverRowV29 = ((usize, u32), bool);

fn source_existing_receiver_write_v53(
    plan: &SourceReferencePlanV29<'_, '_>,
    access: &SourceReferenceAccessRecordV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(20)?;
    if access.key.access != SourceReferenceAccessV29::Write
        || access.key.site.instance != access.instance
        || access.source_local != access.local
        || !access.projections.is_empty()
        || access.loan.is_some()
        || !access.traversed.is_empty()
        || access.shared_path
    {
        return Ok(false);
    }
    let function = plan
        .instances
        .instance(access.instance)
        .ok_or_else(source_backing_error_v29)?
        .declaration();
    let Some(statement) = access.key.site.statement.and_then(|statement| {
        function
            .blocks()
            .get(access.key.site.block.index() as usize)
            .and_then(|block| block.statements().get(statement))
    }) else {
        return Ok(false);
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Ok(false);
    };
    let destination = assignment.destination();
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(input)) = assignment.value().kind()
    else {
        return Ok(false);
    };
    Ok(
        destination as *const SemanticPlaceV1 as usize == access.key.source
            && destination.local() == access.local
            && destination.ty() == access.ty
            && destination.projections().is_empty()
            && input.projections().is_empty()
            && input.ty() == access.ty
            && assignment.value().result_type() == access.ty,
    )
}

fn source_existing_receiver_value_v53(
    plan: &SourceReferencePlanV29<'_, '_>,
    source: &SourceReferenceRepresentationDemandV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(10)?;
    if !source.projections.is_empty() || source.selector_source.is_some() {
        return Ok(false);
    }
    let declaration = plan
        .instances
        .instance(source.instance)
        .and_then(|instance| {
            instance
                .declaration()
                .locals()
                .get(source.local.index() as usize)
        })
        .ok_or_else(source_backing_error_v29)?;
    let node = plan
        .nodes
        .get(source.node)
        .ok_or_else(source_backing_error_v29)?;
    let SourceReferenceNodeKindV29::Plain(Some(anchor)) = node.kind else {
        return Ok(false);
    };
    if node.ty != declaration.ty() || anchor.ty != node.ty {
        return Ok(false);
    }
    let semantic = plan.instances.owner().source_semantic();
    let root = plan
        .instances
        .instance(plan.root)
        .ok_or_else(source_backing_error_v29)?;
    let Some(SemanticTypeShapeV1::Aggregate(fields)) = semantic
        .types()
        .get(anchor.ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(false);
    };
    prepay_argument_shape_v1(semantic, anchor.ty, budget)?;
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(semantic.callables().len(), 4)?,
        argument_product_v1(fields.fields().len(), 20)?,
        32,
    ])?)?;
    budget.reserve_storage(std::mem::size_of::<Type>())?;
    let physical = authenticated_disjoint_slice_parameter(
        semantic.types(),
        semantic.callables(),
        root.declaration(),
        anchor.argument,
        anchor.ty,
    );
    let supported = matches!(physical, Some(Type::Slice(_)));
    drop(physical);
    budget.release_storage(std::mem::size_of::<Type>())?;
    Ok(supported)
}

fn source_existing_receiver_rows_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SourceExistingReceiverRowV29>, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<Vec<SourceExistingReceiverRowV29>>(),
        std::mem::size_of::<Result<Vec<SourceExistingReceiverRowV29>, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<(&SourceReferenceAccessRecordV29, &SemanticFunctionDeclV1, &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1, &SemanticPlaceV1, &SemanticPlaceV1)>(),
        std::mem::size_of::<(&SourceReferenceRepresentationDemandV29, &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1, &SourceReferenceNodeV29, SourceReferenceAnchorV29, Option<Type>, bool)>(),
        std::mem::size_of::<Result<bool, ProductionSemanticKirErrorV1>>(),
    ])?)?;
    let mut rows = emission_vec_v1(plan.loans.len(), budget)?;
    for (index, loan) in plan.loans.iter().enumerate() {
        budget.charge_work(8)?;
        let origin = plan
            .origins
            .get(loan.origin)
            .ok_or_else(source_backing_error_v29)?;
        let declaration = plan
            .instances
            .instance(origin.instance)
            .and_then(|instance| {
                instance
                    .declaration()
                    .locals()
                    .get(origin.local.index() as usize)
            })
            .ok_or_else(source_backing_error_v29)?;
        // NeedsStorage has not yet been resolved during backing selection.
        // Only an already selected promoted loan can use this existing route.
        let existing = (plan.storage == SourceReferenceStorageV29::PromotedOnly
            || matches!(
                plan.cells.strategies.get(index),
                Some(SourceReferenceCellStrategyV29::Promoted)
            ))
            && matches!(
                loan.representation,
                SourceReferenceRepresentationV29::ExistingAllocationBinding(_)
            )
            && source_reference_existing_value_loan_v29(plan, index, declaration.ty(), budget)?;
        rows.push(((origin.instance.index(), origin.local.index()), existing));
    }
    source_existing_receiver_group_v29(&mut rows, budget)?;
    for access in &plan.accesses {
        budget.charge_work(4)?;
        let key = (access.instance.index(), access.local.index());
        charge_execution_cfg_lookup_v29(rows.len(), budget)?;
        let Ok(index) = rows.binary_search_by_key(&key, |row| row.0) else {
            continue;
        };
        // Whole-value transport and original borrows do not observe wrapper
        // layout. Exact whole-owner moves can replace the carrier, but a
        // projected write, field, raw address or tag still needs object storage.
        let whole = access.projections.is_empty()
            && (matches!(
                access.key.access,
                SourceReferenceAccessV29::Read
                    | SourceReferenceAccessV29::Borrow(
                        SemanticBorrowKindV1::Shared | SemanticBorrowKindV1::Mutable
                    )
            ) || source_existing_receiver_write_v53(plan, access, budget)?);
        rows[index].1 &= whole;
    }
    // Authenticate every replacement's original nominal ABI anchor before
    // allowing any entry/write in this local to keep its existing Slice value.
    for source in &plan.representation_demands {
        budget.charge_work(2)?;
        let key = (source.instance.index(), source.local.index());
        charge_execution_cfg_lookup_v29(rows.len(), budget)?;
        if let Ok(index) = rows.binary_search_by_key(&key, |row| row.0) {
            rows[index].1 &= source_existing_receiver_value_v53(plan, source, budget)?;
        }
    }
    Ok(rows)
}

fn source_existing_receiver_group_v29(
    rows: &mut Vec<SourceExistingReceiverRowV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    call_splice_sort_work_v1(argument_product_v1(rows.len(), 2)?, budget)
        .map_err(source_address_call_error_v29)?;
    rows.sort_unstable_by_key(|row| row.0);
    let mut written = 0;
    for read in 0..rows.len() {
        budget.charge_work(3)?;
        let row = rows[read];
        if written != 0 && rows[written - 1].0 == row.0 {
            rows[written - 1].1 &= row.1;
        } else {
            rows[written] = row;
            written += 1;
        }
    }
    rows.truncate(written);
    Ok(())
}

fn source_existing_receiver_v29(
    rows: &[SourceExistingReceiverRowV29],
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(rows.len(), budget)?;
    Ok(rows
        .binary_search_by_key(&(instance.index(), local.index()), |row| row.0)
        .ok()
        .is_some_and(|index| rows[index].1))
}

// This does not remove an original activation. It decides only whether its
// entry representation needs new physical storage. Callers build the locator
// from this exact original plan; layout reconstruction repeats that proof.
fn source_existing_receiver_entry_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    rows: &[SourceExistingReceiverRowV29],
    activation: SourceReferenceStorageActivationV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    budget.charge_work(5)?;
    if activation.origin != SourceReferenceActivationOriginV29::Entry || activation.generation != 0
    {
        return Ok(false);
    }
    let instance = plan
        .instances
        .instance(activation.instance)
        .ok_or_else(source_backing_error_v29)?;
    let declaration = instance
        .declaration()
        .locals()
        .get(activation.local.index() as usize)
        .ok_or_else(source_backing_error_v29)?;
    if plan.instances.instance_reachable(activation.instance) != Some(true)
        || !declaration.role().is_entry_argument()
    {
        return Ok(false);
    }
    source_existing_receiver_v29(rows, activation.instance, activation.local, budget)
}
