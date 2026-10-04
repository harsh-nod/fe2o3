// These receipts authorize movement only. Compiler holders never become
// original objects; final closed memory and reference currentness still apply.
#[derive(Clone, Copy, Debug)]
enum ScopedStoragePayloadV59 {
    Object(ScopedObjectPayloadV29),
    CompilerEnum {
        record: ScopedCompilerEnumAccessV55,
        storage: CompilerEnumPointerStorageV57,
    },
}

impl ScopedStoragePayloadV59 {
    fn check_operation(
        &self,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match self {
            Self::Object(payload) => payload.check_operation(operation, budget),
            Self::CompilerEnum { record, storage } => {
                budget.charge_work(6)?;
                let ScopedCompilerEnumRoleV55::Store { value, .. } = record.role else {
                    return Err(scoped_object_error_v29());
                };
                if !operation.results.is_empty()
                    || operation.kind
                        != OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                            address: record.pointer,
                            value,
                            access: MemoryAccess::new(AddressSpace::Private, storage.alignment),
                        })
                {
                    return Err(scoped_object_error_v29());
                }
                Ok(())
            }
        }
    }
}

fn scoped_storage_compiler_headers_v59() -> Result<usize, ArgumentResourceV1> {
    source_reference_emission_headers_v29::<(
        ScopedStoragePayloadV59,
        [Option<(ValueId, ScopedStorageTypeV29)>; 2],
        CheckedScopedCompilerEnumAccessV55<'static>,
        CompilerEnumPointerStorageV57,
        [usize; 4],
        [&'static (); 8],
    )>()
}

fn scoped_storage_compiler_enum_v59(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    anchor_index: usize,
    operation: &Operation,
    index: &CallSpliceIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        ScopedStoragePayloadV59,
        [Option<(ValueId, ScopedStorageTypeV29)>; 2],
    ),
    ProductionSemanticKirErrorV1,
> {
    plan.check_owner(plan.instances, budget)?;
    budget.source_reference_reserve_v29(plan, scoped_storage_compiler_headers_v59()?)?;
    let anchors = lowered
        .scoped_memory_anchors
        .as_ref()
        .ok_or_else(scoped_object_error_v29)?;
    let archive = lowered
        .execution_observation
        .as_ref()
        .ok_or_else(scoped_object_error_v29)?;
    charge_execution_cfg_lookup_v29(anchors.compiler_enum.len(), budget)?;
    let ordinal = anchors
        .compiler_enum
        .binary_search_by_key(&anchor_index, |row| row.anchor)
        .map_err(|_| scoped_object_error_v29())?;
    let record = &anchors.compiler_enum[ordinal];
    let checked = check_scoped_compiler_enum_access_v55(
        plan.instances,
        instance,
        anchors,
        archive,
        record,
        operation,
        budget,
    )?;
    // Rejoin the genuine source operand and its whole correlated reference
    // tuple, not merely the compiler allocation or an anchor coordinate.
    check_scoped_compiler_enum_reference_plan_v59(plan, instance, archive, &checked, budget)?
        .ok_or_else(scoped_object_error_v29)?;
    let storage = checked.spill.storage.ok_or_else(scoped_object_error_v29)?;
    let layouts = plan
        .storage_root
        .as_ref()
        .ok_or_else(scoped_object_error_v29)?
        .source_layouts(plan.instances, budget)?;
    layouts.check_selected_schema(
        plan.instances.owner(),
        checked.spill.field_type,
        storage.schema,
        budget,
    )?;
    let rows = layouts.rows(plan.instances.owner(), budget)?;
    check_enum_spill_layout_v57(checked.spill, &rows, budget)?;
    let ScopedCompilerEnumRoleV55::Store { value, .. } = record.role else {
        return Err(scoped_object_error_v29());
    };
    let inputs = [
        Some((
            record.pointer,
            ScopedStorageTypeV29::Pointer(
                storage.schema,
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        )),
        Some((
            value,
            ScopedStorageTypeV29::Pointer(
                storage.pointer.pointee,
                storage.pointer.value_space,
                storage.pointer.access,
            ),
        )),
    ];
    for (value, ty) in inputs.iter().flatten() {
        budget.charge_work(1)?;
        if !ty.matches(
            index
                .value(*value, budget)
                .map_err(source_address_call_error_v29)?,
        ) {
            return Err(scoped_object_error_v29());
        }
    }
    let payload = ScopedStoragePayloadV59::CompilerEnum {
        record: *record,
        storage,
    };
    payload.check_operation(operation, budget)?;
    Ok((payload, inputs))
}
