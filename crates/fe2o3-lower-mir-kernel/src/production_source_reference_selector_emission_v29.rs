#[derive(Clone, Copy, Debug)]
enum SourceReferenceSelectorProducerV29 {
    Pending,
    Folded {
        index: u64,
    },
    Address {
        base: ValueId,
        offset: ValueId,
        pointer: ValueId,
        block: BlockId,
        operation: usize,
    },
    ValueTree {
        result: ValueId,
        block: BlockId,
        first: usize,
        end: usize,
    },
}

#[derive(Clone, Copy, Debug)]
struct SourceReferenceSelectorUseV29 {
    original: ValueId,
    memory: Option<usize>,
    scalar: ScalarType,
    length: u64,
    producer: SourceReferenceSelectorProducerV29,
}

// Retained only in the live, source-bound pointer proof. This proves the index
// transport and GEP correspondence, not bounds or physical initialization.
#[derive(Clone, Copy)]
struct SourceReferenceHistorySelectorV29 {
    source: SourceReferenceSelectorV29,
    original: ValueId,
    memory: Option<usize>,
    scalar: ScalarType,
    base: ValueId,
    offset: ValueId,
    pointer: ValueId,
    block: BlockId,
    operation: usize,
}

impl ExecutionAvailabilityV29<'_> {
    fn consume_source_selector(
        &mut self,
        row: SourceReferenceSelectorV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_ledger(budget)?;
        let references = self
            .references
            .ok_or_else(execution_availability_error_v29)?;
        let original = row.check(references.plan.instances, budget)?;
        let event = self
            .occurrences
            .events()
            .get(row.occurrence)
            .ok_or_else(execution_availability_error_v29)?;
        if row.instance != self.instance
            || self.block != Some(execution_event_block_v29(event.site()))
            || !source_reference_selector_place_v29(self.function, event.site(), event.operand())
                .is_some_and(|place| std::ptr::eq(place, original))
            || match row.value {
                SourceReferenceSelectorValueV29::Promoted(value) => {
                    event.resolved()
                        != Some(SsaResolvedEventV1::Use {
                            variable: fe2o3_mir_model::SsaVariableIdV1::new(row.local.index()),
                            value,
                        })
                }
                SourceReferenceSelectorValueV29::Retained { event: original } => {
                    original != row.occurrence || event.is_promoted() || event.resolved().is_some()
                }
            }
        {
            return Err(execution_availability_error_v29());
        }
        self.claim_events(&[row.occurrence], budget)
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn begin_source_selector_v29(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        projection: usize,
        array: SemanticTypeIdV1,
        length: u64,
        original: ValueId,
        scalar: ScalarType,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        if self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .is_none()
        {
            return Ok(None);
        }
        self.with_emission_budget_v1(|this, budget| {
            let cursor = this
                .execution
                .as_mut()
                .ok_or_else(execution_availability_error_v29)?;
            let references = cursor
                .references
                .ok_or_else(execution_availability_error_v29)?;
            let Some((index, row)) = references.plan.selector_at(
                cursor.instance,
                execution_site_v29(block, statement),
                place,
                projection,
                budget,
            )?
            else {
                return Ok(None);
            };
            if row.array != array
                || row.length != length
                || length == 0
                || !matches!(
                    scalar,
                    ScalarType::U8
                        | ScalarType::U16
                        | ScalarType::U32
                        | ScalarType::U64
                        | ScalarType::Index
                )
            {
                return Err(execution_availability_error_v29());
            }
            let memory = match row.value {
                SourceReferenceSelectorValueV29::Promoted(value) => {
                    charge_execution_cfg_lookup_v29(this.semantic_ssa_bindings.len(), budget)?;
                    let held = this
                        .locals
                        .get(row.local.index() as usize)
                        .and_then(Option::as_ref)
                        .ok_or_else(execution_availability_error_v29)?
                        .value()
                        .map_err(|_| execution_availability_error_v29())?;
                    let archived = this
                        .semantic_ssa_bindings
                        .get(&value)
                        .ok_or_else(execution_availability_error_v29)?
                        .value()
                        .map_err(|_| execution_availability_error_v29())?;
                    if held != archived || held != (original, Type::Scalar(scalar)) {
                        return Err(execution_availability_error_v29());
                    }
                    None
                }
                SourceReferenceSelectorValueV29::Retained { event } => {
                    budget.charge_work(10)?;
                    let recorder = this
                        .scoped_memory
                        .as_ref()
                        .ok_or_else(scoped_memory_error_v29)?;
                    let ordinal = recorder
                        .anchors
                        .rows
                        .len()
                        .checked_sub(1)
                        .ok_or_else(scoped_memory_error_v29)?;
                    let anchor = &recorder.anchors.rows[ordinal];
                    let (result, read) =
                        scoped_original_index_payload_v29(&recorder.anchors, anchor, budget)?
                            .ok_or_else(scoped_memory_error_v29)?;
                    check_scoped_index_read_v29(this.function, &cursor.occurrences, read, budget)?;
                    if result != original
                        || read.event != event
                        || read.local != row.local
                        || read.site != execution_site_v29(block, statement)
                        || read.projection as usize != projection
                        || anchor.block != recorder.block.ok_or_else(scoped_memory_error_v29)?
                        || recorder.index_payload.is_some()
                    {
                        return Err(scoped_memory_error_v29());
                    }
                    Some(ordinal)
                }
            };
            references.check(budget)?;
            source_reference_owned_prepay_v29::<SourceReferenceSelectorUseV29>(
                references.plan,
                budget,
            )?;
            source_reference_owned_prepay_v29::<SourceReferenceSelectorProducerV29>(
                references.plan,
                budget,
            )?;
            source_reference_owned_prepay_v29::<Option<usize>>(references.plan, budget)?;
            let claim = references
                .selectors
                .get(index)
                .ok_or_else(execution_availability_error_v29)?;
            if claim.get().is_some() {
                return Err(execution_availability_error_v29());
            }
            cursor.consume_source_selector(row, budget)?;
            claim.set(Some(SourceReferenceSelectorUseV29 {
                original,
                memory,
                scalar,
                length,
                producer: SourceReferenceSelectorProducerV29::Pending,
            }));
            Ok(Some(index))
        })
    }

    fn finish_source_selector_v29(
        &mut self,
        selector: Option<usize>,
        producer: SourceReferenceSelectorProducerV29,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(selector) = selector else {
            return Ok(());
        };
        self.with_emission_budget_v1(|this, budget| {
            let references = this
                .execution
                .as_ref()
                .and_then(|cursor| cursor.references)
                .ok_or_else(execution_availability_error_v29)?;
            references.check(budget)?;
            budget.source_reference_charge_v29(references.plan, 3)?;
            let cell = references
                .selectors
                .get(selector)
                .ok_or_else(execution_availability_error_v29)?;
            let mut claim = cell.get().ok_or_else(execution_availability_error_v29)?;
            if !matches!(claim.producer, SourceReferenceSelectorProducerV29::Pending)
                || matches!(producer, SourceReferenceSelectorProducerV29::Pending)
            {
                return Err(execution_availability_error_v29());
            }
            claim.producer = producer;
            cell.set(Some(claim));
            Ok(())
        })
    }
}

