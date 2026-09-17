// Source cell state is independent of core physical constant propagation.
#[derive(Clone, Copy)]
struct UnitLocalMemorySlotV1 {
    source_type: SemanticTypeIdV1,
    element_type: SemanticTypeIdV1,
    element: ScalarType,
    count: u64,
    bytes: u64,
    alignment: u32,
    array: bool,
    cells: usize,
    physical: usize,
    row: usize,
    array_slot: Option<usize>,
    pointer: ValueId,
}

#[derive(Clone, Copy, Default)]
struct UnitLocalMemoryLocalV1 {
    promoted: bool,
    candidate: bool,
    epoch: u64,
    slot: Option<UnitLocalMemorySlotV1>,
}

#[derive(Clone, Copy)]
struct UnitLocalStoredCellV1 {
    source: usize,
    value: usize,
    physical: usize,
    epoch: u64,
}

struct UnitLocalMemoryStateV1 {
    locals: Vec<UnitLocalMemoryLocalV1>,
    cells: Vec<Option<UnitLocalStoredCellV1>>,
    instance: Option<PrivateArrayInstanceV1>,
    next_access: usize,
    array_slots: usize,
    array_effects: usize,
}

impl UnitLocalMemoryStateV1 {
    fn empty() -> Self {
        Self {
            locals: Vec::new(),
            cells: Vec::new(),
            instance: None,
            next_access: 0,
            array_slots: 0,
            array_effects: 0,
        }
    }

    fn storage_bytes(&self) -> Result<usize, ProductionSemanticKirErrorV1> {
        argument_product_v1(
            self.locals.capacity(),
            std::mem::size_of::<UnitLocalMemoryLocalV1>(),
        )?
        .checked_add(argument_product_v1(
            self.cells.capacity(),
            std::mem::size_of::<Option<UnitLocalStoredCellV1>>(),
        )?)
        .ok_or_else(|| ArgumentResourceV1::Arithmetic.into())
    }
}

struct UnitLocalMemoryWorkV1<'b, 'w>(&'b mut ArgumentBudgetV1<'w>);
impl PrivateArrayChargeV1 for UnitLocalMemoryWorkV1<'_, '_> {
    type Error = ProductionSemanticKirErrorV1;
    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount).map_err(Into::into)
    }
}

