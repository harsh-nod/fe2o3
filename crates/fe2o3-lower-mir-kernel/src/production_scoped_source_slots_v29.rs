// Allocation provenance only: lifetime, initialized reads and relocation need
// independent checks before helper-frame storage may be removed or reused.
include!("production_source_object_allocation_v29.rs");

#[cfg(test)]
type ScopedSlotObserverV29 = fn(
    &ExecutionLifecycleSourceV29<'_>,
    &ExecutionInstancesV29<'_>,
    &mut [Option<LoweredFunctionResultV1>],
    &OwnedScopedSourceSlotsV29,
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
type ScopedSlotCustodyObserverV29 = fn(
    &ExecutionInstancesV29<'_>,
    &[Option<LoweredFunctionResultV1>],
    &mut OwnedScopedSourceSlotsV29,
    Option<&SourceReferenceEmissionV29<'_, '_>>,
    Option<&ExecutionIdentityPlanV1<'_, '_>>,
    usize,
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
type ScopedObjectOriginMutationV29 = fn(
    &[ScopedSlotOriginV29],
    &mut ArgumentBudgetV1<'_>,
) -> Result<Option<Vec<ScopedSlotOriginV29>>, ProductionSemanticKirErrorV1>;

#[cfg(test)]
thread_local! {
    static SCOPED_SLOT_OBSERVER_V29: std::cell::Cell<Option<ScopedSlotObserverV29>> = const { std::cell::Cell::new(None) };
    static SCOPED_SLOT_CUSTODY_OBSERVER_V29: std::cell::Cell<Option<ScopedSlotCustodyObserverV29>> = const { std::cell::Cell::new(None) };
    static SCOPED_OBJECT_ORIGIN_MUTATION_V29: std::cell::Cell<Option<ScopedObjectOriginMutationV29>> = const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedSlotOriginV29 {
    identity: ScopedAllocationIdentityV29,
    source: ScopedAllocationSourceV29,
    semantic_type: SemanticTypeIdV1,
    pointer: ValueId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedSourceSlotV29 {
    instance: ProductionCallInstanceIdV1,
    origin: ScopedSlotOriginV29,
    representation: ScopedSlotRepresentationV29,
    allocation: PrivateArrayPhysicalLocationV1,
}

#[derive(Debug, Eq, PartialEq)]
struct ScopedSourceSlotInstanceV29 {
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    incoming: Option<ProductionCallOccurrenceV1>,
    placement: SemanticEmissionPlacementV1,
    slots: std::ops::Range<usize>,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped storage transport remains gated")
)]
struct OwnedScopedSourceSlotsV29 {
    source: ExecutionCallSourceV29,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    instances: Vec<ScopedSourceSlotInstanceV29>,
    slots: Vec<ScopedSourceSlotV29>,
    pending_memory: Option<scoped_raw_admission_v29::PendingSourceMemoryV29>,
    retained_storage: usize,
}

impl PrivateArrayChargeV1 for ArgumentBudgetV1<'_> {
    type Error = ProductionSemanticKirErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.charge_work(amount).map_err(Into::into)
    }
}

fn scoped_slot_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "scoped source-slot allocation census is incomplete or mismatched",
    )
}

fn scoped_slot_relation_error_v29(
    error: PrivateArrayRelationErrorV1<ProductionSemanticKirErrorV1>,
) -> ProductionSemanticKirErrorV1 {
    match error {
        PrivateArrayRelationErrorV1::Work(error) => error,
        _ => scoped_slot_error_v29(),
    }
}

fn scoped_slot_attempt_v29<T>(
    budget: &mut ArgumentBudgetV1<'_>,
    build: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> Result<T, ProductionSemanticKirErrorV1>,
) -> Result<T, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| build(budget)));
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            Err(error)
        }
        Err(payload) => {
            if let Some(bytes) = budget.storage().checked_sub(floor) {
                let _ = budget.release_storage(bytes);
            }
            std::panic::resume_unwind(payload)
        }
    }
}

