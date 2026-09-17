struct BorrowedAggregateResolvedPlaceV1 {
    value: ValueDef,
    transport: BorrowedAggregateLeafTransportV1,
    dereference: bool,
}

impl SemanticFunctionLoweringV1<'_> {
    fn borrowed_aggregate_place_v1(
        &mut self,
        place: &SemanticPlaceV1,
    ) -> Result<Option<BorrowedAggregateResolvedPlaceV1>, ProductionSemanticKirErrorV1> {
        let local = place.local().index();
        let view = match self.locals.get(local as usize).and_then(Option::as_ref) {
            Some(SemanticValueBindingV1::BorrowedAggregate { view }) => Some(*view),
            _ => None,
        };
        let owner = self
            .borrowed_aggregate_preparation
            .owners
            .contains_key(&local);
        if !owner && view.is_none() {
            return Ok(None);
        }
        if !owner && place.projections().is_empty() {
            return Ok(None);
        }
        self.with_borrowed_aggregate_budget_v1(|this, budget| {
            budget.charge_work(8)?;
            let (shape, values, mut ordinal) = if let Some(view) = view {
                let view = this.borrowed_aggregate_view_v1(view, budget)?;
                if !matches!(place.projections().first(), Some(p) if p.kind() == SemanticProjectionKindV1::Dereference && p.result_type() == view.shape.aggregate_type) {
                    return Err(borrowed_aggregate_error_v1("borrowed aggregate field path must begin at the dereferenced aggregate"));
                }
                (&view.shape, &view.values, 1)
            } else {
                let storage = this.borrowed_aggregate_storage.get(&local).ok_or_else(|| borrowed_aggregate_error_v1("borrowed aggregate owner is uninitialized"))?;
                if !storage.live { return Err(borrowed_aggregate_error_v1("borrowed aggregate owner is dead")); }
                (&storage.shape, &storage.values, 0)
            };
            if place.projections().len() > MAX_SSA_VALUE_COMPONENTS_V1 { return Err(borrowed_aggregate_error_v1("borrowed aggregate place exceeds its structural bound")); }
            let mut path = borrowed_aggregate_vec_v1(place.projections().len(), budget)?;
            let mut ty = shape.aggregate_type;
            while let Some(projection) = place.projections().get(ordinal) {
                budget.charge_work(8)?;
                let SemanticProjectionKindV1::Field(index) = projection.kind() else { break };
                let Some(SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields)) = this.types.get(ty.index() as usize).map(SemanticTypeDeclV1::shape) else { break };
                ty = *fields.fields().get(index as usize).ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if projection.result_type() != ty { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
                path.push(index);
                ordinal += 1;
            }
            let index = shape.leaf_index(&path, budget)?;
            let leaf = &shape.leaves[index];
            if leaf.semantic_type != ty { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
            let dereference = match &place.projections()[ordinal..] {
                [] => false,
                [projection] if projection.kind() == SemanticProjectionKindV1::Dereference
                    && matches!(leaf.transport, BorrowedAggregateLeafTransportV1::InvariantReference { .. }) => {
                    let SemanticTypeShapeV1::Pointer(pointer) = this.types[ty.index() as usize].shape() else { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch) };
                    if projection.result_type() != pointer.pointee() { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
                    ty = pointer.pointee();
                    true
                }
                _ => return Err(borrowed_aggregate_error_v1("borrowed aggregate use is not an exact leaf access")),
            };
            if place.ty() != ty { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
            budget.reserve_storage(2 * std::mem::size_of::<Type>())?;
            let result = BorrowedAggregateResolvedPlaceV1 { value: values[index].clone(), transport: leaf.transport.clone(), dereference };
            let bytes = argument_product_v1(path.capacity(), std::mem::size_of::<u32>())?;
            drop(path);
            budget.release_storage(bytes)?;
            Ok(Some(result))
        })
    }

    fn borrowed_aggregate_read_place_v1(
        &mut self,
        place: &SemanticPlaceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some(resolved) = self.borrowed_aggregate_place_v1(place)? else {
            return Ok(None);
        };
        if matches!(
            resolved.transport,
            BorrowedAggregateLeafTransportV1::ScalarSlot { .. }
        ) || resolved.dereference
        {
            let Type::Pointer(pointer) = &resolved.value.ty else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            let access = memory_access_for_type(self.types, place.ty(), pointer.address_space)?;
            return self
                .emit(
                    operations,
                    pointer.pointee.as_ref().clone(),
                    OperationKind::Load {
                        pointer: resolved.value.id,
                        access,
                    },
                )
                .map(Some);
        }
        Ok(Some(SemanticValueBindingV1::Value {
            id: resolved.value.id,
            ty: resolved.value.ty,
        }))
    }

    fn borrowed_aggregate_address_v1(
        &mut self,
        place: &SemanticPlaceV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some(resolved) = self.borrowed_aggregate_place_v1(place)? else {
            return Ok(None);
        };
        if !matches!(
            resolved.transport,
            BorrowedAggregateLeafTransportV1::ScalarSlot { .. }
        ) && !resolved.dereference
        {
            return Err(borrowed_aggregate_error_v1(
                "invariant reference or slice descriptor address/rebinding is unsupported",
            ));
        }
        Ok(Some(SemanticValueBindingV1::Value {
            id: resolved.value.id,
            ty: resolved.value.ty,
        }))
    }

    fn borrowed_aggregate_store_place_v1(
        &mut self,
        place: &SemanticPlaceV1,
        value: &SemanticValueBindingV1,
        volatility: SemanticVolatilityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(address) = self.borrowed_aggregate_address_v1(place)? else {
            return Ok(false);
        };
        let (pointer, ty) = address.value().map_err(borrowed_aggregate_error_v1)?;
        let Type::Pointer(pointer_type) = ty else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let (value, ty) = value.value().map_err(borrowed_aggregate_error_v1)?;
        if volatility != SemanticVolatilityV1::NonVolatile
            || pointer_type.access != AccessMode::ReadWrite
            || pointer_type.pointee.as_ref() != &ty
        {
            return Err(borrowed_aggregate_error_v1(
                "borrowed aggregate store is volatile, readonly, or changes field type",
            ));
        }
        let access = memory_access_for_type(self.types, place.ty(), pointer_type.address_space)?;
        self.push_memory_store_v1(operations, pointer, value, access, None)?;
        Ok(true)
    }

    fn borrowed_aggregate_borrow_place_v1(
        &mut self,
        place: &SemanticPlaceV1,
        reference_type: SemanticTypeIdV1,
        kind: SemanticBorrowKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        if self.borrowed_aggregate_preparation.locals.is_empty() {
            return Ok(None);
        }
        let selected = self.with_borrowed_aggregate_budget_v1(|this, budget| {
            budget.charge_work(argument_sum_v1(&[
                this.borrowed_aggregate_preparation.locals.len(),
                2,
            ])?)?;
            Ok(this
                .borrowed_aggregate_preparation
                .locals
                .contains(&place.local().index()))
        })?;
        if !selected {
            return Ok(None);
        }
        if borrowed_aggregate_reference_v1(self.types, reference_type) {
            let SemanticTypeShapeV1::Pointer(pointer) =
                self.types[reference_type.index() as usize].shape()
            else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if pointer.pointee() != place.ty()
                || !matches!(
                    (kind, pointer.mutability()),
                    (
                        SemanticBorrowKindV1::Shared,
                        SemanticMutabilityV1::Immutable
                    ) | (SemanticBorrowKindV1::Mutable, SemanticMutabilityV1::Mutable)
                )
            {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate source borrow kind/type changed",
                ));
            }
            if place.projections().is_empty() {
                return self
                    .borrowed_aggregate_borrow_owner_v1(place.local(), reference_type, operations)
                    .map(Some);
            }
            if matches!(place.projections(), [p] if p.kind() == SemanticProjectionKindV1::Dereference && p.result_type() == place.ty())
                && let Some(SemanticValueBindingV1::BorrowedAggregate { view }) = self
                    .locals
                    .get(place.local().index() as usize)
                    .and_then(Option::as_ref)
            {
                return self
                    .borrowed_aggregate_reborrow_v1(*view, reference_type, operations)
                    .map(Some);
            }
            return Err(borrowed_aggregate_error_v1(
                "borrowed aggregate subobject borrows are unsupported",
            ));
        }
        let Some(address) = self.borrowed_aggregate_address_v1(place)? else {
            return Ok(None);
        };
        let (id, ty) = address.value().map_err(borrowed_aggregate_error_v1)?;
        let Type::Pointer(pointer) = ty else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        match kind {
            SemanticBorrowKindV1::Shared if pointer.access == AccessMode::ReadWrite => {
                let to = Type::pointer(
                    pointer.pointee.as_ref().clone(),
                    pointer.address_space,
                    AccessMode::ReadOnly,
                );
                self.emit(
                    operations,
                    to.clone(),
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value: id,
                        to,
                    },
                )
                .map(Some)
            }
            SemanticBorrowKindV1::Shared if pointer.access == AccessMode::ReadOnly => {
                Ok(Some(address))
            }
            SemanticBorrowKindV1::Mutable if pointer.access == AccessMode::ReadWrite => {
                Ok(Some(address))
            }
            _ => Err(borrowed_aggregate_error_v1(
                "borrowed aggregate field borrow widens access",
            )),
        }
    }
}
