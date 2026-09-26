// A private emission transaction, not native admission or allocation-trace equivalence.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrefixV29 {
    instance: ProductionCallInstanceIdV1,
    block: BlockId,
    count: u32,
    initializers: u32,
    first: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StorageV29 {
    pub(super) allocations: usize,
    pub(super) payload_bytes: u64,
    pub(super) alignment: u32,
}

pub(super) struct RelocationV29 {
    root: BlockId,
    root_prefix: u32,
    root_initializers: u32,
    moved: u32,
    prefixes: Vec<PrefixV29>,
    storage: StorageV29,
}

pub(super) struct PreparedV29<'a, 'scope> {
    instances: &'a ExecutionInstancesV29<'scope>,
    emitted: &'a mut [Option<LoweredFunctionResultV1>],
    slots: &'a OwnedScopedSourceSlotsV29,
    relocation: RelocationV29,
    prepared_storage_floor: usize,
}

// Raw input checks do not establish ordinary per-instance pointer/history
// admission. This wrapper has only a pending assembly exit.
pub(super) struct PendingRawPreparedV29<'a, 'scope> {
    fields: PreparedV29<'a, 'scope>,
}

// Construction requires the exclusive live-roster transaction below.
pub(super) struct FramePermitV29<'a, 'scope> {
    instances: &'a ExecutionInstancesV29<'scope>,
    slots: &'a OwnedScopedSourceSlotsV29,
}

#[derive(Clone, Copy)]
pub(super) struct CallFrameV29<'a, 'scope> {
    permit: &'a FramePermitV29<'a, 'scope>,
    plan: &'a ProductionCallInstancePlanV1<'scope>,
    child: ProductionCallInstanceIdV1,
}

impl<'scope> FramePermitV29<'_, 'scope> {
    pub(super) fn for_child<'a>(
        &'a self,
        plan: &'a ProductionCallInstancePlanV1<'scope>,
        child: ProductionCallInstanceIdV1,
    ) -> CallFrameV29<'a, 'scope> {
        CallFrameV29 {
            permit: self,
            plan,
            child,
        }
    }
}

impl CallFrameV29<'_, '_> {
    pub(super) fn check(
        &self,
        block: BlockId,
        ordinal: usize,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        self.permit
            .check(self.plan, self.child, block, ordinal, operation, budget)
    }
}

fn invalid() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "scoped allocation relocation is incomplete or mismatched",
    )
}

fn map_error(error: CallInstanceEmissionErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        CallInstanceEmissionErrorV1::Resource(error) => error.into(),
        _ => invalid(),
    }
}

