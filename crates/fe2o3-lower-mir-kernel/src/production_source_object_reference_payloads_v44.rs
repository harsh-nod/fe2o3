// A reference payload is not a scalar referent. These queries retain the exact
// original SSA occurrence and require an already-addressable checked loan.
fn source_object_reference_payload_error_v44() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "stored source reference requires its exact addressable loan and selected schema",
    )
}

fn source_object_reference_field_v44(declaration: &SemanticTypeDeclV1) -> bool {
    matches!(declaration.shape(), SemanticTypeShapeV1::Pointer(pointer)
        if pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.metadata() == SemanticPointerMetadataV1::None)
}

fn source_object_reference_same_v44(
    plan: &SourceReferencePlanV29<'_, '_>,
    actual: &SemanticSourceReferenceBindingV29,
    original: &SemanticSourceReferenceBindingV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    budget.source_reference_charge_v29(plan, 10)?;
    if actual.owner != original.owner
        || actual.source != original.source
        || actual.ssa != original.ssa
        || actual.root != original.root
        || actual.origin != original.origin
        || actual.source_type != original.source_type
    {
        return Err(source_object_reference_payload_error_v44());
    }
    let ([actual], [original]) = (actual.values.as_slice(), original.values.as_slice()) else {
        return Err(source_object_reference_payload_error_v44());
    };
    if actual.id != original.id || !invocation_equal_types_v1(&actual.ty, &original.ty, budget)? {
        return Err(source_object_reference_payload_error_v44());
    }
    Ok(())
}

fn source_object_archived_reference_v44<'binding>(
    plan: &SourceReferencePlanV29<'_, '_>,
    archive: &'binding ExecutionArchiveV29,
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    original: &SemanticPlaceV1,
    occurrence: ScopedMemoryOccurrenceV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'binding SemanticSourceReferenceBindingV29, ProductionSemanticKirErrorV1> {
    (|| {
        let (ty, copied, definition) = source_object_reference_operand_v44(
            plan, instance, site, role, original, occurrence, budget,
        )?;
        let value = archive.lookup_original_v29(plan.instances, instance, definition, budget)?;
        source_object_projected_reference_v44(plan, value, original, ty, copied, budget)
    })()
    .inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn source_object_current_reference_v44<'binding>(
        &'binding self,
        plan: &SourceReferencePlanV29<'_, '_>,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        original: &SemanticPlaceV1,
        occurrence: ScopedMemoryOccurrenceV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<&'binding SemanticSourceReferenceBindingV29, ProductionSemanticKirErrorV1> {
        (|| {
            let cursor = self
                .execution
                .as_ref()
                .ok_or(ArgumentResourceV1::Accounting)?;
            let references = cursor.references.ok_or(ArgumentResourceV1::Accounting)?;
            references.check(budget)?;
            if !std::ptr::eq(references.plan, plan)
                || scoped_payload_occurrence_v29(cursor, site, role, original, budget)?
                    != Some(occurrence)
            {
                return Err(source_object_reference_payload_error_v44());
            }
            let (ty, copied, definition) = source_object_reference_operand_v44(
                plan,
                cursor.instance,
                site,
                role,
                original,
                occurrence,
                budget,
            )?;
            charge_execution_cfg_lookup_v29(self.semantic_ssa_bindings.len(), budget)?;
            let value = self
                .semantic_ssa_bindings
                .get(&definition)
                .ok_or_else(source_object_reference_payload_error_v44)?;
            source_object_projected_reference_v44(plan, value, original, ty, copied, budget)
        })()
        .inspect_err(|error| source_reference_record_failure_v29(plan, error))
    }
}

