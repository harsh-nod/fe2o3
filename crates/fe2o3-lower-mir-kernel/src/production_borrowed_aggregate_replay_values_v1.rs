impl<'a> BorrowedReplayV1<'a, '_> {
    fn statement(
        &mut self,
        group: usize,
        block: SemanticBlockIdV1,
        ordinal: usize,
        statement: &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
        locals: &mut [BorrowedReplayValueV1],
        native: &mut BorrowedReplaySpanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
                    return self.initialize(
                        group,
                        block,
                        ordinal,
                        assignment.destination(),
                        aggregate.operands(),
                        locals,
                        native,
                        budget,
                    );
                }
                let value = self.rvalue(group, assignment.value(), locals, native, budget)?;
                let destination = assignment.destination();
                if destination.projections().is_empty() {
                    let index = destination.local().index() as usize;
                    if matches!(locals[index], BorrowedReplayValueV1::Owner(_)) {
                        return Err(borrowed_replay_unsupported_v1());
                    }
                    locals[index] = value;
                } else {
                    let value = borrowed_replay_native_v1(value)?;
                    let (field, pointer, access, rest) =
                        self.field(group, destination, locals, budget)?;
                    if rest != destination.projections().len()
                        || access != AccessMode::ReadWrite
                        || !matches!(
                            self.candidates.fields[field].carrier,
                            BorrowedAggregateCarrierCandidateV1::ScalarCell { .. }
                        )
                        || !self.initialized[field]
                    {
                        return Err(borrowed_replay_unsupported_v1());
                    }
                    native.operation(
                        OperationKind::Store {
                            pointer,
                            value,
                            access: self.memory_access(field)?,
                        },
                        None,
                        budget,
                    )?;
                }
            }
            SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) => {
                let value = &mut locals[local.index() as usize];
                if matches!(value, BorrowedReplayValueV1::Owner(_)) {
                    return Err(borrowed_replay_unsupported_v1());
                }
                *value = BorrowedReplayValueV1::Uninitialized;
            }
            SemanticStatementKindV1::Nop => {}
            _ => return Err(borrowed_replay_unsupported_v1()),
        }
        Ok(())
    }

    fn allocate(
        &mut self,
        group: usize,
        index: usize,
        native: &mut BorrowedReplaySpanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(20)?;
        let field = &self.candidates.fields[index];
        let BorrowedAggregateCarrierCandidateV1::ScalarCell {
            pointer: locator,
            allocation,
            ..
        } = &field.carrier
        else {
            return Err(borrowed_replay_mismatch_v1());
        };
        let physical = &self.groups[group].function.canonical.function.id;
        budget.charge_work(argument_sum_v1(&[
            locator.function.as_str().len().min(physical.as_str().len()),
            allocation
                .function
                .as_str()
                .len()
                .min(physical.as_str().len()),
        ])?)?;
        if self.allocated[index]
            || self.initialized[index]
            || locator.function != *physical
            || allocation.function != *physical
            || allocation.location != FunctionOperationLocation::new(native.block.id, native.next)
            || native.next >= native.end
        {
            return Err(borrowed_replay_mismatch_v1());
        }
        let operation = &native.block.operations[native.next];
        let OperationKind::Alloca {
            element,
            count: None,
            address_space: AddressSpace::Private,
            alignment,
        } = &operation.kind
        else {
            return Err(borrowed_replay_mismatch_v1());
        };
        let [pointer] = operation.results.as_slice() else {
            return Err(borrowed_replay_mismatch_v1());
        };
        let types = self.subject.semantic_ssa.source_semantic().types();
        let expected = lower_parameter_scalar_v1(types, field.source_type)?;
        let Type::Scalar(scalar) = expected else {
            return Err(borrowed_replay_mismatch_v1());
        };
        let size = match scalar {
            ScalarType::Bool => 1,
            _ => u64::from(
                scalar
                    .bit_width()
                    .ok_or_else(borrowed_replay_unsupported_v1)?,
            )
            .div_ceil(8),
        };
        let layout = types[field.source_type.index() as usize].layout();
        if pointer.id != locator.value
            || *element != expected
            || !matches!(&pointer.ty, Type::Pointer(pointer)
                if pointer.pointee.as_ref() == element
                    && pointer.address_space == AddressSpace::Private
                    && pointer.access == AccessMode::ReadWrite)
            || layout.size_bytes() != Some(size)
            || u64::from(*alignment) != layout.alignment_bytes()
        {
            return Err(borrowed_replay_mismatch_v1());
        }
        self.allocated[index] = true;
        native.next += 1;
        Ok(())
    }

    fn initialize(
        &mut self,
        group: usize,
        block: SemanticBlockIdV1,
        ordinal: usize,
        destination: &SemanticPlaceV1,
        operands: &[SemanticOperandV1],
        locals: &mut [BorrowedReplayValueV1],
        native: &mut BorrowedReplaySpanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let types = self.subject.semantic_ssa.source_semantic().types();
        if !destination.projections().is_empty() {
            return Err(borrowed_replay_unsupported_v1());
        }
        let ty = &types[destination.ty().index() as usize];
        if ty.layout().size_bytes() == Some(0) {
            for operand in operands {
                if !matches!(
                    self.operand(group, operand, locals, native, budget)?,
                    BorrowedReplayValueV1::Unit
                ) {
                    return Err(borrowed_replay_mismatch_v1());
                }
            }
            locals[destination.local().index() as usize] = BorrowedReplayValueV1::Unit;
            return Ok(());
        }
        if !matches!(
            locals[destination.local().index() as usize],
            BorrowedReplayValueV1::Uninitialized
        ) {
            return Err(borrowed_replay_unsupported_v1());
        }
        let (SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields)) =
            ty.shape()
        else {
            return Err(borrowed_replay_unsupported_v1());
        };
        if operands.len() != fields.fields().len() {
            return Err(borrowed_replay_mismatch_v1());
        }
        let mut values = unit_local_vec_v1(operands.len(), budget)?;
        for operand in operands {
            let value = self.operand(group, operand, locals, native, budget)?;
            unit_local_push_v1(&mut values, value, budget)?;
        }
        let mut owner = None;
        let candidates = self.candidates.fields;
        for (field_number, value) in values.iter().copied().enumerate() {
            if matches!(value, BorrowedReplayValueV1::Unit) {
                continue;
            }
            let value = borrowed_replay_native_v1(value)?;
            let association = self.groups[group].function.source;
            let anchor = BorrowedAggregateSourceAnchorV1::Occurrence(
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                    block: fe2o3_mir_model::SsaBlockIdV1::new(block.index()),
                    statement: u32::try_from(ordinal)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                },
            );
            budget.charge_work(self.candidates.fields.len())?;
            let mut matches = candidates.iter().enumerate().filter(|(_, field)| {
                field.owner.root == self.root
                    && field.owner.function == association.semantic_function
                    && field.owner.local == destination.local()
                    && field.owner.lifetime_start == anchor
                    && field.path.fields.as_ref() == [field_number as u32]
            });
            let (index, field) = matches.next().ok_or_else(borrowed_replay_mismatch_v1)?;
            if matches.next().is_some() || self.initialized[index] {
                return Err(borrowed_replay_mismatch_v1());
            }
            if !self.allocated[index] {
                self.allocate(group, index, native, budget)?;
            }
            owner.get_or_insert(index);
            let physical = &self.groups[group].function.canonical.function.id;
            match &field.carrier {
                BorrowedAggregateCarrierCandidateV1::ScalarCell {
                    pointer,
                    initialization,
                    ..
                } => {
                    if pointer.function != *physical
                        || initialization.function != *physical
                        || initialization.location
                            != FunctionOperationLocation::new(native.block.id, native.next)
                    {
                        return Err(borrowed_replay_mismatch_v1());
                    }
                    native.operation(
                        OperationKind::Store {
                            pointer: pointer.value,
                            value,
                            access: self.memory_access(index)?,
                        },
                        None,
                        budget,
                    )?;
                }
                BorrowedAggregateCarrierCandidateV1::CapturedReference { value: locator } => {
                    if locator.function != *physical
                        || locator.value != value
                        || !matches!(types[field.source_type.index() as usize].shape(), SemanticTypeShapeV1::Pointer(pointer)
                            if pointer.kind() == SemanticPointerKindV1::Reference && pointer.metadata() == SemanticPointerMetadataV1::None)
                    {
                        return Err(borrowed_replay_mismatch_v1());
                    }
                }
                BorrowedAggregateCarrierCandidateV1::WholeSlice { value: locator } => {
                    if locator.function != *physical
                        || locator.value != value
                        || !shared_slice_leaf_v1(types, field.source_type)
                    {
                        return Err(borrowed_replay_mismatch_v1());
                    }
                }
            }
            self.initialized[index] = true;
        }
        locals[destination.local().index() as usize] =
            BorrowedReplayValueV1::Owner(owner.ok_or_else(borrowed_replay_unsupported_v1)?);
        Ok(())
    }

    fn memory_access(&self, field: usize) -> Result<MemoryAccess, ProductionSemanticKirErrorV1> {
        let ty = self.candidates.fields[field].source_type;
        let alignment = self.subject.semantic_ssa.source_semantic().types()[ty.index() as usize]
            .layout()
            .alignment_bytes();
        Ok(MemoryAccess::new(
            AddressSpace::Private,
            u32::try_from(alignment).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        ))
    }

    fn rvalue(
        &mut self,
        group: usize,
        value: &fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1,
        locals: &mut [BorrowedReplayValueV1],
        native: &mut BorrowedReplaySpanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<BorrowedReplayValueV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        match value.kind() {
            SemanticRvalueKindV1::Use(operand) => {
                self.operand(group, operand, locals, native, budget)
            }
            SemanticRvalueKindV1::Borrow { kind, place } => self.borrow(
                group,
                *kind,
                place,
                value.result_type(),
                locals,
                native,
                budget,
            ),
            SemanticRvalueKindV1::Length(place) => {
                let slice =
                    borrowed_replay_native_v1(self.read(group, place, locals, native, budget)?)?;
                let ty = self.native_type(group, slice, budget)?;
                if !matches!(ty, Type::Slice(_)) {
                    return Err(borrowed_replay_unsupported_v1());
                }
                Ok(BorrowedReplayValueV1::Native(native.value(
                    OperationKind::SliceLength { slice },
                    Type::INDEX,
                    budget,
                )?))
            }
            _ => Err(borrowed_replay_unsupported_v1()),
        }
    }

    fn operand(
        &mut self,
        group: usize,
        operand: &SemanticOperandV1,
        locals: &mut [BorrowedReplayValueV1],
        native: &mut BorrowedReplaySpanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<BorrowedReplayValueV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        match operand {
            SemanticOperandV1::Copy(place) => self.read(group, place, locals, native, budget),
            SemanticOperandV1::Move(place) => {
                let result = self.read(group, place, locals, native, budget)?;
                let types = self.subject.semantic_ssa.source_semantic().types();
                if !place.projections().is_empty()
                    || !matches!(
                        types[place.ty().index() as usize].shape(),
                        SemanticTypeShapeV1::Pointer(_)
                    )
                {
                    return Err(borrowed_replay_unsupported_v1());
                }
                locals[place.local().index() as usize] = BorrowedReplayValueV1::Uninitialized;
                Ok(result)
            }
            SemanticOperandV1::Constant(constant) => {
                let types = self.subject.semantic_ssa.source_semantic().types();
                if matches!(constant.value(), SemanticConstantValueV1::ZeroSized)
                    && types[constant.ty().index() as usize].layout().size_bytes() == Some(0)
                {
                    return Ok(BorrowedReplayValueV1::Unit);
                }
                let SemanticConstantValueV1::Scalar(bits) = constant.value() else {
                    return Err(borrowed_replay_unsupported_v1());
                };
                let ty = lower_parameter_scalar_v1(types, constant.ty())?;
                let constant = match (&ty, bits.size_bytes()) {
                    (Type::Scalar(ScalarType::U32), 4) => Constant::U32(bits.bits() as u32),
                    (Type::Scalar(ScalarType::U64), 8) => Constant::U64(bits.bits() as u64),
                    _ => return Err(borrowed_replay_unsupported_v1()),
                };
                Ok(BorrowedReplayValueV1::Native(native.value(
                    OperationKind::Constant(constant),
                    ty,
                    budget,
                )?))
            }
        }
    }

    fn native_type(
        &self,
        group: usize,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a Type, ProductionSemanticKirErrorV1> {
        self.inventory
            .definition_for_value(
                self.groups[group].function.canonical.coordinate,
                value,
                budget,
            )
            .map_err(canonical_call_inventory_error_v1)?
            .map(|definition| definition.ty)
            .ok_or_else(borrowed_replay_mismatch_v1)
    }

    fn field(
        &self,
        group: usize,
        place: &SemanticPlaceV1,
        locals: &[BorrowedReplayValueV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, ValueId, AccessMode, usize), ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let source = &self.subject.semantic_ssa.source_semantic().functions()
            [self.groups[group].function.source.semantic_function.index() as usize];
        let types = self.subject.semantic_ssa.source_semantic().types();
        let (owner, reference, access, start) = match locals[place.local().index() as usize] {
            BorrowedReplayValueV1::Owner(owner) => (owner, None, AccessMode::ReadWrite, 0),
            BorrowedReplayValueV1::Reference(reference) => {
                let borrowed = &self.references[reference];
                if place.projections().first().map(|p| p.kind())
                    != Some(SemanticProjectionKindV1::Dereference)
                {
                    return Err(borrowed_replay_unsupported_v1());
                }
                (borrowed.owner, Some(reference), borrowed.access, 1)
            }
            _ => return Err(borrowed_replay_unsupported_v1()),
        };
        let owner_key = self.candidates.fields[owner].owner;
        let mut ty = source.locals()[place.local().index() as usize].ty();
        if start == 1 {
            let SemanticTypeShapeV1::Pointer(pointer) = types[ty.index() as usize].shape() else {
                return Err(borrowed_replay_mismatch_v1());
            };
            ty = pointer.pointee();
            if place.projections()[0].result_type() != ty {
                return Err(borrowed_replay_mismatch_v1());
            }
        }
        let projection = place
            .projections()
            .get(start)
            .ok_or_else(borrowed_replay_unsupported_v1)?;
        let SemanticProjectionKindV1::Field(number) = projection.kind() else {
            return Err(borrowed_replay_unsupported_v1());
        };
        let (SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields)) =
            types[ty.index() as usize].shape()
        else {
            return Err(borrowed_replay_mismatch_v1());
        };
        let leaf_ty = *fields
            .fields()
            .get(number as usize)
            .ok_or_else(borrowed_replay_mismatch_v1)?;
        if projection.result_type() != leaf_ty {
            return Err(borrowed_replay_mismatch_v1());
        }
        for (index, field) in self.candidates.fields.iter().enumerate() {
            budget.charge_work(4)?;
            if field.owner != owner_key || field.path.fields.as_ref() != [number] {
                continue;
            }
            if field.source_type != leaf_ty || !self.initialized[index] {
                return Err(borrowed_replay_mismatch_v1());
            }
            let value = if let Some(reference) = reference {
                budget.charge_work(self.references[reference].values.len())?;
                self.references[reference]
                    .values
                    .iter()
                    .find(|(field, _)| *field == index)
                    .map(|(_, value)| *value)
                    .ok_or_else(borrowed_replay_mismatch_v1)?
            } else {
                match &field.carrier {
                    BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer, .. } => {
                        pointer.value
                    }
                    BorrowedAggregateCarrierCandidateV1::CapturedReference { value }
                    | BorrowedAggregateCarrierCandidateV1::WholeSlice { value } => value.value,
                }
            };
            return Ok((index, value, access, start + 1));
        }
        Err(borrowed_replay_mismatch_v1())
    }

    fn read(
        &mut self,
        group: usize,
        place: &SemanticPlaceV1,
        locals: &[BorrowedReplayValueV1],
        native: &mut BorrowedReplaySpanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<BorrowedReplayValueV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let local = *locals
            .get(place.local().index() as usize)
            .ok_or_else(borrowed_replay_mismatch_v1)?;
        if place.projections().is_empty() {
            return match local {
                BorrowedReplayValueV1::Uninitialized | BorrowedReplayValueV1::Owner(_) => {
                    Err(borrowed_replay_unsupported_v1())
                }
                _ => Ok(local),
            };
        }
        let (value, mut ty, first) = match local {
            BorrowedReplayValueV1::Native(value) => {
                let function = &self.subject.semantic_ssa.source_semantic().functions()
                    [self.groups[group].function.source.semantic_function.index() as usize];
                (
                    value,
                    function.locals()[place.local().index() as usize].ty(),
                    0,
                )
            }
            _ => {
                let (field, pointer, _, first) = self.field(group, place, locals, budget)?;
                let record = &self.candidates.fields[field];
                let value = if matches!(
                    record.carrier,
                    BorrowedAggregateCarrierCandidateV1::ScalarCell { .. }
                ) {
                    native.value(
                        OperationKind::Load {
                            pointer,
                            access: self.memory_access(field)?,
                        },
                        lower_parameter_scalar_v1(
                            self.subject.semantic_ssa.source_semantic().types(),
                            record.source_type,
                        )?,
                        budget,
                    )?
                } else {
                    pointer
                };
                (value, record.source_type, first)
            }
        };
        let mut value = value;
        let types = self.subject.semantic_ssa.source_semantic().types();
        for projection in &place.projections()[first..] {
            budget.charge_work(5)?;
            let SemanticTypeShapeV1::Pointer(pointer) = types[ty.index() as usize].shape() else {
                return Err(borrowed_replay_unsupported_v1());
            };
            if projection.kind() != SemanticProjectionKindV1::Dereference
                || pointer.kind() != SemanticPointerKindV1::Reference
                || pointer.metadata() != SemanticPointerMetadataV1::None
                || projection.result_type() != pointer.pointee()
            {
                return Err(borrowed_replay_unsupported_v1());
            }
            ty = pointer.pointee();
            let Type::Pointer(physical) = self.native_type(group, value, budget)? else {
                return Err(borrowed_replay_mismatch_v1());
            };
            let element = lower_parameter_scalar_v1(types, ty)?;
            if *physical.pointee != element {
                return Err(borrowed_replay_mismatch_v1());
            }
            value = native.value(
                OperationKind::Load {
                    pointer: value,
                    access: MemoryAccess::new(
                        physical.address_space,
                        u32::try_from(types[ty.index() as usize].layout().alignment_bytes())
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    ),
                },
                element,
                budget,
            )?;
        }
        if ty != place.ty() {
            return Err(borrowed_replay_mismatch_v1());
        }
        Ok(BorrowedReplayValueV1::Native(value))
    }

    fn borrow(
        &mut self,
        group: usize,
        kind: SemanticBorrowKindV1,
        place: &SemanticPlaceV1,
        reference_ty: SemanticTypeIdV1,
        locals: &[BorrowedReplayValueV1],
        native: &mut BorrowedReplaySpanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<BorrowedReplayValueV1, ProductionSemanticKirErrorV1> {
        let types = self.subject.semantic_ssa.source_semantic().types();
        let shape = borrowed_aggregate_shape_v1(types, reference_ty, budget)?;
        let expected_kind = if shape.access == AccessMode::ReadOnly {
            SemanticBorrowKindV1::Shared
        } else {
            SemanticBorrowKindV1::Mutable
        };
        if kind != expected_kind || place.ty() != shape.aggregate_type {
            return Err(borrowed_replay_mismatch_v1());
        }
        let (owner, parent) = match locals[place.local().index() as usize] {
            BorrowedReplayValueV1::Owner(owner) if place.projections().is_empty() => (owner, None),
            BorrowedReplayValueV1::Reference(reference)
                if place.projections().len() == 1
                    && place.projections()[0].kind() == SemanticProjectionKindV1::Dereference =>
            {
                if self.references[reference].access == AccessMode::ReadOnly
                    && shape.access != AccessMode::ReadOnly
                {
                    return Err(borrowed_replay_mismatch_v1());
                }
                (self.references[reference].owner, Some(reference))
            }
            _ => return Err(borrowed_replay_unsupported_v1()),
        };
        let mut values = unit_local_vec_v1(shape.leaves.len(), budget)?;
        for leaf in &shape.leaves {
            let [number] = leaf.path.fields.as_ref() else {
                return Err(borrowed_replay_unsupported_v1());
            };
            let mut found = None;
            for (index, field) in self.candidates.fields.iter().enumerate() {
                budget.charge_work(4)?;
                if field.owner == self.candidates.fields[owner].owner
                    && field.path.fields.as_ref() == [*number]
                {
                    if found.replace(index).is_some()
                        || !self.initialized[index]
                        || field.source_type != leaf.semantic_type
                    {
                        return Err(borrowed_replay_mismatch_v1());
                    }
                }
            }
            let field = found.ok_or_else(borrowed_replay_mismatch_v1)?;
            let mut value = if let Some(parent) = parent {
                budget.charge_work(self.references[parent].values.len())?;
                self.references[parent]
                    .values
                    .iter()
                    .find(|(index, _)| *index == field)
                    .map(|(_, value)| *value)
                    .ok_or_else(borrowed_replay_mismatch_v1)?
            } else {
                match &self.candidates.fields[field].carrier {
                    BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer, .. } => {
                        pointer.value
                    }
                    BorrowedAggregateCarrierCandidateV1::CapturedReference { value }
                    | BorrowedAggregateCarrierCandidateV1::WholeSlice { value } => value.value,
                }
            };
            let expected = leaf.parameter_type(shape.access, budget)?;
            let actual = self.native_type(group, value, budget)?;
            if actual != &expected {
                if !matches!((actual, &expected), (Type::Pointer(a), Type::Pointer(b))
                    if a.pointee == b.pointee && a.address_space == b.address_space && a.access == AccessMode::ReadWrite && b.access == AccessMode::ReadOnly)
                {
                    return Err(borrowed_replay_mismatch_v1());
                }
                value = native.restrict_pointer(value, &expected, budget)?;
            }
            unit_local_push_v1(&mut values, (field, value), budget)?;
        }
        let reference = self.push_reference(
            BorrowedReplayReferenceV1 {
                group,
                owner,
                access: shape.access,
                values,
            },
            budget,
        )?;
        Ok(BorrowedReplayValueV1::Reference(reference))
    }

    fn call(
        &mut self,
        group: usize,
        block: SemanticBlockIdV1,
        source: &SemanticDirectCallV1,
        locals: &mut [BorrowedReplayValueV1],
        native: &mut BorrowedReplaySpanV1<'_>,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let caller = self.groups[group].function.source;
        budget.charge_work(self.calls.len())?;
        let mut matches = self.calls.iter().filter(|binding| {
            binding.site.caller.correspondence_owner == self.root
                && binding.site.caller.semantic_function == caller.semantic_function
                && binding.site.anchor.semantic_block == block
        });
        let binding = matches.next().ok_or_else(borrowed_replay_mismatch_v1)?;
        if matches.next().is_some() || !std::ptr::eq(binding.site.source, source) {
            return Err(borrowed_replay_mismatch_v1());
        }
        let callee_group = binding.callee;
        let callee = self.groups[callee_group].function.source;
        let physical = self.groups[callee_group].function.canonical.function;
        let function = &self.subject.semantic_ssa.source_semantic().functions()
            [callee.semantic_function.index() as usize];
        let semantic = self.subject.semantic_ssa.source_semantic();
        let types = semantic.types();
        let argument = source
            .arguments()
            .first()
            .ok_or_else(borrowed_replay_unsupported_v1)?;
        let reference_ty = semantic_operand_type(argument);
        let shape = borrowed_aggregate_shape_v1(types, reference_ty, budget)?;
        let abi = function.abi();
        budget.charge_work(argument_sum_v1(&[
            abi.arguments().len(),
            abi.source_input_types().len(),
            function.locals().len(),
        ])?)?;
        if abi.source_input_types().first() != Some(&reference_ty)
            || abi.can_unwind()
            || abi.c_variadic()
            || !abi.hidden_arguments().is_empty()
            || abi.canon_abi() != SemanticCanonAbiV1::Rust
            || abi.source_argument_ownership().first()
                != Some(&if shape.access == AccessMode::ReadOnly {
                    SemanticSourceArgumentOwnershipV1::SharedBorrow
                } else {
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow
                })
            || !matches!(abi.arguments().first(), Some(a) if a.ty() == reference_ty
                && a.role() == SemanticAbiArgumentRoleV1::Source && matches!(a.mode(), SemanticAbiPassModeV1::Direct(_))
                && a.value().adjusted().is_none() && a.value().pointee_override().is_none())
            || abi
                .arguments()
                .iter()
                .skip(1)
                .any(|a| !matches!(a.mode(), SemanticAbiPassModeV1::Ignore))
            || abi
                .source_input_types()
                .iter()
                .skip(1)
                .any(|ty| types[ty.index() as usize].layout().size_bytes() != Some(0))
        {
            return Err(borrowed_replay_unsupported_v1());
        }
        let BorrowedReplayValueV1::Reference(reference) =
            self.operand(group, argument, locals, native, budget)?
        else {
            return Err(borrowed_replay_unsupported_v1());
        };
        if self.references[reference].access != shape.access {
            return Err(borrowed_replay_mismatch_v1());
        }
        for operand in source.arguments().iter().skip(1) {
            if !matches!(
                self.operand(group, operand, locals, native, budget)?,
                BorrowedReplayValueV1::Unit
            ) {
                return Err(borrowed_replay_unsupported_v1());
            }
        }
        let mut formals = unit_local_vec_v1(shape.leaves.len(), budget)?;
        let mut arguments = unit_local_vec_v1(shape.leaves.len(), budget)?;
        let parameters = &physical
            .body
            .as_ref()
            .ok_or_else(borrowed_replay_mismatch_v1)?
            .parameters;
        if parameters.len() != shape.leaves.len()
            || physical.signature.parameters.len() != shape.leaves.len()
        {
            return Err(borrowed_replay_mismatch_v1());
        }
        let owner = self.references[reference].owner;
        for (slot, leaf) in shape.leaves.iter().enumerate() {
            let [number] = leaf.path.fields.as_ref() else {
                return Err(borrowed_replay_unsupported_v1());
            };
            let mut actual = None;
            for &(index, value) in &self.references[reference].values {
                budget.charge_work(3)?;
                if self.candidates.fields[index].path.fields.as_ref() == [*number] {
                    actual = Some((index, value));
                }
            }
            let (field, value) = actual.ok_or_else(borrowed_replay_mismatch_v1)?;
            let expected = leaf.parameter_type(shape.access, budget)?;
            if physical.signature.parameters[slot] != expected
                || self.native_type(group, value, budget)? != &expected
            {
                return Err(borrowed_replay_mismatch_v1());
            }
            let mut found = None;
            for (index, row) in self.candidates.calls.iter().enumerate() {
                budget.charge_work(15)?;
                if row.root != self.root
                    || row.caller != caller.semantic_function
                    || row.block != block
                    || row.physical_argument as usize != slot
                {
                    continue;
                }
                if found.replace(index).is_some()
                    || row.source_argument != 0
                    || row.tuple_field.is_some()
                    || row.call.function != self.groups[group].function.canonical.function.id
                    || row.call.location
                        != FunctionOperationLocation::new(native.block.id, native.next)
                    || row.actual_owner != self.candidates.fields[owner].owner
                    || row.actual_path != self.candidates.fields[field].path
                    || row.actual.function != row.call.function
                    || row.actual.value != value
                    || row.callee != callee.semantic_function
                    || row.formal.function != physical.id
                    || row.formal.value != parameters[slot]
                    || row.physical_parameter as usize != slot
                    || row.formal_path.fields.as_ref() != [*number]
                    || !matches!(function.locals().get(row.formal_local.index() as usize), Some(local)
                        if local.role() == SemanticLocalRoleV1::Argument(0) && local.ty() == reference_ty)
                {
                    return Err(borrowed_replay_mismatch_v1());
                }
            }
            self.call_claims[found.ok_or_else(borrowed_replay_mismatch_v1)?] = true;
            unit_local_push_v1(&mut arguments, value, budget)?;
            unit_local_push_v1(&mut formals, (field, parameters[slot]), budget)?;
        }
        native.call(&physical.id, &arguments, budget)?;
        let incoming = self.push_reference(
            BorrowedReplayReferenceV1 {
                group: callee_group,
                owner,
                access: shape.access,
                values: formals,
            },
            budget,
        )?;
        self.function(callee_group, Some(incoming), depth + 1, budget)
    }
}