impl<'a, 'r> SourceLocalCursorV1<'a, 'r> {
    fn memory_candidate(
        &mut self,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let state = self
            .memory
            .locals
            .get_mut(place.local().index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        if !state.promoted
            && !matches!(
                place.projections().first().map(|p| p.kind()),
                Some(SemanticProjectionKindV1::Dereference)
            )
        {
            state.candidate = true;
        }
        Ok(())
    }

    fn memory_layout(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<UnitLocalMemorySlotV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(5)?;
        let types = self.input.subject.semantic_ssa.source_semantic().types();
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        let (element_type, count, array) = match declaration.shape() {
            SemanticTypeShapeV1::Array { element, length } => (*element, *length, true),
            _ => (ty, 1, false),
        };
        let element = self.source_scalar(element_type, budget)?;
        let facts = private_retained_slot_facts_v1(
            types,
            element_type,
            &mut UnitLocalMemoryWorkV1(budget),
        )?
        .ok_or_else(unit_local_mismatch_v1)?;
        budget.charge_work(10)?;
        if facts.element != PrivateRetainedElementFactsV1::Scalar(element) || count == 0 {
            return Err(unit_local_mismatch_v1());
        }
        let extent = count
            .checked_mul(facts.size)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if extent > i64::MAX as u64
            || declaration.layout().is_uninhabited()
            || declaration.layout().size_bytes() != Some(extent)
            || declaration.layout().alignment_bytes() != u64::from(facts.alignment)
        {
            return Err(unit_local_mismatch_v1());
        }
        if array
            && !matches!(declaration.layout().fields(), SemanticFieldsShapeV1::Array { stride_bytes, count: actual }
            if *stride_bytes == facts.size && *actual == count)
        {
            return Err(unit_local_mismatch_v1());
        }
        Ok(UnitLocalMemorySlotV1 {
            source_type: ty,
            element_type,
            element,
            count,
            bytes: facts.size,
            alignment: facts.alignment,
            array,
            cells: 0,
            physical: 0,
            row: 0,
            array_slot: None,
            pointer: ValueId(0),
        })
    }

    fn initialize_memory(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        if !self.memory.locals.is_empty() || !self.memory.cells.is_empty() {
            return Err(unit_local_mismatch_v1());
        }
        self.memory.locals = unit_local_vec_v1(self.input.source.locals().len(), budget)?;
        for _ in self.input.source.locals() {
            unit_local_push_v1(
                &mut self.memory.locals,
                UnitLocalMemoryLocalV1::default(),
                budget,
            )?;
        }
        budget.charge_work(argument_product_v1(
            self.input.plan.plan().promoted_variables().len(),
            2,
        )?)?;
        for local in self.input.plan.plan().promoted_variables() {
            self.memory
                .locals
                .get_mut(local.get() as usize)
                .ok_or_else(unit_local_mismatch_v1)?
                .promoted = true;
        }
        budget.charge_work(argument_product_v1(
            self.input.plan.retained_cross_edge_variables().len(),
            3,
        )?)?;
        for local in self.input.plan.retained_cross_edge_variables() {
            let state = self
                .memory
                .locals
                .get_mut(local.get() as usize)
                .ok_or_else(unit_local_mismatch_v1)?;
            state.candidate = !state.promoted;
        }
        for block in self.input.source.blocks() {
            budget.charge_work(2)?;
            for statement in block.statements() {
                budget.charge_work(2)?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        if !assignment.destination().projections().is_empty() {
                            self.memory_candidate(assignment.destination(), budget)?;
                        }
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Borrow { place, .. }
                            | SemanticRvalueKindV1::AddressOf { place, .. } => {
                                self.memory_candidate(place, budget)?
                            }
                            SemanticRvalueKindV1::Load(load) => {
                                self.memory_candidate(load.source(), budget)?
                            }
                            _ => {}
                        }
                    }
                    SemanticStatementKindV1::Store(store) => {
                        self.memory_candidate(store.destination(), budget)?
                    }
                    SemanticStatementKindV1::AtomicRmw(_)
                    | SemanticStatementKindV1::AtomicCompareExchange(_) => {
                        return Err(self.error("local helper atomic source memory is unsupported"));
                    }
                    _ => {}
                }
            }
            if let SemanticTerminatorKindV1::Call(_) = block.terminator().kind() {
                return Err(self.error("local helper body calls are unsupported"));
            }
        }
        let mut cells = 0usize;
        let mut allocations = 0usize;
        for local in 0..self.memory.locals.len() {
            budget.charge_work(2)?;
            if !self.memory.locals[local].candidate {
                continue;
            }
            let declaration = &self.input.source.locals()[local];
            if declaration.role().is_entry_argument() {
                return Err(unit_local_mismatch_v1());
            }
            let mut slot = self.memory_layout(declaration.ty(), budget)?;
            budget.charge_work(4)?;
            slot.cells = cells;
            cells = cells
                .checked_add(
                    usize::try_from(slot.count).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                )
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            allocations = allocations
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            self.memory.locals[local].slot = Some(slot);
        }
        budget.charge_work(3)?;
        if allocations
            != self
                .input
                .body
                .allocations
                .1
                .checked_sub(self.input.body.allocations.0)
                .ok_or(ArgumentResourceV1::Arithmetic)?
        {
            return Err(unit_local_mismatch_v1());
        }
        self.memory.cells = unit_local_vec_v1(cells, budget)?;
        for _ in 0..cells {
            unit_local_push_v1(&mut self.memory.cells, None, budget)?;
        }
        let records: &'a PrivateArrayCorrespondenceV1 =
            &self.input.subject.correspondence.private_arrays;
        budget.charge_work(argument_product_v1(records.instances.len(), 5)?)?;
        for instance in &records.instances {
            if instance.owner == self.input.key.root && instance.function == self.input.key.function
            {
                if self.memory.instance.is_some()
                    || instance.module_function_ordinal != self.input.key.physical
                    || instance.slot_start > instance.slot_end
                    || instance.slot_end > records.slots.len()
                    || instance.effect_start > instance.effect_end
                    || instance.effect_end > records.effects.len()
                {
                    return Err(unit_local_mismatch_v1());
                }
                self.memory.instance = Some(*instance);
            }
        }
        if let Some(instance) = self.memory.instance {
            budget.charge_work(argument_product_v1(
                instance.effect_end - instance.effect_start,
                12,
            )?)?;
            let mut previous = None;
            for effect in &records.effects[instance.effect_start..instance.effect_end] {
                let key = private_array_effect_key_v1(effect);
                if effect.owner != self.input.key.root
                    || effect.function != self.input.key.function
                    || private_array_role_key_v1(effect.role).is_none()
                    || previous.is_some_and(|old| old >= key)
                {
                    return Err(unit_local_mismatch_v1());
                }
                previous = Some(key);
            }
        }
        let mut span = None;
        budget.charge_work(argument_product_v1(
            self.input
                .subject
                .correspondence
                .synthetic_operation_spans
                .len(),
            4,
        )?)?;
        for row in &self.input.subject.correspondence.synthetic_operation_spans {
            if row.correspondence_owner == self.input.key.root
                && row.semantic_function == self.input.key.function
                && row.rule == SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage
            {
                if span.is_some() {
                    return Err(unit_local_mismatch_v1());
                }
                span = Some(row);
            }
        }
        self.memory.next_access = self.input.body.accesses.0;
        if allocations == 0 {
            if span.is_some() || self.memory.instance.is_some() {
                return Err(unit_local_mismatch_v1());
            }
            return Ok(());
        }
        let span = span.ok_or_else(unit_local_mismatch_v1)?;
        let (block_ordinal, block) = self.physical_block(span.kernel_ir_block, budget)?;
        budget.charge_work(4)?;
        if block.id != BlockId(self.input.source.entry().index())
            || span.first_operation_ordinal != 0
            || self.native_span.is_some()
        {
            return Err(unit_local_mismatch_v1());
        }
        self.native_span = Some((block_ordinal, 0, span.operation_count as usize));
        let mut allocation = self.input.body.allocations.0;
        for local in 0..self.memory.locals.len() {
            budget.charge_work(2)?;
            let Some(mut slot) = self.memory.locals[local].slot else {
                continue;
            };
            let count = if slot.array {
                Some(self.memory_index_constant(slot.count, budget)?)
            } else {
                None
            };
            let (operation, at) = self.take_native_operation(budget)?;
            budget.charge_work(16)?;
            let OperationKind::Alloca {
                element,
                count: actual_count,
                address_space,
                alignment,
            } = &operation.kind
            else {
                return Err(unit_local_mismatch_v1());
            };
            let physical = self
                .input
                .physical
                .allocations
                .get(allocation)
                .ok_or_else(unit_local_mismatch_v1)?;
            if *element != Type::Scalar(slot.element)
                || *actual_count != count
                || *address_space != AddressSpace::Private
                || *alignment != slot.alignment
                || operation.results.len() != 1
                || !unit_local_private_pointer_v1(&operation.results[0].ty, slot.element, budget)?
                || physical.location().function_ordinal() != self.input.key.physical
                || physical.location().block() != block.id
                || physical.location().operation() != at.operation as usize
                || physical.pointer() != operation.results[0].id
                || physical.element() != slot.element
                || physical.element_bytes() != slot.bytes
                || physical.count() != slot.count
                || physical.byte_extent()
                    != slot
                        .count
                        .checked_mul(slot.bytes)
                        .ok_or(ArgumentResourceV1::Arithmetic)?
                || physical.alignment() != slot.alignment
            {
                return Err(unit_local_mismatch_v1());
            }
            slot.pointer = physical.pointer();
            slot.physical = allocation;
            if slot.array {
                let instance = self.memory.instance.ok_or_else(unit_local_mismatch_v1)?;
                budget.charge_work(19)?;
                let index = instance
                    .slot_start
                    .checked_add(self.memory.array_slots)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                if index >= instance.slot_end {
                    return Err(unit_local_mismatch_v1());
                }
                let row = records.slots[index];
                if row.local as usize != local
                    || row.owner != self.input.key.root
                    || row.function != self.input.key.function
                {
                    return Err(unit_local_mismatch_v1());
                }
                if row.semantic_type != slot.source_type
                    || row.element_type != slot.element_type
                    || row.pointer != slot.pointer
                    || Some(row.count) != count
                    || row.length != slot.count
                    || row.element_facts.element
                        != PrivateRetainedElementFactsV1::Scalar(slot.element)
                    || row.element_facts.size != slot.bytes
                    || row.element_facts.alignment != slot.alignment
                    || row.alloca_location.block_ordinal != block_ordinal
                    || row.alloca_location.block != block.id
                    || row.alloca_location.operation != at.operation as usize
                    || row.count_location.block_ordinal != block_ordinal
                    || row.count_location.block != block.id
                    || row.count_location.operation.checked_add(1) != Some(at.operation as usize)
                {
                    return Err(unit_local_mismatch_v1());
                }
                slot.array_slot = Some(index);
                self.memory.array_slots = self
                    .memory
                    .array_slots
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            slot.row = unit_local_push_v1(
                &mut self.rows.memory,
                UnitLocalMemoryRowV1::Allocation {
                    key: self.input.key,
                    local: SemanticLocalIdV1::from_index(
                        u32::try_from(local).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    ),
                    source_type: slot.source_type,
                    physical_allocation: allocation,
                    array_slot: slot.array_slot,
                    element: slot.element,
                    count: slot.count,
                    element_bytes: slot.bytes,
                    alignment: slot.alignment,
                },
                budget,
            )?;
            budget.charge_work(3)?;
            self.locals
                .get_mut(local)
                .ok_or_else(unit_local_mismatch_v1)?
                .allocation = Some(slot.row);
            self.memory.locals[local].slot = Some(slot);
            allocation += 1;
        }
        if self.peek_native_operation(budget)?.is_some() {
            return Err(unit_local_mismatch_v1());
        }
        self.native_span = None;
        budget.charge_work(2)?;
        if self.memory.array_slots
            != self
                .memory
                .instance
                .map_or(0, |i| i.slot_end - i.slot_start)
        {
            return Err(unit_local_mismatch_v1());
        }
        Ok(())
    }

    fn memory_index_constant(
        &mut self,
        expected: u64,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        let (operation, _) = self.take_native_operation(budget)?;
        budget.charge_work(4)?;
        if operation.kind != OperationKind::Constant(Constant::Index(expected))
            || operation.results.len() != 1
            || operation.results[0].ty != Type::Scalar(ScalarType::Index)
        {
            return Err(unit_local_mismatch_v1());
        }
        Ok(operation.results[0].id)
    }

    fn memory_slot(
        &self,
        local: SemanticLocalIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<UnitLocalMemorySlotV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let slot = self
            .memory
            .locals
            .get(local.index() as usize)
            .and_then(|s| s.slot)
            .ok_or_else(unit_local_mismatch_v1)?;
        if self
            .locals
            .get(local.index() as usize)
            .and_then(|s| s.allocation)
            != Some(slot.row)
        {
            return Err(unit_local_mismatch_v1());
        }
        Ok(slot)
    }

    fn memory_address(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        place: &SemanticPlaceV1,
        component: Option<u32>,
        diagnostic: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(UnitLocalMemorySlotV1, u64, ValueId, Option<usize>), ProductionSemanticKirErrorV1>
    {
        let slot = self.memory_slot(place.local(), budget)?;
        budget.charge_work(5)?;
        if !slot.array {
            if !place.projections().is_empty()
                || component.is_some()
                || place.ty() != slot.source_type
            {
                return Err(unit_local_mismatch_v1());
            }
            return Ok((slot, 0, slot.pointer, None));
        }
        let mut index_row = None;
        let cell = if let Some(component) = component {
            if !place.projections().is_empty() || place.ty() != slot.source_type || diagnostic {
                return Err(unit_local_mismatch_v1());
            }
            u64::from(component)
        } else {
            if place.projections().len() != 1
                || place.ty() != slot.element_type
                || place.projections()[0].result_type() != slot.element_type
            {
                return Err(unit_local_mismatch_v1());
            }
            match place.projections()[0].kind() {
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length,
                    from_end,
                } => {
                    if minimum_length > slot.count {
                        return Err(unit_local_mismatch_v1());
                    }
                    if from_end {
                        slot.count
                            .checked_sub(offset)
                            .ok_or_else(unit_local_mismatch_v1)?
                    } else {
                        offset
                    }
                }
                SemanticProjectionKindV1::Index(local) => {
                    let mode = if diagnostic {
                        UnitLocalOperandUseV1::Diagnostic
                    } else {
                        UnitLocalOperandUseV1::Next
                    };
                    let row = self.resolve_index_local(site, role, 0, local, mode, budget)?;
                    budget.charge_work(3)?;
                    let value = *self.value(row).ok_or_else(unit_local_mismatch_v1)?;
                    let scalar = self.source_scalar(value.source_type, budget)?;
                    if scalar == ScalarType::Bool {
                        return Err(unit_local_mismatch_v1());
                    }
                    index_row = Some(row);
                    value
                        .known_bits
                        .ok_or_else(|| self.error("local helper source index is not known"))?
                }
                _ => return Err(self.error("local helper memory projection is unsupported")),
            }
        };
        budget.charge_work(2)?;
        if cell >= slot.count {
            return Err(self.error("local helper source index is out of bounds"));
        }
        if diagnostic {
            return Ok((slot, cell, slot.pointer, index_row));
        }
        let offset = if let Some(row) = index_row {
            let value = *self.value(row).ok_or_else(unit_local_mismatch_v1)?;
            let UnitLocalNativeValueV1::Scalar {
                value: original,
                scalar,
                ..
            } = value.native
            else {
                return Err(unit_local_mismatch_v1());
            };
            if self
                .peek_native_operation(budget)?
                .is_some_and(|(o, _)| matches!(o.kind, OperationKind::Constant(Constant::Index(_))))
            {
                self.memory_index_constant(cell, budget)?
            } else {
                budget.charge_work(3)?;
                let path = fe2o3_kernel_ir::plan_integer_cast_v1(scalar, ScalarType::Index)
                    .ok_or_else(unit_local_mismatch_v1)?;
                let mut previous = original;
                for (kind, target) in path.into_iter().flatten() {
                    let (operation, _) = self.take_native_operation(budget)?;
                    budget.charge_work(5)?;
                    if operation.results.len() != 1
                        || operation.results[0].ty != Type::Scalar(target)
                        || !matches!(&operation.kind, OperationKind::Cast { kind: actual, value, to } if *actual == kind && *value == previous && *to == Type::Scalar(target))
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    previous = operation.results[0].id;
                }
                previous
            }
        } else {
            self.memory_index_constant(cell, budget)?
        };
        let (operation, _) = self.take_native_operation(budget)?;
        budget.charge_work(5)?;
        if operation.results.len() != 1
            || !unit_local_private_pointer_v1(&operation.results[0].ty, slot.element, budget)?
            || !matches!(&operation.kind, OperationKind::GetElementPointer { base, offset: actual } if *base == slot.pointer && *actual == offset)
        {
            return Err(unit_local_mismatch_v1());
        }
        Ok((slot, cell, operation.results[0].id, index_row))
    }

    fn memory_effect(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        place: &SemanticPlaceV1,
        component: Option<u32>,
        slot: UnitLocalMemorySlotV1,
        cell: u64,
        pointer: ValueId,
        index_row: Option<usize>,
        at: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        write: bool,
        value: ValueId,
        value_definition: Option<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<([usize; 7], usize), ProductionSemanticKirErrorV1> {
        budget.charge_work(16)?;
        let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
            site
        else {
            return Err(self.error("local helper native memory access requires a statement site"));
        };
        let (class, ordinal) =
            private_array_role_key_v1(role).ok_or_else(unit_local_mismatch_v1)?;
        let key = [
            self.input.key.root.index() as usize,
            self.input.key.function.index() as usize,
            block.get() as usize,
            statement as usize,
            class as usize,
            ordinal as usize,
            component.unwrap_or(0) as usize,
        ];
        let index = self.memory.next_access;
        if index >= self.input.body.accesses.1 {
            return Err(unit_local_mismatch_v1());
        }
        let physical = self.input.physical.accesses[index];
        let body = self
            .input
            .physical_function
            .body
            .as_ref()
            .ok_or_else(unit_local_mismatch_v1)?;
        let actual_block = body
            .blocks
            .get(at.block.block as usize)
            .ok_or_else(unit_local_mismatch_v1)?
            .id;
        if physical.location().function_ordinal() != self.input.key.physical
            || physical.location().block() != actual_block
            || physical.location().operation() != at.operation as usize
            || physical
                .allocation()
                .checked_add(self.input.body.allocations.0)
                != Some(slot.physical)
            || physical.cell() != cell
            || physical.pointer() != pointer
            || physical.value() != value
            || physical.kind()
                != if write {
                    fe2o3_kernel_ir::LocalFrameAccessKindV1::Write
                } else {
                    fe2o3_kernel_ir::LocalFrameAccessKindV1::Read
                }
        {
            return Err(unit_local_mismatch_v1());
        }
        if slot.array {
            let instance = self.memory.instance.ok_or_else(unit_local_mismatch_v1)?;
            let records = &self.input.subject.correspondence.private_arrays;
            let effects = &records.effects[instance.effect_start..instance.effect_end];
            let found = assert_origin_find_v1(effects, budget, |effect, budget| {
                budget.charge_work(9)?;
                Ok(private_array_effect_key_v1(effect).cmp(&key))
            })
            .map_err(call_index_error_v1)?
            .ok_or_else(unit_local_mismatch_v1)?;
            let effect = &effects[found];
            let span = self.statement_span(site, budget)?;
            budget.charge_work(23)?;
            if effect.local != place.local().index()
                || effect.semantic_type != place.ty()
                || effect.memory_location.block != actual_block
                || effect.memory_location.block_ordinal != at.block.block as usize
                || effect.memory_location.operation != at.operation as usize
                || effect.gep != pointer
                || effect.gep_location.block != actual_block
                || effect.gep_location.block_ordinal != at.block.block as usize
                || effect.gep_location.operation.checked_add(1) != Some(at.operation as usize)
                || effect.access
                    != if write {
                        PrivateArrayAccessV1::Write
                    } else {
                        PrivateArrayAccessV1::Read
                    }
                || effect.source_first_operation != span.first_operation_ordinal as usize
                || effect.source_end_operation
                    != (span.first_operation_ordinal as usize)
                        .checked_add(span.operation_count as usize)
                        .ok_or(ArgumentResourceV1::Arithmetic)?
                || span.kernel_ir_block != actual_block
                || effect.source_first_operation > effect.gep_location.operation
                || effect.source_end_operation <= at.operation as usize
            {
                return Err(unit_local_mismatch_v1());
            }
            let gep = body
                .blocks
                .get(effect.gep_location.block_ordinal)
                .and_then(|b| b.operations.get(effect.gep_location.operation))
                .ok_or_else(unit_local_mismatch_v1)?;
            if gep.results.len() != 1
                || gep.results[0].id != pointer
                || !matches!(&gep.kind, OperationKind::GetElementPointer { base, offset } if *base == slot.pointer && *offset == effect.offset)
            {
                return Err(unit_local_mismatch_v1());
            }
            if let Some(offset) = effect.offset_location {
                budget.charge_work(5)?;
                let block = body
                    .blocks
                    .get(offset.block_ordinal)
                    .ok_or_else(unit_local_mismatch_v1)?;
                let definition = block
                    .operations
                    .get(offset.operation)
                    .ok_or_else(unit_local_mismatch_v1)?;
                if block.id != offset.block
                    || definition.results.len() != 1
                    || definition.results[0].id != effect.offset
                    || definition.results[0].ty != Type::Scalar(ScalarType::Index)
                {
                    return Err(unit_local_mismatch_v1());
                }
            }
            match (component, effect.original_index) {
                (
                    Some(expected),
                    PrivateArrayIndexV1::InitializerElement {
                        component: actual,
                        value:
                            PrivateArrayInitializerValueV1::LiteralScalar {
                                value: stored,
                                definition,
                            },
                    },
                ) => {
                    budget.charge_work(8)?;
                    let actual_definition = value_definition.ok_or_else(unit_local_mismatch_v1)?;
                    if expected != actual
                        || stored != value
                        || definition.block != actual_block
                        || definition.block_ordinal != at.block.block as usize
                        || definition.operation >= effect.gep_location.operation
                        || actual_definition.block.function.0 as usize != self.input.key.physical
                        || actual_definition.block.block as usize != definition.block_ordinal
                        || actual_definition.operation as usize != definition.operation
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                }
                (
                    None,
                    PrivateArrayIndexV1::ConstantIndex {
                        offset,
                        min_length,
                        from_end,
                    },
                ) => {
                    budget.charge_work(2)?;
                    if !matches!(place.projections()[0].kind(), SemanticProjectionKindV1::ConstantIndex { offset: a, minimum_length: b, from_end: c } if a == offset && b == min_length && c == from_end)
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                }
                (
                    None,
                    PrivateArrayIndexV1::Local {
                        local,
                        semantic_type,
                        original,
                        physical_type,
                        direct_definition,
                    },
                ) => {
                    budget.charge_work(6)?;
                    let row = self
                        .value(index_row.ok_or_else(unit_local_mismatch_v1)?)
                        .ok_or_else(unit_local_mismatch_v1)?;
                    let UnitLocalNativeValueV1::Scalar {
                        value,
                        scalar,
                        definition,
                    } = row.native
                    else {
                        return Err(unit_local_mismatch_v1());
                    };
                    if !matches!(place.projections()[0].kind(), SemanticProjectionKindV1::Index(index) if index.index() == local)
                        || row.source_type != semantic_type
                        || value != original
                        || scalar != physical_type
                    {
                        return Err(unit_local_mismatch_v1());
                    }
                    if let Some(direct) = direct_definition {
                        budget.charge_work(3)?;
                        let Some(definition) = definition else {
                            return Err(unit_local_mismatch_v1());
                        };
                        if definition.block.block as usize != direct.block_ordinal
                            || definition.operation as usize != direct.operation
                            || body.blocks.get(direct.block_ordinal).map(|b| b.id)
                                != Some(direct.block)
                        {
                            return Err(unit_local_mismatch_v1());
                        }
                    }
                }
                _ => return Err(unit_local_mismatch_v1()),
            }
            self.memory.array_effects = self
                .memory
                .array_effects
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        self.memory.next_access = index.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok((key, index))
    }

    fn read_memory_place(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        place: &SemanticPlaceV1,
        origin: UnitLocalValueOriginV1,
        source_ssa: Option<SsaValueV1>,
        use_kind: UnitLocalOperandUseV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let diagnostic = matches!(use_kind, UnitLocalOperandUseV1::Diagnostic);
        let (slot, cell, pointer, index_row) =
            self.memory_address(site, role, place, None, diagnostic, budget)?;
        budget.charge_work(7)?;
        let cell_index = slot
            .cells
            .checked_add(usize::try_from(cell).map_err(|_| ArgumentResourceV1::Arithmetic)?)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let epoch = self
            .memory
            .locals
            .get(place.local().index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?
            .epoch;
        let latest = self
            .memory
            .cells
            .get(cell_index)
            .copied()
            .flatten()
            .filter(|cell| cell.epoch == epoch)
            .ok_or_else(|| self.error("local helper source cell is not initialized"))?;
        let previous = *self
            .value(latest.value)
            .ok_or_else(unit_local_mismatch_v1)?;
        if previous.source_type != place.ty() {
            return Err(unit_local_mismatch_v1());
        }
        if diagnostic {
            return self.rows.append_value(
                UnitLocalValueRowV1 {
                    key: self.input.key,
                    origin,
                    role: Some(role),
                    source_type: place.ty(),
                    source_ssa,
                    native: UnitLocalNativeValueV1::DiagnosticOnly,
                    recipe: UnitLocalValueRecipeV1::Copy {
                        input: latest.value,
                    },
                    known_bits: previous.known_bits,
                },
                budget,
            );
        }
        let (operation, at) = self.take_native_operation(budget)?;
        budget.charge_work(7)?;
        if operation.results.len() != 1
            || operation.results[0].ty != Type::Scalar(slot.element)
            || !matches!(&operation.kind, OperationKind::Load { pointer: actual, access } if *actual == pointer && *access == MemoryAccess::new(AddressSpace::Private, slot.alignment))
        {
            return Err(unit_local_mismatch_v1());
        }
        let native = self.check_native_use(
            UnitLocalNativeValueV1::Scalar {
                value: operation.results[0].id,
                scalar: slot.element,
                definition: Some(at),
            },
            use_kind,
            budget,
        )?;
        let (source_key, physical_access) = self.memory_effect(
            site,
            role,
            place,
            None,
            slot,
            cell,
            pointer,
            index_row,
            at,
            false,
            operation.results[0].id,
            Some(at),
            budget,
        )?;
        budget.charge_work(3)?;
        if self.input.physical.accesses[physical_access].initializing_store()
            != Some(self.input.physical.accesses[latest.physical].location())
        {
            return Err(unit_local_mismatch_v1());
        }
        let access = self.rows.memory.len();
        let value = self.rows.append_value(
            UnitLocalValueRowV1 {
                key: self.input.key,
                origin,
                role: Some(role),
                source_type: place.ty(),
                source_ssa,
                native,
                recipe: UnitLocalValueRecipeV1::Load { access },
                known_bits: previous.known_bits,
            },
            budget,
        )?;
        unit_local_push_v1(
            &mut self.rows.memory,
            UnitLocalMemoryRowV1::Access {
                source_key,
                key: self.input.key,
                local: place.local(),
                allocation: slot.row,
                cell,
                physical_access,
                value,
                latest_source_store: Some(latest.source),
            },
            budget,
        )?;
        Ok(value)
    }

    fn write_memory_place(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        place: &SemanticPlaceV1,
        component: Option<u32>,
        value: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let (slot, cell, pointer, index_row) =
            self.memory_address(site, role, place, component, false, budget)?;
        budget.charge_work(6)?;
        let row = *self.value(value).ok_or_else(unit_local_mismatch_v1)?;
        let UnitLocalNativeValueV1::Scalar {
            value: stored,
            scalar,
            definition,
        } = row.native
        else {
            return Err(unit_local_mismatch_v1());
        };
        if row.key != self.input.key
            || row.source_type != slot.element_type
            || scalar != slot.element
        {
            return Err(unit_local_mismatch_v1());
        }
        if let Some(component) = component {
            budget.charge_work(6)?;
            let UnitLocalValueOriginV1::Constant { occurrence } = row.origin else {
                return Err(unit_local_mismatch_v1());
            };
            let constant = self
                .input
                .occurrences
                .constants()
                .get(occurrence)
                .ok_or_else(unit_local_mismatch_v1)?;
            let operand =
                fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::RvalueOperand(component);
            if !matches!(row.recipe, UnitLocalValueRecipeV1::Literal { .. })
                || definition.is_none()
                || constant.site() != site
                || constant.operand() != operand
                || row.role != Some(operand)
                || role != fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination
            {
                return Err(unit_local_mismatch_v1());
            }
        }
        let (operation, at) = self.take_native_operation(budget)?;
        budget.charge_work(5)?;
        if !operation.results.is_empty()
            || !matches!(&operation.kind, OperationKind::Store { pointer: actual, value, access }
            if *actual == pointer && *value == stored && *access == MemoryAccess::new(AddressSpace::Private, slot.alignment))
        {
            return Err(unit_local_mismatch_v1());
        }
        let (source_key, physical_access) = self.memory_effect(
            site, role, place, component, slot, cell, pointer, index_row, at, true, stored,
            definition, budget,
        )?;
        let source = unit_local_push_v1(
            &mut self.rows.memory,
            UnitLocalMemoryRowV1::Access {
                source_key,
                key: self.input.key,
                local: place.local(),
                allocation: slot.row,
                cell,
                physical_access,
                value,
                latest_source_store: None,
            },
            budget,
        )?;
        budget.charge_work(4)?;
        let index = slot
            .cells
            .checked_add(usize::try_from(cell).map_err(|_| ArgumentResourceV1::Arithmetic)?)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let epoch = self
            .memory
            .locals
            .get(place.local().index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?
            .epoch;
        *self
            .memory
            .cells
            .get_mut(index)
            .ok_or_else(unit_local_mismatch_v1)? = Some(UnitLocalStoredCellV1 {
            source,
            value,
            physical: physical_access,
            epoch,
        });
        Ok(())
    }

    fn invalidate_memory_local(
        &mut self,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: Option<fe2o3_pliron::ProductionSemanticSsaOperandRoleV1>,
        local: SemanticLocalIdV1,
        reason: UnitLocalInvalidateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let state = self
            .memory
            .locals
            .get_mut(local.index() as usize)
            .ok_or_else(unit_local_mismatch_v1)?;
        state.epoch = state
            .epoch
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        unit_local_push_v1(
            &mut self.rows.memory,
            UnitLocalMemoryRowV1::Invalidate {
                key: self.input.key,
                site,
                role,
                local,
                reason,
            },
            budget,
        )?;
        Ok(())
    }

    fn finish_memory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        if self.memory.next_access != self.input.body.accesses.1
            || self.memory.array_slots
                != self
                    .memory
                    .instance
                    .map_or(0, |i| i.slot_end - i.slot_start)
            || self.memory.array_effects
                != self
                    .memory
                    .instance
                    .map_or(0, |i| i.effect_end - i.effect_start)
        {
            return Err(unit_local_mismatch_v1());
        }
        Ok(())
    }
}

fn unit_local_private_pointer_v1(
    ty: &Type,
    element: ScalarType,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    Ok(
        matches!(ty, Type::Pointer(pointer) if pointer.pointee.as_ref() == &Type::Scalar(element)
        && pointer.address_space == AddressSpace::Private && pointer.access == AccessMode::ReadWrite),
    )
}