fn source_object_reference_operand_v44(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    original: &SemanticPlaceV1,
    occurrence: ScopedMemoryOccurrenceV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(SemanticTypeIdV1, bool, SsaValueV1), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<(SemanticTypeIdV1, bool, SsaValueV1)>(plan, budget)?;
    budget.source_reference_charge_v29(plan, 10)?;
    let function = plan
        .instances
        .instance(instance)
        .ok_or_else(source_object_reference_payload_error_v44)?
        .declaration();
    let copied = match scoped_source_operand_v29(function, site, role) {
        Some(SemanticOperandV1::Copy(place)) if std::ptr::eq(place, original) => true,
        Some(SemanticOperandV1::Move(place)) if std::ptr::eq(place, original) => false,
        _ => return Err(source_object_reference_payload_error_v44()),
    };
    let occurrences = plan
        .instances
        .occurrences(instance)
        .ok_or_else(source_object_reference_payload_error_v44)?;
    check_scoped_payload_occurrence_v29(&occurrences, site, role, original, occurrence, budget)?;
    let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = occurrence else {
        return Err(source_object_reference_payload_error_v44());
    };
    let ty = function
        .locals()
        .get(original.local().index() as usize)
        .ok_or_else(source_object_reference_payload_error_v44)?
        .ty();
    Ok((ty, copied, definition))
}

fn source_object_projected_reference_v44<'binding>(
    plan: &SourceReferencePlanV29<'_, '_>,
    mut value: &'binding SemanticValueBindingV1,
    original: &SemanticPlaceV1,
    mut ty: SemanticTypeIdV1,
    copied: bool,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'binding SemanticSourceReferenceBindingV29, ProductionSemanticKirErrorV1> {
    source_reference_owned_prepay_v29::<(
        Option<u32>,
        SemanticTypeIdV1,
        Option<usize>,
        [&(); 8],
        [usize; 8],
    )>(plan, budget)?;
    let types = plan.instances.owner().source_semantic().types();
    let mut selected_variant = None;
    for projection in original.projections() {
        budget.source_reference_charge_v29(plan, 8)?;
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(source_object_reference_payload_error_v44)?;
        match (projection.kind(), declaration.shape(), value) {
            (
                SemanticProjectionKindV1::Downcast(index),
                SemanticTypeShapeV1::Enum { variants, .. },
                SemanticValueBindingV1::Enum {
                    semantic_type,
                    variant: Some(actual),
                    ..
                },
            ) if selected_variant.is_none()
                && *semantic_type == ty
                && *actual == index
                && projection.result_type() == ty
                && variants
                    .get(index as usize)
                    .is_some_and(|row| !row.is_uninhabited()) =>
            {
                selected_variant = Some(index);
                continue;
            }
            (
                SemanticProjectionKindV1::Field(index),
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                SemanticValueBindingV1::Aggregate(values),
            ) if selected_variant.is_none() && fields.fields().len() == values.len() => {
                ty = *fields
                    .fields()
                    .get(index as usize)
                    .ok_or_else(source_object_reference_payload_error_v44)?;
                value = values
                    .get(index as usize)
                    .ok_or_else(source_object_reference_payload_error_v44)?;
            }
            (
                SemanticProjectionKindV1::Field(index),
                SemanticTypeShapeV1::Enum { variants, .. },
                SemanticValueBindingV1::Enum {
                    semantic_type,
                    variant,
                    payloads,
                    ..
                },
            ) if *semantic_type == ty && *variant == selected_variant => {
                let selected = selected_variant
                    .take()
                    .ok_or_else(source_object_reference_payload_error_v44)?;
                let fields = variants
                    .get(selected as usize)
                    .ok_or_else(source_object_reference_payload_error_v44)?
                    .fields()
                    .fields();
                charge_execution_cfg_lookup_v29(payloads.len(), budget)?;
                let values = payloads
                    .get(&selected)
                    .ok_or_else(source_object_reference_payload_error_v44)?;
                if fields.len() != values.len() {
                    return Err(source_object_reference_payload_error_v44());
                }
                ty = *fields
                    .get(index as usize)
                    .ok_or_else(source_object_reference_payload_error_v44)?;
                value = values
                    .get(index as usize)
                    .ok_or_else(source_object_reference_payload_error_v44)?;
            }
            (
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                },
                SemanticTypeShapeV1::Array { element, length },
                SemanticValueBindingV1::Aggregate(values),
            ) if selected_variant.is_none()
                && u64::try_from(values.len()).ok() == Some(*length)
                && minimum_length <= *length =>
            {
                let index =
                    source_static_constant_index_v29(*length, offset, minimum_length, from_end)?;
                let index = usize::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                ty = *element;
                value = values
                    .get(index)
                    .ok_or_else(source_object_reference_payload_error_v44)?;
            }
            _ => return Err(source_object_reference_payload_error_v44()),
        }
        if projection.result_type() != ty {
            return Err(source_object_reference_payload_error_v44());
        }
    }
    let SemanticValueBindingV1::SourceReference(binding) = value else {
        return Err(source_object_reference_payload_error_v44());
    };
    if selected_variant.is_some() || ty != original.ty() || binding.source_type != ty {
        return Err(source_object_reference_payload_error_v44());
    }
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(source_object_reference_payload_error_v44());
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || (pointer.mutability() == SemanticMutabilityV1::Mutable && copied)
    {
        return Err(source_object_reference_payload_error_v44());
    }
    source_reference_validate_binding_v29(plan, binding, budget)?;
    Ok(binding)
}