type SourceSelectorDefinitionV29<'a> = (&'a Type, Option<&'a Operation>, Option<(BlockId, usize)>);

include!("production_source_selector_constants_v29.rs");

fn source_selector_constant_v29(
    operation: &Operation,
    value: ValueId,
    scalar: ScalarType,
) -> Option<u64> {
    if operation.results.len() != 1
        || operation.results[0].id != value
        || operation.results[0].ty != Type::Scalar(scalar)
    {
        return None;
    }
    match (&operation.kind, scalar) {
        (OperationKind::Constant(Constant::U8(value)), ScalarType::U8) => Some(u64::from(*value)),
        (OperationKind::Constant(Constant::U16(value)), ScalarType::U16) => Some(u64::from(*value)),
        (OperationKind::Constant(Constant::U32(value)), ScalarType::U32) => Some(u64::from(*value)),
        (OperationKind::Constant(Constant::U64(value)), ScalarType::U64)
        | (OperationKind::Constant(Constant::Index(value)), ScalarType::Index) => Some(*value),
        _ => None,
    }
}

impl SourceReferenceEmissionV29<'_, '_> {
    fn check_source_selectors(
        &self,
        emitted: &[Option<LoweredFunctionResultV1>],
        history: &mut Vec<SourceReferenceHistorySelectorV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        if !history.is_empty() || history.capacity() < self.plan.selectors.len() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if self.plan.selectors.is_empty() && self.plan.descriptors.is_empty() {
            return Ok(());
        }
        let before = budget.storage();
        source_reference_owned_prepay_v29::<
            BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'_>>,
        >(self.plan, budget)?;
        let mut definitions = BTreeMap::new();
        for (instance, lowered) in emitted.iter().enumerate() {
            budget.source_reference_charge_v29(self.plan, 2)?;
            let original = self
                .plan
                .instances
                .id_at(instance)
                .ok_or_else(execution_call_error_v29)?;
            match self.plan.instances.instance_reachable(original) {
                Some(false) if lowered.is_none() => continue,
                Some(true) => {}
                _ => return Err(execution_call_error_v29()),
            }
            let lowered = lowered.as_ref().ok_or_else(execution_call_error_v29)?;
            if lowered.source_call_instance.map(|id| id.index()) != Some(instance) {
                return Err(execution_call_error_v29());
            }
            let body = lowered
                .function
                .body
                .as_ref()
                .ok_or_else(execution_call_error_v29)?;
            for (&value, ty) in body
                .parameters
                .iter()
                .zip(&lowered.function.signature.parameters)
            {
                self.insert_selector_definition(
                    &mut definitions,
                    instance,
                    value,
                    (ty, None, None),
                    budget,
                )?;
            }
            for block in &body.blocks {
                budget.source_reference_charge_v29(self.plan, 1)?;
                for value in &block.parameters {
                    self.insert_selector_definition(
                        &mut definitions,
                        instance,
                        value.id,
                        (&value.ty, None, None),
                        budget,
                    )?;
                }
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    budget.source_reference_charge_v29(self.plan, 1)?;
                    for value in &operation.results {
                        self.insert_selector_definition(
                            &mut definitions,
                            instance,
                            value.id,
                            (&value.ty, Some(operation), Some((block.id, ordinal))),
                            budget,
                        )?;
                    }
                }
            }
        }
        for (index, row) in self.plan.selectors.iter().enumerate() {
            row.check(self.plan.instances, budget)?;
            budget.source_reference_charge_v29(self.plan, 5)?;
            let used = self
                .selectors
                .get(index)
                .and_then(|claim| claim.get())
                .ok_or_else(execution_availability_error_v29)?;
            let instance = row.instance.index();
            let definition =
                self.selector_definition(&definitions, instance, used.original, budget)?;
            if *definition.0 != Type::Scalar(used.scalar) || used.length != row.length {
                return Err(execution_availability_error_v29());
            }
            match (row.value, used.memory) {
                (SourceReferenceSelectorValueV29::Promoted(_), None) => {}
                (SourceReferenceSelectorValueV29::Retained { event }, Some(ordinal)) => {
                    budget.source_reference_charge_v29(self.plan, 8)?;
                    let lowered = emitted
                        .get(instance)
                        .and_then(Option::as_ref)
                        .ok_or_else(scoped_memory_error_v29)?;
                    let anchors = lowered
                        .scoped_memory_anchors
                        .as_ref()
                        .ok_or_else(scoped_memory_error_v29)?;
                    let anchor = anchors
                        .rows
                        .get(ordinal)
                        .ok_or_else(scoped_memory_error_v29)?;
                    let (result, read) =
                        scoped_original_index_payload_v29(anchors, anchor, budget)?
                            .ok_or_else(scoped_memory_error_v29)?;
                    let original = self
                        .plan
                        .instances
                        .instance(row.instance)
                        .ok_or_else(scoped_memory_error_v29)?;
                    let occurrences = self
                        .plan
                        .instances
                        .occurrences(row.instance)
                        .ok_or_else(scoped_memory_error_v29)?;
                    check_scoped_index_payload_v29(
                        original.declaration(),
                        &occurrences,
                        anchor,
                        definition.1.ok_or_else(scoped_memory_error_v29)?,
                        result,
                        read,
                        budget,
                    )?;
                    if result != used.original
                        || read.event != event
                        || read.local != row.local
                        || read.projection as usize != row.projection
                        || definition.2 != Some((anchor.block, anchor.position))
                    {
                        return Err(scoped_memory_error_v29());
                    }
                }
                _ => return Err(scoped_memory_error_v29()),
            }
            match used.producer {
                SourceReferenceSelectorProducerV29::Pending => {
                    return Err(execution_availability_error_v29());
                }
                SourceReferenceSelectorProducerV29::Folded { index } => {
                    budget.source_reference_charge_v29(self.plan, 3)?;
                    if index >= row.length
                        || source_selector_unsigned_constant_v29(
                            used.original,
                            used.scalar,
                            definitions.len(),
                            budget,
                            |value, budget| {
                                self.selector_definition(&definitions, instance, value, budget)
                                    .map(|(ty, operation, _)| (ty, operation))
                            },
                        )? != Some(index)
                    {
                        return Err(execution_availability_error_v29());
                    }
                }
                SourceReferenceSelectorProducerV29::Address {
                    base,
                    offset,
                    pointer,
                    block,
                    operation,
                } => {
                    let gep = self.selector_definition(&definitions, instance, pointer, budget)?;
                    if gep.2 != Some((block, operation))
                        || !matches!(gep.1.map(|op| &op.kind),
                        Some(OperationKind::GetElementPointer { base: actual, offset: index }) if *actual == base && *index == offset)
                    {
                        return Err(execution_availability_error_v29());
                    }
                    self.check_selector_span(
                        row,
                        emitted[instance].as_ref().unwrap(),
                        block,
                        operation,
                        budget,
                    )?;
                    self.check_selector_offset(
                        row,
                        used,
                        offset,
                        &definitions,
                        block,
                        operation,
                        budget,
                    )?;
                    budget.source_reference_charge_v29(self.plan, 1)?;
                    if history.len() == history.capacity() {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    history.push(SourceReferenceHistorySelectorV29 {
                        source: *row,
                        original: used.original,
                        memory: used.memory,
                        scalar: used.scalar,
                        base,
                        offset,
                        pointer,
                        block,
                        operation,
                    });
                }
                SourceReferenceSelectorProducerV29::ValueTree {
                    result,
                    block,
                    first,
                    end,
                } => {
                    if end > first {
                        self.check_selector_span(
                            row,
                            emitted[instance].as_ref().unwrap(),
                            block,
                            first,
                            budget,
                        )?;
                        self.check_selector_span(
                            row,
                            emitted[instance].as_ref().unwrap(),
                            block,
                            end - 1,
                            budget,
                        )?;
                    }
                    self.check_selector_value_tree(
                        row,
                        used,
                        result,
                        block,
                        first,
                        end,
                        &definitions,
                        budget,
                    )?;
                }
            }
        }
        self.check_source_descriptors(emitted, &definitions, budget)?;
        let owned = budget
            .storage()
            .checked_sub(before)
            .ok_or(ArgumentResourceV1::Accounting)?;
        drop(definitions);
        budget.release_storage(owned)?;
        call_splice_sort_work_v1(history.len(), budget).map_err(|error| match error {
            CallInstanceEmissionErrorV1::Resource(error) => error.into(),
            _ => execution_availability_error_v29(),
        })?;
        history.sort_unstable_by_key(|row| (row.source.instance.index(), row.pointer));
        budget.source_reference_charge_v29(self.plan, history.len())?;
        if history.windows(2).any(|rows| {
            (rows[0].source.instance, rows[0].pointer) == (rows[1].source.instance, rows[1].pointer)
        }) {
            return Err(execution_availability_error_v29());
        }
        Ok(())
    }

