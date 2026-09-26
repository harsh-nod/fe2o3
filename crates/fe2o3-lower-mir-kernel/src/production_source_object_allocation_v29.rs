// Allocation identity is independent of value availability and initialization.
include!("production_source_object_emission_v29.rs");
include!("production_source_array_allocation_v29.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedAllocationIdentityV29 {
    LegacyLocal(u32),
    OriginalObject { local: u32, generation: u32 },
    OperandSnapshot { site: ExecutionSiteV29, role: ExecutionOperandV29, ordinal: u32 },
    CallResultDestination { site: ExecutionSiteV29, ordinal: u32 },
    ReturnDestination { site: ExecutionSiteV29, ordinal: u32 },
    EntryValue { local: u32, argument: u32 },
}

impl ScopedAllocationIdentityV29 {
    fn legacy_local(self) -> Result<u32, ProductionSemanticKirErrorV1> {
        match self {
            Self::LegacyLocal(local) => Ok(local),
            Self::OriginalObject { .. } | Self::OperandSnapshot { .. }
            | Self::CallResultDestination { .. } | Self::ReturnDestination { .. }
            | Self::EntryValue { .. } => Err(scoped_object_allocation_error_v29()),
        }
    }

    fn original_local(self) -> Option<u32> {
        match self {
            Self::LegacyLocal(local) | Self::OriginalObject { local, .. } => Some(local),
            Self::OperandSnapshot { .. } | Self::CallResultDestination { .. }
            | Self::ReturnDestination { .. } | Self::EntryValue { .. } => None,
        }
    }

    fn key(self) -> (u8, u32, u32, Option<(u32, Option<u32>)>, Option<(u8, u32)>) {
        match self {
            Self::LegacyLocal(local) => (0, local, 0, None, None),
            Self::OriginalObject { local, generation } => (1, local, generation, None, None),
            Self::OperandSnapshot { site, role, ordinal } =>
                (2, ordinal, 0, Some(scoped_memory_site_key_v29(site)), Some(scoped_allocation_role_key_v29(role))),
            Self::CallResultDestination { site, ordinal } =>
                (3, ordinal, 0, Some(scoped_memory_site_key_v29(site)), None),
            Self::ReturnDestination { site, ordinal } =>
                (4, ordinal, 0, Some(scoped_memory_site_key_v29(site)), None),
            Self::EntryValue { local, argument } => (5, local, argument, None, None),
        }
    }
}

impl Ord for ScopedAllocationIdentityV29 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.key().cmp(&other.key()) }
}

impl PartialOrd for ScopedAllocationIdentityV29 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

fn scoped_allocation_role_key_v29(role: ExecutionOperandV29) -> (u8, u32) {
    match role {
        ExecutionOperandV29::RvalueOperand(index) => (0, index),
        ExecutionOperandV29::RvaluePlace => (1, 0),
        ExecutionOperandV29::Destination => (2, 0),
        ExecutionOperandV29::StoreValue => (3, 0),
        ExecutionOperandV29::StoreDestination => (4, 0),
        ExecutionOperandV29::AtomicAddress => (5, 0),
        ExecutionOperandV29::AtomicValue => (6, 0),
        ExecutionOperandV29::AtomicExpected => (7, 0),
        ExecutionOperandV29::AtomicReplacement => (8, 0),
        ExecutionOperandV29::AtomicDestination => (9, 0),
        ExecutionOperandV29::StatementPlace => (10, 0),
        ExecutionOperandV29::Assume => (11, 0),
        ExecutionOperandV29::StorageLive => (12, 0),
        ExecutionOperandV29::StorageDead => (13, 0),
        ExecutionOperandV29::CallArgument(index) => (14, index),
        ExecutionOperandV29::CallDestinationAddress => (15, 0),
        ExecutionOperandV29::TailCallArgument(index) => (16, index),
        ExecutionOperandV29::SwitchDiscriminant => (17, 0),
        ExecutionOperandV29::DropPlace => (18, 0),
        ExecutionOperandV29::AssertCondition => (19, 0),
        ExecutionOperandV29::AssertMessage(index) => (20, index),
        ExecutionOperandV29::ReturnValue => (21, 0),
        ExecutionOperandV29::ElidedBorrowDestination => (22, 0),
    }
}