fn prologue_counts(
    instances: &ExecutionInstancesV29<'_>,
    instance: &ScopedSourceSlotInstanceV29,
    slots: &OwnedScopedSourceSlotsV29,
    entry: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(u32, u32), ProductionSemanticKirErrorV1> {
    let rows = slots
        .slots
        .get(instance.slots.clone())
        .ok_or_else(invalid)?;
    visit_scoped_slot_initializers_v29(
        instances,
        instance.instance,
        entry,
        rows,
        budget,
        |_, _, _, _| Ok(()),
    )
}

fn storage(
    slots: &[ScopedSourceSlotV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<StorageV29, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_product_v1(slots.len(), 7)?)?;
    let mut result = StorageV29 {
        allocations: slots.len(),
        payload_bytes: 0,
        alignment: 1,
    };
    for slot in slots {
        let (bytes, alignment) = match (slot.origin.identity, slot.origin.source, slot.representation) {
            (ScopedAllocationIdentityV29::LegacyLocal(_), ScopedAllocationSourceV29::Legacy | ScopedAllocationSourceV29::OriginalArray { .. }, ScopedSlotRepresentationV29::ScalarArray(scalar)) => {
                slot.scalar_array()?;
                if scalar.element.size.checked_mul(scalar.length) != Some(scalar.bytes) {
                    return Err(invalid());
                }
                (scalar.bytes, scalar.element.alignment)
            }
            (ScopedAllocationIdentityV29::OriginalObject { .. }, ScopedAllocationSourceV29::OriginalObject { schema: original, .. }, ScopedSlotRepresentationV29::Object { schema, bytes, alignment }) if original == schema => (bytes, alignment),
            _ => return Err(invalid()),
        };
        result.payload_bytes = result
            .payload_bytes
            .checked_add(bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        result.alignment = result.alignment.max(alignment);
    }
    Ok(result)
}

pub(super) fn prepare<'a, 'scope>(
    instances: &'a ExecutionInstancesV29<'scope>,
    emitted: &'a mut [Option<LoweredFunctionResultV1>],
    slots: &'a OwnedScopedSourceSlotsV29,
    max_elements: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PreparedV29<'a, 'scope>, ProductionSemanticKirErrorV1> {
    prepare_with_references(instances, emitted, slots, max_elements, None, budget)
}

pub(super) fn prepare_with_references<'a, 'scope>(
    instances: &'a ExecutionInstancesV29<'scope>,
    emitted: &'a mut [Option<LoweredFunctionResultV1>],
    slots: &'a OwnedScopedSourceSlotsV29,
    max_elements: usize,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PreparedV29<'a, 'scope>, ProductionSemanticKirErrorV1> {
    prepare_with_identities_v1(
        instances,
        emitted,
        slots,
        max_elements,
        references,
        None,
        budget,
    )
}

pub(super) fn prepare_with_identities_v1<'a, 'scope>(
    instances: &'a ExecutionInstancesV29<'scope>,
    emitted: &'a mut [Option<LoweredFunctionResultV1>],
    slots: &'a OwnedScopedSourceSlotsV29,
    max_elements: usize,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    identities: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PreparedV29<'a, 'scope>, ProductionSemanticKirErrorV1> {
    scoped_raw_admission_v29::require_original_zero_raw_v29(
        instances,
        references.map(|references| references.plan),
        budget,
    )?;
    prepare_source_inputs_v29(
        instances,
        emitted,
        slots,
        max_elements,
        references,
        identities,
        None,
        budget,
    )
}

pub(super) fn prepare_pending_raw_v29<'a, 'scope>(
    original: &scoped_raw_admission_v29::SourceRootPreparationV29<'_, '_, '_>,
    instances: &'a ExecutionInstancesV29<'scope>,
    emitted: &'a mut [Option<LoweredFunctionResultV1>],
    slots: &'a OwnedScopedSourceSlotsV29,
    max_elements: usize,
    identities: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingRawPreparedV29<'a, 'scope>, ProductionSemanticKirErrorV1> {
    let references = original.check_raw(instances, budget)?;
    let fields = prepare_source_inputs_v29(
        instances,
        emitted,
        slots,
        max_elements,
        Some(references),
        identities,
        Some(original),
        budget,
    )?;
    Ok(PendingRawPreparedV29 { fields })
}

fn prepare_source_inputs_v29<'a, 'scope>(
    instances: &'a ExecutionInstancesV29<'scope>,
    emitted: &'a mut [Option<LoweredFunctionResultV1>],
    slots: &'a OwnedScopedSourceSlotsV29,
    max_elements: usize,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    identities: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    raw: Option<&scoped_raw_admission_v29::SourceRootPreparationV29<'_, '_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PreparedV29<'a, 'scope>, ProductionSemanticKirErrorV1> {
    if let Some(references) = references {
        references.plan.check_owner(instances, budget)?;
        references.check(budget)?;
    }
    scoped_slot_attempt_v29(budget, |budget| {
        if let Some(original) = raw {
            let references = original.check_raw(instances, budget)?;
            scoped_slot_uses_v29::check_pending_raw_source_inputs_v29(
                instances,
                emitted,
                slots,
                max_elements,
                references,
                identities,
                budget,
            )?;
        } else {
            scoped_slot_uses_v29::check_scoped_source_slot_uses_with_identities_v1(
                instances,
                emitted,
                slots,
                max_elements,
                references,
                identities,
                budget,
            )?;
        }
        let root = slots.instances.first().ok_or_else(invalid)?;
        if root.instance != instances.root() {
            return Err(invalid());
        }
        let body = emitted
            .first()
            .and_then(Option::as_ref)
            .and_then(|row| row.function.body.as_ref())
            .ok_or_else(invalid)?;
        let entry = body.blocks.first().ok_or_else(invalid)?.id;
        let (root_prefix, root_initializers) =
            prologue_counts(instances, root, slots, entry, budget)?;
        let mut prefixes = emission_vec_v1(slots.instances.len().saturating_sub(1), budget)?;
        let mut first = root_prefix;
        for row in slots.instances.iter().skip(1) {
            budget.charge_work(2)?;
            let source = emitted
                .get(row.instance.index())
                .and_then(Option::as_ref)
                .and_then(|row| row.function.body.as_ref())
                .ok_or_else(invalid)?;
            let block = source.blocks.first().ok_or_else(invalid)?.id;
            let (count, initializers) = prologue_counts(instances, row, slots, block, budget)?;
            if count == 0 {
                continue;
            }
            prefixes.push(PrefixV29 {
                instance: row.instance,
                block,
                count,
                initializers,
                first,
            });
            first = first
                .checked_add(count)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        // A previously slot-free root did not run the slot-history entry check.
        if !prefixes.is_empty() {
            for block in &body.blocks {
                budget.charge_work(1)?;
                block
                    .terminator
                    .as_ref()
                    .ok_or_else(invalid)?
                    .try_visit_edges_v1(|target, arguments| {
                        budget.charge_work(argument_sum_v1(&[1, arguments.len()])?)?;
                        if target == entry {
                            return Err(invalid());
                        }
                        Ok(())
                    })?;
            }
        }
        let allocation_storage = storage(&slots.slots, budget)?;
        Ok(PreparedV29 {
            instances,
            emitted,
            slots,
            prepared_storage_floor: budget.storage(),
            relocation: RelocationV29 {
                root: entry,
                root_prefix,
                root_initializers,
                moved: first - root_prefix,
                prefixes,
                storage: allocation_storage,
            },
        })
    })
    .inspect_err(|error| {
        if let Some(references) = references {
            source_reference_record_failure_v29(references.plan, error);
        }
    })
}

impl FramePermitV29<'_, '_> {
    pub(super) fn check(
        &self,
        plan: &ProductionCallInstancePlanV1<'_>,
        child: ProductionCallInstanceIdV1,
        block: BlockId,
        ordinal: usize,
        operation: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        if self.slots.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(1 + self.slots.slots.len())?;
        if !std::ptr::eq(plan, self.instances) {
            return Err(CallInstanceEmissionErrorV1::CalleeFrameAllocation);
        }
        let slot = self
            .slots
            .slots
            .iter()
            .find(|slot| slot.allocation.block == block && slot.allocation.operation == ordinal)
            .ok_or(CallInstanceEmissionErrorV1::CalleeFrameAllocation)?;
        let mut owner = slot.instance;
        while owner != child {
            budget.charge_work(1)?;
            owner = self
                .instances
                .incoming(owner)
                .ok_or(CallInstanceEmissionErrorV1::CalleeFrameAllocation)?
                .occurrence()
                .caller;
        }
        check_scoped_slot_alloca_v29(slot, operation, budget)
        .map_err(|error| match error {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
            _ => CallInstanceEmissionErrorV1::CalleeFrameAllocation,
        })
    }
}

impl RelocationV29 {
    pub(super) fn matches_replay(
        &self,
        other: &Self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Self {
            root,
            root_prefix,
            root_initializers,
            moved,
            prefixes,
            storage,
        } = self;
        budget.charge_work(size_of::<Self>())?;
        if (*root, *root_prefix, *root_initializers, *moved, *storage)
            != (
                other.root,
                other.root_prefix,
                other.root_initializers,
                other.moved,
                other.storage,
            )
            || prefixes.len() != other.prefixes.len()
        {
            return Ok(false);
        }
        budget.charge_work(argument_product_v1(prefixes.len(), size_of::<PrefixV29>())?)?;
        Ok(prefixes == &other.prefixes)
    }

    fn ordinary_span(
        &self,
        span: InstancePhysicalSpanV1,
    ) -> Result<InstancePhysicalSpanV1, ProductionSemanticKirErrorV1> {
        if self.moved == 0 {
            return Ok(span);
        }
        if span.block == self.root {
            if span.first < self.root_prefix {
                return Err(invalid());
            }
            return Ok(InstancePhysicalSpanV1 {
                first: span
                    .first
                    .checked_add(self.moved)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
                ..span
            });
        }
        if let Some(prefix) = self.prefixes.iter().find(|row| row.block == span.block) {
            return Ok(InstancePhysicalSpanV1 {
                first: span.first.checked_sub(prefix.count).ok_or_else(invalid)?,
                ..span
            });
        }
        Ok(span)
    }

    fn mapped(
        &self,
        row: InstanceMappedSpanV1,
    ) -> Result<InstanceMappedSpanV1, ProductionSemanticKirErrorV1> {
        if self.moved == 0 {
            return Ok(row);
        }
        let mut mapped = row;
        if matches!(row.source, InstanceSpanSourceV1::Synthetic(source)
            if source.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage)
        {
            let original = row.source.coordinates().2;
            if row.removed_call.is_some()
                || row.segments != [Some(original), None]
                || original.first != 0
            {
                return Err(invalid());
            }
            if original.block == self.root {
                if original.count
                    != self
                        .root_prefix
                        .checked_add(self.root_initializers)
                        .ok_or(ArgumentResourceV1::Arithmetic)?
                {
                    return Err(invalid());
                }
                mapped.segments = [
                    Some(InstancePhysicalSpanV1 {
                        block: self.root,
                        first: 0,
                        count: self.root_prefix,
                    }),
                    if self.root_initializers == 0 {
                        None
                    } else {
                        Some(InstancePhysicalSpanV1 {
                            block: self.root,
                            first: self
                                .root_prefix
                                .checked_add(self.moved)
                                .ok_or(ArgumentResourceV1::Arithmetic)?,
                            count: self.root_initializers,
                        })
                    },
                ];
            } else {
                let prefix = self
                    .prefixes
                    .iter()
                    .find(|prefix| prefix.instance == row.instance)
                    .ok_or_else(invalid)?;
                if original.block != prefix.block
                    || original.count
                        != prefix
                            .count
                            .checked_add(prefix.initializers)
                            .ok_or(ArgumentResourceV1::Arithmetic)?
                {
                    return Err(invalid());
                }
                mapped.segments = [
                    Some(InstancePhysicalSpanV1 {
                        block: self.root,
                        first: prefix.first,
                        count: prefix.count,
                    }),
                    if prefix.initializers == 0 {
                        None
                    } else {
                        Some(InstancePhysicalSpanV1 {
                            block: prefix.block,
                            first: 0,
                            count: prefix.initializers,
                        })
                    },
                ];
            }
        } else {
            for span in mapped.segments.iter_mut().flatten() {
                *span = self.ordinary_span(*span)?;
            }
        }
        Ok(mapped)
    }

    pub(super) fn assertion_span(
        &self,
        span: InstancePhysicalSpanV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<InstancePhysicalSpanV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(argument_sum_v1(&[3, self.prefixes.len()])?)?;
        self.ordinary_span(span)
    }

    #[cfg(test)]
    pub(super) fn storage(&self) -> StorageV29 {
        self.storage
    }

    fn final_census(
        &self,
        pending: &PendingScopedRootEmissionV29,
        instances: &ProductionCallInstancePlanV1<'_>,
        slots: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let function = &pending.function;
        let body = function.body.as_ref().ok_or_else(invalid)?;
        let entry = body.blocks.first().ok_or_else(invalid)?;
        if entry.id != self.root || storage(&slots.slots, budget)? != self.storage {
            return Err(invalid());
        }
        let mut count = 0_usize;
        for block in &body.blocks {
            budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
            for operation in &block.operations {
                if matches!(operation.kind, OperationKind::Alloca { .. }) {
                    if block.id != self.root {
                        return Err(invalid());
                    }
                    count = argument_sum_v1(&[count, 1])?;
                }
            }
        }
        if count != self.storage.allocations {
            return Err(invalid());
        }
        for slot in &slots.slots {
            budget.charge_work(argument_sum_v1(&[4, self.prefixes.len()])?)?;
            let offset = if slot.instance == slots.instances[0].instance {
                0
            } else {
                self.prefixes
                    .iter()
                    .find(|row| row.instance == slot.instance)
                    .ok_or_else(invalid)?
                    .first as usize
            };
            let operation = argument_sum_v1(&[offset, slot.allocation.operation])?;
            let allocation = entry.operations.get(operation).ok_or_else(invalid)?;
            check_scoped_slot_alloca_v29(slot, allocation, budget)?;
            if let Some((value, count)) = slot.representation.count() {
                let scalar = slot.scalar_array()?;
                let location = PrivateArrayPhysicalLocationV1 {
                    block_ordinal: 0,
                    block: self.root,
                    operation: argument_sum_v1(&[offset, count.operation])?,
                };
                if private_array_unsigned_operation_v1(
                    body,
                    location,
                    value,
                    ScalarType::Index,
                    budget,
                )
                .map_err(scoped_slot_relation_error_v29)?
                    != scalar.length
                {
                    return Err(invalid());
                }
            }
        }
        with_canonical_call_scratch_v1(budget, |budget| {
            let parts =
                ScopedDeferredScalarViewV29::for_pending_root(instances, pending, self, budget)?;
            let mut scratch = 0;
            let index = call_splice_index_with_deferred_parts_v29(
                function,
                Some(&parts),
                budget,
                &mut scratch,
            )
            .map_err(map_error)?;
            call_splice_check_body_v1(function, &index, false, budget).map_err(map_error)?;
            Ok(())
        })
    }

    fn apply(
        &self,
        pending: &mut PendingScopedRootEmissionV29,
        instances: &ProductionCallInstancePlanV1<'_>,
        slots: &OwnedScopedSourceSlotsV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.moved == 0 {
            return self.final_census(pending, instances, slots, budget);
        }
        let body = pending.function.body.as_ref().ok_or_else(invalid)?;
        let entry = body.blocks.first().ok_or_else(invalid)?;
        if entry.id != self.root || entry.operations.len() < self.root_prefix as usize {
            return Err(invalid());
        }
        let required = argument_sum_v1(&[entry.operations.len(), self.moved as usize])?;
        enforce_limit(
            ProductionSemanticKirResourceV1::Operations,
            required,
            MAX_BLOCK_OPERATIONS_V1,
        )?;
        let mut replacement = emission_vec_v1(required, budget)?;
        let mut donor_indices = emission_vec_v1(self.prefixes.len(), budget)?;
        for prefix in &self.prefixes {
            budget.charge_work(argument_sum_v1(&[3, body.blocks.len()])?)?;
            let index = body
                .blocks
                .iter()
                .position(|block| block.id == prefix.block)
                .ok_or_else(invalid)?;
            if index == 0 || body.blocks[index].operations.len() < prefix.count as usize {
                return Err(invalid());
            }
            donor_indices.push(index);
        }
        budget.charge_work(argument_product_v1(
            pending.coordinates.spans.rows.len(),
            argument_sum_v1(&[16, argument_product_v1(2, self.prefixes.len())?])?,
        )?)?;
        for &row in &pending.coordinates.spans.rows {
            self.mapped(row)?;
        }
        budget.charge_work(body.blocks.len())?;
        let shifted_operations = body
            .blocks
            .iter()
            .map(|block| block.operations.len())
            .try_fold(0_usize, |sum, count| {
                sum.checked_add(count).ok_or(ArgumentResourceV1::Arithmetic)
            })?;
        let donor_bytes =
            argument_product_v1(donor_indices.capacity(), std::mem::size_of::<usize>())?;
        // All move/reservation/index checks precede mutation. Only prefix payloads
        // move; initialization and every nonallocation operation keep their order.
        budget.charge_work(argument_sum_v1(&[
            required,
            self.prefixes.len(),
            body.blocks.len(),
            argument_product_v1(
                pending.coordinates.spans.rows.len(),
                argument_sum_v1(&[16, argument_product_v1(2, self.prefixes.len())?])?,
            )?,
            shifted_operations,
        ])?)?;
        let body = pending.function.body.as_mut().unwrap();
        let mut root = std::mem::take(&mut body.blocks[0].operations);
        replacement.extend(root.drain(..self.root_prefix as usize));
        for (prefix, &index) in self.prefixes.iter().zip(&donor_indices) {
            replacement.extend(body.blocks[index].operations.drain(..prefix.count as usize));
        }
        replacement.append(&mut root);
        body.blocks[0].operations = replacement;
        for row in &mut pending.coordinates.spans.rows {
            *row = self
                .mapped(*row)
                .expect("preflight checked immutable prefix map");
        }
        drop(donor_indices);
        budget.release_storage(donor_bytes)?;
        self.final_census(pending, instances, slots, budget)
    }
}

impl PreparedV29<'_, '_> {
    // The enclosing root-emission attempt owns failure/panic cleanup, including
    // the preparation reservation. A failed assembly consumes this attempt.
    // additional_storage_bytes covers assembly only, not the prepared prefixes.
    pub(super) fn assemble(
        self,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<PendingScopedRootEmissionV29, ProductionSemanticKirErrorV1> {
        self.assemble_with_references(limits, None, budget)
    }

    pub(super) fn assemble_with_references(
        self,
        limits: ProductionSemanticKirLimitsV1,
        references: Option<&SourceReferencePlanV29<'_, '_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<PendingScopedRootEmissionV29, ProductionSemanticKirErrorV1> {
        if self.slots.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.prepared_storage_floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        let permit = FramePermitV29 {
            instances: self.instances,
            slots: self.slots,
        };
        let pending = assemble_pending_scoped_root_inner_v29(
            self.instances,
            self.emitted,
            limits,
            Some(&permit),
            references,
            budget,
        )?;
        self.relocation
            .complete_pending(pending, self.instances, self.slots, floor, budget)
    }
}

impl PendingRawPreparedV29<'_, '_> {
    pub(super) fn assemble_pending(
        self,
        original: &scoped_raw_admission_v29::SourceRootPreparationV29<'_, '_, '_>,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<scoped_raw_admission_v29::PendingRawAssemblyV29, ProductionSemanticKirErrorV1> {
        original.check_raw(self.fields.instances, budget)?;
        if self.fields.slots.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.fields.prepared_storage_floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let floor = budget.storage();
        let permit = FramePermitV29 {
            instances: self.fields.instances,
            slots: self.fields.slots,
        };
        let pending = scoped_raw_admission_v29::assemble_pending_raw_scoped_root_v29(
            original,
            self.fields.instances,
            self.fields.emitted,
            limits,
            Some(&permit),
            budget,
        )?;
        pending.relocate(
            original,
            self.fields.instances,
            self.fields.slots,
            self.fields.relocation,
            floor,
            budget,
        )
    }
}

impl RelocationV29 {
    pub(super) fn complete_pending(
        self,
        mut pending: PendingScopedRootEmissionV29,
        instances: &ExecutionInstancesV29<'_>,
        slots: &OwnedScopedSourceSlotsV29,
        floor: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<PendingScopedRootEmissionV29, ProductionSemanticKirErrorV1> {
        self.apply(&mut pending, instances, slots, budget)?;
        pending.slot_relocation = Some(self);
        replay_pending_instance_asserts_v1(&pending, instances, budget)?;
        pending.additional_storage_bytes = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(pending)
    }
}
