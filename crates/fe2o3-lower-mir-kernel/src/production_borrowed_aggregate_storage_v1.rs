// These are SSA bindings, not alias/lifetime certificates. Independent replay
// must establish the complete source ownership and use relation.
struct BorrowedAggregateStorageV1 {
    owner: BorrowedAggregateSourceOwnerV1,
    shape: BorrowedAggregateShapeV1,
    values: Vec<ValueDef>,
    live: bool,
}

#[derive(Debug)]
struct BorrowedAggregateViewV1 {
    owner: BorrowedAggregateSourceOwnerV1,
    shape: BorrowedAggregateShapeV1,
    values: Vec<ValueDef>,
}

#[derive(Default)]
struct BorrowedAggregatePreparationV1 {
    // Only selected owners and their aggregate-reference closure, not all
    // pointer-shaped locals. Legacy transparent references stay on their path.
    locals: BTreeSet<u32>,
    owners: BTreeMap<u32, BorrowedAggregateShapeV1>,
}

fn borrowed_aggregate_reference_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return false;
    };
    matches!(
        types
            .get(pointer.pointee().index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Aggregate(_) | SemanticTypeShapeV1::Tuple(_))
    )
}

fn prepare_borrowed_aggregates_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    plan: &LoweredFunctionPlanV1,
    signatures: &BTreeMap<SemanticFunctionIdV1, LoweredFunctionSignatureV1>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<BorrowedAggregatePreparationV1, ProductionSemanticKirErrorV1> {
    borrowed_aggregate_build_v1(budget, |budget| {
        let mut result = BorrowedAggregatePreparationV1::default();
        seed_borrowed_aggregate_locals_v1(
            types,
            function,
            callables,
            plan,
            signatures,
            &mut result,
            budget,
        )?;
        if result.locals.is_empty() {
            return Ok(result);
        }
        expand_borrowed_aggregate_locals_v1(types, function, &mut result, budget)?;
        for &local in &result.locals {
            budget.charge_work(argument_sum_v1(&[plan.parameter_local_bindings.len(), 4])?)?;
            let selected_formal = plan.parameter_local_bindings.iter().any(|binding| {
                matches!(binding,
                    PlannedParameterLocalBindingV1::BorrowedAggregate { local: formal, .. }
                        if *formal == local as usize
                )
            });
            if function.locals()[local as usize].role().is_entry_argument() && !selected_formal {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate reference originates at an unselected entry binding",
                ));
            }
        }
        let mut seen = borrowed_aggregate_vec_v1::<bool>(function.blocks().len(), budget)?;
        seen.resize(function.blocks().len(), false);
        let mut next = Some(function.entry());
        while let Some(block) = next {
            budget.charge_work(8)?;
            let visited = seen
                .get_mut(block.index() as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if *visited {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate CFG loops are unsupported",
                ));
            }
            *visited = true;
            let source = &function.blocks()[block.index() as usize];
            next = match source.terminator().kind() {
                SemanticTerminatorKindV1::Return => None,
                SemanticTerminatorKindV1::Goto(edge) => Some(edge.target()),
                SemanticTerminatorKindV1::Call(call)
                    if matches!(call.unwind(), SemanticUnwindActionV1::Unreachable) =>
                {
                    Some(
                        call.destination()
                            .ok_or_else(|| {
                                borrowed_aggregate_error_v1(
                                    "borrowed aggregate call has no continuation",
                                )
                            })?
                            .edge()
                            .target(),
                    )
                }
                _ => {
                    if borrowed_aggregate_terminator_uses_v1(
                        source.terminator().kind(),
                        &result.locals,
                        budget,
                    )? {
                        return Err(borrowed_aggregate_error_v1(
                            "borrowed aggregate control merges are unsupported",
                        ));
                    }
                    None
                }
            };
        }
        for (index, source) in function.blocks().iter().enumerate() {
            budget.charge_work(2)?;
            if !seen[index] && borrowed_aggregate_block_uses_v1(source, &result.locals, budget)? {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate state is used beyond the straight-line affected region",
                ));
            }
        }
        let bytes = seen.capacity();
        drop(seen);
        budget.release_storage(bytes)?;
        Ok(result)
    })
}