    fn insert_selector_definition<'a>(
        &self,
        definitions: &mut BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'a>>,
        instance: usize,
        value: ValueId,
        definition: SourceSelectorDefinitionV29<'a>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        reserve_execution_cfg_map_entry_v29::<(usize, ValueId), SourceSelectorDefinitionV29<'_>>(
            definitions.len(),
            budget,
        )?;
        if definitions.insert((instance, value), definition).is_some() {
            return Err(execution_availability_error_v29());
        }
        Ok(())
    }

    fn selector_definition<'a>(
        &self,
        definitions: &BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'a>>,
        instance: usize,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceSelectorDefinitionV29<'a>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(definitions.len(), budget)?;
        definitions
            .get(&(instance, value))
            .copied()
            .ok_or_else(execution_availability_error_v29)
    }

    fn check_selector_span(
        &self,
        row: &SourceReferenceSelectorV29,
        lowered: &LoweredFunctionResultV1,
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_source_occurrence_span(
            row.instance,
            row.occurrence,
            lowered,
            block,
            operation,
            budget,
        )
    }

    fn check_source_occurrence_span(
        &self,
        instance: ProductionCallInstanceIdV1,
        occurrence: usize,
        lowered: &LoweredFunctionResultV1,
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let occurrences = self
            .plan
            .instances
            .occurrences(instance)
            .ok_or_else(execution_availability_error_v29)?;
        let (source_block, statement) =
            scoped_memory_site_key_v29(occurrences.events()[occurrence].site());
        let count = if let Some(statement) = statement {
            budget.charge_work(argument_product_v1(
                lowered.statement_operation_spans.len(),
                6,
            )?)?;
            lowered
                .statement_operation_spans
                .iter()
                .filter(|span| {
                    span.semantic_block.index() == source_block
                        && span.statement_ordinal == statement
                        && span.kernel_ir_block == block
                        && operation >= span.first_operation_ordinal as usize
                        && (span.first_operation_ordinal as usize)
                            .checked_add(span.operation_count as usize)
                            .is_some_and(|end| operation < end)
                })
                .count()
        } else {
            budget.charge_work(argument_product_v1(
                lowered.terminator_operation_spans.len(),
                5,
            )?)?;
            lowered
                .terminator_operation_spans
                .iter()
                .filter(|span| {
                    span.semantic_block.index() == source_block
                        && span.kernel_ir_block == block
                        && operation >= span.first_operation_ordinal as usize
                        && (span.first_operation_ordinal as usize)
                            .checked_add(span.operation_count as usize)
                            .is_some_and(|end| operation < end)
                })
                .count()
        };
        if count != 1 {
            return Err(execution_availability_error_v29());
        }
        Ok(())
    }

    fn check_selector_offset(
        &self,
        row: &SourceReferenceSelectorV29,
        used: SourceReferenceSelectorUseV29,
        offset: ValueId,
        definitions: &BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'_>>,
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_source_index_normalization(
            row.instance.index(),
            used.original,
            used.scalar,
            Some(row.length),
            offset,
            definitions,
            block,
            operation,
            budget,
        )
    }

    fn check_source_index_normalization(
        &self,
        instance: usize,
        original_value: ValueId,
        original_scalar: ScalarType,
        fixed_extent: Option<u64>,
        offset: ValueId,
        definitions: &BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'_>>,
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let original = self.selector_definition(definitions, instance, original_value, budget)?;
        let lowered = self.selector_definition(definitions, instance, offset, budget)?;
        if *lowered.0 != Type::INDEX || *original.0 != Type::Scalar(original_scalar) {
            return Err(execution_availability_error_v29());
        }
        if let Some(constant) = source_selector_unsigned_constant_v29(
            original_value,
            original_scalar,
            definitions.len(),
            budget,
            |value, budget| {
                self.selector_definition(definitions, instance, value, budget)
                    .map(|(ty, operation, _)| (ty, operation))
            },
        )? {
            budget.charge_work(3)?;
            if fixed_extent.is_some_and(|length| constant >= length) {
                return Err(execution_availability_error_v29());
            }
            if lowered
                .1
                .and_then(|op| source_selector_constant_v29(op, offset, ScalarType::Index))
                == Some(constant)
            {
                return Ok(());
            }
            // Keep the existing literal fixed-array rule. A runtime descriptor
            // may retain the exact lossless cast instead of folding its index.
            if fixed_extent.is_some() {
                return Err(execution_availability_error_v29());
            }
        }
        let path = plan_integer_cast_v1(original_scalar, ScalarType::Index)
            .ok_or_else(execution_availability_error_v29)?;
        let mut value = offset;
        for (kind, scalar) in path.into_iter().flatten().rev() {
            let definition = self.selector_definition(definitions, instance, value, budget)?;
            if *definition.0 != Type::Scalar(scalar)
                || definition
                    .2
                    .is_none_or(|(owner, ordinal)| owner != block || ordinal >= operation)
            {
                return Err(execution_availability_error_v29());
            }
            let Some(OperationKind::Cast {
                kind: actual,
                value: input,
                to,
            }) = definition.1.map(|op| &op.kind)
            else {
                return Err(execution_availability_error_v29());
            };
            if *actual != kind || *to != Type::Scalar(scalar) {
                return Err(execution_availability_error_v29());
            }
            value = *input;
        }
        if value != original_value {
            return Err(execution_availability_error_v29());
        }
        Ok(())
    }

    fn check_selector_value_tree(
        &self,
        row: &SourceReferenceSelectorV29,
        used: SourceReferenceSelectorUseV29,
        result: ValueId,
        block: BlockId,
        first: usize,
        end: usize,
        definitions: &BTreeMap<(usize, ValueId), SourceSelectorDefinitionV29<'_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let count = usize::try_from(row.length).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let expected = count
            .checked_sub(1)
            .and_then(|count| count.checked_mul(3))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if end.checked_sub(first) != Some(expected) {
            return Err(execution_availability_error_v29());
        }
        // The producer already represents these fields. Validation uses only
        // logarithmic pending depth, never an array-cardinality state vector.
        let depth = row
            .length
            .checked_ilog2()
            .ok_or_else(execution_availability_error_v29)? as usize
            + 2;
        let mut pending = source_reference_scratch_v29(depth, budget)?;
        emission_push_v1(&mut pending, (result, 0_u64, row.length), budget)?;
        while let Some((value, start, count)) = pending.pop() {
            budget.charge_work(6)?;
            let definition =
                self.selector_definition(definitions, row.instance.index(), value, budget)?;
            if count == 1 {
                if definition.2.is_some_and(|(owner, ordinal)| {
                    owner == block && ordinal >= first && ordinal < end
                }) {
                    return Err(execution_availability_error_v29());
                }
                continue;
            }
            if definition
                .2
                .is_none_or(|(owner, ordinal)| owner != block || ordinal < first || ordinal >= end)
            {
                return Err(execution_availability_error_v29());
            }
            let Some(OperationKind::Select {
                condition,
                true_value,
                false_value,
            }) = definition.1.map(|op| &op.kind)
            else {
                return Err(execution_availability_error_v29());
            };
            let compare =
                self.selector_definition(definitions, row.instance.index(), *condition, budget)?;
            let Some(OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            }) = compare.1.map(|op| &op.kind)
            else {
                return Err(execution_availability_error_v29());
            };
            if *lhs != used.original
                || *compare.0 != Type::BOOL
                || compare.2.is_none_or(|(owner, ordinal)| {
                    owner != block || ordinal < first || ordinal >= end
                })
            {
                return Err(execution_availability_error_v29());
            }
            let midpoint = count / 2;
            let split = start
                .checked_add(midpoint)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let bound =
                self.selector_definition(definitions, row.instance.index(), *rhs, budget)?;
            if bound
                .1
                .and_then(|op| source_selector_constant_v29(op, *rhs, used.scalar))
                != Some(split)
                || bound.2.is_none_or(|(owner, ordinal)| {
                    owner != block || ordinal < first || ordinal >= end
                })
            {
                return Err(execution_availability_error_v29());
            }
            emission_push_v1(
                &mut pending,
                (*false_value, split, count - midpoint),
                budget,
            )?;
            emission_push_v1(&mut pending, (*true_value, start, midpoint), budget)?;
        }
        source_reference_drop_scratch_v29(pending, budget)
    }
}