#[derive(Clone, Debug)]
enum SemanticRetainedStorageV29 {
    ScalarArray {
        kernel_type: Type,
        alignment: u32,
        array: Option<SemanticRetainedArrayLayoutV1>,
    },
    Object { cell: usize, schema: fe2o3_kernel_ir::StorageLayoutIdV1, bytes: u64, alignment: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedAllocationSourceV29 {
    Legacy,
    OriginalArray { schema: fe2o3_kernel_ir::StorageLayoutIdV1 },
    OriginalObject { cell: usize, schema: fe2o3_kernel_ir::StorageLayoutIdV1 },
}

impl SemanticRetainedStorageV29 {
    fn scalar_array(&self) -> Result<(&Type, u32, Option<SemanticRetainedArrayLayoutV1>), ProductionSemanticKirErrorV1> {
        match self {
            Self::ScalarArray { kernel_type, alignment, array } => Ok((kernel_type, *alignment, *array)),
            Self::Object { .. } => Err(scoped_object_allocation_error_v29()),
        }
    }

    fn value_type(&self) -> Type {
        match self {
            Self::ScalarArray { kernel_type, .. } => kernel_type.clone(),
            Self::Object { schema, .. } => Type::StorageObject(*schema),
        }
    }

    fn alignment(&self) -> u32 {
        match self {
            Self::ScalarArray { alignment, .. } | Self::Object { alignment, .. } => *alignment,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedScalarArraySlotV29 {
    element_type: SemanticTypeIdV1,
    element: PrivateRetainedSlotFactsV1,
    length: u64,
    bytes: u64,
    count: Option<(ValueId, PrivateArrayPhysicalLocationV1)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedSlotRepresentationV29 {
    ScalarArray(ScopedScalarArraySlotV29),
    Object { schema: fe2o3_kernel_ir::StorageLayoutIdV1, bytes: u64, alignment: u32 },
}

impl ScopedSlotRepresentationV29 {
    fn scalar_array(self) -> Result<ScopedScalarArraySlotV29, ProductionSemanticKirErrorV1> {
        match self {
            Self::ScalarArray(row) => Ok(row),
            Self::Object { .. } => Err(scoped_object_allocation_error_v29()),
        }
    }

    fn count(self) -> Option<(ValueId, PrivateArrayPhysicalLocationV1)> {
        match self {
            Self::ScalarArray(row) => row.count,
            Self::Object { .. } => None,
        }
    }

    fn bytes(self) -> u64 {
        match self {
            Self::ScalarArray(row) => row.bytes,
            Self::Object { bytes, .. } => bytes,
        }
    }
}

impl ScopedSlotOriginV29 {
    fn legacy_local(self) -> Result<u32, ProductionSemanticKirErrorV1> {
        if !matches!(self.source, ScopedAllocationSourceV29::Legacy | ScopedAllocationSourceV29::OriginalArray { .. }) {
            return Err(scoped_object_allocation_error_v29());
        }
        self.identity.legacy_local()
    }
}

impl ScopedSourceSlotV29 {
    fn scalar_array(self) -> Result<ScopedScalarArraySlotV29, ProductionSemanticKirErrorV1> {
        self.origin.identity.legacy_local()?;
        if !matches!(self.origin.source, ScopedAllocationSourceV29::Legacy | ScopedAllocationSourceV29::OriginalArray { .. }) {
            return Err(scoped_object_allocation_error_v29());
        }
        let scalar = self.representation.scalar_array()?;
        if matches!(self.origin.source, ScopedAllocationSourceV29::OriginalArray { .. })
            && scalar.count.is_none()
        { return Err(scoped_object_allocation_error_v29()); }
        Ok(scalar)
    }

    fn legacy_local(self) -> Result<u32, ProductionSemanticKirErrorV1> {
        self.scalar_array()?;
        self.origin.identity.legacy_local()
    }
}

fn scoped_object_allocation_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, "typed allocation identity or representation requires its exact source contract")
}

fn clone_retained_storage_v29(
    storage: &SemanticRetainedStorageV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticRetainedStorageV29, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    Ok(match storage {
        SemanticRetainedStorageV29::ScalarArray { kernel_type, alignment, array } =>
            SemanticRetainedStorageV29::ScalarArray {
                kernel_type: emission_binding_clone_type_v1(kernel_type, budget)?,
                alignment: *alignment, array: *array,
            },
        SemanticRetainedStorageV29::Object { cell, schema, bytes, alignment } =>
            SemanticRetainedStorageV29::Object {
                cell: *cell, schema: *schema, bytes: *bytes, alignment: *alignment,
            },
    })
}

fn clone_retained_local_slots_v29(
    slots: &BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>, ProductionSemanticKirErrorV1> {
    type Slots = BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>;
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<Slots>(),
        argument_product_v1(2, std::mem::size_of::<Result<Slots, ProductionSemanticKirErrorV1>>())?,
    ])?)?;
    let mut output = BTreeMap::new();
    for (&identity, slot) in slots {
        reserve_execution_cfg_map_entry_v29::<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>(output.len(), budget)?;
        budget.charge_work(3)?;
        let storage = clone_retained_storage_v29(&slot.storage, budget)?;
        output.insert(identity, SemanticRetainedLocalSlotV1 {
            pointer: slot.pointer, semantic_type: slot.semantic_type, storage,
        });
    }
    Ok(output)
}

fn lookup_legacy_retained_slot_v29<'a>(
    slots: &'a BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>,
    local: u32,
    budget: Option<&mut dyn SemanticEmissionBudgetV1>,
) -> Result<Option<&'a SemanticRetainedLocalSlotV1>, ProductionSemanticKirErrorV1> {
    if let Some(budget) = budget {
        charge_execution_cfg_lookup_v29(slots.len(), budget)?;
        charge_execution_cfg_lookup_v29(slots.len(), budget)?;
        budget.charge_work(2)?;
    }
    let objects = ScopedAllocationIdentityV29::OriginalObject { local, generation: 0 }
        ..=ScopedAllocationIdentityV29::OriginalObject { local, generation: u32::MAX };
    if slots.range(objects).next().is_some() {
        return Err(scoped_object_allocation_error_v29());
    }
    let slot = slots.get(&ScopedAllocationIdentityV29::LegacyLocal(local));
    if let Some(slot) = slot { slot.storage.scalar_array()?; }
    Ok(slot)
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn legacy_retained_slot_v29(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<Option<&SemanticRetainedLocalSlotV1>, ProductionSemanticKirErrorV1> {
        let plan = self.execution.as_ref().and_then(|cursor| cursor.references).map(|references| references.plan);
        if let Some(plan) = plan {
            self.emission_work.as_deref_mut().ok_or(ArgumentResourceV1::Accounting)?
                .source_reference_owner_v29(plan)?;
        }
        match self.emission_work.as_mut() {
            Some(budget) => lookup_legacy_retained_slot_v29(
                &self.retained_local_slots, local.index(), Some(&mut **budget),
            ),
            None => lookup_legacy_retained_slot_v29(&self.retained_local_slots, local.index(), None),
        }
    }
}

fn check_scoped_object_alloca_v29(
    operation: &Operation,
    pointer: ValueId,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    alignment: u32,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(9)?;
    let [result] = operation.results.as_slice() else {
        return Err(scoped_object_allocation_error_v29());
    };
    if result.id != pointer || !matches!(&result.ty,
        Type::Pointer(pointer) if pointer.address_space == AddressSpace::Private
            && pointer.access == AccessMode::ReadWrite
            && pointer.pointee.as_ref() == &Type::StorageObject(schema))
        || !matches!(&operation.kind, OperationKind::Alloca {
            element: Type::StorageObject(actual_schema), count: None,
            address_space: AddressSpace::Private, alignment: actual_alignment,
        } if *actual_schema == schema && *actual_alignment == alignment)
    { return Err(scoped_object_allocation_error_v29()); }
    Ok(())
}

fn check_scoped_slot_alloca_v29(
    slot: &ScopedSourceSlotV29,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    match (slot.origin.identity, slot.origin.source, slot.representation) {
        (ScopedAllocationIdentityV29::LegacyLocal(_), ScopedAllocationSourceV29::Legacy | ScopedAllocationSourceV29::OriginalArray { .. },
         ScopedSlotRepresentationV29::ScalarArray(scalar)) => {
            slot.scalar_array()?;
            private_retained_check_allocation_operation_v1(
                operation, slot.origin.pointer, scalar.count.map(|row| row.0), scalar.element, budget,
            ).map_err(scoped_slot_relation_error_v29)
        }
        (ScopedAllocationIdentityV29::OriginalObject { .. },
         ScopedAllocationSourceV29::OriginalObject { schema: original, .. },
         ScopedSlotRepresentationV29::Object { schema, alignment, .. }) if original == schema => {
            check_scoped_object_alloca_v29(operation, slot.origin.pointer, schema, alignment, budget)
        }
        _ => Err(scoped_object_allocation_error_v29()),
    }
}

include!("production_source_object_lane_v29.rs");

#[allow(clippy::too_many_arguments)]
fn source_object_storage_matches_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    cell: usize,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    extent: Option<(u64, u32)>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    // Authenticate before the scratch entry debit. The original query is
    // immutable; its representation and return envelopes die inside this unit
    // scope. Allocation-plan consumers retain the separate owned query below.
    plan.check_owner(plan.instances, budget)?;
    let mut matched = false;
    with_canonical_call_scratch_v1(budget, |budget| {
        let checked = source_object_storage_v29(plan, cell, instance, local, generation, schema, budget)?;
        budget.charge_work(4)?;
        matched = matches!(checked, SemanticRetainedStorageV29::Object {
            cell: actual_cell, schema: actual_schema, bytes, alignment,
        } if actual_cell == cell && actual_schema == schema
            && extent.is_none_or(|expected| expected == (bytes, alignment)));
        Ok(())
    }).inspect_err(|error| source_reference_record_failure_v29(plan, error))?;
    Ok(matched)
}

