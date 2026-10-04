// This retained claim permits only a checked constructor Store operand. It
// does not authorize a payload Load, dereference, or reference reconstruction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceCompilerEnumReferenceV55 {
    loan: SourceSsaLoanV36,
    slot: usize,
    backing: SourceCompilerEnumReferenceBackingV55,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceCompilerEnumReferenceBackingV55 {
    Scalar,
    Object(fe2o3_kernel_ir::StorageLayoutIdV1),
}

fn retain_compiler_enum_reference_v55(
    references: &SourceReferenceEmissionV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    archive: &ExecutionArchiveV29,
    checked: &CheckedScopedCompilerEnumAccessV55<'_>,
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceCompilerEnumReferenceV55>, ProductionSemanticKirErrorV1> {
    let plan = references.plan;
    references.check(budget)?;
    let result = (|| {
        source_reference_owned_prepay_v29::<(
            Option<SourceCompilerEnumReferenceV55>,
            SourceSsaPhysicalV36,
            SourceReferenceScalarCellV29,
            [&(); 8],
        )>(plan, budget)?;
        let Some(original) = check_scoped_compiler_enum_reference_store_v55(
            references, instance, archive, checked, budget,
        )?
        else {
            return Ok(None);
        };
        let loan = original.origin.single_loan()?;
        let (_, cell) = plan
            .backing_cell(loan, budget)?
            .ok_or_else(source_enum_tag_error_v55)?;
        budget.source_reference_charge_v29(plan, 12)?;
        let SemanticValueBindingV1::Enum { payloads, .. } = checked.binding else {
            return Err(source_enum_tag_error_v55());
        };
        charge_execution_cfg_lookup_v29(payloads.len(), budget)?;
        let field = payloads
            .get(&checked.spill.variant)
            .and_then(|fields| fields.get(checked.spill.field as usize))
            .ok_or_else(source_enum_tag_error_v55)?;
        let SourceSsaPhysicalV36::Value {
            value,
            ty: SourceSsaCarrierTypeV36::Pointer { .. },
            loan: Some(retained),
        } = retain_source_carrier_leaf_v37(
            plan.instances,
            references,
            checked.spill.field_type,
            field,
            budget,
        )?
        else {
            return Err(source_enum_tag_error_v55());
        };
        let ScopedCompilerEnumRoleV55::Store { value: actual, .. } = checked.record.role else {
            return Err(source_enum_tag_error_v55());
        };
        let Type::Pointer(pointer) = &checked.spill.element else {
            return Err(source_enum_tag_error_v55());
        };
        if value != actual
            || retained.loan != loan
            || retained.origin_instance != cell.instance
            || retained.origin_local != cell.local
            || retained.origin_generation != cell.generation
            || retained.origin_type != cell.ty
            || retained.carrier != ProductionSourceReferenceCarrierV38::MemoryPointer
            || !plan.origins[retained.origin].projections.is_empty()
        {
            return Err(source_enum_tag_error_v55());
        }
        let (slot, backing) = match cell.kind {
            SourceBackingKindV29::Scalar => {
                if !matches!(
                    pointer.address_space,
                    AddressSpace::Private | AddressSpace::Generic
                ) || !matches!(*pointer.pointee, Type::Scalar(_))
                {
                    return Err(source_enum_tag_error_v55());
                }
                let slot = source_address_original_slot_v29(
                    plan.instances,
                    slots,
                    cell.instance,
                    cell.local,
                    cell.ty,
                    budget,
                )?;
                let scalar = slots
                    .slots
                    .get(slot)
                    .ok_or_else(source_enum_tag_error_v55)?
                    .scalar_array()?;
                if scalar.length != 1
                    || !scalar
                        .element
                        .element
                        .matches_borrowed(&pointer.pointee, budget)?
                {
                    return Err(source_enum_tag_error_v55());
                }
                (slot, SourceCompilerEnumReferenceBackingV55::Scalar)
            }
            SourceBackingKindV29::Object(schema) => {
                if pointer.address_space != AddressSpace::Private
                    || pointer.pointee.as_ref() != &Type::StorageObject(schema)
                {
                    return Err(source_enum_tag_error_v55());
                }
                let slot = source_address_object_slot_v29(
                    plan.instances,
                    plan,
                    slots,
                    cell.instance,
                    cell.local,
                    cell.generation,
                    cell.ty,
                    budget,
                )?;
                let owner = plan.instances.owner();
                let layouts = plan
                    .storage_root
                    .as_ref()
                    .ok_or_else(source_enum_tag_error_v55)?
                    .source_layouts(plan.instances, budget)?;
                layouts.check_selected_schema(owner, cell.ty, schema, budget)?;
                let layouts = layouts.rows(owner, budget)?;
                let layout = layouts
                    .get(schema.0 as usize)
                    .ok_or_else(source_enum_tag_error_v55)?;
                let fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(scalar) = layout.kind else {
                    return Err(source_reference_error_v29(
                        "compiler enum reference spill requires a checked scalar referent schema",
                    ));
                };
                if lower_scalar_type(owner.source_semantic().types(), cell.ty)?
                    != Type::Scalar(scalar)
                {
                    return Err(source_enum_tag_error_v55());
                }
                (slot, SourceCompilerEnumReferenceBackingV55::Object(schema))
            }
        };
        Ok(Some(SourceCompilerEnumReferenceV55 {
            loan: retained,
            slot,
            backing,
        }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
