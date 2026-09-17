impl<'a> BorrowedReplayV1<'a, '_> {
    fn check_fields(
        &mut self,
        root_group: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let root = self.groups[root_group].function.source;
        let source = &self.subject.semantic_ssa.source_semantic().functions()
            [root.semantic_function.index() as usize];
        for (index, field) in self.candidates.fields.iter().enumerate() {
            budget.charge_work(10)?;
            if field.owner.root != self.root
                || field.owner.function != root.semantic_function
                || field.path.fields.len() != 1
                || field.source_definition != field.owner.lifetime_start
            {
                return Err(borrowed_replay_unsupported_v1());
            }
            let BorrowedAggregateSourceAnchorV1::Occurrence(
                fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement },
            ) = field.source_definition
            else {
                return Err(borrowed_replay_unsupported_v1());
            };
            let declaration = source
                .locals()
                .get(field.owner.local.index() as usize)
                .ok_or_else(borrowed_replay_mismatch_v1)?;
            let types = self.subject.semantic_ssa.source_semantic().types();
            let Some(SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields)) =
                types
                    .get(declaration.ty().index() as usize)
                    .map(SemanticTypeDeclV1::shape)
            else {
                return Err(borrowed_replay_mismatch_v1());
            };
            if fields.fields().get(field.path.fields[0] as usize) != Some(&field.source_type) {
                return Err(borrowed_replay_mismatch_v1());
            }
            let initializer = source
                .blocks()
                .get(block.get() as usize)
                .and_then(|b| b.statements().get(statement as usize))
                .ok_or_else(borrowed_replay_mismatch_v1)?;
            if !matches!(initializer.kind(), SemanticStatementKindV1::Assign(a)
                if a.destination().local() == field.owner.local
                    && a.destination().projections().is_empty()
                    && a.value().result_type() == declaration.ty()
                    && matches!(a.value().kind(), SemanticRvalueKindV1::Aggregate(_)))
            {
                return Err(borrowed_replay_mismatch_v1());
            }
            for earlier in &self.candidates.fields[..index] {
                budget.charge_work(4)?;
                if earlier.owner == field.owner && earlier.path == field.path {
                    return Err(borrowed_replay_mismatch_v1());
                }
                if let (
                    BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer: left, .. },
                    BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer: right, .. },
                ) = (&earlier.carrier, &field.carrier)
                    && left == right
                {
                    return Err(borrowed_replay_mismatch_v1());
                }
            }
            if !matches!(
                field.carrier,
                BorrowedAggregateCarrierCandidateV1::ScalarCell { .. }
            ) {
                self.allocated[index] = true;
            }
        }
        Ok(())
    }

    fn push_reference(
        &mut self,
        reference: BorrowedReplayReferenceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if self.references.len() == self.references.capacity() {
            let before = self.references.capacity();
            budget.reserve_storage(std::mem::size_of::<BorrowedReplayReferenceV1>())?;
            self.references
                .try_reserve_exact(1)
                .map_err(|_| ArgumentResourceV1::Allocation)?;
            let extra = self
                .references
                .capacity()
                .checked_sub(
                    before
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.reserve_storage(argument_product_v1(
                extra,
                std::mem::size_of::<BorrowedReplayReferenceV1>(),
            )?)?;
        }
        let index = self.references.len();
        self.references.push(reference);
        Ok(index)
    }

    fn function(
        &mut self,
        group: usize,
        incoming: Option<usize>,
        depth: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        if depth >= 32 || self.active[group] {
            return Err(borrowed_replay_unsupported_v1());
        }
        self.active[group] = true;
        self.function_claims[group] = true;
        let subject = self.subject;
        let association = self.groups[group].function.source;
        let source = &subject.semantic_ssa.source_semantic().functions()
            [association.semantic_function.index() as usize];
        let physical = self.groups[group].function.canonical.function;
        let body = physical
            .body
            .as_ref()
            .ok_or_else(borrowed_replay_mismatch_v1)?;
        if body.blocks.first().map(|block| block.id) != Some(BlockId(source.entry().index())) {
            return Err(borrowed_replay_mismatch_v1());
        }
        let types = subject.semantic_ssa.source_semantic().types();
        if !physical.signature.results.is_empty()
            || !matches!(
                types[source.abi().source_output_type().index() as usize].shape(),
                SemanticTypeShapeV1::Unit
            )
            || !matches!(
                source.abi().return_value().mode(),
                SemanticAbiPassModeV1::Ignore
            )
        {
            return Err(borrowed_replay_unsupported_v1());
        }
        let mut locals = unit_local_filled_v1(
            source.locals().len(),
            BorrowedReplayValueV1::Uninitialized,
            budget,
        )?;
        let mut visited = unit_local_filled_v1(source.blocks().len(), false, budget)?;
        validate_parameter_correspondence_v1(
            subject.semantic_ssa.source_semantic(),
            association,
            physical,
            self.groups[group].parameters(),
            budget,
        )?;
        if let Some(reference) = incoming {
            for (index, local) in source.locals().iter().enumerate() {
                budget.charge_work(1)?;
                match local.role() {
                    SemanticLocalRoleV1::Argument(0) => {
                        locals[index] = BorrowedReplayValueV1::Reference(reference)
                    }
                    role if role.is_entry_argument()
                        && types[local.ty().index() as usize].layout().size_bytes() == Some(0) =>
                    {
                        locals[index] = BorrowedReplayValueV1::Unit
                    }
                    role if role.is_entry_argument() => {
                        return Err(borrowed_replay_unsupported_v1());
                    }
                    _ => {}
                }
            }
        } else {
            if !self.groups[group].components.is_empty() {
                return Err(borrowed_replay_unsupported_v1());
            }
            for binding in self.groups[group].direct {
                budget.charge_work(1)?;
                locals[binding.semantic_local.index() as usize] =
                    BorrowedReplayValueV1::Native(binding.kernel_ir_value);
            }
            for binding in self.groups[group].ignored {
                budget.charge_work(1)?;
                locals[binding.semantic_local.index() as usize] = BorrowedReplayValueV1::Unit;
            }
        }
        let mut current = source.entry();
        loop {
            budget.charge_work(5)?;
            let seen = visited
                .get_mut(current.index() as usize)
                .ok_or_else(borrowed_replay_mismatch_v1)?;
            if *seen {
                return Err(borrowed_replay_unsupported_v1());
            }
            *seen = true;
            let source_block = &source.blocks()[current.index() as usize];
            budget.charge_work(body.blocks.len())?;
            let block = body
                .blocks
                .iter()
                .find(|b| b.id == BlockId(current.index()))
                .ok_or_else(borrowed_replay_mismatch_v1)?;
            if !block.parameters.is_empty() {
                return Err(borrowed_replay_unsupported_v1());
            }
            let mut next = 0;
            if current == source.entry() && incoming.is_none() {
                // A statement-owned allocation stays in that statement's replay,
                // including when its initializer emits no native operations.
                let prologue_end = if source_block.statements().is_empty() {
                    budget.charge_work(self.groups[group].spans.len())?;
                    self.groups[group]
                        .spans
                        .iter()
                        .find(|span| span.semantic_block == current)
                        .map(|span| span.first_operation_ordinal as usize)
                } else {
                    budget.charge_work(subject.correspondence.statement_operation_spans.len())?;
                    subject
                        .correspondence
                        .statement_operation_spans
                        .iter()
                        .find(|span| {
                            span.correspondence_owner == self.root
                                && span.semantic_function == association.semantic_function
                                && span.semantic_block == current
                                && span.statement_ordinal == 0
                        })
                        .map(|span| span.first_operation_ordinal as usize)
                }
                .ok_or_else(borrowed_replay_mismatch_v1)?;
                while next < prologue_end
                    && matches!(
                        block.operations.get(next),
                        Some(Operation {
                            kind: OperationKind::Alloca { .. },
                            ..
                        })
                    )
                {
                    budget.charge_work(5)?;
                    let mut found = None;
                    for (index, field) in self.candidates.fields.iter().enumerate() {
                        budget.charge_work(3)?;
                        if let BorrowedAggregateCarrierCandidateV1::ScalarCell {
                            allocation, ..
                        } = &field.carrier
                            && allocation.location == FunctionOperationLocation::new(block.id, next)
                        {
                            if found.replace(index).is_some() {
                                return Err(borrowed_replay_mismatch_v1());
                            }
                        }
                    }
                    let mut native = BorrowedReplaySpanV1 {
                        block,
                        next,
                        end: argument_sum_v1(&[next, 1])?,
                    };
                    self.allocate(
                        group,
                        found.ok_or_else(borrowed_replay_mismatch_v1)?,
                        &mut native,
                        budget,
                    )?;
                    native.finish()?;
                    next = native.next;
                }
            }
            for (ordinal, statement) in source_block.statements().iter().enumerate() {
                if incoming.is_none() && !borrowed_replay_statement_supported_v1(statement.kind()) {
                    self.closed_root_suffix(
                        group, current, ordinal, next, &visited, &locals, budget,
                    )?;
                    self.active[group] = false;
                    return Ok(());
                }
                budget.charge_work(subject.correspondence.statement_operation_spans.len())?;
                let mut spans = subject
                    .correspondence
                    .statement_operation_spans
                    .iter()
                    .filter(|span| {
                        span.correspondence_owner == self.root
                            && span.semantic_function == association.semantic_function
                            && span.semantic_block == current
                            && span.statement_ordinal as usize == ordinal
                    });
                let span = spans.next().ok_or_else(borrowed_replay_mismatch_v1)?;
                if spans.next().is_some()
                    || span.kernel_ir_block != block.id
                    || span.first_operation_ordinal as usize != next
                {
                    return Err(borrowed_replay_mismatch_v1());
                }
                let end = argument_sum_v1(&[next, span.operation_count as usize])?;
                if end > block.operations.len() {
                    return Err(borrowed_replay_mismatch_v1());
                }
                let mut native = BorrowedReplaySpanV1 { block, next, end };
                self.statement(
                    group,
                    current,
                    ordinal,
                    statement,
                    &mut locals,
                    &mut native,
                    budget,
                )?;
                native.finish()?;
                next = end;
            }
            budget.charge_work(self.groups[group].spans.len())?;
            let mut spans = self.groups[group]
                .spans
                .iter()
                .filter(|span| span.semantic_block == current);
            let span = spans.next().ok_or_else(borrowed_replay_mismatch_v1)?;
            if spans.next().is_some()
                || span.kernel_ir_block != block.id
                || span.first_operation_ordinal as usize != next
                || argument_sum_v1(&[next, span.operation_count as usize])?
                    != block.operations.len()
            {
                return Err(borrowed_replay_mismatch_v1());
            }
            let mut native = BorrowedReplaySpanV1 {
                block,
                next,
                end: block.operations.len(),
            };
            if incoming.is_none()
                && !matches!(
                    source_block.terminator().kind(),
                    SemanticTerminatorKindV1::Return | SemanticTerminatorKindV1::Goto(_)
                )
                && !matches!(source_block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                    if matches!(subject.semantic_ssa.source_semantic().callables().get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::Defined { .. })))
            {
                self.closed_root_suffix(
                    group,
                    current,
                    source_block.statements().len(),
                    next,
                    &visited,
                    &locals,
                    budget,
                )?;
                self.active[group] = false;
                return Ok(());
            }
            let target = match source_block.terminator().kind() {
                SemanticTerminatorKindV1::Return => {
                    if !matches!(&block.terminator, Some(Terminator::Return { values })
                        if values.is_empty())
                    {
                        return Err(borrowed_replay_mismatch_v1());
                    }
                    None
                }
                SemanticTerminatorKindV1::Goto(edge) => Some(edge.target()),
                SemanticTerminatorKindV1::Call(call) => {
                    self.call(
                        group,
                        current,
                        call,
                        &mut locals,
                        &mut native,
                        depth,
                        budget,
                    )?;
                    let destination = call
                        .destination()
                        .ok_or_else(borrowed_replay_unsupported_v1)?;
                    if !destination.place().projections().is_empty()
                        || !matches!(
                            types[destination.place().ty().index() as usize].shape(),
                            SemanticTypeShapeV1::Unit
                        )
                    {
                        return Err(borrowed_replay_unsupported_v1());
                    }
                    locals[destination.place().local().index() as usize] =
                        BorrowedReplayValueV1::Unit;
                    Some(destination.edge().target())
                }
                _ => return Err(borrowed_replay_unsupported_v1()),
            };
            native.finish()?;
            let Some(target) = target else {
                break;
            };
            if !matches!(&block.terminator, Some(Terminator::Branch { target: actual, arguments })
                if *actual == BlockId(target.index()) && arguments.is_empty())
            {
                return Err(borrowed_replay_unsupported_v1());
            }
            current = target;
        }
        budget.charge_work(visited.len())?;
        if visited.iter().any(|v| !v) || body.blocks.len() != visited.len() {
            return Err(borrowed_replay_unsupported_v1());
        }
        self.active[group] = false;
        Ok(())
    }

    // Census every remaining observation, including unreachable syntactic paths.
    // The diagnostic ends in refusal until exact-subject sealed coverage and the
    // whole-source suffix gate are integrated. It is not suffix authority.
    fn closed_root_suffix(
        &self,
        group: usize,
        current: SemanticBlockIdV1,
        first_statement: usize,
        first_operation: usize,
        prefix: &[bool],
        locals: &[BorrowedReplayValueV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(argument_sum_v1(&[
            self.initialized.len(),
            self.call_claims.len(),
            4,
        ])?)?;
        if self.initialized.iter().any(|v| !v) || self.call_claims.iter().any(|v| !v) {
            return Err(borrowed_replay_unsupported_v1());
        }
        let semantic = self.subject.semantic_ssa.source_semantic();
        let source = &semantic.functions()
            [self.groups[group].function.source.semantic_function.index() as usize];
        let mut forbidden = unit_local_filled_v1(locals.len(), false, budget)?;
        for (index, local) in source.locals().iter().enumerate() {
            budget.charge_work(3)?;
            forbidden[index] = matches!(
                locals[index],
                BorrowedReplayValueV1::Owner(_) | BorrowedReplayValueV1::Reference(_)
            );
            if let BorrowedReplayValueV1::Native(value) = locals[index] {
                forbidden[index] |= self.is_carrier(group, value, budget)?;
            }
            // A moved reference and a reused owner local remain forbidden. New
            // values of the same owner type cannot smuggle another borrow into
            // the delegated suffix without joining this relation.
            let referent = match semantic.types()[local.ty().index() as usize].shape() {
                SemanticTypeShapeV1::Pointer(pointer) => pointer.pointee(),
                _ => local.ty(),
            };
            for field in self.candidates.fields {
                budget.charge_work(3)?;
                if referent == source.locals()[field.owner.local.index() as usize].ty() {
                    forbidden[index] = true;
                }
            }
        }
        for (index, block) in source.blocks().iter().enumerate() {
            budget.charge_work(3)?;
            if prefix[index] && index != current.index() as usize {
                continue;
            }
            let start = if index == current.index() as usize {
                first_statement
            } else {
                0
            };
            for statement in &block.statements()[start..] {
                borrowed_replay_suffix_statement_v1(statement.kind(), &forbidden, budget)?;
            }
            borrowed_replay_suffix_terminator_v1(
                block.terminator().kind(),
                semantic.callables(),
                &forbidden,
                budget,
            )?;
            block.terminator().kind().try_for_each_edge(|edge| {
                budget.charge_work(3)?;
                let target = edge.target().index() as usize;
                // This initial suffix rule admits forward acyclic CFG only.
                if target <= index || *prefix.get(target).ok_or_else(borrowed_replay_mismatch_v1)? {
                    return Err(borrowed_replay_unsupported_v1());
                }
                Ok(())
            })?;
        }
        let physical = self.groups[group].function.canonical;
        let blocks = &self.inventory.blocks()[physical.blocks.clone()];
        for block in blocks {
            budget.charge_work(4)?;
            let id = block.block.id.0 as usize;
            let in_prefix = prefix.get(id).copied().unwrap_or(false);
            if in_prefix && id != current.index() as usize {
                continue;
            }
            let first = if id == current.index() as usize {
                first_operation
            } else {
                0
            };
            for operation in &self.inventory.operations()[block.operations.clone()] {
                budget.charge_work(2)?;
                if (operation.coordinate.operation as usize) < first {
                    continue;
                }
                for operand in &self.inventory.uses()[operation.operands.clone()] {
                    if self.is_carrier(group, operand.value, budget)? {
                        return Err(borrowed_replay_unsupported_v1());
                    }
                }
                // No unchecked defined helper can enter the borrowed closure.
                if matches!(operation.operation.kind, OperationKind::Call { .. }) {
                    return Err(borrowed_replay_unsupported_v1());
                }
            }
            for operand in &self.inventory.uses()[block.terminator_uses.clone()] {
                if self.is_carrier(group, operand.value, budget)? {
                    return Err(borrowed_replay_unsupported_v1());
                }
            }
            for edge in &self.inventory.edges()[block.edges.clone()] {
                budget.charge_work(3)?;
                if edge.target.block <= block.coordinate.block
                    || prefix
                        .get(edge.target_id.0 as usize)
                        .copied()
                        .unwrap_or(false)
                {
                    return Err(borrowed_replay_unsupported_v1());
                }
            }
        }
        Err(unsupported(
            self.root.index(),
            None,
            None,
            "borrowed aggregate root suffix requires sealed coverage and checked source composition",
        ))
    }

    fn is_carrier(
        &self,
        group: usize,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let function = &self.groups[group].function.canonical.function.id;
        for field in self.candidates.fields {
            let locator = match &field.carrier {
                BorrowedAggregateCarrierCandidateV1::ScalarCell { pointer, .. } => pointer,
                BorrowedAggregateCarrierCandidateV1::CapturedReference { value }
                | BorrowedAggregateCarrierCandidateV1::WholeSlice { value } => value,
            };
            budget.charge_work(argument_sum_v1(&[
                4,
                function.as_str().len(),
                locator.function.as_str().len(),
            ])?)?;
            if locator.function == *function && locator.value == value {
                return Ok(true);
            }
        }
        for reference in &self.references {
            budget.charge_work(2)?;
            if reference.group != group {
                continue;
            }
            for (_, carrier) in &reference.values {
                budget.charge_work(1)?;
                if *carrier == value {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
