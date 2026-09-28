// Flat primitive aggregates reuse the existing Record -> Scalar geometry.
// This predicate is also used during construction, before an owned plan exists.
fn source_flat_aggregate_object_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    source_reference_emission_prepay_v29::<bool>(budget)?;
    source_reference_emission_prepay_v29::<Option<&SemanticTypeDeclV1>>(budget)?;
    source_reference_emission_prepay_v29::<Option<&SemanticTypeShapeV1>>(budget)?;
    source_reference_emission_prepay_v29::<&SemanticTypeDeclV1>(budget)?;
    source_reference_emission_prepay_v29::<&SemanticTypeShapeV1>(budget)?;
    source_reference_emission_prepay_v29::<&[SemanticTypeIdV1]>(budget)?;
    source_reference_emission_prepay_v29::<
        &fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1,
    >(budget)?;
    source_reference_emission_prepay_v29::<Option<&SemanticTypeIdV1>>(budget)?;
    source_reference_emission_prepay_v29::<&SemanticTypeIdV1>(budget)?;
    source_reference_emission_prepay_v29::<std::slice::Iter<'_, SemanticTypeIdV1>>(budget)?;
    budget.charge_work(4)?;
    let fields = match types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    {
        Some(SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields)) => {
            fields.fields()
        }
        _ => return Ok(false),
    };
    if fields.is_empty() || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Ok(false);
    }
    for field in fields {
        budget.charge_work(3)?;
        if !matches!(
            types
                .get(field.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

struct SourceAggregateObjectAccessV29 {
    loan: SourceObjectLoanV29,
    dereference: usize,
}

fn source_aggregate_object_endpoint_access_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceAggregateObjectAccessV29>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<Option<SourceAggregateObjectAccessV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticPlaceV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticTypeShapeV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<u32>>(plan, budget)?;
        let types = plan.instances.owner().source_semantic().types();
        budget.source_reference_charge_v29(plan, 3)?;
        let shape = types
            .get(endpoint.root_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
            .ok_or_else(scoped_object_error_v29)?;
        if !matches!(
            shape,
            SemanticTypeShapeV1::Tuple(_) | SemanticTypeShapeV1::Aggregate(_)
        ) {
            return Ok(None);
        }
        let Some(checked) = source_aggregate_object_access_v29(plan, site, place, access, budget)?
        else {
            return Err(scoped_object_pending_v29());
        };
        budget.source_reference_charge_v29(plan, 12)?;
        let ScopedObjectSourceV29::Place {
            site: original_site,
            role,
            local,
            prefix,
        } = endpoint.source
        else {
            return Err(scoped_object_error_v29());
        };
        let original_statement = match site.statement {
            Some(row) => Some(u32::try_from(row).map_err(|_| ArgumentResourceV1::Arithmetic)?),
            None => None,
        };
        if original_site != execution_site_v29(site.block, original_statement)
            || local != place.local()
            || !scoped_object_original_place_v29(
                plan.instances
                    .instance(site.instance)
                    .ok_or_else(scoped_object_error_v29)?
                    .declaration(),
                original_site,
                role,
            )
            .is_some_and(|original| std::ptr::eq(original, place))
            || !matches!(endpoint.object, ScopedObjectIdentityV29::Reference {
                instance, site: actual_site, role: actual_role, dereference_prefix,
            } if instance == site.instance && actual_site == original_site && actual_role == role
                && dereference_prefix as usize == checked.dereference + 1)
            || endpoint.root_type != checked.loan.original.ty
            || checked.loan.original.kind != SourceBackingKindV29::Object(endpoint.root_schema)
            || prefix as usize > place.projections().len()
            || !matches!(
                (prefix as usize).checked_sub(checked.dereference + 1),
                Some(0 | 1)
            )
            || endpoint.source_path.count != prefix as usize
            || endpoint.path.count != prefix as usize - checked.dereference - 1
            || (endpoint.path.count == 0
                && (endpoint.root_type != endpoint.projected_type
                    || endpoint.root_schema != endpoint.projected_schema))
            || (endpoint.path.count == 1 && endpoint.projected_type != place.ty())
        {
            return Err(scoped_object_error_v29());
        }
        Ok(Some(checked))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_aggregate_object_expected_location_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    source_index: &SourceAddressSourceIndexV29<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    endpoint: ScopedObjectEndpointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceStaticObjectLocationV29>, ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<Option<SourceStaticObjectLocationV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceStaticObjectLocationV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticPlaceV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceReferenceSiteV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&[SemanticProjectionV1]>>(plan, budget)?;
        source_reference_owned_prepay_v29::<u64>(plan, budget)?;
        source_reference_owned_prepay_v29::<usize>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceReferenceScalarCellV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<SourceReferenceAccessV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceReferenceAccessV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<&PendingInstanceSidecarsV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticFunctionDeclV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticPlaceV1>(plan, budget)?;
        let ScopedObjectIdentityV29::Reference { instance, .. } = endpoint.object else {
            return Ok(None);
        };
        let ScopedObjectSourceV29::Place {
            site, role, prefix, ..
        } = endpoint.source
        else {
            return Ok(None);
        };
        source_index.sidecar(instance, budget)?;
        let original = instances
            .instance(instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        let place = scoped_object_original_place_v29(original, site, role)
            .ok_or_else(scoped_object_error_v29)?;
        let (block, statement) = scoped_memory_site_key_v29(site);
        let site = SourceReferenceSiteV29 {
            instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|value| value as usize),
        };
        let access = source_reference_raw_original_access_v29(original, site, place, budget)?
            .ok_or_else(scoped_object_error_v29)?;
        let Some(checked) = source_aggregate_object_endpoint_access_v29(
            plan, site, place, access, endpoint, budget,
        )?
        else {
            return Ok(None);
        };
        let row = checked.loan.original;
        let slot = source_address_object_slot_v29(
            instances,
            plan,
            slots,
            row.instance,
            row.local,
            row.generation,
            row.ty,
            budget,
        )?;
        let layouts = plan
            .storage_root
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?
            .source_layouts(instances, budget)?;
        let path = place
            .projections()
            .get(checked.dereference + 1..prefix as usize)
            .ok_or_else(scoped_object_error_v29)?;
        let (mut ty, mut schema, mut offset) = (endpoint.root_type, endpoint.root_schema, 0u64);
        let selected = layouts.visit_selected_components(
            instances.owner(),
            ty,
            schema,
            path,
            budget,
            |component, budget| {
                budget.charge_work(6)?;
                if component.source_type != ty || component.source_schema != schema {
                    return Err(scoped_object_error_v29());
                }
                let source_storage_v29::SourceSelectedComponentKindV29::Field {
                    byte_offset, ..
                } = component.kind
                else {
                    return Err(scoped_object_pending_v29());
                };
                offset = offset
                    .checked_add(byte_offset)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                ty = component.result_type;
                schema = component
                    .result_schema
                    .ok_or_else(scoped_object_error_v29)?;
                Ok(())
            },
        )?;
        if ty != endpoint.projected_type
            || schema != endpoint.projected_schema
            || !matches!(selected, source_storage_v29::SourceSelectedProjectionV29::Physical { ty: t, schema: s } if t == ty && s == schema)
        {
            return Err(scoped_object_error_v29());
        }
        Ok(Some(SourceStaticObjectLocationV29 {
            slot,
            offset,
            schema: Some(schema),
        }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_aggregate_object_access_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    site: SourceReferenceSiteV29,
    place: &SemanticPlaceV1,
    access: SourceReferenceAccessV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<SourceAggregateObjectAccessV29>, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<Option<SourceAggregateObjectAccessV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceAggregateObjectAccessV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeIdV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticTypeIdV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<&[SemanticTypeIdV1]>(plan, budget)?;
        source_reference_owned_prepay_v29::<&[SemanticProjectionV1]>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticProjectionV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticProjectionV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&[SemanticProjectionV1]>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&[usize]>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&usize>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SourceReferenceAccessRecordV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<SourceObjectLoanV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            std::iter::Enumerate<std::slice::Iter<'_, SemanticProjectionV1>>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<(usize, &SemanticProjectionV1)>>(plan, budget)?;
        source_reference_owned_prepay_v29::<bool>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeDeclV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticTypeShapeV1>>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 10)?;
        if plan.storage == SourceReferenceStorageV29::PromotedOnly
            || place.projections().len() > 256
        {
            return Ok(None);
        }
        let types = plan.instances.owner().source_semantic().types();
        let function = plan
            .instances
            .instance(site.instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        let mut ty = function
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(scoped_object_error_v29)?
            .ty();
        for (dereference, projection) in place.projections().iter().enumerate() {
            budget.source_reference_charge_v29(plan, 5)?;
            if let SemanticProjectionKindV1::Field(field) = projection.kind() {
                let fields = match types
                    .get(ty.index() as usize)
                    .map(SemanticTypeDeclV1::shape)
                {
                    Some(
                        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                    ) => fields.fields(),
                    _ => return Ok(None),
                };
                ty = *fields
                    .get(field as usize)
                    .ok_or_else(scoped_object_error_v29)?;
                if projection.result_type() != ty {
                    return Err(scoped_object_error_v29());
                }
                continue;
            }
            if projection.kind() != SemanticProjectionKindV1::Dereference {
                return Ok(None);
            }
            let holder = ty;
            let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
                .get(holder.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Ok(None);
            };
            if pointer.kind() != SemanticPointerKindV1::Reference
                || pointer.metadata() != SemanticPointerMetadataV1::None
            {
                return Ok(None);
            }
            // Reborrowing a whole primitive Object uses this same exact
            // parent-pointer path, not an invented Read access. Aggregate
            // Read/Write suffixes remain restricted to the existing family.
            budget.source_reference_charge_v29(plan, 2)?;
            let primitive_borrow = matches!(
                access,
                SourceReferenceAccessV29::Borrow(
                    SemanticBorrowKindV1::Shared | SemanticBorrowKindV1::Mutable
                )
            ) && matches!(
                types
                    .get(pointer.pointee().index() as usize)
                    .map(SemanticTypeDeclV1::shape),
                Some(SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_))
            );
            if !primitive_borrow
                && !source_flat_aggregate_object_v29(types, pointer.pointee(), budget)?
            {
                return Ok(None);
            }
            if projection.result_type() != pointer.pointee() {
                return Err(scoped_object_error_v29());
            }
            let suffix = &place.projections()[dereference + 1..];
            let allowed = match access {
                SourceReferenceAccessV29::Borrow(
                    SemanticBorrowKindV1::Shared | SemanticBorrowKindV1::Mutable,
                ) => suffix.is_empty(),
                SourceReferenceAccessV29::Read | SourceReferenceAccessV29::Write => {
                    suffix.len() == 1
                        && matches!(suffix[0].kind(), SemanticProjectionKindV1::Field(_))
                }
                _ => false,
            };
            if !allowed {
                return Ok(None);
            }
            ty = pointer.pointee();
            if let Some(field) = suffix.first() {
                let SemanticProjectionKindV1::Field(index) = field.kind() else {
                    return Err(scoped_object_error_v29());
                };
                let fields = match types[ty.index() as usize].shape() {
                    SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                        fields.fields()
                    }
                    _ => return Err(scoped_object_error_v29()),
                };
                ty = *fields
                    .get(index as usize)
                    .ok_or_else(scoped_object_error_v29)?;
                if field.result_type() != ty {
                    return Err(scoped_object_error_v29());
                }
            }
            if place.ty() != ty {
                return Err(scoped_object_error_v29());
            }
            let source = source_reference_access_at_v29(plan, site, place, access, budget)?;
            let Some(loan) = source.loan else {
                return Ok(None);
            };
            let Some(checked) = source_object_loan_v29(plan, loan, budget)? else {
                return Ok(None);
            };
            budget.source_reference_charge_v29(plan, argument_sum_v1(&[15, suffix.len()])?)?;
            if plan.loans[loan].source_type != holder
                || checked.original.ty != pointer.pointee()
                || source.ty != ty
                || source.instance != checked.original.instance
                || source.local != checked.original.local
                || source.generation != checked.original.generation
                || plan.projections.get(source.projections.clone()) != Some(suffix)
                || plan
                    .access_loans
                    .get(source.traversed.clone())
                    .and_then(|rows| rows.last())
                    != Some(&loan)
                || (matches!(
                    access,
                    SourceReferenceAccessV29::Write
                        | SourceReferenceAccessV29::Borrow(SemanticBorrowKindV1::Mutable)
                ) && (source.shared_path
                    || plan.loans[loan].kind != SemanticBorrowKindV1::Mutable))
            {
                return Err(scoped_object_error_v29());
            }
            return Ok(Some(SourceAggregateObjectAccessV29 {
                loan: checked,
                dereference,
            }));
        }
        Ok(None)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn source_aggregate_object_holder_v29<'a>(
    plan: &SourceReferencePlanV29<'_, '_>,
    mut binding: &'a SemanticValueBindingV1,
    place: &SemanticPlaceV1,
    access: &SourceAggregateObjectAccessV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a SemanticSourceReferenceBindingV29, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<&SemanticSourceReferenceBindingV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticValueBindingV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticValueBindingV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<std::slice::Iter<'_, SemanticProjectionV1>>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<Option<&SemanticProjectionV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticProjectionV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<&Vec<SemanticValueBindingV1>>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 5)?;
        for projection in &place.projections()[..access.dereference] {
            budget.source_reference_charge_v29(plan, 3)?;
            let (SemanticProjectionKindV1::Field(field), SemanticValueBindingV1::Aggregate(fields)) =
                (projection.kind(), binding)
            else {
                return Err(scoped_object_error_v29());
            };
            binding = fields
                .get(field as usize)
                .ok_or_else(scoped_object_error_v29)?;
        }
        let SemanticValueBindingV1::SourceReference(binding) = binding else {
            return Err(scoped_object_error_v29());
        };
        source_reference_validate_binding_v29(plan, binding, budget)?;
        if binding.origin.single_loan()? != access.loan.loan
            || !matches!(binding.values.as_slice(), [value] if value.ty == access.loan.pointer_type)
        {
            return Err(scoped_object_error_v29());
        }
        Ok(binding)
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn check_source_aggregate_object_root_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    index: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    endpoint: ScopedObjectEndpointV29,
    pointer: ValueId,
    capture: ScopedMemoryOccurrenceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<()>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&SemanticPlaceV1>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceReferenceSiteV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<ScopedMemoryOccurrenceV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<SourceReferenceAccessV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<SourceReferenceAccessV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<
            Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<
            &production_call_instances_v1::ProductionCallInstanceV1<'_>,
        >(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticFunctionDeclV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticPlaceV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<ProductionSemanticSsaFunctionOccurrencesV1<'_>>>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<ProductionSemanticSsaFunctionOccurrencesV1<'_>>(
            plan, budget,
        )?;
        source_reference_owned_prepay_v29::<&PendingInstanceSidecarsV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&ExecutionArchiveV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&ExecutionArchiveV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticValueBindingV1>(plan, budget)?;
        source_reference_owned_prepay_v29::<&SemanticSourceReferenceBindingV29>(plan, budget)?;
        let ScopedObjectSourceV29::Place { site, role, .. } = endpoint.source else {
            return Err(scoped_object_error_v29());
        };
        let original = instances
            .instance(instance)
            .ok_or_else(scoped_object_error_v29)?
            .declaration();
        let place = scoped_object_original_place_v29(original, site, role)
            .ok_or_else(scoped_object_error_v29)?;
        let (block, statement) = scoped_memory_site_key_v29(site);
        let original_site = SourceReferenceSiteV29 {
            instance,
            block: SemanticBlockIdV1::from_index(block),
            statement: statement.map(|value| value as usize),
        };
        let access =
            source_reference_raw_original_access_v29(original, original_site, place, budget)?
                .ok_or_else(scoped_object_error_v29)?;
        let checked = source_aggregate_object_endpoint_access_v29(
            plan,
            original_site,
            place,
            access,
            endpoint,
            budget,
        )?
        .ok_or_else(scoped_object_error_v29)?;
        if endpoint.path.count != 0 {
            return Err(scoped_object_error_v29());
        }
        let sidecar = index.sidecar(instance, budget)?;
        let archive = sidecar
            .execution_observation
            .as_ref()
            .ok_or_else(execution_archive_error_v29)?;
        let occurrences = instances
            .occurrences(instance)
            .ok_or_else(execution_archive_error_v29)?;
        check_scoped_payload_occurrence_v29(&occurrences, site, role, place, capture, budget)?;
        let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = capture else {
            return Err(scoped_object_pending_v29());
        };
        let binding = archive.lookup_original_v29(instances, instance, definition, budget)?;
        let selected = source_aggregate_object_holder_v29(plan, binding, place, &checked, budget)?;
        budget.charge_work(1)?;
        if selected.values[0].id != pointer {
            return Err(scoped_object_error_v29());
        }
        Ok(())
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

fn check_source_aggregate_object_value_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    index: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    graph: &SourceAddressMemoryV29<'_>,
    anchors: &ScopedMemoryAnchorsV29,
    payload_index: &SourceObjectPayloadIndexV29,
    endpoint: ScopedObjectEndpointV29,
    pointer: ValueId,
    capture: ScopedMemoryOccurrenceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    plan.check_owner(instances, budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<()>(plan, budget)?;
        source_reference_owned_prepay_v29::<Option<&ScopedMemoryAnchorV29>>(plan, budget)?;
        source_reference_owned_prepay_v29::<&ScopedMemoryAnchorV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<&ScopedObjectPayloadV29>(plan, budget)?;
        source_reference_owned_prepay_v29::<&Operation>(plan, budget)?;
        if endpoint.path.count != 1 {
            return Err(scoped_object_error_v29());
        }
        let anchor = payload_index.project_anchor(index, instance, anchors, pointer, budget)?;
        let row = anchors
            .rows
            .get(anchor)
            .ok_or_else(scoped_object_error_v29)?;
        let payload = anchors.object_payload(row, budget)?;
        let (
            ScopedObjectOperationV29::Project { base, .. },
            ScopedObjectRoleV29::Project { source, projected },
        ) = (payload.operation, payload.role)
        else {
            return Err(scoped_object_error_v29());
        };
        budget.charge_work(2)?;
        if projected != endpoint {
            return Err(scoped_object_error_v29());
        }
        let actual = source_address_original_operation_v29(
            index.pending,
            graph,
            instance,
            row.block,
            row.position,
            budget,
        )?;
        payload.check_operation(actual, budget)?;
        check_source_aggregate_object_root_v29(
            instances, plan, index, instance, source, base, capture, budget,
        )
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn source_aggregate_object_pointer_v29(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
    ) -> Result<Option<(SourceAggregateObjectAccessV29, ValueId)>, ProductionSemanticKirErrorV1>
    {
        self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(scoped_object_error_v29)?;
            references.check(budget)?;
            let plan = references.plan;
            source_reference_owned_prepay_v29::<Option<(SourceAggregateObjectAccessV29, ValueId)>>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<(SourceAggregateObjectAccessV29, ValueId)>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<Option<&Option<SemanticValueBindingV1>>>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<Option<&SemanticValueBindingV1>>(plan, budget)?;
            source_reference_owned_prepay_v29::<&SemanticValueBindingV1>(plan, budget)?;
            let Some(checked) =
                source_aggregate_object_access_v29(plan, site, place, access, budget)?
            else {
                return Ok(None);
            };
            let binding = this
                .locals
                .get(place.local().index() as usize)
                .and_then(Option::as_ref)
                .ok_or_else(scoped_object_error_v29)?;
            let binding =
                source_aggregate_object_holder_v29(plan, binding, place, &checked, budget)?;
            Ok(Some((checked, binding.values[0].id)))
        })
    }

    fn source_aggregate_object_reborrow_v29(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        child: &SourceObjectLoanV29,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let kind = self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(scoped_object_error_v29)?;
            references.check(budget)?;
            source_reference_owned_prepay_v29::<Option<SemanticValueBindingV1>>(
                references.plan,
                budget,
            )?;
            source_reference_owned_prepay_v29::<SemanticValueBindingV1>(references.plan, budget)?;
            source_reference_owned_prepay_v29::<SemanticBorrowKindV1>(references.plan, budget)?;
            Ok(references.plan.loans[child.loan].kind)
        })?;
        let Some((parent, pointer)) = self.source_aggregate_object_pointer_v29(
            site,
            place,
            SourceReferenceAccessV29::Borrow(kind),
        )?
        else {
            return Err(scoped_object_pending_v29());
        };
        let ty = self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(scoped_object_error_v29)?;
            references.check(budget)?;
            source_reference_owned_prepay_v29::<Type>(references.plan, budget)?;
            budget.source_reference_charge_v29(references.plan, 8)?;
            if references.plan.loans[child.loan].parent != Some(parent.loan.loan)
                || child.original != parent.loan.original
                || child.cell != parent.loan.cell
                || parent.dereference + 1 != place.projections().len()
            {
                return Err(scoped_object_error_v29());
            }
            execution_cfg_clone_type_v29(&child.pointer_type, budget)
        })?;
        let same_type = self.with_emission_budget_v1(|this, budget| {
            let plan = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(scoped_object_error_v29)?
                .plan;
            source_reference_owned_prepay_v29::<bool>(plan, budget)?;
            source_reference_owned_prepay_v29::<(&Type, &Type)>(plan, budget)?;
            source_reference_owned_prepay_v29::<(
                &fe2o3_kernel_ir::PointerType,
                &fe2o3_kernel_ir::PointerType,
            )>(plan, budget)?;
            budget.source_reference_charge_v29(plan, 8)?;
            invocation_equal_types_v1(&parent.loan.pointer_type, &ty, budget)
        })?;
        if same_type {
            return Ok(Some(SemanticValueBindingV1::Value { id: pointer, ty }));
        }
        if !matches!((&parent.loan.pointer_type, &ty), (Type::Pointer(from), Type::Pointer(to))
            if from.pointee == to.pointee && from.address_space == AddressSpace::Private
                && to.address_space == AddressSpace::Private && from.access == AccessMode::ReadWrite
                && to.access == AccessMode::ReadOnly && kind == SemanticBorrowKindV1::Shared)
        {
            return Err(scoped_object_error_v29());
        }
        let result_type = self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(scoped_object_error_v29)?;
            source_reference_owned_prepay_v29::<Type>(references.plan, budget)?;
            execution_cfg_clone_type_v29(&ty, budget)
        })?;
        self.emit(
            operations,
            result_type,
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: pointer,
                to: ty,
            },
        )
        .map(Some)
    }

    fn source_aggregate_object_endpoint_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        access: SourceReferenceAccessV29,
        operations: &mut Vec<Operation>,
    ) -> Result<
        Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
        ProductionSemanticKirErrorV1,
    > {
        let site = self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(scoped_object_error_v29)?;
            let references = cursor.references.ok_or_else(scoped_object_error_v29)?;
            source_reference_owned_prepay_v29::<
                Option<(ScopedObjectEndpointV29, ValueId, MemoryAccess)>,
            >(references.plan, budget)?;
            source_reference_owned_prepay_v29::<SourceReferenceSiteV29>(references.plan, budget)?;
            Ok(SourceReferenceSiteV29 {
                instance: cursor.instance,
                block,
                statement: statement.map(|value| value as usize),
            })
        })?;
        let Some((checked, pointer)) =
            self.source_aggregate_object_pointer_v29(site, place, access)?
        else {
            return Ok(None);
        };
        let (endpoint, permission) = self.with_emission_budget_v1(|this, budget| {
            let plan = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(scoped_object_error_v29)?
                .plan;
            source_reference_owned_prepay_v29::<(ScopedObjectEndpointV29, AccessMode)>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<Option<ScopedMemoryFrameV29>>(plan, budget)?;
            source_reference_owned_prepay_v29::<ScopedMemoryFrameV29>(plan, budget)?;
            source_reference_owned_prepay_v29::<Option<&SemanticPlaceV1>>(plan, budget)?;
            source_reference_owned_prepay_v29::<ScopedObjectPathV29>(plan, budget)?;
            source_reference_owned_prepay_v29::<std::slice::Iter<'_, SemanticProjectionV1>>(
                plan, budget,
            )?;
            source_reference_owned_prepay_v29::<Option<&SemanticProjectionV1>>(plan, budget)?;
            source_reference_owned_prepay_v29::<&SemanticProjectionV1>(plan, budget)?;
            source_reference_owned_prepay_v29::<&fe2o3_kernel_ir::PointerType>(plan, budget)?;
            let frame = this
                .scoped_memory
                .as_ref()
                .and_then(|row| row.frame)
                .ok_or_else(scoped_object_error_v29)?;
            let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
                return Err(scoped_object_pending_v29());
            };
            budget.source_reference_charge_v29(plan, 8)?;
            if frame.site != execution_site_v29(block, statement)
                || !scoped_object_original_place_v29(this.function, frame.site, role)
                    .is_some_and(|original| std::ptr::eq(original, place))
            {
                return Err(scoped_object_error_v29());
            }
            let count = argument_sum_v1(&[checked.dereference, 1])?;
            let mut path = source_reference_owned_vec_v29(plan, count, budget)?;
            for projection in &place.projections()[..count] {
                budget.source_reference_charge_v29(plan, 1)?;
                path.push(ScopedObjectComponentV29::Original {
                    projection: *projection,
                    selector: None,
                });
            }
            let source_path = this
                .scoped_memory
                .as_mut()
                .ok_or_else(scoped_object_error_v29)?
                .anchors
                .append_object_path(&path, budget)?;
            let prefix = u32::try_from(count).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let SourceBackingKindV29::Object(schema) = checked.loan.original.kind else {
                return Err(scoped_object_error_v29());
            };
            let Type::Pointer(pointer_type) = &checked.loan.pointer_type else {
                return Err(scoped_object_error_v29());
            };
            Ok((
                ScopedObjectEndpointV29 {
                    object: ScopedObjectIdentityV29::Reference {
                        instance: site.instance,
                        site: frame.site,
                        role,
                        dereference_prefix: prefix,
                    },
                    source: ScopedObjectSourceV29::Place {
                        site: frame.site,
                        role,
                        local: place.local(),
                        prefix,
                    },
                    root_type: checked.loan.original.ty,
                    projected_type: checked.loan.original.ty,
                    root_schema: schema,
                    projected_schema: schema,
                    source_path,
                    path: ScopedObjectPathV29 { first: 0, count: 0 },
                },
                pointer_type.access,
            ))
        })?;
        self.source_object_project_path_v29(
            place,
            checked.dereference + 1,
            place.projections().len(),
            endpoint,
            pointer,
            permission,
            MemoryAccess::new(AddressSpace::Private, 1),
            operations,
        )
        .map(Some)
    }
}