fn capture_scoped_slot_origins_v29(
    slots: &BTreeMap<ScopedAllocationIdentityV29, SemanticRetainedLocalSlotV1>,
    backing: Option<SourceFunctionBackingViewV29<'_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ScopedSlotOriginV29>, ProductionSemanticKirErrorV1> {
    scoped_slot_attempt_v29(budget, |budget| {
        let mut rows = emission_vec_v1(slots.len(), budget)?;
        budget.charge_work(argument_product_v1(slots.len(), 6)?)?;
        for (&identity, slot) in slots {
            let source = match (identity, &slot.storage) {
                (ScopedAllocationIdentityV29::LegacyLocal(local), SemanticRetainedStorageV29::ScalarArray { array, .. }) => {
                    match backing.map(|backing| backing.array_schema(local, budget)).transpose()?.flatten() {
                        Some(schema) if array.is_some() => ScopedAllocationSourceV29::OriginalArray { schema },
                        Some(_) => return Err(scoped_object_allocation_error_v29()),
                        None => ScopedAllocationSourceV29::Legacy,
                    }
                }
                (ScopedAllocationIdentityV29::OriginalObject { .. }, SemanticRetainedStorageV29::Object { cell, schema, .. }) =>
                    ScopedAllocationSourceV29::OriginalObject { cell: *cell, schema: *schema },
                _ => return Err(scoped_object_allocation_error_v29()),
            };
            rows.push(ScopedSlotOriginV29 {
                identity,
                source,
                semantic_type: slot.semantic_type,
                pointer: slot.pointer,
            });
        }
        Ok(rows)
    })
}

