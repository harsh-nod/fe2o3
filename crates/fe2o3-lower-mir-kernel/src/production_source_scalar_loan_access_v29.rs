// A static holder path is not a projection into the borrowed scalar. Raw
// pointer paths still use their separate exact prefix and origin census.
#[derive(Clone, Copy, Debug)]
struct SourceScalarLoanV29 {
    loan: usize,
    cell: SourceReferenceScalarCellV29,
}

fn source_scalar_static_holder_index_needed_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    anchors: &ScopedMemoryAnchorsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<bool>(budget)?;
    source_reference_emission_prepay_v29::<
        Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
    >(budget)?;
    source_reference_emission_prepay_v29::<
        &production_call_instances_v1::ProductionCallInstanceV1<'_>,
    >(budget)?;
    source_reference_emission_prepay_v29::<Option<ScopedMemoryFrameV29>>(budget)?;
    source_reference_emission_prepay_v29::<ScopedMemoryFrameV29>(budget)?;
    source_reference_emission_prepay_v29::<Option<&SemanticPlaceV1>>(budget)?;
    source_reference_emission_prepay_v29::<&SemanticPlaceV1>(budget)?;
    source_reference_emission_prepay_v29::<std::slice::Iter<'_, ScopedMemoryAnchorV29>>(budget)?;
    source_reference_emission_prepay_v29::<std::slice::Iter<'_, SemanticProjectionV1>>(budget)?;
    budget.charge_work(2)?;
    let function = instances
        .instance(instance)
        .ok_or_else(source_raw_physical_error_v29)?
        .declaration();
    for row in &anchors.rows {
        budget.charge_work(4)?;
        if !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. }) {
            continue;
        }
        let Some(frame) = row.source else {
            continue;
        };
        let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
            continue;
        };
        let Some(place) = scoped_source_place_v29(function, frame.site, role) else {
            continue;
        };
        let count = place.projections().len();
        if !(2..=256).contains(&count)
            || place.projections()[count - 1].kind() != SemanticProjectionKindV1::Dereference
        {
            continue;
        }
        budget.charge_work(count - 1)?;
        if place.projections()[..count - 1]
            .iter()
            .all(|projection| matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn source_scalar_loan_access_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceScalarLoanV29>, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<Option<SourceScalarLoanV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceScalarLoanV29>(plan, budget)?;
        let Some(holder) = source_object_loan_holder_type_v29(plan, site, place, budget)? else {
            return Ok(None);
        };
        source_reference_owned_prepay_v29::<&SourceReferenceAccessRecordV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceCellStrategyV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<(usize, SourceReferenceScalarCellV29)>>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<(usize, SourceReferenceScalarCellV29)>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceLoanV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SourceReferenceLoanV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SourceReferenceOriginV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SourceReferenceOriginV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&[usize]>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&usize>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
        let source = source_reference_access_at_v29(plan, site, place, access, budget)?;
        let Some(loan) = source.loan else {
            return Ok(None);
        };
        budget.source_reference_charge_v29(plan, 18)?;
        if !matches!(
            plan.cells.strategies.get(loan),
            Some(SourceReferenceCellStrategyV29::Scalar(_))
        ) {
            return Ok(None);
        }
        let (_, cell) = plan
            .backing_cell(loan, budget)?
            .ok_or_else(source_raw_physical_error_v29)?;
        let record = plan
            .loans
            .get(loan)
            .ok_or_else(source_raw_physical_error_v29)?;
        let origin = plan
            .origins
            .get(record.origin)
            .ok_or_else(source_raw_physical_error_v29)?;
        if !matches!(
            access,
            SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write
        ) || cell.kind != SourceBackingKindV29::Scalar
            || record.source_type != holder
            || !matches!(
                record.kind,
                SemanticBorrowKindV1::Shared | SemanticBorrowKindV1::Mutable
            )
            || source.ty != cell.ty
            || source.ty != place.ty()
            || source.instance != cell.instance
            || source.local != cell.local
            || source.generation != cell.generation
            || origin.ty != cell.ty
            || !origin.projections.is_empty()
            || !source.projections.is_empty()
            || plan
                .access_loans
                .get(source.traversed.clone())
                .and_then(|rows| rows.last())
                != Some(&loan)
            || (access == SourceReferenceAccessV29::Write
                && (source.shared_path || record.kind != SemanticBorrowKindV1::Mutable))
            || !matches!(
                plan.instances
                    .owner()
                    .source_semantic()
                    .types()
                    .get(cell.ty.index() as usize)
                    .map(|row| row.shape()),
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
            )
        {
            return Err(source_raw_physical_error_v29());
        }
        Ok(Some(SourceScalarLoanV29 { loan, cell }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn check_source_scalar_loan_holder_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    index: &SourceAddressSourceIndexV29<'_>,
    source: &SourceAddressAccessSourceV29,
    row: &ScopedMemoryAnchorV29,
    payload: ScopedMemoryPayloadV29,
    payload_index: &SourceObjectPayloadIndexV29,
    pointer: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<()>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticPlaceV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticPlaceV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<(
            ExecutionSiteV29,
            ExecutionOperandV29,
            usize,
            SourceReferenceAccessV29,
        )>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<ScopedMemoryFrameV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<ScopedMemoryFrameV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        >(plan, budget)?;
        budget.source_reference_charge_v29(plan, 5)?;
        let frame = row.source.ok_or_else(source_raw_physical_error_v29)?;
        let function = instances
            .instance(source.instance)
            .ok_or_else(source_raw_physical_error_v29)?
            .declaration();
        let (site, role, prefix, access) = match payload {
            ScopedMemoryPayloadV29::Load { read, .. } if read.prefix > 1 => (
                read.site,
                read.role,
                read.prefix as usize,
                SourceReferenceAccessV29::Read,
            ),
            ScopedMemoryPayloadV29::Store { .. } => {
                let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                    return Ok(());
                };
                let place = scoped_source_place_v29(function, frame.site, role)
                    .ok_or_else(source_raw_physical_error_v29)?;
                if place.projections().len() <= 1 {
                    return Ok(());
                }
                (
                    frame.site,
                    role,
                    place.projections().len(),
                    SourceReferenceAccessV29::Write,
                )
            }
            _ => return Ok(()),
        };
        let place = scoped_source_place_v29(function, site, role)
            .ok_or_else(source_raw_physical_error_v29)?;
        if prefix != place.projections().len() {
            return Ok(());
        }
        source_reference_owned_prepay_v29::<SourceReferenceSiteV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<(u32, Option<u32>)>(plan, budget)?;
        let (block, statement) = scoped_memory_site_key_v29(site);
        let original_site = SourceReferenceSiteV29 {
            instance: source.instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|value| value as usize),
        };
        let Some(checked) =
            source_scalar_loan_access_v29(plan, original_site, place, access, budget)?
        else {
            return Ok(());
        };
        source_reference_owned_prepay_v29::<ScopedMemoryOccurrenceV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticValueBindingV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<ProductionSemanticSsaFunctionOccurrencesV1<'_>>>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<&PendingInstanceSidecarsV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&ExecutionArchiveV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&ExecutionArchiveV29>(plan, budget)?;
        let occurrences = instances
            .occurrences(source.instance)
            .ok_or_else(execution_archive_error_v29)?;
        let capture = payload_index.base_occurrence(
            index,
            source.instance,
            &occurrences,
            site,
            role,
            place,
            budget,
        )?;
        let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = capture else {
            return Err(source_raw_physical_error_v29());
        };
        let sidecar = index.sidecar(source.instance, budget)?;
        let archive = sidecar
            .execution_observation
            .as_ref()
            .ok_or_else(execution_archive_error_v29)?;
        let binding =
            archive.lookup_original_v29(instances, source.instance, definition, budget)?;
        check_source_scalar_loan_binding_v29(plan, &checked, binding, place, pointer, budget)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn check_source_scalar_loan_binding_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    checked: &SourceScalarLoanV29,
    binding: &SemanticValueBindingV1,
    place: &SemanticPlaceV1,
    pointer: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<()>(plan, budget)?;
        let pointer_type =
            source_reference_cell_pointer_type_v29(plan, checked.loan, checked.cell, budget)?;
        let selected =
            source_safe_loan_holder_v29(plan, binding, place, checked.loan, &pointer_type, budget)?;
        budget.source_reference_charge_v29(plan, 1)?;
        if selected.values[0].id != pointer {
            return Err(source_raw_physical_error_v29());
        }
        Ok(())
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
