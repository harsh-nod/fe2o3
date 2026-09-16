#[derive(Clone, Copy)]
struct PrivateArrayPendingAddressV1 {
    original_index: PrivateArrayIndexV1,
    offset_location: Option<PrivateArrayPhysicalLocationV1>,
    gep_location: PrivateArrayPhysicalLocationV1,
    offset: ValueId,
    gep: ValueId,
}

#[derive(Clone, Copy)]
struct PrivateArrayStatementFrameV1 {
    block: u32,
    statement: u32,
    first_operation: usize,
    first_effect: usize,
}

#[derive(Clone, Copy)]
struct PrivateArrayRecorderCheckpointV1 {
    slots: usize,
    effects: usize,
    definitions: usize,
    cursor: usize,
    pending: Option<PrivateArrayPendingAddressV1>,
    frame: Option<PrivateArrayStatementFrameV1>,
    expected: usize,
}

struct PrivateArrayFunctionRecorderV1<'a> {
    work: PrivateArrayRecorderWorkV1<'a>,
    enabled: bool,
    limit: usize,
    block: Option<(usize, BlockId)>,
    frame: Option<PrivateArrayStatementFrameV1>,
    slots: PrivateArrayBufferV1<PrivateArraySlotV1>,
    effects: PrivateArrayBufferV1<PrivateArrayEffectV1>,
    definitions: PrivateArrayBufferV1<PrivateArrayDirectUnsignedDefinitionV1>,
    expected: PrivateArrayBufferV1<PrivateArrayExpectedPlaceV1<'a>>,
    cursor: usize,
    pending: Option<PrivateArrayPendingAddressV1>,
    outer_payload: PrivateArrayPayloadV1,
}

#[derive(Clone, Copy, Default)]
struct PrivateArrayPayloadV1 {
    occupied: usize,
    capacity: usize,
}

impl PrivateArrayPayloadV1 {
    fn add<W: PrivateArrayChargeV1<Error = ProductionSemanticKirErrorV1>>(
        self,
        other: Self,
        work: &mut W,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        work.charge_private_array_work(2)?;
        Ok(Self {
            occupied: self
                .occupied
                .checked_add(other.occupied)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
            capacity: self
                .capacity
                .checked_add(other.capacity)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
        })
    }
}

fn private_array_buffer_payload_v1<
    T,
    W: PrivateArrayChargeV1<Error = ProductionSemanticKirErrorV1>,
>(
    buffer: &PrivateArrayBufferV1<T>,
    extra: usize,
    work: &mut W,
) -> Result<PrivateArrayPayloadV1, ProductionSemanticKirErrorV1> {
    // A pending before-operation reservation remains occupied while other buffers grow.
    work.charge_private_array_work(3)?;
    let needed = buffer
        .rows
        .len()
        .checked_add(extra)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
        .max(buffer.admitted_end);
    let capacity = if needed <= buffer.logical_capacity {
        buffer.logical_capacity
    } else {
        work.charge_private_array_work(5)?;
        let remaining = buffer
            .capacity_limit
            .checked_sub(buffer.logical_capacity)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let growth = buffer.logical_capacity.max(1).min(remaining);
        buffer
            .logical_capacity
            .checked_add(growth)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .max(needed)
    };
    work.charge_private_array_work(2)?;
    Ok(PrivateArrayPayloadV1 {
        occupied: needed
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
        capacity: capacity
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
    })
}

fn private_array_role_key_v1(
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
) -> Option<(u8, u32)> {
    use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;
    match role {
        Role::RvalueOperand(ordinal) => Some((0, ordinal)),
        Role::RvaluePlace => Some((1, 0)),
        Role::Destination => Some((2, 0)),
        Role::StoreValue => Some((3, 0)),
        Role::StoreDestination => Some((4, 0)),
        _ => None,
    }
}

fn private_array_compare_keys_v1<const N: usize, W: PrivateArrayChargeV1>(
    left: [usize; N],
    right: [usize; N],
    work: &mut W,
) -> Result<std::cmp::Ordering, W::Error> {
    for (left, right) in left.into_iter().zip(right) {
        work.charge_private_array_work(1)?;
        let order = left.cmp(&right);
        if order != std::cmp::Ordering::Equal {
            return Ok(order);
        }
    }
    Ok(std::cmp::Ordering::Equal)
}