#[allow(clippy::too_many_arguments)]
fn source_object_storage_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    cell: usize,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticRetainedStorageV29, ProductionSemanticKirErrorV1> {
    let result = (|| {
        plan.check_owner(plan.instances, budget)?;
        source_reference_owned_prepay_v29::<SemanticRetainedStorageV29>(plan, budget)?;
        plan.charge(12, budget)?;
        let row = plan.cells.rows.get(cell).ok_or_else(scoped_object_allocation_error_v29)?;
        let original = plan.instances.instance(instance)
            .ok_or_else(scoped_object_allocation_error_v29)?;
        let declaration = original.declaration().locals().get(local.index() as usize)
            .ok_or_else(scoped_object_allocation_error_v29)?;
        if plan.instances.instance_reachable(instance) != Some(true)
            || !plan.has_storage_demands || plan.storage_demands.is_none()
            || row.instance != instance || row.local != local || row.generation != generation
            || row.ty != declaration.ty() || row.kind != SourceBackingKindV29::Object(schema)
        { return Err(scoped_object_allocation_error_v29()); }
        let layouts = plan.storage_root.as_ref()
            .ok_or_else(scoped_object_allocation_error_v29)?
            .source_layouts(plan.instances, budget)?;
        layouts.check_selected_schema(plan.instances.owner(), row.ty, schema, budget)?;
        let physical = layouts.rows(plan.instances.owner(), budget)?;
        plan.charge(3, budget)?;
        let selected = physical.get(schema.0 as usize)
            .ok_or_else(scoped_object_allocation_error_v29)?;
        if selected.alignment == 0 || !selected.alignment.is_power_of_two() {
            return Err(scoped_object_allocation_error_v29());
        }
        Ok(SemanticRetainedStorageV29::Object {
            cell, schema, bytes: selected.size, alignment: selected.alignment,
        })
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

impl SourceFunctionBackingViewV29<'_> {
    fn visit_object_allocations(
        self,
        private_candidates: &BTreeSet<u32>,
        budget: &mut dyn SemanticEmissionBudgetV1,
        mut visit: impl FnMut(
            ScopedAllocationIdentityV29,
            SemanticRetainedLocalSlotPlanV1,
            &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.layouts.check(budget)?;
        let plan = self.layouts.references;
        charge_execution_cfg_lookup_v29(self.layouts.backing.len(), budget)?;
        let range = (self.instance.index(), 0, 0)..=(self.instance.index(), u32::MAX, u32::MAX);
        for (&(_, local, generation), &backing) in self.layouts.backing.range(range) {
            let ordinal = backing.cell;
            budget.source_reference_charge_v29(plan, 3)?;
            let local = SemanticLocalIdV1::from_index(local);
            let (checked_ordinal, cell) = self.cell(local, generation, budget)?
                .ok_or_else(scoped_object_allocation_error_v29)?;
            if checked_ordinal != ordinal { return Err(scoped_object_allocation_error_v29()); }
            charge_execution_cfg_lookup_v29(private_candidates.len(), budget)?;
            if backing.array.is_some() && private_candidates.contains(&local.index()) { continue; }
            let schema = match cell.kind {
                SourceBackingKindV29::Scalar => continue,
                SourceBackingKindV29::Object(schema) => schema,
            };
            let storage = budget.source_object_storage_v29(
                plan, ordinal, self.instance, local, generation, schema,
            )?;
            let (representative, physical, _) = plan.physical_object_cell(ordinal, budget)?;
            if physical.instance != self.instance || physical.local != local
                || physical.ty != cell.ty || physical.kind != cell.kind
            { return Err(scoped_object_allocation_error_v29()); }
            // Every logical cell above is authenticated. Only the unique
            // checked representative publishes the physical allocation.
            if representative != ordinal { continue; }
            source_reference_owned_prepay_v29::<SemanticRetainedLocalSlotPlanV1>(plan, budget)?;
            visit(
                ScopedAllocationIdentityV29::OriginalObject { local: local.index(), generation },
                SemanticRetainedLocalSlotPlanV1 { semantic_type: cell.ty, storage },
                budget,
            )?;
        }
        Ok(())
    }
}