impl SemanticFunctionLoweringV1<'_> {
    fn with_borrowed_aggregate_budget_v1<T>(
        &mut self,
        body: impl FnOnce(
            &mut Self,
            &mut dyn BorrowedAggregateBudgetV1,
        ) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let budget = self.borrowed_aggregate_work.take().ok_or_else(|| {
            borrowed_aggregate_error_v1("borrowed aggregate emission has no shared resource ledger")
        })?;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self, &mut *budget)));
        self.borrowed_aggregate_work = Some(budget);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn borrowed_aggregate_initialize_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        local: SemanticLocalIdV1,
        value: &SemanticValueBindingV1,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_borrowed_aggregate_budget_v1(|this, budget| {
            budget.charge_work(8)?;
            if this.borrowed_aggregate_storage.contains_key(&local.index()) { return Err(borrowed_aggregate_error_v1("borrowed aggregate owner reinitialization is unsupported")); }
            let shape = this.borrowed_aggregate_preparation.owners.get(&local.index()).ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?.clone_in(budget)?;
            let function = this.borrowed_aggregate_function.as_ref().ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            budget.reserve_storage(function.as_str().len())?;
            let function = function.clone();
            let site = BorrowedAggregateSourceAnchorV1::Occurrence(fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block: SsaBlockIdV1::new(block.index()), statement: statement.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)? });
            let owner = BorrowedAggregateSourceOwnerV1 { root: this.correspondence_owner, function: this.semantic_function, local, lifetime_start: site };
            let mut values = borrowed_aggregate_vec_v1(shape.leaves.len(), budget)?;
            for leaf in &shape.leaves {
                let mut field = value;
                for index in &leaf.path.fields {
                    budget.charge_work(3)?;
                    let SemanticValueBindingV1::Aggregate(fields) = field else { return Err(borrowed_aggregate_error_v1("borrowed aggregate initialization is not a structural aggregate")); };
                    field = fields.get(*index as usize).ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                }
                budget.charge_work(5)?;
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                let (id, ty) = field.value().map_err(borrowed_aggregate_error_v1)?;
                let expected = match &leaf.transport {
                    BorrowedAggregateLeafTransportV1::ScalarSlot { value_type, .. } => value_type,
                    BorrowedAggregateLeafTransportV1::InvariantReference { kernel_type } | BorrowedAggregateLeafTransportV1::InvariantSlice { kernel_type } => kernel_type,
                };
                if &ty != expected { return Err(borrowed_aggregate_error_v1("borrowed aggregate field initializer changes type, access, or address space")); }
                values.push(ValueDef::new(id, ty));
            }
            for (leaf, value) in shape.leaves.iter().zip(&mut values) {
                budget.charge_work(12)?;
                budget.reserve_storage(argument_product_v1(function.as_str().len(), 3)?)?;
                let carrier = match &leaf.transport {
                    BorrowedAggregateLeafTransportV1::ScalarSlot { value_type, alignment } => {
                        let allocation = FunctionOperationLocation::new(BlockId(block.index()), operations.len());
                        let pointer_ty = leaf.parameter_type(AccessMode::ReadWrite, budget)?;
                        let (pointer, ty) = this.emit(operations, pointer_ty, OperationKind::Alloca { element: value_type.clone(), count: None, address_space: AddressSpace::Private, alignment: *alignment })?.value().map_err(borrowed_aggregate_error_v1)?;
                        let initialization = FunctionOperationLocation::new(BlockId(block.index()), operations.len());
                        this.push_memory_store_v1(operations, pointer, value.id, MemoryAccess::new(AddressSpace::Private, *alignment), None)?;
                        *value = ValueDef::new(pointer, ty);
                        BorrowedAggregateCarrierCandidateV1::ScalarCell {
                            pointer: BorrowedAggregateValueLocatorV1 { function: function.clone(), value: pointer },
                            allocation: BorrowedAggregateOperationLocatorV1 { function: function.clone(), location: allocation },
                            initialization: BorrowedAggregateOperationLocatorV1 { function: function.clone(), location: initialization },
                        }
                    }
                    BorrowedAggregateLeafTransportV1::InvariantReference { .. } => BorrowedAggregateCarrierCandidateV1::CapturedReference { value: BorrowedAggregateValueLocatorV1 { function: function.clone(), value: value.id } },
                    BorrowedAggregateLeafTransportV1::InvariantSlice { .. } => BorrowedAggregateCarrierCandidateV1::WholeSlice { value: BorrowedAggregateValueLocatorV1 { function: function.clone(), value: value.id } },
                };
                let path = leaf.path.clone_in(budget)?;
                borrowed_aggregate_push_v1(&mut this.borrowed_aggregate_fields, BorrowedAggregateFieldCandidateV1 { owner, path, source_type: leaf.semantic_type, source_definition: site, carrier }, budget)?;
            }
            budget.reserve_storage(std::mem::size_of::<BorrowedAggregateStorageV1>() + 4 * std::mem::size_of::<usize>())?;
            this.borrowed_aggregate_storage.insert(local.index(), BorrowedAggregateStorageV1 { owner, shape, values, live: true });
            Ok(())
        })
    }

    fn borrowed_aggregate_borrow_owner_v1(
        &mut self,
        local: SemanticLocalIdV1,
        reference_type: SemanticTypeIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.with_borrowed_aggregate_budget_v1(|this, budget| {
            let shape = borrowed_aggregate_shape_v1(this.types, reference_type, budget)?;
            let storage = this
                .borrowed_aggregate_storage
                .get(&local.index())
                .ok_or_else(|| {
                    borrowed_aggregate_error_v1("borrowed aggregate owner is uninitialized")
                })?;
            budget.charge_work(4)?;
            if !storage.live || !shape.same_fields(&storage.shape, budget)? {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate owner is dead or has a different layout",
                ));
            }
            let owner = storage.owner;
            let mut values = borrowed_aggregate_vec_v1(storage.values.len(), budget)?;
            for value in &storage.values {
                budget.charge_work(3)?;
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                values.push(value.clone());
            }
            this.borrowed_aggregate_make_view_v1(owner, shape, values, operations, budget)
        })
    }

    fn borrowed_aggregate_reborrow_v1(
        &mut self,
        view: usize,
        reference_type: SemanticTypeIdV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.with_borrowed_aggregate_budget_v1(|this, budget| {
            let shape = borrowed_aggregate_shape_v1(this.types, reference_type, budget)?;
            let old = this.borrowed_aggregate_view_v1(view, budget)?;
            if !shape.same_fields(&old.shape, budget)?
                || (old.shape.access == AccessMode::ReadOnly
                    && shape.access != AccessMode::ReadOnly)
            {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate reborrow changes layout or widens access",
                ));
            }
            let owner = old.owner;
            let mut values = borrowed_aggregate_vec_v1(old.values.len(), budget)?;
            for value in &old.values {
                budget.charge_work(3)?;
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                values.push(value.clone());
            }
            this.borrowed_aggregate_make_view_v1(owner, shape, values, operations, budget)
        })
    }

    fn borrowed_aggregate_make_view_v1(
        &mut self,
        owner: BorrowedAggregateSourceOwnerV1,
        shape: BorrowedAggregateShapeV1,
        mut values: Vec<ValueDef>,
        operations: &mut Vec<Operation>,
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        for (value, leaf) in values.iter_mut().zip(&shape.leaves) {
            let expected = leaf.parameter_type(shape.access, budget)?;
            if value.ty == expected {
                continue;
            }
            if !matches!((&value.ty, &expected), (Type::Pointer(actual), Type::Pointer(expected)) if actual.pointee == expected.pointee && actual.address_space == expected.address_space && actual.access == AccessMode::ReadWrite && expected.access == AccessMode::ReadOnly)
            {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate view changes pointer space or widens access",
                ));
            }
            let (id, ty) = self
                .emit(
                    operations,
                    expected.clone(),
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value: value.id,
                        to: expected,
                    },
                )?
                .value()
                .map_err(borrowed_aggregate_error_v1)?;
            *value = ValueDef::new(id, ty);
        }
        let view = self.borrowed_aggregate_views.len();
        borrowed_aggregate_push_v1(
            &mut self.borrowed_aggregate_views,
            BorrowedAggregateViewV1 {
                owner,
                shape,
                values,
            },
            budget,
        )?;
        Ok(SemanticValueBindingV1::BorrowedAggregate { view })
    }

    fn borrowed_aggregate_view_v1(
        &self,
        index: usize,
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<&BorrowedAggregateViewV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let view = self
            .borrowed_aggregate_views
            .get(index)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if matches!(
            view.owner.lifetime_start,
            BorrowedAggregateSourceAnchorV1::Occurrence(_)
        ) {
            let storage = self
                .borrowed_aggregate_storage
                .get(&view.owner.local.index())
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !storage.live || storage.owner != view.owner {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate view outlives its source owner",
                ));
            }
        }
        Ok(view)
    }

    fn borrowed_aggregate_call_component_v1(
        &mut self,
        view: usize,
        component: usize,
    ) -> Result<(ValueId, Type), ProductionSemanticKirErrorV1> {
        self.with_borrowed_aggregate_budget_v1(|this, budget| {
            let view = this.borrowed_aggregate_view_v1(view, budget)?;
            let value = view
                .values
                .get(component)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            budget.charge_work(3)?;
            budget.reserve_storage(std::mem::size_of::<Type>())?;
            Ok((value.id, value.ty.clone()))
        })
    }

    fn borrowed_aggregate_invalidate_owner_v1(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self
            .borrowed_aggregate_preparation
            .owners
            .contains_key(&local.index())
        {
            return Ok(());
        }
        self.with_borrowed_aggregate_budget_v1(|this, budget| {
            budget.charge_work(8)?;
            if let Some(storage) = this.borrowed_aggregate_storage.get_mut(&local.index()) {
                storage.live = false;
            }
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_borrowed_aggregate_call_v1(
        &mut self,
        block: SemanticBlockIdV1,
        ordinal: u32,
        callee: SemanticFunctionIdV1,
        callee_id: &FunctionId,
        parameters: &[HelperCallArgumentV1],
        bindings: &[SemanticValueBindingV1],
        arguments: &[ValueId],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !parameters
            .iter()
            .any(|parameter| parameter.borrowed.is_some())
        {
            return Ok(());
        }
        self.with_borrowed_aggregate_budget_v1(|this, budget| {
            budget.charge_work(parameters.len())?;
            for (slot, parameter) in parameters.iter().enumerate() {
                let Some(formal) = &parameter.borrowed else {
                    continue;
                };
                let Some(SemanticValueBindingV1::BorrowedAggregate { view }) =
                    bindings.get(parameter.source_argument as usize)
                else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                let view = this.borrowed_aggregate_view_v1(*view, budget)?;
                let component = parameter
                    .component
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let leaf = view
                    .shape
                    .leaves
                    .get(component)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                budget.charge_work(argument_sum_v1(&[
                    leaf.path.fields.len(),
                    formal.path.fields.len(),
                    12,
                ])?)?;
                if leaf.path != formal.path
                    || parameter.tuple_field.is_some()
                    || arguments.get(slot) != view.values.get(component).map(|value| &value.id)
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let caller_id = this
                    .borrowed_aggregate_function
                    .as_ref()
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                budget.reserve_storage(argument_sum_v1(&[
                    argument_product_v1(caller_id.as_str().len(), 2)?,
                    callee_id.as_str().len(),
                ])?)?;
                let row = BorrowedAggregateCallCandidateV1 {
                    root: this.correspondence_owner,
                    caller: this.semantic_function,
                    block,
                    call: BorrowedAggregateOperationLocatorV1 {
                        function: caller_id.clone(),
                        location: FunctionOperationLocation::new(
                            BlockId(block.index()),
                            ordinal as usize,
                        ),
                    },
                    source_argument: parameter.source_argument,
                    tuple_field: parameter.tuple_field,
                    actual_owner: view.owner,
                    actual_path: leaf.path.clone_in(budget)?,
                    actual: BorrowedAggregateValueLocatorV1 {
                        function: caller_id.clone(),
                        value: arguments[slot],
                    },
                    physical_argument: u32::try_from(slot)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    callee,
                    formal_local: formal.local,
                    formal_path: formal.path.clone_in(budget)?,
                    formal: BorrowedAggregateValueLocatorV1 {
                        function: callee_id.clone(),
                        value: formal.value,
                    },
                    physical_parameter: u32::try_from(slot)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                };
                borrowed_aggregate_push_v1(&mut this.borrowed_aggregate_calls, row, budget)?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod production_borrowed_aggregate_storage_v1_tests {
    use super::*;
    include!("production_borrowed_aggregate_storage_v1_tests.rs");
}