fn private_array_binary_search_v1<T, const N: usize, W: PrivateArrayChargeV1>(
    rows: &[T],
    key: impl Fn(&T) -> [usize; N],
    target: [usize; N],
    work: &mut W,
) -> Result<Result<usize, usize>, W::Error> {
    let mut left = 0;
    let mut right = rows.len();
    loop {
        work.charge_private_array_work(1)?;
        if left == right {
            return Ok(Err(left));
        }
        // Subtraction, midpoint addition, and indexed row lookup. Key fields pay below.
        work.charge_private_array_work(3)?;
        let middle = left + (right - left) / 2;
        match private_array_compare_keys_v1(key(&rows[middle]), target, work)? {
            std::cmp::Ordering::Less => {
                work.charge_private_array_work(1)?;
                left = middle + 1;
            }
            std::cmp::Ordering::Equal => return Ok(Ok(middle)),
            std::cmp::Ordering::Greater => right = middle,
        }
    }
}

fn private_array_visit_rvalue_operands_v1<'s, E>(
    rvalue: &'s SemanticRvalueKindV1,
    mut visit: impl FnMut(&'s SemanticOperandV1, usize) -> Result<(), E>,
) -> Result<(), E> {
    match rvalue {
        SemanticRvalueKindV1::Use(operand)
        | SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => visit(operand, 0),
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            visit(left, 0)?;
            visit(right, 1)
        }
        SemanticRvalueKindV1::CheckedBinary(checked) => {
            visit(checked.left(), 0)?;
            visit(checked.right(), 1)
        }
        SemanticRvalueKindV1::UncheckedBinary(unchecked) => {
            visit(unchecked.left(), 0)?;
            visit(unchecked.right(), 1)
        }
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for (ordinal, operand) in aggregate.operands().iter().enumerate() {
                visit(operand, ordinal)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

impl<'a> PrivateArrayFunctionRecorderV1<'a> {
    fn new(
        work: PrivateArrayRecorderWorkV1<'a>,
        enabled: bool,
        limit: usize,
        outer_payload: PrivateArrayPayloadV1,
    ) -> Self {
        Self {
            work,
            enabled,
            limit,
            block: None,
            frame: None,
            slots: PrivateArrayBufferV1::new(limit),
            effects: PrivateArrayBufferV1::new(limit),
            definitions: PrivateArrayBufferV1::new(limit),
            expected: PrivateArrayBufferV1::new(limit),
            cursor: 0,
            pending: None,
            outer_payload,
        }
    }

    fn location(
        &self,
        operation: usize,
    ) -> Result<PrivateArrayPhysicalLocationV1, ProductionSemanticKirErrorV1> {
        let (block_ordinal, block) = self
            .block
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        Ok(PrivateArrayPhysicalLocationV1 {
            block_ordinal,
            block,
            operation,
        })
    }

    fn checkpoint(&self) -> PrivateArrayRecorderCheckpointV1 {
        PrivateArrayRecorderCheckpointV1 {
            slots: self.slots.rows.len(),
            effects: self.effects.rows.len(),
            definitions: self.definitions.rows.len(),
            cursor: self.cursor,
            pending: self.pending,
            frame: self.frame,
            expected: self.expected.rows.len(),
        }
    }

    fn rollback(&mut self, checkpoint: PrivateArrayRecorderCheckpointV1) {
        self.slots.truncate(checkpoint.slots);
        self.effects.truncate(checkpoint.effects);
        self.definitions.truncate(checkpoint.definitions);
        self.cursor = checkpoint.cursor;
        self.pending = checkpoint.pending;
        self.frame = checkpoint.frame;
        self.expected.truncate(checkpoint.expected);
        // Capacity and the shared first-denial work ledger deliberately survive.
    }

    fn begin_block(
        &mut self,
        block: SemanticBlockIdV1,
        actual: BlockId,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.enabled {
            return Ok(());
        }
        self.work.charge_private_array_work(2)?;
        if actual != BlockId(block.index()) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let ordinal = match self.block {
            Some((ordinal, _)) => ordinal
                .checked_add(1)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
            None => 0,
        };
        self.block = Some((ordinal, actual));
        self.frame = None;
        self.pending = None;
        Ok(())
    }

    fn prepare_slot(
        &mut self,
        types: &[SemanticTypeDeclV1],
        local: u32,
        slot: &SemanticRetainedLocalSlotV1,
        emitted: usize,
    ) -> Result<PrivateRetainedArrayFactsV1, ProductionSemanticKirErrorV1> {
        let facts =
            private_retained_array_facts_v1(types, slot.semantic_type, self.limit, &mut self.work)?
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.work.charge_private_array_work(3)?;
        let actual = slot
            .array
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if actual.length != facts.length
            || actual.element != facts.element_type
            || slot.alignment != facts.element.alignment
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        if !facts
            .element
            .element
            .matches_borrowed(&slot.kernel_type, &mut self.work)?
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.work.charge_private_array_work(6)?;
        let next = emitted
            .checked_add(2)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let occupied = self
            .slots
            .rows
            .len()
            .checked_add(self.effects.rows.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if next > self.limit
            || occupied > next
            || self
                .slots
                .rows
                .last()
                .is_some_and(|previous| previous.local >= local)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.admit_payload(1, 0, 0, 0)?;
        self.slots.reserve(1, next, &mut self.work)?;
        Ok(facts)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Keep source ownership, physical coordinates, and the admission frontier explicit"
    )]
    fn commit_slot(
        &mut self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        local: u32,
        slot: &SemanticRetainedLocalSlotV1,
        facts: PrivateRetainedArrayFactsV1,
        count: ValueId,
        first_operation: usize,
        emitted: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.work.charge_private_array_work(3)?;
        let occupied = self
            .slots
            .rows
            .len()
            .checked_add(self.effects.rows.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if occupied > emitted {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.work.charge_private_array_work(1)?;
        let alloca = first_operation
            .checked_add(1)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let row = PrivateArraySlotV1 {
            owner,
            function,
            local,
            semantic_type: slot.semantic_type,
            count_location: self.location(first_operation)?,
            alloca_location: self.location(alloca)?,
            pointer: slot.pointer,
            count,
            length: facts.length,
            element_type: facts.element_type,
            element_facts: facts.element,
        };
        self.slots.push(row, &mut self.work)
    }

    fn into_rows(mut self) -> Result<PrivateArrayFunctionRowsV1, ProductionSemanticKirErrorV1> {
        let payload = if self.enabled {
            let slots = private_array_buffer_payload_v1(&self.slots, 0, &mut self.work)?;
            slots.add(
                private_array_buffer_payload_v1(&self.effects, 0, &mut self.work)?,
                &mut self.work,
            )?
        } else {
            PrivateArrayPayloadV1::default()
        };
        Ok(PrivateArrayFunctionRowsV1 {
            active: self.enabled,
            slots: self.slots.into_rows(),
            effects: self.effects.into_rows(),
            payload,
        })
    }

    fn admit_payload(
        &mut self,
        extra_slots: usize,
        extra_effects: usize,
        extra_definitions: usize,
        extra_expected: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut payload = self.outer_payload;
        payload = payload.add(
            private_array_buffer_payload_v1(&self.slots, extra_slots, &mut self.work)?,
            &mut self.work,
        )?;
        payload = payload.add(
            private_array_buffer_payload_v1(&self.effects, extra_effects, &mut self.work)?,
            &mut self.work,
        )?;
        payload = payload.add(
            private_array_buffer_payload_v1(&self.definitions, extra_definitions, &mut self.work)?,
            &mut self.work,
        )?;
        let _payload = payload.add(
            private_array_buffer_payload_v1(&self.expected, extra_expected, &mut self.work)?,
            &mut self.work,
        )?;
        Ok(())
    }

    fn expected_place(
        &mut self,
        place: &'a SemanticPlaceV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.work.charge_private_array_work(2)?;
        if place.projections().is_empty() {
            return Ok(());
        }
        let local = place.local().index();
        if private_array_binary_search_v1(
            &self.slots.rows,
            |slot| [slot.local as usize],
            [local as usize],
            &mut self.work,
        )?
        .is_err()
        {
            return Ok(());
        }
        self.work.charge_private_array_work(2)?;
        let [projection] = place.projections() else {
            return Ok(());
        };
        if !matches!(
            projection.kind(),
            SemanticProjectionKindV1::Index(_) | SemanticProjectionKindV1::ConstantIndex { .. }
        ) {
            return Ok(());
        }
        self.admit_payload(0, 0, 0, 1)?;
        self.expected.reserve(1, self.limit, &mut self.work)?;
        self.expected.push(
            PrivateArrayExpectedPlaceV1 {
                place,
                role,
                initializer_component: None,
            },
            &mut self.work,
        )
    }

    fn expected_initializer(
        &mut self,
        function: &SemanticFunctionDeclV1,
        assignment: &'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        // Shape, slot lookup key, aggregate count and scalar layout predicates.
        self.work.charge_private_array_work(20)?;
        let place = assignment.destination();
        if !place.projections().is_empty() {
            return Ok(false);
        }
        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
            return Ok(false);
        };
        if aggregate.kind() != &SemanticAggregateKindV1::Array {
            return Ok(false);
        }
        let Some(local) = function.locals().get(place.local().index() as usize) else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if local.role().is_entry_argument() {
            return Ok(false);
        }
        let Ok(slot_index) = private_array_binary_search_v1(
            &self.slots.rows,
            |slot| [slot.local as usize],
            [place.local().index() as usize],
            &mut self.work,
        )?
        else {
            return Ok(false);
        };
        let slot = self.slots.rows[slot_index];
        if place.ty() != slot.semantic_type
            || assignment.value().result_type() != slot.semantic_type
            || u64::try_from(aggregate.operands().len()).ok() != Some(slot.length)
            || !matches!(
                slot.element_facts.element,
                PrivateRetainedElementFactsV1::Scalar(_)
            )
        {
            return Ok(false);
        }
        let minimum_operations = aggregate
            .operands()
            .len()
            .checked_mul(4)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        enforce_limit(
            ProductionSemanticKirResourceV1::Operations,
            minimum_operations,
            self.limit,
        )?;
        for operand in aggregate.operands() {
            self.work.charge_private_array_work(5)?;
            let SemanticOperandV1::Constant(constant) = operand else {
                return Ok(false);
            };
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Ok(false);
            };
            if constant.ty() != slot.element_type
                || u64::from(value.size_bytes()) != slot.element_facts.size
            {
                return Ok(false);
            }
        }
        self.admit_payload(0, 0, 0, aggregate.operands().len())?;
        self.expected
            .reserve(aggregate.operands().len(), self.limit, &mut self.work)?;
        for component in 0..aggregate.operands().len() {
            self.work.charge_private_array_work(1)?;
            let component = u32::try_from(component)
                .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            self.expected.push(
                PrivateArrayExpectedPlaceV1 {
                    place,
                    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination,
                    initializer_component: Some(component),
                },
                &mut self.work,
            )?;
        }
        Ok(true)
    }

    fn expected_operand(
        &mut self,
        operand: &'a SemanticOperandV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.work.charge_private_array_work(1)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                self.expected_place(place, role)
            }
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }

    fn begin_statement(
        &mut self,
        function: &'a SemanticFunctionDeclV1,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        kind: &SemanticStatementKindV1,
        first_operation: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.enabled {
            return Ok(());
        }
        self.work.charge_private_array_work(4)?;
        // The real lowering loop enters one statement at a time; no nested frame is supported.
        if self.frame.is_some() || !self.expected.rows.is_empty() || self.pending.is_some() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.expected.truncate(0);
        self.cursor = 0;
        self.pending = None;
        self.frame = None;
        let Some(statement) = statement else {
            return Ok(());
        };
        self.work.charge_private_array_work(3)?;
        let source = function
            .blocks()
            .get(block.index() as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .kind();
        if !std::ptr::eq(source, kind) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        use fe2o3_pliron::ProductionSemanticSsaOperandRoleV1 as Role;
        self.work.charge_private_array_work(1)?;
        match source {
            SemanticStatementKindV1::Assign(assignment) => {
                private_array_visit_rvalue_operands_v1(
                    assignment.value().kind(),
                    |operand, ordinal| -> Result<(), ProductionSemanticKirErrorV1> {
                        self.work.charge_private_array_work(1)?;
                        let ordinal = u32::try_from(ordinal)
                            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        self.expected_operand(operand, Role::RvalueOperand(ordinal))?;
                        Ok(())
                    },
                )?;
                self.work.charge_private_array_work(1)?;
                if let SemanticRvalueKindV1::Load(load) = assignment.value().kind() {
                    self.expected_place(load.source(), Role::RvaluePlace)?;
                }
                if !self.expected_initializer(function, assignment)? {
                    self.expected_place(assignment.destination(), Role::Destination)?;
                }
            }
            SemanticStatementKindV1::Store(store) => {
                self.expected_operand(store.value(), Role::StoreValue)?;
                self.expected_place(store.destination(), Role::StoreDestination)?;
            }
            _ => {}
        }
        self.frame = Some(PrivateArrayStatementFrameV1 {
            block: block.index(),
            statement,
            first_operation,
            first_effect: self.effects.rows.len(),
        });
        Ok(())
    }

    fn finish_statement(
        &mut self,
        end_operation: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.enabled {
            return Ok(());
        }
        self.work.charge_private_array_work(1)?;
        let Some(frame) = self.frame else {
            return Ok(());
        };
        self.work.charge_private_array_work(1)?;
        if self.cursor != self.expected.rows.len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        for row in &mut self.effects.rows[frame.first_effect..] {
            self.work.charge_private_array_work(2)?;
            if row.memory_location.operation >= end_operation {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            row.source_end_operation = end_operation;
        }
        self.expected.truncate(0);
        self.frame = None;
        self.pending = None;
        Ok(())
    }

    fn prepare_unsigned(
        &mut self,
        ty: &Type,
        kind: &OperationKind,
        id: ValueId,
        operation: usize,
        emitted: usize,
    ) -> Result<Option<PrivateArrayDirectUnsignedDefinitionV1>, ProductionSemanticKirErrorV1> {
        if !self.enabled {
            return Ok(None);
        }
        self.work.charge_private_array_work(1)?;
        let (scalar, constant) = match kind {
            OperationKind::Constant(Constant::U8(value)) => (ScalarType::U8, u64::from(*value)),
            OperationKind::Constant(Constant::U16(value)) => (ScalarType::U16, u64::from(*value)),
            OperationKind::Constant(Constant::U32(value)) => (ScalarType::U32, u64::from(*value)),
            OperationKind::Constant(Constant::U64(value)) => (ScalarType::U64, *value),
            OperationKind::Constant(Constant::Index(value)) => (ScalarType::Index, *value),
            _ => return Ok(None),
        };
        self.work.charge_private_array_work(3)?;
        let next = emitted
            .checked_add(1)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if next > self.limit || ty != &Type::Scalar(scalar) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.admit_payload(0, 0, 1, 0)?;
        self.definitions.reserve(1, next, &mut self.work)?;
        Ok(Some(PrivateArrayDirectUnsignedDefinitionV1 {
            value: id,
            scalar,
            constant,
            location: self.location(operation)?,
        }))
    }

    fn commit_unsigned(
        &mut self,
        row: Option<PrivateArrayDirectUnsignedDefinitionV1>,
        emitted: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(row) = row else {
            return Ok(());
        };
        self.work.charge_private_array_work(3)?;
        if self.definitions.rows.len() >= emitted
            || self
                .definitions
                .rows
                .last()
                .is_some_and(|previous| previous.value.0 >= row.value.0)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.definitions.push(row, &mut self.work)
    }

    fn direct_definition(
        &mut self,
        value: ValueId,
        scalar: ScalarType,
    ) -> Result<Option<PrivateArrayDirectUnsignedDefinitionV1>, ProductionSemanticKirErrorV1> {
        if !self.enabled {
            return Ok(None);
        }
        let index = private_array_binary_search_v1(
            &self.definitions.rows,
            |row| [row.value.0 as usize],
            [value.0 as usize],
            &mut self.work,
        )?;
        let Ok(index) = index else {
            return Ok(None);
        };
        self.work.charge_private_array_work(2)?;
        let row = self.definitions.rows[index];
        if row.scalar != scalar {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(Some(row))
    }

    fn prepare_effect(&mut self, emitted: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.enabled || self.frame.is_none() {
            return Ok(());
        }
        self.work.charge_private_array_work(5)?;
        let next = emitted
            .checked_add(1)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let occupied = self
            .slots
            .rows
            .len()
            .checked_add(self.effects.rows.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if next > self.limit || occupied > next {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.admit_payload(0, 1, 0, 0)?;
        self.effects.reserve(1, next, &mut self.work)
    }

    fn prepare_initializer_address(
        &mut self,
        place: &SemanticPlaceV1,
        component: usize,
        value: ValueId,
        offset: ValueId,
        gep: ValueId,
        gep_operation: usize,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        if !self.enabled {
            return Ok(false);
        }
        self.work.charge_private_array_work(3)?;
        let Some(frame) = self.frame else {
            return Ok(false);
        };
        let Some(expected) = self.expected.rows.get(self.cursor).copied() else {
            return Ok(false);
        };
        let Some(recorded_component) = expected.initializer_component else {
            return Ok(false);
        };
        self.work.charge_private_array_work(8)?;
        if !std::ptr::eq(expected.place, place)
            || u32::try_from(component).ok() != Some(recorded_component)
            || self.pending.is_some()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let definition = frame
            .first_operation
            .checked_add(component)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let offset_location = self
            .direct_definition(offset, ScalarType::Index)?
            .map(|row| row.location);
        self.pending = Some(PrivateArrayPendingAddressV1 {
            original_index: PrivateArrayIndexV1::InitializerElement {
                component: recorded_component,
                value: PrivateArrayInitializerValueV1::LiteralScalar {
                    value,
                    definition: self.location(definition)?,
                },
            },
            offset_location,
            gep_location: self.location(gep_operation)?,
            offset,
            gep,
        });
        Ok(true)
    }

    fn commit_effect(
        &mut self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        place: &SemanticPlaceV1,
        access: PrivateArrayAccessV1,
        operation: usize,
        emitted: usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.enabled {
            return Ok(());
        }
        self.work.charge_private_array_work(1)?;
        let Some(frame) = self.frame else {
            self.pending = None;
            return Ok(());
        };
        self.work.charge_private_array_work(3)?;
        let expected = self
            .expected
            .rows
            .get(self.cursor)
            .copied()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !std::ptr::eq(expected.place, place) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let pending = self
            .pending
            .take()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.work.charge_private_array_work(2)?;
        let role_key = private_array_role_key_v1(expected.role)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if let Some(previous) = self
            .effects
            .rows
            .get(frame.first_effect..)
            .and_then(|rows| rows.last())
        {
            self.work.charge_private_array_work(1)?;
            let previous_key = private_array_role_key_v1(previous.role)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let order = private_array_compare_keys_v1(
                [previous_key.0 as usize, previous_key.1 as usize, previous.original_index.component() as usize],
                [role_key.0 as usize, role_key.1 as usize, pending.original_index.component() as usize],
                &mut self.work,
            )?;
            self.work.charge_private_array_work(1)?;
            if order != std::cmp::Ordering::Less || previous.memory_location.operation >= operation
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        self.work.charge_private_array_work(3)?;
        let occupied = self
            .slots
            .rows
            .len()
            .checked_add(self.effects.rows.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if occupied > emitted {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let row = PrivateArrayEffectV1 {
            owner,
            function,
            semantic_block: frame.block,
            semantic_statement: frame.statement,
            role: expected.role,
            local: place.local().index(),
            semantic_type: place.ty(),
            source_first_operation: frame.first_operation,
            source_end_operation: 0,
            original_index: pending.original_index,
            offset_location: pending.offset_location,
            gep_location: pending.gep_location,
            memory_location: self.location(operation)?,
            offset: pending.offset,
            gep: pending.gep,
            access,
        };
        self.effects.push(row, &mut self.work)?;
        self.work.charge_private_array_work(1)?;
        self.cursor = self
            .cursor
            .checked_add(1)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        Ok(())
    }
}

#[derive(Default)]
struct PrivateArrayFunctionRowsV1 {
    active: bool,
    slots: Vec<PrivateArraySlotV1>,
    effects: Vec<PrivateArrayEffectV1>,
    payload: PrivateArrayPayloadV1,
}

struct PrivateArrayMergeV1 {
    active: bool,
    capacity_limit: usize,
    recorded_instance_operations: usize,
    instances: PrivateArrayBufferV1<PrivateArrayInstanceV1>,
    slots: PrivateArrayBufferV1<PrivateArraySlotV1>,
    effects: PrivateArrayBufferV1<PrivateArrayEffectV1>,
}

impl PrivateArrayMergeV1 {
    fn new(capacity_limit: usize) -> Self {
        Self {
            active: false,
            capacity_limit,
            recorded_instance_operations: 0,
            instances: PrivateArrayBufferV1::new(capacity_limit),
            slots: PrivateArrayBufferV1::new(capacity_limit),
            effects: PrivateArrayBufferV1::new(capacity_limit),
        }
    }

    fn payload<W: PrivateArrayChargeV1<Error = ProductionSemanticKirErrorV1>>(
        &self,
        extra_instances: usize,
        extra_slots: usize,
        extra_effects: usize,
        work: &mut W,
    ) -> Result<PrivateArrayPayloadV1, ProductionSemanticKirErrorV1> {
        let instances = private_array_buffer_payload_v1(&self.instances, extra_instances, work)?;
        let slots = private_array_buffer_payload_v1(&self.slots, extra_slots, work)?;
        let effects = private_array_buffer_payload_v1(&self.effects, extra_effects, work)?;
        instances.add(slots, work)?.add(effects, work)
    }

    fn append(
        &mut self,
        source: PrivateArrayCorrespondenceV1,
        source_payload: PrivateArrayPayloadV1,
        work: &mut PrivateArrayLazyBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !source.active {
            if source.recorded_instance_operations != 0
                || !source.instances.is_empty()
                || !source.slots.is_empty()
                || !source.effects.is_empty()
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            return Ok(());
        }
        work.charge_private_array_work(2)?;
        if source.slots.is_empty() || source.instances.is_empty() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let capacity_limit = self.capacity_limit;
        // Six checked additions and three source/aggregate/frontier comparisons.
        work.charge_private_array_work(9)?;
        let emitted = self
            .recorded_instance_operations
            .checked_add(source.recorded_instance_operations)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let source_rows = source
            .slots
            .len()
            .checked_add(source.effects.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let slots = self
            .slots
            .rows
            .len()
            .checked_add(source.slots.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let effects = self
            .effects
            .rows
            .len()
            .checked_add(source.effects.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let instances = self
            .instances
            .rows
            .len()
            .checked_add(source.instances.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let rows = slots
            .checked_add(effects)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if source_rows > source.recorded_instance_operations
            || rows > emitted
            || emitted > capacity_limit
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        work.charge_private_array_work(2)?;
        if instances > slots || source.instances.len() > source.slots.len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let pending = self.payload(
            source.instances.len(),
            source.slots.len(),
            source.effects.len(),
            work,
        )?;
        let _coexisting = pending.add(source_payload, work)?;
        self.instances
            .reserve(source.instances.len(), slots, work)?;
        self.slots.reserve(source.slots.len(), emitted, work)?;
        self.effects.reserve(source.effects.len(), emitted, work)?;
        for row in source.instances {
            self.instances.push(row, work)?;
        }
        for row in source.slots {
            self.slots.push(row, work)?;
        }
        for row in source.effects {
            self.effects.push(row, work)?;
        }
        self.recorded_instance_operations = emitted;
        self.active = true;
        Ok(())
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Keep instance identity and independently shared transfer accounting explicit"
    )]
    fn append_function(
        &mut self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        ordinal: usize,
        emitted_operations: usize,
        rows: PrivateArrayFunctionRowsV1,
        outer: Option<&PrivateArrayMergeV1>,
        work: &mut PrivateArrayLazyBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // No proof phase is entered for a function without retained-array rows.
        if !rows.active {
            if !rows.slots.is_empty() || !rows.effects.is_empty() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            return Ok(());
        }
        work.charge_private_array_work(1)?;
        if rows.slots.is_empty() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let capacity_limit = self.capacity_limit;
        // The one-element stack array avoids a separate instance allocation.
        let instance = PrivateArrayInstanceV1 {
            owner,
            function,
            lowered_function_ordinal: ordinal,
            module_function_ordinal: ordinal,
            slot_start: 0,
            slot_end: 0,
            effect_start: 0,
            effect_end: 0,
        };
        work.charge_private_array_work(5)?;
        let emitted = self
            .recorded_instance_operations
            .checked_add(emitted_operations)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let row_count = rows
            .slots
            .len()
            .checked_add(rows.effects.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let total_rows = self
            .slots
            .rows
            .len()
            .checked_add(self.effects.rows.len())
            .and_then(|n| n.checked_add(row_count))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        work.charge_private_array_work(4)?;
        if row_count > emitted_operations
            || total_rows > emitted
            || emitted > capacity_limit
            || (rows.slots.is_empty() && !rows.effects.is_empty())
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let extra_instance = usize::from(!rows.slots.is_empty());
        work.charge_private_array_work(3)?;
        let slots = self
            .slots
            .rows
            .len()
            .checked_add(rows.slots.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let _effects = self
            .effects
            .rows
            .len()
            .checked_add(rows.effects.len())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let _instances = self
            .instances
            .rows
            .len()
            .checked_add(extra_instance)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let pending = self.payload(extra_instance, rows.slots.len(), rows.effects.len(), work)?;
        let pending = pending.add(rows.payload, work)?;
        if let Some(outer) = outer {
            let _coexisting = pending.add(outer.payload(0, 0, 0, work)?, work)?;
        }
        if extra_instance != 0 {
            self.instances.reserve(1, slots, work)?;
            self.instances.push(instance, work)?;
        }
        self.slots.reserve(rows.slots.len(), emitted, work)?;
        self.effects.reserve(rows.effects.len(), emitted, work)?;
        for row in rows.slots {
            self.slots.push(row, work)?;
        }
        for row in rows.effects {
            self.effects.push(row, work)?;
        }
        self.recorded_instance_operations = emitted;
        self.active = true;
        Ok(())
    }

    fn into_correspondence(
        self,
        work: &mut PrivateArrayLazyBudgetV1,
    ) -> Result<(PrivateArrayCorrespondenceV1, PrivateArrayPayloadV1), ProductionSemanticKirErrorV1>
    {
        let payload = if !self.active {
            PrivateArrayPayloadV1::default()
        } else {
            self.payload(0, 0, 0, work)?
        };
        Ok((
            PrivateArrayCorrespondenceV1 {
                active: self.active,
                recorded_instance_operations: self.recorded_instance_operations,
                instances: self.instances.into_rows(),
                slots: self.slots.into_rows(),
                effects: self.effects.into_rows(),
            },
            payload,
        ))
    }
}

fn private_array_heapsort_v1<T, const N: usize, W: PrivateArrayChargeV1>(
    rows: &mut [T],
    key: impl Fn(&T) -> [usize; N],
    work: &mut W,
    arithmetic: impl Fn() -> W::Error,
) -> Result<(), W::Error> {
    fn sift<T, const N: usize, W: PrivateArrayChargeV1>(
        rows: &mut [T],
        mut root: usize,
        end: usize,
        key: &impl Fn(&T) -> [usize; N],
        work: &mut W,
        arithmetic: &impl Fn() -> W::Error,
    ) -> Result<(), W::Error> {
        loop {
            // Sift visit, two checked child-index operations, and child-bound comparison.
            work.charge_private_array_work(4)?;
            let Some(left) = root.checked_mul(2).and_then(|n| n.checked_add(1)) else {
                return Err(arithmetic());
            };
            if left >= end {
                return Ok(());
            }
            // Checked right index and right-bound comparison.
            work.charge_private_array_work(2)?;
            let right = left.checked_add(1).ok_or_else(arithmetic)?;
            let mut child = left;
            if right < end {
                // Candidate visit and two indexed row lookups; fields pay individually.
                work.charge_private_array_work(3)?;
                if private_array_compare_keys_v1(key(&rows[left]), key(&rows[right]), work)?
                    == std::cmp::Ordering::Less
                {
                    child = right;
                }
            }
            // Parent/child comparison site and its two indexed row lookups.
            work.charge_private_array_work(3)?;
            if private_array_compare_keys_v1(key(&rows[root]), key(&rows[child]), work)?
                != std::cmp::Ordering::Less
            {
                return Ok(());
            }
            work.charge_private_array_work(1)?;
            rows.swap(root, child);
            root = child;
        }
    }
    work.charge_private_array_work(1)?;
    for root in (0..rows.len() / 2).rev() {
        sift(rows, root, rows.len(), &key, work, &arithmetic)?;
    }
    for end in (1..rows.len()).rev() {
        work.charge_private_array_work(1)?;
        rows.swap(0, end);
        sift(rows, 0, end, &key, work, &arithmetic)?;
    }
    Ok(())
}

fn private_array_effect_key_v1(row: &PrivateArrayEffectV1) -> [usize; 7] {
    let role = private_array_role_key_v1(row.role).unwrap_or((u8::MAX, u32::MAX));
    [
        row.owner.index() as usize,
        row.function.index() as usize,
        row.semantic_block as usize,
        row.semantic_statement as usize,
        role.0 as usize,
        role.1 as usize,
        row.original_index.component() as usize,
    ]
}

fn private_array_order_correspondence_v1(
    rows: &mut PrivateArrayCorrespondenceV1,
    work: &mut PrivateArrayLazyBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    private_array_heapsort_v1(
        &mut rows.instances,
        |row| [row.owner.index() as usize, row.function.index() as usize],
        work,
        || ProductionSemanticKirErrorV1::CorrespondenceMismatch,
    )?;
    private_array_heapsort_v1(
        &mut rows.slots,
        |row| {
            [
                row.owner.index() as usize,
                row.function.index() as usize,
                row.local as usize,
            ]
        },
        work,
        || ProductionSemanticKirErrorV1::CorrespondenceMismatch,
    )?;
    private_array_heapsort_v1(&mut rows.effects, private_array_effect_key_v1, work, || {
        ProductionSemanticKirErrorV1::CorrespondenceMismatch
    })?;
    let mut slot = 0;
    let mut effect = 0;
    let mut previous_instance = None;
    for instance in &mut rows.instances {
        let key = [
            instance.owner.index() as usize,
            instance.function.index() as usize,
        ];
        if let Some(previous) = previous_instance
            && private_array_compare_keys_v1(previous, key, work)? != std::cmp::Ordering::Less
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        previous_instance = Some(key);
        instance.slot_start = slot;
        instance.effect_start = effect;
        let mut previous_local = None;
        loop {
            work.charge_private_array_work(1)?;
            let Some(row) = rows.slots.get(slot) else {
                break;
            };
            if private_array_compare_keys_v1(
                [row.owner.index() as usize, row.function.index() as usize],
                key,
                work,
            )? != std::cmp::Ordering::Equal
            {
                break;
            }
            work.charge_private_array_work(2)?;
            if previous_local.is_some_and(|previous| previous >= row.local) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            previous_local = Some(row.local);
            slot = slot
                .checked_add(1)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        }
        let mut previous_effect: Option<&PrivateArrayEffectV1> = None;
        loop {
            work.charge_private_array_work(1)?;
            let Some(row) = rows.effects.get(effect) else {
                break;
            };
            if private_array_compare_keys_v1(
                [row.owner.index() as usize, row.function.index() as usize],
                key,
                work,
            )? != std::cmp::Ordering::Equal
            {
                break;
            }
            work.charge_private_array_work(1)?;
            if private_array_role_key_v1(row.role).is_none() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            if let Some(previous) = previous_effect {
                if private_array_compare_keys_v1(
                    private_array_effect_key_v1(previous),
                    private_array_effect_key_v1(row),
                    work,
                )? != std::cmp::Ordering::Less
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                if private_array_compare_keys_v1(
                    [
                        previous.semantic_block as usize,
                        previous.semantic_statement as usize,
                    ],
                    [row.semantic_block as usize, row.semantic_statement as usize],
                    work,
                )? == std::cmp::Ordering::Equal
                {
                    work.charge_private_array_work(3)?;
                    if previous.memory_location.block_ordinal != row.memory_location.block_ordinal
                        || previous.memory_location.block != row.memory_location.block
                        || previous.memory_location.operation >= row.memory_location.operation
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
            }
            previous_effect = Some(row);
            work.charge_private_array_work(1)?;
            effect = effect
                .checked_add(1)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        }
        instance.slot_end = slot;
        instance.effect_end = effect;
        work.charge_private_array_work(1)?;
        if instance.slot_start == instance.slot_end {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    work.charge_private_array_work(2)?;
    if slot != rows.slots.len() || effect != rows.effects.len() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}