fn source_object_reference_value_v44<'binding>(
    plan: &SourceReferencePlanV29<'_, '_>,
    binding: &'binding SemanticSourceReferenceBindingV29,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<&'binding ValueDef, ProductionSemanticKirErrorV1> {
    (|| {
    plan.check_owner(plan.instances, budget)?;
    source_reference_owned_prepay_v29::<(
        SourceReferenceBindingOriginV29,
        fe2o3_kernel_ir::StoragePointerV1,
        [&(); 8],
        [usize; 8],
    )>(plan, budget)?;
    source_reference_validate_binding_v29(plan, binding, budget)?;
    plan.charge(12, budget)?;
    let loan = binding.origin.single_loan()?;
    if !matches!(
        plan.cells.strategies.get(loan),
        Some(SourceReferenceCellStrategyV29::Object(_) | SourceReferenceCellStrategyV29::Scalar(_))
    ) {
        return Err(source_object_reference_payload_error_v44());
    }
    let owner = plan.instances.owner();
    let declaration = owner
        .source_semantic()
        .types()
        .get(binding.source_type.index() as usize)
        .ok_or_else(source_object_reference_payload_error_v44)?;
    let SemanticTypeShapeV1::Pointer(original) = declaration.shape() else {
        return Err(source_object_reference_payload_error_v44());
    };
    let [value] = binding.values.as_slice() else {
        return Err(source_object_reference_payload_error_v44());
    };
    let Type::Pointer(actual) = &value.ty else {
        return Err(source_object_reference_payload_error_v44());
    };
    if original.kind() != SemanticPointerKindV1::Reference
        || original.metadata() != SemanticPointerMetadataV1::None
        || actual.address_space != AddressSpace::Private
    {
        return Err(source_object_reference_payload_error_v44());
    }
    let layouts = plan
        .storage_root
        .as_ref()
        .ok_or_else(source_object_reference_payload_error_v44)?
        .source_layouts(plan.instances, budget)?;
    layouts.check_selected_schema(owner, binding.source_type, schema, budget)?;
    let rows = layouts.rows(owner, budget)?;
    let row = rows
        .get(schema.0 as usize)
        .ok_or_else(source_object_reference_payload_error_v44)?;
    let fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(selected) = row.kind else {
        return Err(source_object_reference_payload_error_v44());
    };
    let encoded = match original.address_space() {
        0 => AddressSpace::Generic,
        5 => AddressSpace::Private,
        _ => return Err(source_object_reference_payload_error_v44()),
    };
    let access = match original.mutability() {
        SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
        SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
    };
    let pointee = rows
        .get(selected.pointee.0 as usize)
        .ok_or_else(source_object_reference_payload_error_v44)?;
    let exact_pointee = actual.pointee.as_ref() == &Type::StorageObject(selected.pointee)
        || matches!((&pointee.kind, actual.pointee.as_ref()),
            (fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(left), Type::Scalar(right)) if left == right);
    if selected.value_space != actual.address_space
        || selected.encoded_space != encoded
        || selected.access != access
        || actual.access != access
        || selected.stored_bits != original.pointer_width_bits()
        || row.size.checked_mul(8) != Some(u64::from(selected.stored_bits))
        || declaration.layout().size_bytes() != Some(row.size)
        || declaration.layout().alignment_bytes() != u64::from(row.alignment)
        || !exact_pointee
    {
        return Err(source_object_reference_payload_error_v44());
    }
    Ok(value)
    })().inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
