use super::*;

#[path = "production_scoped_slot_selector_bounds_v29.rs"]
mod bounds;

pub(super) struct Source<'a, 'plan, 'source, 'kir> {
    pub(super) proof: &'a SourceReferenceCellPointerProofV29<'plan, 'source, 'kir>,
    pub(super) instance: usize,
    pub(super) lowered: &'kir LoweredFunctionResultV1,
}

#[derive(Clone, Copy)]
struct Selected {
    receipt: usize,
    slot: usize,
    bounded: bool,
}

type SpanKey = (u32, Option<u32>, BlockId);

pub(super) struct Context<'a, 'plan, 'source, 'kir> {
    source: Source<'a, 'plan, 'source, 'kir>,
    selected: Vec<Selected>,
    resets: Vec<(BlockId, usize, usize)>,
    anchors: BTreeMap<(BlockId, usize), &'kir ScopedMemoryAnchorV29>,
    spans: BTreeMap<SpanKey, Vec<std::ops::Range<usize>>>,
    blocks: BTreeMap<u32, BlockId>,
    prepared: bool,
}

impl<'a, 'plan, 'source, 'kir> Context<'a, 'plan, 'source, 'kir> {
    pub(super) fn new(
        source: Source<'a, 'plan, 'source, 'kir>,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<Self> {
        source
            .proof
            .plan
            .check_owner(source.proof.plan.instances, budget)?;
        budget.charge_work(3)?;
        if budget.storage() < source.proof.floor
            || !std::ptr::eq(function, &source.lowered.function)
            || source.lowered.source_call_instance.map(|id| id.index()) != Some(source.instance)
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        Ok(Self {
            source,
            selected: Vec::new(),
            resets: Vec::new(),
            anchors: BTreeMap::new(),
            spans: BTreeMap::new(),
            blocks: BTreeMap::new(),
            prepared: false,
        })
    }

    fn row(&self, index: usize) -> &SourceReferenceHistorySelectorV29 {
        &self.source.proof.selectors[self.selected[index].receipt]
    }

    fn find(
        &self,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<Option<usize>> {
        budget.charge_work(call_splice_search_work_v1(self.selected.len()))?;
        Ok(self
            .selected
            .binary_search_by_key(&pointer, |selected| {
                self.source.proof.selectors[selected.receipt].pointer
            })
            .ok())
    }

    fn definition(
        &self,
        graph: &SlotUseGraphV29<'_>,
        definitions: &[Option<PrivateArrayPhysicalLocationV1>],
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<Option<&'kir Operation>> {
        budget.charge_work(4)?;
        let Some(location) = definitions
            .get(graph.value(value, budget)?)
            .copied()
            .flatten()
        else {
            return Ok(None);
        };
        let body = self
            .source
            .lowered
            .function
            .body
            .as_ref()
            .ok_or_else(scoped_slot_error_v29)?;
        let block = body
            .blocks
            .get(location.block_ordinal)
            .ok_or_else(scoped_slot_error_v29)?;
        if block.id != location.block {
            return Err(scoped_slot_error_v29());
        }
        Ok(Some(
            block
                .operations
                .get(location.operation)
                .ok_or_else(scoped_slot_error_v29)?,
        ))
    }

    fn add_span(
        &mut self,
        key: SpanKey,
        first: u32,
        count: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<()> {
        charge_execution_cfg_lookup_v29(self.spans.len(), budget)?;
        if !self.spans.contains_key(&key) {
            reserve_execution_cfg_map_entry_v29::<SpanKey, Vec<std::ops::Range<usize>>>(
                self.spans.len(),
                budget,
            )?;
            self.spans.insert(key, Vec::new());
        }
        let first = usize::try_from(first).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let count = usize::try_from(count).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        emission_push_v1(
            self.spans.get_mut(&key).ok_or_else(scoped_slot_error_v29)?,
            first..argument_sum_v1(&[first, count])?,
            budget,
        )
    }

    fn source_block(&self, block: u32, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<BlockId> {
        charge_execution_cfg_lookup_v29(self.blocks.len(), budget)?;
        self.blocks
            .get(&block)
            .copied()
            .ok_or_else(scoped_slot_error_v29)
    }

    pub(super) fn prepare(
        &mut self,
        graph: &SlotUseGraphV29<'_>,
        definitions: &[Option<PrivateArrayPhysicalLocationV1>],
        slots: &[ScopedSourceSlotV29],
        first_slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<()> {
        if self.prepared {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let proof = self.source.proof;
        budget.charge_work(argument_product_v1(
            2,
            call_splice_search_work_v1(proof.selectors.len()),
        )?)?;
        let first = proof
            .selectors
            .partition_point(|row| row.source.instance.index() < self.source.instance);
        let end = proof
            .selectors
            .partition_point(|row| row.source.instance.index() <= self.source.instance);
        self.selected = emission_vec_v1(end - first, budget)?;
        for receipt in first..end {
            budget.charge_work(9)?;
            let row = proof.selectors[receipt];
            let source = row.source.check(proof.plan.instances, budget)?;
            let occurrences = proof
                .plan
                .instances
                .occurrences(row.source.instance)
                .ok_or_else(scoped_slot_error_v29)?;
            let event = occurrences
                .events()
                .get(row.source.occurrence)
                .ok_or_else(scoped_slot_error_v29)?;
            let (_, original) = proof
                .plan
                .selector_at(
                    row.source.instance,
                    event.site(),
                    source,
                    row.source.projection,
                    budget,
                )?
                .ok_or_else(scoped_slot_error_v29)?;
            if original != row.source {
                return Err(invalid("scoped selector changed its original source key"));
            }
            let Some(slot_index) = graph.exact(row.base, budget)? else {
                continue;
            };
            let slot = slot_index
                .checked_sub(first_slot)
                .and_then(|i| slots.get(i))
                .ok_or_else(scoped_slot_error_v29)?;
            let scalar = slot.scalar_array()?;
            if slot.origin.pointer != row.base
                || slot.instance != row.source.instance
                || slot.legacy_local()? != source.local().index()
                || slot.origin.semantic_type != row.source.array
                || scalar.element_type != row.source.element
                || scalar.length != row.source.length
                || scalar.length == 0
                || argument_sum_v1(&[row.source.projection, 1])? != source.projections().len()
            {
                return Err(invalid(
                    "scoped selector differs from its original array allocation",
                ));
            }
            if let Some(operation) = self.definition(graph, definitions, row.offset, budget)?
                && source_selector_constant_v29(operation, row.offset, ScalarType::Index).is_some()
            {
                continue;
            }
            let body = self
                .source
                .lowered
                .function
                .body
                .as_ref()
                .ok_or_else(scoped_slot_error_v29)?;
            let block = graph.target(row.block, budget)?;
            let operation = block
                .operations
                .get(row.operation)
                .ok_or_else(scoped_slot_error_v29)?;
            if !matches!(operation.kind, OperationKind::GetElementPointer { base, offset }
                if base == row.base && offset == row.offset)
                || graph.result(operation)?.id != row.pointer
                || graph.ty(row.original, budget)? != &Type::Scalar(row.scalar)
                || graph.ty(row.offset, budget)? != &Type::Scalar(ScalarType::Index)
                || body.blocks.is_empty()
            {
                return Err(invalid("scoped selector changed its exact emitted GEP"));
            }
            self.selected.push(Selected {
                receipt,
                slot: slot_index,
                bounded: false,
            });
        }
        if self.selected.is_empty() {
            self.prepared = true;
            return Ok(());
        }
        let lowered = self.source.lowered;
        let instance = lowered
            .source_call_instance
            .ok_or_else(scoped_slot_error_v29)?;
        let occurrences = proof
            .plan
            .instances
            .occurrences(instance)
            .ok_or_else(scoped_slot_error_v29)?;
        for block in &lowered.blocks {
            budget.charge_work(2)?;
            if block.semantic_function != occurrences.function() {
                return Err(scoped_slot_error_v29());
            }
            reserve_execution_cfg_map_entry_v29::<u32, BlockId>(self.blocks.len(), budget)?;
            if self
                .blocks
                .insert(block.semantic_block.index(), block.kernel_ir_block)
                .is_some()
            {
                return Err(scoped_slot_error_v29());
            }
        }
        let anchors = lowered
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_memory_error_v29)?;
        for row in &anchors.rows {
            budget.charge_work(1)?;
            if matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                return Err(scoped_object_pending_v29());
            }
            if !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. }) {
                continue;
            }
            reserve_execution_cfg_map_entry_v29::<(BlockId, usize), &ScopedMemoryAnchorV29>(
                self.anchors.len(),
                budget,
            )?;
            if self
                .anchors
                .insert((row.block, row.position), row)
                .is_some()
            {
                return Err(scoped_memory_error_v29());
            }
        }
        for span in &lowered.statement_operation_spans {
            budget.charge_work(1)?;
            self.add_span(
                (
                    span.semantic_block.index(),
                    Some(span.statement_ordinal),
                    span.kernel_ir_block,
                ),
                span.first_operation_ordinal,
                span.operation_count,
                budget,
            )?;
        }
        for span in &lowered.terminator_operation_spans {
            budget.charge_work(1)?;
            self.add_span(
                (span.semantic_block.index(), None, span.kernel_ir_block),
                span.first_operation_ordinal,
                span.operation_count,
                budget,
            )?;
        }
        bounds::check(self, graph, definitions, budget)?;
        self.check_transport(graph, budget)?;
        self.build_resets(budget)?;
        self.prepared = true;
        Ok(())
    }

    fn check_transport(
        &self,
        graph: &SlotUseGraphV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<()> {
        for (_, block) in &graph.blocks {
            budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
            for operation in &block.operations {
                let mut ordinal = 0;
                if let OperationKind::InlineAssembly(assembly) = &operation.kind {
                    budget.charge_work(assembly.operands.len())?;
                }
                operation.kind.try_visit_operands(|value| {
                    budget.charge_work(1)?;
                    let allowed = ordinal == 0 && matches!(operation.kind,
                        OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
                        | OperationKind::Store { .. } | OperationKind::GuardedStore { .. });
                    ordinal = argument_sum_v1(&[ordinal, 1])?;
                    if self.find(value, budget)?.is_some() && !allowed {
                        return Err(invalid("saved symbolic slot pointers require an exact iteration origin relation"));
                    }
                    Ok(())
                })?;
            }
            let term = block
                .terminator
                .as_ref()
                .ok_or_else(scoped_slot_error_v29)?;
            if let Terminator::Return { values } = term {
                for &value in values {
                    budget.charge_work(1)?;
                    if self.find(value, budget)?.is_some() {
                        return Err(invalid(
                            "symbolic slot pointer return requires an exact origin relation",
                        ));
                    }
                }
            }
            term.try_visit_edges_v1(|_, arguments| {
                for &value in arguments {
                    budget.charge_work(1)?;
                    if self.find(value, budget)?.is_some() {
                        return Err(invalid("symbolic slot pointer edge requires an exact iteration origin relation"));
                    }
                }
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })?;
        }
        Ok(())
    }

    fn build_resets(&mut self, budget: &mut ArgumentBudgetV1<'_>) -> UseResult<()> {
        let plan = self.source.proof.plan;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<(usize, usize)>>(),
            std::mem::size_of::<Result<Vec<(usize, usize)>, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut pairs = emission_vec_v1(self.selected.len(), budget)?;
        for selected in &self.selected {
            budget.charge_work(1)?;
            pairs.push((
                self.source.proof.selectors[selected.receipt]
                    .source
                    .canonical,
                selected.slot,
            ));
        }
        call_splice_sort_work_v1(pairs.len(), budget).map_err(graph_error)?;
        pairs.sort_unstable();
        budget.charge_work(pairs.len())?;
        pairs.dedup();
        charge_execution_cfg_lookup_v29(plan.selector_redefinitions.len(), budget)?;
        for ((_, block), keys) in plan
            .selector_redefinitions
            .range((self.source.instance, 0)..=(self.source.instance, u32::MAX))
        {
            budget.charge_work(1)?;
            let mapped = self.source_block(*block, budget)?;
            for &key in keys {
                budget.charge_work(argument_product_v1(
                    2,
                    call_splice_search_work_v1(pairs.len()),
                )?)?;
                let first = pairs.partition_point(|row| row.0 < key);
                let end = pairs.partition_point(|row| row.0 <= key);
                for &(_, slot) in &pairs[first..end] {
                    budget.charge_work(1)?;
                    emission_push_v1(&mut self.resets, (mapped, slot, key), budget)?;
                }
            }
        }
        call_splice_sort_work_v1(self.resets.len(), budget).map_err(graph_error)?;
        self.resets.sort_unstable();
        budget.charge_work(self.resets.len())?;
        self.resets.dedup();
        Ok(())
    }

    pub(super) fn resets(&self) -> &[(BlockId, usize, usize)] {
        &self.resets
    }

    pub(super) fn cell(
        &self,
        pointer: ValueId,
        slot: usize,
        base: ValueId,
        offset: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<Option<usize>> {
        let Some(index) = self.find(pointer, budget)? else {
            return Ok(None);
        };
        budget.charge_work(4)?;
        let row = self.row(index);
        if !self.prepared
            || !self.selected[index].bounded
            || self.selected[index].slot != slot
            || row.base != base
            || row.offset != offset
        {
            return Err(invalid("scoped selector has no current checked bounds"));
        }
        Ok(Some(row.source.canonical))
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn access(
        &self,
        pointer: ValueId,
        key: usize,
        slot_index: usize,
        block: BlockId,
        operation: usize,
        writing: bool,
        access: &MemoryAccess,
        slot: &ScopedSourceSlotV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> UseResult<()> {
        let index = self.find(pointer, budget)?.ok_or_else(|| {
            invalid("scoped symbolic access is not the direct original indexed producer")
        })?;
        let row = self.row(index);
        self.cell(pointer, slot_index, row.base, row.offset, budget)?;
        budget.charge_work(10)?;
        let scalar = slot.scalar_array()?;
        if row.source.canonical != key
            || row.block != block
            || operation <= row.operation
            || slot.origin.pointer != row.base
            || scalar.length != row.source.length
            || scalar
                .length
                .checked_mul(scalar.element.size)
                .is_none_or(|bytes| bytes > scalar.bytes)
            || !access.alignment.is_power_of_two()
            || access.alignment > scalar.element.alignment
            || scalar.element.size % u64::from(access.alignment) != 0
            || access.address_space != AddressSpace::Private
        {
            return Err(invalid(
                "scoped symbolic access exceeds its exact source cell bounds or alignment",
            ));
        }
        let occurrences = self
            .source
            .proof
            .plan
            .instances
            .occurrences(row.source.instance)
            .ok_or_else(scoped_slot_error_v29)?;
        let event = occurrences
            .events()
            .get(row.source.occurrence)
            .ok_or_else(scoped_slot_error_v29)?;
        let expected_write = matches!(
            event.operand(),
            ExecutionOperandV29::Destination | ExecutionOperandV29::StoreDestination
        );
        if writing != expected_write {
            return Err(invalid(
                "scoped symbolic access changed its original read/write role",
            ));
        }
        charge_execution_cfg_lookup_v29(self.anchors.len(), budget)?;
        let anchor = self
            .anchors
            .get(&(block, operation))
            .ok_or_else(scoped_memory_error_v29)?;
        if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Access { pointer: actual, .. } if actual == pointer)
            || anchor.source
                != Some(ScopedMemoryFrameV29::operand(
                    event.site(),
                    Some(event.operand()),
                ))
        {
            return Err(invalid(
                "scoped symbolic access has no exact source access anchor",
            ));
        }
        let (source_block, statement) = scoped_memory_site_key_v29(event.site());
        charge_execution_cfg_lookup_v29(self.spans.len(), budget)?;
        let spans = self
            .spans
            .get(&(source_block, statement, block))
            .ok_or_else(scoped_memory_error_v29)?;
        budget.charge_work(spans.len())?;
        if spans
            .iter()
            .filter(|span| span.contains(&row.operation) && span.contains(&operation))
            .count()
            != 1
        {
            return Err(invalid(
                "scoped symbolic pointer escaped its original indexed access span",
            ));
        }
        Ok(())
    }
}
