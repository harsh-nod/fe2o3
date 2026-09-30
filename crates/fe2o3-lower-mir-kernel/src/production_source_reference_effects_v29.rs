// Promotion is selected only after source flow and every reachable effect agree.

#[derive(Clone, Copy)]
enum SourceReferenceEffectV29 {
    ReadReferent,
    WriteReferent,
    ReadPayload,
    WritePayload,
    ObserveAddress,
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn check_place_access(
        &self,
        place: &SourceReferencePlaceV29,
        access: SourceReferenceAccessV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut mutable_holder = false;
        if access == SourceReferenceAccessV29::Read {
            for loan in self.node_loans(place.node, budget)? {
                budget.charge_work(1)?;
                mutable_holder |= self.plan.loans[loan].kind == SemanticBorrowKindV1::Mutable;
            }
        }
        for index in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            let loan = &self.plan.loans[index];
            let origin = &self.plan.origins[loan.origin];
            if origin.instance != place.instance
                || origin.local != place.local
                || !self.epochs_overlap(
                    origin.instance,
                    origin.local,
                    origin.generation,
                    place.generation,
                    budget,
                )?
                || !self.paths_overlap(
                    place.instance,
                    place.local,
                    &self.plan.projections[origin.projections.clone()],
                    &place.projections,
                    budget,
                )?
                || !self.active(index, None, budget)?
            {
                continue;
            }
            if let Some(selected) = place.loan
                && self.descendant(selected, index, budget)?
            {
                continue;
            }
            if loan.kind == SemanticBorrowKindV1::Mutable
                || access == SourceReferenceAccessV29::Write
                || mutable_holder
            {
                return Err(source_reference_error_v29(
                    "source reference access bypasses a live loan",
                ));
            }
        }
        Ok(())
    }

    fn node_loans(
        &self,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<usize>, ProductionSemanticKirErrorV1> {
        let mut pending = source_reference_scratch_v29(1, budget)?;
        pending.push(node);
        let mut result = source_reference_scratch_v29(0, budget)?;
        while let Some(node) = pending.pop() {
            budget.charge_work(1)?;
            match self
                .plan
                .nodes
                .get(node)
                .ok_or_else(|| {
                    source_reference_error_v29("source reference node is outside its owner")
                })?
                .kind
            {
                SourceReferenceNodeKindV29::Plain(_)
                | SourceReferenceNodeKindV29::Absent
                | SourceReferenceNodeKindV29::Discriminant(_)
                | SourceReferenceNodeKindV29::Address(_) => {}
                SourceReferenceNodeKindV29::Loan(loan) => {
                    emission_push_v1(&mut result, loan, budget)?
                }
                SourceReferenceNodeKindV29::Enum { first, count } => {
                    for offset in 0..count {
                        let member = self.plan.enum_member(first, count, offset, budget)?;
                        let alternative = self.plan.enum_alternative(member, budget)?;
                        for field in 0..alternative.count {
                            budget.charge_work(1)?;
                            let child = *self
                                .plan
                                .children
                                .get(argument_sum_v1(&[alternative.first, field])?)
                                .ok_or_else(source_reference_enum_error_v29)?;
                            if child >= node {
                                return Err(source_reference_enum_error_v29());
                            }
                            emission_push_v1(&mut pending, child, budget)?;
                        }
                    }
                }
                SourceReferenceNodeKindV29::EnumView(view) => {
                    let view = self
                        .plan
                        .enum_views
                        .get(view)
                        .ok_or_else(source_reference_enum_error_v29)?;
                    if view.source >= node || view.child_count == 0 {
                        return Err(source_reference_enum_error_v29());
                    }
                    for field in 0..view.child_count {
                        budget.charge_work(1)?;
                        let child = *self
                            .plan
                            .children
                            .get(argument_sum_v1(&[view.children, field])?)
                            .ok_or_else(source_reference_enum_error_v29)?;
                        if child >= node {
                            return Err(source_reference_enum_error_v29());
                        }
                        emission_push_v1(&mut pending, child, budget)?;
                    }
                }
                SourceReferenceNodeKindV29::Aggregate { first, count } => {
                    let end = argument_sum_v1(&[first, count])?;
                    for &child in self.plan.children.get(first..end).ok_or_else(|| {
                        source_reference_error_v29("source reference child range differs")
                    })? {
                        // Construction only points at earlier nodes, so the arena
                        // cannot hide recursion or an uncharged cycle.
                        if child >= node {
                            return Err(source_reference_error_v29(
                                "source reference node graph is cyclic",
                            ));
                        }
                        emission_push_v1(&mut pending, child, budget)?;
                    }
                }
            }
        }
        Ok(result)
    }

    fn contains_loan(
        &self,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        Ok(!self.node_loans(node, budget)?.is_empty())
    }

    fn active(
        &self,
        loan: usize,
        excluding: Option<ProductionCallInstanceIdV1>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        for (instance, state) in self.frames.iter().enumerate() {
            budget.charge_work(1)?;
            if excluding.is_some_and(|excluded| excluded.index() == instance) {
                continue;
            }
            let Some(state) = state else {
                continue;
            };
            for local in &self.plan.states[*state] {
                budget.charge_work(1)?;
                if let Some(node) = local.node {
                    let loans = self.node_loans(node, budget)?;
                    budget.charge_work(loans.len())?;
                    if loans.contains(&loan) {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }

    fn descendant(
        &self,
        mut child: usize,
        ancestor: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        for _ in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            if child == ancestor {
                return Ok(true);
            }
            let Some(parent) = self.plan.loans.get(child).and_then(|loan| loan.parent) else {
                return Ok(false);
            };
            if parent >= child {
                return Err(source_reference_error_v29(
                    "source reference loan ancestry is cyclic",
                ));
            }
            child = parent;
        }
        Err(source_reference_error_v29(
            "source reference loan ancestry exceeds its roster",
        ))
    }

    fn overlaps(
        &self,
        left: usize,
        right: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let a = &self.plan.origins[left];
        let b = &self.plan.origins[right];
        if a.instance != b.instance
            || a.local != b.local
            || !self.epochs_overlap(a.instance, a.local, a.generation, b.generation, budget)?
        {
            return Ok(false);
        }
        self.paths_overlap(
            a.instance,
            a.local,
            &self.plan.projections[a.projections.clone()],
            &self.plan.projections[b.projections.clone()],
            budget,
        )
    }

    fn paths_overlap(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        a: &[SemanticProjectionV1],
        b: &[SemanticProjectionV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let types = self.plan.instances.owner().source_semantic().types();
        let mut parent = self
            .plan
            .instances
            .instance(instance)
            .ok_or_else(|| {
                source_reference_error_v29("source reference overlap instance is missing")
            })?
            .declaration()
            .locals()
            .get(local.index() as usize)
            .ok_or_else(|| source_reference_error_v29("source reference overlap local is missing"))?
            .ty();
        budget.charge_work(a.len().min(b.len()))?;
        for (a, b) in a.iter().zip(b) {
            if a != b {
                let (SemanticProjectionKindV1::Field(left), SemanticProjectionKindV1::Field(right)) =
                    (a.kind(), b.kind())
                else {
                    return Ok(true);
                };
                if left == right {
                    return Ok(true);
                }
                budget.charge_work(8)?;
                let declaration = &types[parent.index() as usize];
                let (SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields)) =
                    declaration.shape()
                else {
                    return Ok(true);
                };
                let SemanticFieldsShapeV1::Arbitrary {
                    source_order_offsets_bytes: offsets,
                    ..
                } = declaration.layout().fields()
                else {
                    return Ok(true);
                };
                let range = |field: u32, result| -> Option<(u64, u64)> {
                    let ty = *fields.fields().get(field as usize)?;
                    if ty != result {
                        return None;
                    }
                    let start = *offsets.get(field as usize)?;
                    let size = types.get(ty.index() as usize)?.layout().size_bytes()?;
                    let end = start.checked_add(size)?;
                    (end <= declaration.layout().size_bytes()?).then_some((start, end))
                };
                let (Some(left), Some(right)) =
                    (range(left, a.result_type()), range(right, b.result_type()))
                else {
                    return Ok(true);
                };
                return Ok(!(left.1 <= right.0 || right.1 <= left.0));
            }
            parent = a.result_type();
        }
        Ok(true)
    }

    fn effect(
        &mut self,
        loan: usize,
        effect: SourceReferenceEffectV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let origin = self
            .plan
            .loans
            .get(loan)
            .ok_or_else(|| source_reference_error_v29("source reference effect loan is missing"))?
            .origin;
        let ordinal = self.next_effect_ordinal(budget)?;
        for candidate in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            if !self.overlaps(origin, self.plan.loans[candidate].origin, budget)?
                || (candidate != loan && !self.active(candidate, None, budget)?)
            {
                continue;
            }
            self.record_effect(candidate, effect, ordinal, budget)?;
        }
        Ok(())
    }

    fn check_loan_use(
        &self,
        loan: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let record =
            self.plan.loans.get(loan).ok_or_else(|| {
                source_reference_error_v29("source reference use loan is missing")
            })?;
        let origin = &self.plan.origins[record.origin];
        let local = self.local(origin.instance, origin.local)?;
        if local.generation != origin.generation || local.node.is_none() {
            return Err(source_reference_error_v29(
                "source reference referent is dead or replaced",
            ));
        }
        for child in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            if child != loan
                && self.descendant(child, loan, budget)?
                && self.active(child, None, budget)?
            {
                return Err(source_reference_error_v29(
                    "source reference parent is suspended by a live reborrow",
                ));
            }
        }
        Ok(())
    }

    fn borrow(
        &mut self,
        site: SourceReferenceSiteV29,
        kind: SemanticBorrowKindV1,
        source: &SemanticPlaceV1,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let exact = self
            .plan
            .instances
            .borrow_at(
                site.instance,
                site.block,
                site.statement.ok_or_else(|| {
                    source_reference_error_v29("source reference borrow lacks a statement")
                })?,
                budget,
            )
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => source_reference_error_v29(
                    "source reference borrow differs from its source occurrence",
                ),
            })?;
        if !std::ptr::eq(exact.source, source) || exact.kind != kind || exact.destination.ty() != ty
        {
            return Err(source_reference_error_v29(
                "source reference borrow occurrence differs",
            ));
        }
        let types = self.plan.instances.owner().source_semantic().types();
        if execution_cfg_nominal_kind_v29(types, ty)?.is_some() {
            return self.plain(ty, budget);
        }
        if let Some(node) = self.external_reference_borrow_v29(site, kind, source, ty, budget)? {
            return Ok(node);
        }
        let local = self.local(site.instance, source.local())?;
        let node = local
            .node
            .ok_or_else(|| source_reference_error_v29("source reference borrows a dead holder"))?;
        // This already-supported same-type fat-reference reborrow is an alias of
        // its existing allocation binding, not a reference to the descriptor cell.
        if source.projections().len() == 1
            && source.projections()[0].kind() == SemanticProjectionKindV1::Dereference
            && self.plan.nodes[node].ty == ty
            && matches!(types.get(ty.index() as usize).map(|ty| ty.shape()),
                Some(SemanticTypeShapeV1::Pointer(pointer)) if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == SemanticMutabilityV1::Immutable
                    && pointer.metadata() == SemanticPointerMetadataV1::SliceLength)
            && matches!(
                self.plan.nodes[node].kind,
                SourceReferenceNodeKindV29::Plain(_)
            )
        {
            return Ok(node);
        }
        let resolved = self.resolve_reference_place(
            site,
            source,
            SourceReferenceAccessV29::Borrow(kind),
            budget,
        )?;
        let parent = resolved.loan;
        let key = (
            site.instance.index(),
            site.block.index(),
            site.statement.unwrap(),
        );
        charge_execution_cfg_lookup_v29(self.loan_sites.len(), budget)?;
        if let Some(&existing) = self.loan_sites.get(&key) {
            let loan = &self.plan.loans[existing];
            let original = &self.plan.origins[loan.origin];
            budget.charge_work(argument_sum_v1(&[resolved.projections.len(), 12])?)?;
            if loan.kind != kind
                || loan.source_type != ty
                || loan.parent != parent
                || parent == Some(existing)
                || original.instance != resolved.instance
                || original.local != resolved.local
                || original.generation != resolved.generation
                || original.ty != source.ty()
                || self.plan.projections[original.projections.clone()] != resolved.projections
            {
                return Err(source_reference_cfg_obligation_v29());
            }
            let origin = loan.origin;
            let original_value = original.value;
            let original_anchor = original.anchor;
            // Scalar values are abstract, not first-iteration snapshots. Losing
            // an anchor is monotone; a differing tracked loan still refuses.
            let value = self.merge_node(original_value, resolved.value, budget)?;
            self.plan.origins[origin].value = value;
            self.plan.origins[origin].anchor = if original_anchor == resolved.anchor {
                original_anchor
            } else {
                None
            };
            for previous in 0..self.plan.loans.len() {
                budget.charge_work(1)?;
                if !self.overlaps(origin, self.plan.loans[previous].origin, budget)?
                    || !self.active(previous, None, budget)?
                {
                    continue;
                }
                if let Some(parent) = parent
                    && self.descendant(parent, previous, budget)?
                {
                    continue;
                }
                if kind == SemanticBorrowKindV1::Mutable
                    || self.plan.loans[previous].kind == SemanticBorrowKindV1::Mutable
                {
                    return Err(source_reference_error_v29(
                        "source reference has conflicting live loans",
                    ));
                }
            }
            return self.node(ty, SourceReferenceNodeKindV29::Loan(existing), budget);
        }
        let first = self.plan.projections.len();
        for &projection in &resolved.projections {
            emission_push_v1(&mut self.plan.projections, projection, budget)?;
        }
        let origin = self.plan.origins.len();
        emission_push_v1(
            &mut self.plan.origins,
            SourceReferenceOriginV29 {
                instance: resolved.instance,
                local: resolved.local,
                generation: resolved.generation,
                value: resolved.value,
                ty: source.ty(),
                projections: first..self.plan.projections.len(),
                anchor: resolved.anchor,
            },
            budget,
        )?;
        for previous in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            if !self.overlaps(origin, self.plan.loans[previous].origin, budget)?
                || !self.active(previous, None, budget)?
            {
                continue;
            }
            if let Some(parent) = parent
                && self.descendant(parent, previous, budget)?
            {
                continue;
            }
            if kind == SemanticBorrowKindV1::Mutable
                || self.plan.loans[previous].kind == SemanticBorrowKindV1::Mutable
            {
                return Err(source_reference_error_v29(
                    "source reference has conflicting live loans",
                ));
            }
        }
        let loan = self.plan.loans.len();
        emission_push_v1(
            &mut self.plan.loans,
            SourceReferenceLoanV29 {
                site,
                source_type: ty,
                kind,
                origin,
                parent,
                effects: SourceReferenceEffectsV29::default(),
                representation: SourceReferenceRepresentationV29::NeedsAddressable(
                    SourceReferenceCellNeedV29::UnrepresentedType,
                ),
            },
            budget,
        )?;
        reserve_execution_cfg_map_entry_v29::<(usize, u32, usize), usize>(
            self.loan_sites.len(),
            budget,
        )?;
        if self.loan_sites.insert(key, loan).is_some() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.node(ty, SourceReferenceNodeKindV29::Loan(loan), budget)
    }

    fn end_storage(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut state = self.local(instance, local)?;
        for loan in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            let origin = &self.plan.origins[self.plan.loans[loan].origin];
            if origin.instance == instance
                && origin.local == local
                && self.active(loan, None, budget)?
            {
                return Err(source_reference_error_v29(
                    "source reference referent storage dies with a live loan",
                ));
            }
        }
        self.expire_addresses(instance, Some(local), None, budget)?;
        self.invalidate_discriminant_values(Some(instance), Some(local), None, budget)?;
        state.node = None;
        self.set_local(instance, local, state)?;
        self.transfer_storage_lifetime(instance, local, false, budget)
    }

    fn check_frame_exit(
        &self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        for loan in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            if self.plan.origins[self.plan.loans[loan].origin].instance == instance
                && self.active(loan, Some(instance), budget)?
            {
                return Err(source_reference_error_v29(
                    "source reference outlives its referent call frame",
                ));
            }
        }
        Ok(())
    }

    fn write_place(
        &mut self,
        site: SourceReferenceSiteV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferencePlaceV29, ProductionSemanticKirErrorV1> {
        let resolved =
            self.resolve_reference_place(site, place, SourceReferenceAccessV29::Write, budget)?;
        let ordinal = self.next_effect_ordinal(budget)?;
        for loan in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            let origin = &self.plan.origins[self.plan.loans[loan].origin];
            if origin.instance == resolved.instance
                && origin.local == resolved.local
                && origin.generation == resolved.generation
                && self.active(loan, None, budget)?
                && self.paths_overlap(
                    resolved.instance,
                    resolved.local,
                    &self.plan.projections[origin.projections.clone()],
                    &resolved.projections,
                    budget,
                )?
            {
                self.record_effect(
                    loan,
                    SourceReferenceEffectV29::WriteReferent,
                    ordinal,
                    budget,
                )?;
            }
        }
        Ok(resolved)
    }

    fn observe_address(
        &mut self,
        place: &SourceReferencePlaceV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let local = self.local(place.instance, place.local)?;
        for loan in 0..self.plan.loans.len() {
            budget.charge_work(1)?;
            let origin = &self.plan.origins[self.plan.loans[loan].origin];
            if origin.instance == place.instance && origin.local == place.local {
                self.effect(loan, SourceReferenceEffectV29::ObserveAddress, budget)?;
            }
        }
        if let Some(node) = local.node {
            self.observe_node_address(node, budget)?;
        }
        Ok(())
    }

    fn observe_node_address(
        &mut self,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        for loan in self.node_loans(node, budget)? {
            self.check_loan_use(loan, budget)?;
            self.effect(loan, SourceReferenceEffectV29::ObserveAddress, budget)?;
        }
        Ok(())
    }

    fn assertion_failure_effects(
        &mut self,
        site: SourceReferenceSiteV29,
        message: &fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        use fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1 as Message;
        budget.charge_work(2)?;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<usize>(),
            std::mem::size_of::<Result<usize, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let success = self.frame(site.instance)?;
        let failure = self.clone_state(success, budget)?;
        self.frames[site.instance.index()] = Some(failure);
        let result = (|| {
            let mut observe = |operand: &SemanticOperandV1| {
                let node = self.operand(site, operand, budget)?;
                self.observe_node_address(node, budget)
            };
            match message {
                Message::BoundsCheck { length, index }
                | Message::Overflow {
                    left: length,
                    right: index,
                    ..
                }
                | Message::MisalignedPointerDereference {
                    required_alignment: length,
                    found_alignment: index,
                } => {
                    observe(length)?;
                    observe(index)?;
                }
                Message::DivisionByZero(operand) | Message::RemainderByZero(operand) => {
                    observe(operand)?
                }
                Message::NullPointerDereference
                | Message::ResumedAfterReturn
                | Message::ResumedAfterPanic => {}
            }
            Ok(())
        })();
        // Keep ordered diagnostic effects in their own state, including on error.
        self.frames[site.instance.index()] = Some(success);
        result
    }

    fn intrinsic(
        &mut self,
        callable: &SemanticCallableDeclV1,
        arguments: &[usize],
        call: &SemanticDirectCallV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation, binding, ..
        } = callable
        else {
            return Err(source_reference_error_v29(
                "source reference call has unknown external effects",
            ));
        };
        let carrier = match operation {
            SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { disjoint_slice, .. }
            | SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
                disjoint_slice,
                ..
            } => Some((*disjoint_slice, false, false)),
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                disjoint_slice, ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                disjoint_slice,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                disjoint_slice,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                disjoint_slice,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                disjoint_slice,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                disjoint_slice,
                ..
            } => Some((*disjoint_slice, true, true)),
            SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                disjoint_slice,
                ..
            } => Some((*disjoint_slice, false, true)),
            _ => None,
        };
        let shared_witness = match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { index_witness, .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { index_witness, .. } => {
                Some((0, *index_witness))
            }
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                grid_leader,
                ..
            } => Some((1, *grid_leader)),
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                block_witness,
                ..
            } => Some((1, *block_witness)),
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                tile_witness,
                ..
            } => Some((1, *tile_witness)),
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                stripe_witness,
                ..
            } => Some((1, *stripe_witness)),
            SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                witness,
                kind:
                    SemanticWriteOnlyDisjointWriteKindV1::GridExclusive
                    | SemanticWriteOnlyDisjointWriteKindV1::Block { .. }
                    | SemanticWriteOnlyDisjointWriteKindV1::Tiled2d { .. }
                    | SemanticWriteOnlyDisjointWriteKindV1::RowStriped2d { .. },
                ..
            } => Some((1, *witness)),
            _ => None,
        };
        for (argument, &node) in arguments.iter().enumerate() {
            budget.charge_work(1)?;
            for loan in self.node_loans(node, budget)? {
                self.check_loan_use(loan, budget)?;
                if let Some((witness_argument, witness)) = shared_witness
                    && argument == witness_argument
                {
                    // The admitted intrinsic reads its capability, not the
                    // allocation payload governed by that capability.
                    budget.charge_work(12)?;
                    let record = &self.plan.loans[loan];
                    let origin = &self.plan.origins[record.origin];
                    let value = &self.plan.nodes[node];
                    let source_type = self
                        .plan
                        .instances
                        .owner()
                        .source_semantic()
                        .types()
                        .get(record.source_type.index() as usize);
                    if binding.abi().source_input_types().get(argument) != Some(&record.source_type)
                        || value.ty != record.source_type
                        || !matches!(value.kind, SourceReferenceNodeKindV29::Loan(actual) if actual == loan)
                        || record.kind != SemanticBorrowKindV1::Shared
                        || origin.ty != witness
                        || !matches!(source_type.map(|ty| ty.shape()),
                            Some(SemanticTypeShapeV1::Pointer(pointer))
                                if pointer.kind() == SemanticPointerKindV1::Reference
                                    && pointer.mutability() == SemanticMutabilityV1::Immutable
                                    && pointer.pointee() == witness)
                    {
                        return Err(source_reference_error_v29(
                            "source reference shared intrinsic witness differs",
                        ));
                    }
                    self.effect(loan, SourceReferenceEffectV29::ReadReferent, budget)?;
                    continue;
                }
                let Some((carrier, reads, writes)) = carrier else {
                    return Err(source_reference_error_v29(
                        "source reference intrinsic effect is not represented",
                    ));
                };
                let origin = &self.plan.origins[self.plan.loans[loan].origin];
                if argument != 0
                    || origin.ty != carrier
                    || origin.projections.len() != 0
                    || origin.anchor.is_none()
                    || (writes && self.plan.loans[loan].kind != SemanticBorrowKindV1::Mutable)
                {
                    return Err(source_reference_error_v29(
                        "source reference allocation-view operand differs",
                    ));
                }
                self.effect(loan, SourceReferenceEffectV29::ReadReferent, budget)?;
                if reads {
                    self.effect(loan, SourceReferenceEffectV29::ReadPayload, budget)?;
                }
                if writes {
                    self.effect(loan, SourceReferenceEffectV29::WritePayload, budget)?;
                }
            }
        }
        let ty = binding.abi().source_output_type();
        if call
            .destination()
            .is_some_and(|destination| destination.place().ty() != ty)
        {
            return Err(source_reference_error_v29(
                "source reference intrinsic result type differs",
            ));
        }
        self.plain(ty, budget)
    }

    fn finish_effects(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let semantic = self.plan.instances.owner().source_semantic();
        let root = self
            .plan
            .instances
            .instance(self.plan.root)
            .ok_or_else(|| source_reference_error_v29("source reference root is missing"))?
            .declaration();
        for index in 0..self.plan.loans.len() {
            budget.charge_work(5)?;
            let loan = &self.plan.loans[index];
            let origin = &self.plan.origins[loan.origin];
            let representation = if loan.effects.address_observations != 0 {
                SourceReferenceRepresentationV29::NeedsAddressable(
                    SourceReferenceCellNeedV29::AddressObservation,
                )
            } else if loan.effects.referent_writes != 0 {
                SourceReferenceRepresentationV29::NeedsAddressable(
                    SourceReferenceCellNeedV29::ReferentWrite,
                )
            } else if let Some(anchor) = origin.anchor
                && anchor.ty == origin.ty
            {
                prepay_argument_shape_v1(semantic, anchor.ty, budget)?;
                if authenticated_disjoint_slice_parameter(
                    semantic.types(),
                    semantic.callables(),
                    root,
                    anchor.argument,
                    anchor.ty,
                )
                .is_some()
                {
                    SourceReferenceRepresentationV29::ExistingAllocationBinding(anchor)
                } else if self.snapshot_type(origin.ty, budget)? {
                    SourceReferenceRepresentationV29::StableReferent
                } else {
                    SourceReferenceRepresentationV29::NeedsAddressable(
                        SourceReferenceCellNeedV29::UnrepresentedType,
                    )
                }
            } else if self.snapshot_type(origin.ty, budget)? {
                SourceReferenceRepresentationV29::StableReferent
            } else {
                SourceReferenceRepresentationV29::NeedsAddressable(
                    SourceReferenceCellNeedV29::UnrepresentedType,
                )
            };
            self.plan.loans[index].representation = representation;
        }
        Ok(())
    }

    fn snapshot_type(
        &self,
        ty: SemanticTypeIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let types = self.plan.instances.owner().source_semantic().types();
        let mut pending = source_reference_scratch_v29(1, budget)?;
        pending.push(ty);
        let mut visited = 0_usize;
        while let Some(ty) = pending.pop() {
            budget.charge_work(1)?;
            visited = argument_sum_v1(&[visited, 1])?;
            if visited > MAX_SSA_VALUE_COMPONENTS_V1 {
                return Ok(false);
            }
            let Some(declaration) = types.get(ty.index() as usize) else {
                return Ok(false);
            };
            if declaration.layout().is_uninhabited() {
                return Ok(false);
            }
            match declaration.shape() {
                SemanticTypeShapeV1::Scalar(_)
                | SemanticTypeShapeV1::ValidityScalar(_)
                | SemanticTypeShapeV1::Unit => {}
                SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields) => {
                    for &field in fields.fields() {
                        emission_push_v1(&mut pending, field, budget)?;
                    }
                }
                SemanticTypeShapeV1::Array { element, length } => {
                    budget.charge_work(1)?;
                    if *length != 0 {
                        emission_push_v1(&mut pending, *element, budget)?;
                    }
                }
                SemanticTypeShapeV1::Pointer(pointer)
                    if pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.metadata() == SemanticPointerMetadataV1::SliceLength => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }
}