fn scoped_slot_candidates_v29(
    function: &SemanticFunctionDeclV1,
    ssa: &ProductionSemanticSsaFunctionPlanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<u8>, ProductionSemanticKirErrorV1> {
    let mut flags = emission_vec_v1(function.locals().len(), budget)?;
    budget.charge_work(function.locals().len())?;
    flags.resize(function.locals().len(), 0);
    budget.charge_work(ssa.plan().promoted_variables().len())?;
    for local in ssa.plan().promoted_variables() {
        let flag = flags
            .get_mut(local.get() as usize)
            .ok_or_else(scoped_slot_error_v29)?;
        if *flag != 0 {
            return Err(scoped_slot_error_v29());
        }
        *flag = 1;
    }
    budget.charge_work(ssa.retained_cross_edge_variables().len())?;
    for local in ssa.retained_cross_edge_variables() {
        let flag = flags
            .get_mut(local.get() as usize)
            .ok_or_else(scoped_slot_error_v29)?;
        if *flag != 1 {
            *flag = 2;
        }
    }
    visit_private_slot_roots_v1(function, budget, |local, budget| {
        budget.charge_work(2)?;
        let flag = flags
            .get_mut(local as usize)
            .ok_or_else(scoped_slot_error_v29)?;
        if *flag != 1 {
            *flag = 2;
        }
        Ok(())
    })?;
    Ok(flags)
}

fn scoped_slot_prologue_v29<'a>(
    lowered: &'a LoweredFunctionResultV1,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<&'a SemanticKirSyntheticOperationSpanV1>, ProductionSemanticKirErrorV1> {
    budget.charge_work(lowered.synthetic_operation_spans.len())?;
    let mut spans = lowered
        .synthetic_operation_spans
        .iter()
        .filter(|span| span.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage);
    let span = spans.next();
    if spans.next().is_some() {
        return Err(scoped_slot_error_v29());
    }
    if let Some(span) = span {
        budget.charge_work(5)?;
        if span.correspondence_owner != root
            || span.semantic_function != function
            || span.kernel_ir_block != block
            || span.first_operation_ordinal != 0
            || span.operation_count == 0
        {
            return Err(scoped_slot_error_v29());
        }
    }
    Ok(span)
}

// Coordinates are derived from the original declarations and allocation
// roster. A yielded location still needs the independent ABI-bound Store check.
fn visit_scoped_slot_initializers_v29(
    instances: &ExecutionInstancesV29<'_>,
    id: ProductionCallInstanceIdV1,
    entry: BlockId,
    slots: &[ScopedSourceSlotV29],
    budget: &mut ArgumentBudgetV1<'_>,
    mut visit: impl FnMut(
        usize,
        &ScopedSourceSlotV29,
        PrivateArrayPhysicalLocationV1,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(u32, u32), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let declaration = instances
        .instance(id)
        .ok_or_else(scoped_slot_error_v29)?
        .declaration();
    let mut cursor = 0_usize;
    let mut previous = None;
    for slot in slots {
        budget.charge_work(9)?;
        let local_index = slot.origin.identity.original_local().ok_or_else(scoped_slot_error_v29)?;
        let local = declaration
            .locals()
            .get(local_index as usize)
            .ok_or_else(scoped_slot_error_v29)?;
        if slot.instance != id
            || local.ty() != slot.origin.semantic_type
            || previous.is_some_and(|identity| identity >= slot.origin.identity)
        {
            return Err(scoped_slot_error_v29());
        }
        previous = Some(slot.origin.identity);
        let mut location = PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block: entry,
            operation: cursor,
        };
        if let Some((_, count)) = slot.representation.count() {
            if count != location {
                return Err(scoped_slot_error_v29());
            }
            cursor = argument_sum_v1(&[cursor, 1])?;
            location.operation = cursor;
        }
        if slot.allocation != location {
            return Err(scoped_slot_error_v29());
        }
        cursor = argument_sum_v1(&[cursor, 1])?;
    }
    let allocations = u32::try_from(cursor).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let mut initializers = 0_u32;
    for (index, slot) in slots.iter().enumerate() {
        budget.charge_work(4)?;
        let local_index = slot.origin.identity.original_local().ok_or_else(scoped_slot_error_v29)?;
        let local = &declaration.locals()[local_index as usize];
        if !local.role().is_entry_argument() {
            continue;
        }
        if slot.origin.identity.legacy_local()? != local_index
            || slot.representation.scalar_array()?.count.is_some()
        {
            return Err(scoped_slot_error_v29());
        }
        visit(
            index,
            slot,
            PrivateArrayPhysicalLocationV1 {
                block_ordinal: 0,
                block: entry,
                operation: cursor,
            },
            budget,
        )?;
        cursor = argument_sum_v1(&[cursor, 1])?;
        initializers = initializers
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    allocations
        .checked_add(initializers)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    Ok((allocations, initializers))
}

#[allow(clippy::too_many_arguments)]
fn scoped_array_slot_v29(
    lowered: &LoweredFunctionResultV1,
    body: &FunctionBody,
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    origin: ScopedSlotOriginV29,
    array: PrivateRetainedArrayFactsV1,
    index: usize,
    location: PrivateArrayPhysicalLocationV1,
    placement: SemanticEmissionPlacementV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(ValueId, PrivateArrayPhysicalLocationV1), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let recorded = lowered
        .private_arrays
        .slots
        .get(index)
        .ok_or_else(scoped_slot_error_v29)?;
    let allocation = PrivateArrayPhysicalLocationV1 {
        operation: location
            .operation
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?,
        ..location
    };
    if !lowered.private_arrays.active
        || lowered.private_arrays.placement != placement
        || recorded.owner != root
        || recorded.function != function
        || recorded.local != origin.identity.legacy_local()?
        || recorded.semantic_type != origin.semantic_type
        || recorded.pointer != origin.pointer
        || recorded.length != array.length
        || recorded.element_type != array.element_type
        || recorded.count_location != location
        || recorded.alloca_location != allocation
        || !private_array_slot_facts_equal_v1(recorded.element_facts, array.element, budget)?
    {
        return Err(scoped_slot_error_v29());
    }
    let count = private_array_unsigned_operation_v1(
        body,
        location,
        recorded.count,
        ScalarType::Index,
        budget,
    )
    .map_err(scoped_slot_relation_error_v29)?;
    if count != array.length {
        return Err(scoped_slot_error_v29());
    }
    Ok((recorded.count, location))
}

#[allow(clippy::too_many_arguments)]
fn scoped_object_slot_representation_v29(
    references: &SourceReferencePlanV29<'_, '_>,
    cell: usize,
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    generation: u32,
    schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedSlotRepresentationV29, ProductionSemanticKirErrorV1> {
    references.check_owner(references.instances, budget)?;
    let mut representation = None;
    with_canonical_call_scratch_v1(budget, |budget| {
        let selected = source_object_storage_v29(
            references, cell, instance, local, generation, schema, budget,
        )?;
        let SemanticRetainedStorageV29::Object { bytes, alignment, .. } = selected else {
            return Err(scoped_object_allocation_error_v29());
        };
        representation = Some(ScopedSlotRepresentationV29::Object { schema, bytes, alignment });
        // Only fixed-size checked facts escape. The owned query value and
        // both return envelopes die before this unit-return scope refunds.
        Ok(())
    }).inspect_err(|error| source_reference_record_failure_v29(references, error))?;
    representation.ok_or_else(scoped_object_allocation_error_v29)
}

#[allow(clippy::too_many_arguments)]
fn append_scoped_source_slots_v29(
    instances: &ExecutionInstancesV29<'_>,
    id: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    source: ExecutionCallSourceV29,
    max_elements: usize,
    slots: &mut Vec<ScopedSourceSlotV29>,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    identities: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    object_cells: &mut [bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedSourceSlotInstanceV29, ProductionSemanticKirErrorV1> {
    let events = lowered
        .lifecycle_events
        .as_ref()
        .ok_or_else(scoped_slot_error_v29)?;
    events.check_identity(instances, id, budget)?;
    budget.charge_work(4)?;
    if lowered.source_call_instance != Some(id) {
        return Err(scoped_slot_error_v29());
    }
    let instance = instances.instance(id).ok_or_else(scoped_slot_error_v29)?;
    let declaration = instance.declaration();
    let origins = lowered
        .scoped_slot_origins
        .as_ref()
        .ok_or_else(scoped_slot_error_v29)?;
    #[cfg(test)]
    let mutated_origins = SCOPED_OBJECT_ORIGIN_MUTATION_V29.get()
        .map(|mutate| mutate(origins, budget)).transpose()?.flatten();
    #[cfg(test)]
    let origins = mutated_origins.as_deref().unwrap_or(origins);
    let body = lowered
        .function
        .body
        .as_ref()
        .ok_or_else(scoped_slot_error_v29)?;
    let (invocation_prefix, _) =
        invocation_checked_prefix_v1(instance, source.root, lowered, budget)?;
    let entry = match &lowered.invocation_entry {
        Some(relation) if invocation_prefix == 1 => relation
            .layout
            .preheader
            .ok_or_else(scoped_slot_error_v29)?,
        None if invocation_prefix == 0 => events.placement.block(declaration.entry().index())?,
        _ => return Err(scoped_slot_error_v29()),
    };
    budget.charge_work(body.blocks.len())?;
    if body.blocks.first().is_none_or(|block| block.id != entry)
        || body.blocks.iter().filter(|block| block.id == entry).count() != 1
    {
        return Err(scoped_slot_error_v29());
    }
    let prologue =
        scoped_slot_prologue_v29(lowered, source.root, instance.function(), entry, budget)?;
    let scratch_floor = budget.storage();
    let mut candidates = scoped_slot_candidates_v29(declaration, instance.ssa(), budget)?;
    for origin in origins {
        budget.charge_work(2)?;
        if let ScopedAllocationIdentityV29::OriginalObject { local, .. } = origin.identity {
            *candidates.get_mut(local as usize).ok_or_else(scoped_slot_error_v29)? = 0;
        }
    }
    if let Some(references) = references {
        references.check_owner(instances, budget)?;
        for (local, candidate) in candidates.iter_mut().enumerate() {
            budget.charge_work(1)?;
            if *candidate != 2 {
                continue;
            }
            let local = u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            if identities
                .map(|identities| {
                    identities.retained_slot_omission_v1(instances, references, id, local, budget)
                })
                .transpose()?
                .unwrap_or(false)
                || source_reference_existing_value_local_v29(references, id, local, budget)?
            {
                *candidate = 0;
            }
        }
    }
    let scratch_bytes = budget
        .storage()
        .checked_sub(scratch_floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.charge_work(candidates.len())?;
    let legacy_count = candidates.iter().filter(|&&flag| flag == 2).count();
    let count = origins.len();
    if prologue.is_some() != (count != 0) {
        return Err(scoped_slot_error_v29());
    }
    let start = slots.len();
    let mut cursor = 0;
    let mut arrays = 0;
    let mut legacy_seen = 0;
    let mut previous = None;
    let types = instances.owner().source_semantic().types();
    for &origin in origins {
        budget.charge_work(5)?;
        let local = origin.identity.original_local().ok_or_else(scoped_slot_error_v29)?;
        let local_decl = declaration.locals().get(local as usize).ok_or_else(scoped_slot_error_v29)?;
        if origin.semantic_type != local_decl.ty() || previous.is_some_and(|key| key >= origin.identity) {
            return Err(scoped_slot_error_v29());
        }
        previous = Some(origin.identity);
        let location = PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block: entry,
            operation: cursor,
        };
        let representation = match (origin.identity, origin.source) {
            (ScopedAllocationIdentityV29::LegacyLocal(_), ScopedAllocationSourceV29::Legacy | ScopedAllocationSourceV29::OriginalArray { .. }) => {
                if matches!(origin.source, ScopedAllocationSourceV29::OriginalArray { .. }) && references.is_none() {
                    return Err(scoped_object_allocation_error_v29());
                }
                if candidates[local as usize] != 2 { return Err(scoped_slot_error_v29()); }
                candidates[local as usize] = 0;
                legacy_seen = argument_sum_v1(&[legacy_seen, 1])?;
                let (element_type, element, length, count) = if let Some(array) =
                    private_retained_array_facts_v1(types, local_decl.ty(), max_elements, budget)?
                {
                    let count = scoped_array_slot_v29(lowered, body, source.root, instance.function(),
                        origin, array, arrays, location, events.placement, budget)?;
                    arrays = argument_sum_v1(&[arrays, 1])?;
                    cursor = argument_sum_v1(&[cursor, 1])?;
                    (array.element_type, array.element, array.length, Some(count))
                } else {
                    if matches!(origin.source, ScopedAllocationSourceV29::OriginalArray { .. }) {
                        return Err(scoped_object_allocation_error_v29());
                    }
                    let element = private_retained_slot_facts_v1(types, local_decl.ty(), budget)?
                        .ok_or_else(scoped_slot_error_v29)?;
                    (local_decl.ty(), element, 1, None)
                };
                let bytes = element.size.checked_mul(length).ok_or(ArgumentResourceV1::Arithmetic)?;
                ScopedSlotRepresentationV29::ScalarArray(ScopedScalarArraySlotV29 {
                    element_type, element, length, bytes, count,
                })
            }
            (ScopedAllocationIdentityV29::OriginalObject { generation, .. },
                ScopedAllocationSourceV29::OriginalObject { cell, schema }) => {
                let references = references.ok_or_else(scoped_object_allocation_error_v29)?;
                let (representative, _, _) = references.physical_object_cell(cell, budget)?;
                if representative != cell { return Err(scoped_object_allocation_error_v29()); }
                let representation = scoped_object_slot_representation_v29(references, cell, id,
                    SemanticLocalIdV1::from_index(local), generation, schema, budget)?;
                let seen = object_cells.get_mut(cell).ok_or_else(scoped_object_allocation_error_v29)?;
                if *seen { return Err(scoped_object_allocation_error_v29()); }
                *seen = true;
                representation
            }
            _ => return Err(scoped_object_allocation_error_v29()),
        };
        let allocation = PrivateArrayPhysicalLocationV1 {
            operation: cursor,
            ..location
        };
        let operation = private_array_operation_v1(body, allocation, budget)?
            .ok_or_else(scoped_slot_error_v29)?;
        match representation {
            ScopedSlotRepresentationV29::ScalarArray(row) => {
                private_retained_check_allocation_operation_v1(operation, origin.pointer,
                    row.count.map(|(value, _)| value), row.element, budget)
                    .map_err(scoped_slot_relation_error_v29)?;
            }
            ScopedSlotRepresentationV29::Object { schema, alignment, .. } => {
                check_scoped_object_alloca_v29(operation, origin.pointer, schema, alignment, budget)?;
            }
        }
        budget.charge_work(slots.len())?;
        if slots
            .iter()
            .any(|previous| previous.origin.pointer == origin.pointer)
        {
            return Err(scoped_slot_error_v29());
        }
        emission_push_v1(
            slots,
            ScopedSourceSlotV29 {
                instance: id,
                origin,
                representation,
                allocation,
            },
            budget,
        )?;
        cursor = cursor
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    let (allocations, initializers) = visit_scoped_slot_initializers_v29(
        instances,
        id,
        entry,
        &slots[start..],
        budget,
        |_, slot, location, budget| {
            let references = references.ok_or_else(scoped_slot_error_v29)?;
            let initialization = private_array_operation_v1(body, location, budget)?
                .ok_or_else(scoped_slot_error_v29)?;
            source_reference_retained_scalar_entry_store_v29(
                references,
                id,
                slot.origin,
                lowered,
                initialization,
                budget,
            )
        },
    )?;
    budget.charge_work(4)?;
    if legacy_seen != legacy_count || allocations as usize != cursor
        || prologue.map_or(0, |span| span.operation_count)
            != allocations
                .checked_add(initializers)
                .ok_or(ArgumentResourceV1::Arithmetic)?
        || lowered.private_arrays.slots.len() != arrays
        || lowered.private_arrays.active != (arrays != 0)
    {
        return Err(scoped_slot_error_v29());
    }
    let mut actual_allocations = 0_usize;
    for block in &body.blocks {
        budget.charge_work(
            block
                .operations
                .len()
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?;
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::Alloca { .. }) {
                actual_allocations = actual_allocations
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
    }
    if actual_allocations != count {
        return Err(scoped_slot_error_v29());
    }
    lowered
        .scoped_initialization
        .as_ref()
        .ok_or_else(scoped_initialization_error_v29)?
        .check_custody(instances, id, events, origins, budget)?;
    drop(candidates);
    budget.release_storage(scratch_bytes)?;
    Ok(ScopedSourceSlotInstanceV29 {
        instance: id,
        function: instance.function(),
        incoming: instances.incoming(id).map(|call| call.occurrence()),
        placement: events.placement,
        slots: start..slots.len(),
    })
}

fn derive_scoped_source_slots_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    max_elements: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedScopedSourceSlotsV29, ProductionSemanticKirErrorV1> {
    let root_events = emitted
        .get(instances.root().index())
        .and_then(Option::as_ref)
        .and_then(|lowered| lowered.lifecycle_events.as_ref())
        .ok_or_else(scoped_slot_error_v29)?;
    if root_events.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    // Omissions and retained entry initializers both need the source owner:
    // an ordinary scalar argument can have a complete, non-cell census.
    let (needs_plan, needs_storage) = with_canonical_call_scratch_v1(budget, |budget| {
        let mut needs_plan = false;
        let mut needs_storage = false;
        for (index, lowered) in emitted.iter().enumerate() {
            budget.charge_work(3)?;
            let id = instances.id_at(index).ok_or_else(scoped_slot_error_v29)?;
            let row = instances.instance(id).ok_or_else(scoped_slot_error_v29)?;
            if instances.instance_reachable(id) == Some(false) {
                if lowered.is_some() { return Err(scoped_slot_error_v29()); }
                continue;
            }
            let origins = lowered
                .as_ref()
                .and_then(|lowered| lowered.scoped_slot_origins.as_ref())
                .ok_or_else(scoped_slot_error_v29)?;
            let candidates = scoped_slot_candidates_v29(row.declaration(), row.ssa(), budget)?;
            budget.charge_work(candidates.len())?;
            let mut retained = 0_usize;
            for (local, &flag) in candidates.iter().enumerate() {
                if flag == 2 {
                    budget.charge_work(2)?;
                    retained = retained
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    needs_plan |= row.declaration().locals()[local].role().is_entry_argument();
                }
            }
            needs_plan |= retained != origins.len();
            budget.charge_work(origins.len())?;
            let typed = origins.iter().any(|origin| origin.source != ScopedAllocationSourceV29::Legacy);
            needs_storage |= typed;
            needs_plan |= typed;
        }
        Ok((needs_plan, needs_storage))
    })?;
    if needs_storage {
        derive_scoped_source_slots_with_demanded_plan_v29(instances, emitted, max_elements, budget)
    } else if needs_plan {
        with_source_reference_plan_v29(instances, budget, |references, budget| {
            derive_scoped_source_slots_with_references_v29(
                instances,
                emitted,
                max_elements,
                Some(references),
                budget,
            )
        })
    } else {
        derive_scoped_source_slots_with_references_v29(
            instances,
            emitted,
            max_elements,
            None,
            budget,
        )
    }
}

// Rebuild from original module demands, never from physical schema IDs. The
// production path already has a plan and uses it directly; standalone replays
// must not substitute a promoted-only plan for typed storage.
fn derive_scoped_source_slots_with_demanded_plan_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    max_elements: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedScopedSourceSlotsV29, ProductionSemanticKirErrorV1> {
    scoped_slot_attempt_v29(budget, |budget| {
        let owner = instances.owner();
        let roots = owner.source_semantic().roots();
        budget.charge_work(argument_sum_v1(&[roots.len(), 1])?)?;
        let root = instances.instance(instances.root()).ok_or_else(scoped_slot_error_v29)?.function();
        let ordinal = roots.iter().position(|&candidate| candidate == root).ok_or_else(scoped_slot_error_v29)?;
        let headers = argument_sum_v1(&[
            std::mem::size_of::<source_storage_demands_v29::SourceStorageDemandsV29<'_>>(),
            std::mem::size_of::<source_storage_demands_v29::RootStorageDemandsV29<'_, '_>>(),
            std::mem::size_of::<Result<OwnedScopedSourceSlotsV29, ProductionSemanticKirErrorV1>>(),
        ])?;
        budget.reserve_storage(headers)?;
        let demands = source_storage_demands_v29::SourceStorageDemandsV29::collect(owner, budget)?;
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            owner, demands.types(owner, budget)?,
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(), budget,
        )?;
        let requests = demands.root_lens(owner, ordinal, budget)?;
        let output = source_storage_v29::with_source_storage_descriptor_demands_root_v29(
            &mut layouts, instances, None, requests, budget,
            |references, _storage, budget| {
                derive_scoped_source_slots_with_references_v29(
                    instances, emitted, max_elements, Some(references), budget,
                ).map_err(Into::into)
            },
        )?;
        layouts.release(budget)?;
        demands.discard(budget)?;
        budget.release_storage(headers)?;
        Ok(output)
    })
}

fn derive_scoped_source_slots_with_references_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    max_elements: usize,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedScopedSourceSlotsV29, ProductionSemanticKirErrorV1> {
    derive_scoped_source_slots_with_identities_v1(
        instances,
        emitted,
        max_elements,
        references,
        None,
        budget,
    )
}

fn derive_scoped_source_slots_with_identities_v1(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    max_elements: usize,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    identities: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedScopedSourceSlotsV29, ProductionSemanticKirErrorV1> {
    if identities.is_some() && references.is_none() {
        return Err(execution_identity_error_v1());
    }
    if let Some(references) = references {
        references.check_owner(instances, budget)?;
    }
    let root_events = emitted
        .get(instances.root().index())
        .and_then(Option::as_ref)
        .and_then(|lowered| lowered.lifecycle_events.as_ref())
        .ok_or_else(scoped_slot_error_v29)?;
    if root_events.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    scoped_slot_attempt_v29(budget, |budget| {
        let floor = budget.storage();
        budget.charge_work(1)?;
        if emitted.len() != instances.instances().len() {
            return Err(scoped_slot_error_v29());
        }
        let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
        let mut instance_rows = emission_vec_v1(emitted.len(), budget)?;
        let mut slots = Vec::new();
        let scratch_floor = budget.storage();
        source_reference_emission_prepay_v29::<Vec<bool>>(budget)?;
        let cells = references.map_or(0, |references| references.cells.rows.len());
        let mut object_cells = emission_vec_v1(cells, budget)?;
        budget.charge_work(cells)?;
        object_cells.resize(cells, false);
        let scratch_bytes = budget.storage().checked_sub(scratch_floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        for (index, lowered) in emitted.iter().enumerate() {
            budget.charge_work(2)?;
            let id = instances.id_at(index).ok_or_else(scoped_slot_error_v29)?;
            if instances.instance_reachable(id) == Some(false) {
                if lowered.is_some() { return Err(scoped_slot_error_v29()); }
                continue;
            }
            let lowered = lowered.as_ref().ok_or_else(scoped_slot_error_v29)?;
            instance_rows.push(append_scoped_source_slots_v29(
                instances,
                id,
                lowered,
                source,
                max_elements,
                &mut slots,
                references,
                identities,
                &mut object_cells,
                budget,
            )?);
        }
        if let Some(references) = references {
            if !references.cells.physical_backings.is_empty() {
                check_source_physical_backing_coverage_v29(references, &slots, &mut object_cells, budget)?;
            }
            check_source_array_cell_coverage_v29(references, &slots, &mut object_cells, budget)?;
        }
        drop(object_cells);
        budget.release_storage(scratch_bytes)?;
        Ok(OwnedScopedSourceSlotsV29 {
            source,
            ledger: budget.work_ledger_identity_v1(),
            instances: instance_rows,
            slots,
            pending_memory: None,
            retained_storage: budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        })
    })
}
