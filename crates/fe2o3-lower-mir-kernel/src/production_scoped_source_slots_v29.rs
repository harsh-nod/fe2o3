// Allocation provenance only: lifetime, initialized reads and relocation need
// independent checks before helper-frame storage may be removed or reused.
#[cfg(test)]
type ScopedSlotObserverV29 = fn(
    &ExecutionInstancesV29<'_>,
    &mut [Option<LoweredFunctionResultV1>],
    &OwnedScopedSourceSlotsV29,
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1>;

#[cfg(test)]
thread_local! {
    static SCOPED_SLOT_OBSERVER_V29: std::cell::Cell<Option<ScopedSlotObserverV29>> = const { std::cell::Cell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedSlotOriginV29 {
    local: u32,
    semantic_type: SemanticTypeIdV1,
    pointer: ValueId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedSourceSlotV29 {
    instance: ProductionCallInstanceIdV1,
    origin: ScopedSlotOriginV29,
    element_type: SemanticTypeIdV1,
    element: PrivateRetainedSlotFactsV1,
    length: u64,
    bytes: u64,
    count: Option<(ValueId, PrivateArrayPhysicalLocationV1)>,
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
    slots: &BTreeMap<u32, SemanticRetainedLocalSlotV1>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<ScopedSlotOriginV29>, ProductionSemanticKirErrorV1> {
    scoped_slot_attempt_v29(budget, |budget| {
        let mut rows = emission_vec_v1(slots.len(), budget)?;
        budget.charge_work(argument_product_v1(slots.len(), 3)?)?;
        for (&local, slot) in slots {
            rows.push(ScopedSlotOriginV29 {
                local,
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
        || recorded.local != origin.local
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
fn append_scoped_source_slots_v29(
    instances: &ExecutionInstancesV29<'_>,
    id: ProductionCallInstanceIdV1,
    lowered: &LoweredFunctionResultV1,
    source: ExecutionCallSourceV29,
    max_elements: usize,
    slots: &mut Vec<ScopedSourceSlotV29>,
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
    let body = lowered
        .function
        .body
        .as_ref()
        .ok_or_else(scoped_slot_error_v29)?;
    let entry = events.placement.block(declaration.entry().index())?;
    budget.charge_work(body.blocks.len())?;
    if body.blocks.first().is_none_or(|block| block.id != entry)
        || body.blocks.iter().filter(|block| block.id == entry).count() != 1
    {
        return Err(scoped_slot_error_v29());
    }
    let prologue =
        scoped_slot_prologue_v29(lowered, source.root, instance.function(), entry, budget)?;
    let scratch_floor = budget.storage();
    let candidates = scoped_slot_candidates_v29(declaration, instance.ssa(), budget)?;
    let scratch_bytes = budget
        .storage()
        .checked_sub(scratch_floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.charge_work(candidates.len())?;
    let count = candidates.iter().filter(|&&flag| flag == 2).count();
    if origins.len() != count || prologue.is_some() != (count != 0) {
        return Err(scoped_slot_error_v29());
    }
    let start = slots.len();
    let mut cursor = 0;
    let mut arrays = 0;
    let types = instances.owner().source_semantic().types();
    for (local, &candidate) in candidates.iter().enumerate() {
        budget.charge_work(1)?;
        if candidate != 2 {
            continue;
        }
        budget.charge_work(5)?;
        let origin = origins[slots.len() - start];
        let local_decl = &declaration.locals()[local];
        if origin.local as usize != local
            || origin.semantic_type != local_decl.ty()
            || local_decl.role().is_entry_argument()
        {
            return Err(scoped_slot_error_v29());
        }
        let location = PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block: entry,
            operation: cursor,
        };
        let (element_type, element, length, count) = if let Some(array) =
            private_retained_array_facts_v1(types, local_decl.ty(), max_elements, budget)?
        {
            let count = scoped_array_slot_v29(
                lowered,
                body,
                source.root,
                instance.function(),
                origin,
                array,
                arrays,
                location,
                events.placement,
                budget,
            )?;
            arrays += 1;
            cursor = cursor
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            (array.element_type, array.element, array.length, Some(count))
        } else {
            let element = private_retained_slot_facts_v1(types, local_decl.ty(), budget)?
                .ok_or_else(scoped_slot_error_v29)?;
            (local_decl.ty(), element, 1, None)
        };
        let allocation = PrivateArrayPhysicalLocationV1 {
            operation: cursor,
            ..location
        };
        let operation = private_array_operation_v1(body, allocation, budget)?
            .ok_or_else(scoped_slot_error_v29)?;
        private_retained_check_allocation_operation_v1(
            operation,
            origin.pointer,
            count.map(|(value, _)| value),
            element,
            budget,
        )
        .map_err(scoped_slot_relation_error_v29)?;
        budget.charge_work(slots.len())?;
        if slots
            .iter()
            .any(|previous| previous.origin.pointer == origin.pointer)
        {
            return Err(scoped_slot_error_v29());
        }
        let bytes = element
            .size
            .checked_mul(length)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        emission_push_v1(
            slots,
            ScopedSourceSlotV29 {
                instance: id,
                origin,
                element_type,
                element,
                length,
                bytes,
                count,
                allocation,
            },
            budget,
        )?;
        cursor = cursor
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
    }
    budget.charge_work(4)?;
    if prologue.map_or(0, |span| span.operation_count as usize) != cursor
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
    scoped_slot_attempt_v29(budget, |budget| {
        let floor = budget.storage();
        budget.charge_work(1)?;
        if emitted.len() != instances.instances().len() {
            return Err(scoped_slot_error_v29());
        }
        let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
        let mut instance_rows = emission_vec_v1(emitted.len(), budget)?;
        let mut slots = Vec::new();
        for (index, lowered) in emitted.iter().enumerate() {
            budget.charge_work(2)?;
            let id = instances.id_at(index).ok_or_else(scoped_slot_error_v29)?;
            let lowered = lowered.as_ref().ok_or_else(scoped_slot_error_v29)?;
            instance_rows.push(append_scoped_source_slots_v29(
                instances,
                id,
                lowered,
                source,
                max_elements,
                &mut slots,
                budget,
            )?);
        }
        Ok(OwnedScopedSourceSlotsV29 {
            source,
            ledger: budget.work_ledger_identity_v1(),
            instances: instance_rows,
            slots,
            retained_storage: budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        })
    })
}
