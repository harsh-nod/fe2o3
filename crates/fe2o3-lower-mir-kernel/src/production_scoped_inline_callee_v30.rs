// A live source index for unchanged scalar inline instructions. It permits
// movement only; original operand correspondence and final replay still apply.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScopedInlineChainV30 {
    first: Option<usize>,
    last: Option<usize>,
    count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedInlineSourceV30 {
    instance: ProductionCallInstanceIdV1,
    source: SemanticKirTerminatorOperationSpanV1,
    span: usize,
    identity: fe2o3_kernel_ir::AssemblySourceIdentity,
    instruction: fe2o3_mir_model::semantic_mir_v1::SemanticGfx942InlineU32V30,
    next: Option<usize>,
}

struct ScopedInlineCalleeV30<'a> {
    function: &'a Function,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

struct ScopedInlineActualV30<'a> {
    location: (BlockId, u32),
    operation: &'a Operation,
    seen: bool,
}

impl ProductionInstanceCorrespondenceV1<'_, '_> {
    fn append_inline_source_v30(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        first_span: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<ScopedInlineChainV30> {
        use InstanceCorrespondenceErrorV1::Source as Refused;
        let original = self.plan.instance(instance).ok_or(Refused)?;
        let calls = self.plan.calls(instance).ok_or(Refused)?;
        let mut count = 0usize;
        // Independent original-call census, once per emitted source instance.
        budget.charge_work(calls.len())?;
        for call in calls {
            if matches!(
                call.callable(),
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_),
                    ..
                }
            ) {
                budget.charge_work(2)?;
                match self.plan.call_control(call.occurrence()) {
                    Some(ProductionCallControlV1::Unreachable) => continue,
                    Some(ProductionCallControlV1::MayReturn) if call.child().is_none() => {}
                    _ => return Err(Refused),
                }
                count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
            }
        }
        if count == 0 {
            return Ok(ScopedInlineChainV30::default());
        }
        let semantic = self.plan.owner().source_semantic();
        self.inline.reserve(count, budget, &mut self.storage)?;
        let first = self.inline.rows.len();
        let end = first
            .checked_add(count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        let spans = self.spans.rows.get(first_span..).ok_or(Refused)?;
        budget.charge_work(spans.len())?;
        for (offset, mapped) in spans.iter().enumerate() {
            let InstanceSpanSourceV1::Terminator(source) = mapped.source else {
                continue;
            };
            let block = original
                .declaration()
                .blocks()
                .get(source.semantic_block.index() as usize)
                .ok_or(Refused)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(instruction),
                binding,
                ..
            }) = semantic.callables().get(call.callee().index() as usize)
            else {
                continue;
            };
            budget.charge_work(192)?;
            match self.plan.call_control(ProductionCallOccurrenceV1 {
                caller: instance,
                block: source.semantic_block,
            }) {
                Some(ProductionCallControlV1::Unreachable) => continue,
                Some(ProductionCallControlV1::MayReturn) => {}
                _ => return Err(Refused),
            }
            if self.inline.rows.len() >= end
                || mapped.instance != instance
                || source.semantic_function != original.function()
            {
                return Err(Refused);
            }
            let identity = gfx942_inline_scalar_correspondence_v30::checked_source_instruction(
                semantic,
                original.declaration(),
                call,
                *instruction,
                binding.abi(),
            )
            .ok_or(Refused)?;
            self.inline.rows.push(ScopedInlineSourceV30 {
                instance,
                source,
                span: first_span
                    .checked_add(offset)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
                identity,
                instruction: *instruction,
                next: None,
            });
        }
        if self.inline.rows.len() != end {
            return Err(Refused);
        }
        // Original statement identities are unique per instance. Physical span
        // ordinals remain stable when later splices update their coordinates.
        let sort_work = count
            .checked_mul(32)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        call_splice_sort_work_v1(sort_work, budget)?;
        self.inline.rows[first..end].sort_unstable_by_key(|row| row.identity.statement);
        budget.charge_work(sort_work)?;
        if self.inline.rows[first..end]
            .windows(2)
            .any(|pair| pair[0].identity.statement == pair[1].identity.statement)
        {
            return Err(Refused);
        }
        budget.charge_work(count)?;
        for index in first..end - 1 {
            self.inline.rows[index].next = Some(index + 1);
        }
        Ok(ScopedInlineChainV30 {
            first: Some(first),
            last: Some(end - 1),
            count,
        })
    }

    fn join_inline_source_v30(
        &mut self,
        caller: usize,
        child: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        budget.charge_work(4)?;
        let left = self.seeds.rows[caller].inline;
        let right = self.seeds.rows[child].inline;
        let count = left
            .count
            .checked_add(right.count)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if let Some(last) = left.last {
            let tail = self
                .inline
                .rows
                .get_mut(last)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            if tail.next.is_some() {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            tail.next = right.first;
        }
        self.seeds.rows[caller].inline = ScopedInlineChainV30 {
            first: left.first.or(right.first),
            last: right.last.or(left.last),
            count,
        };
        self.seeds.rows[child].inline = ScopedInlineChainV30::default();
        Ok(())
    }
}

impl ScopedInlineCalleeV30<'_> {
    fn check(
        &self,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(1)?;
        if !std::ptr::eq(self.function, function) {
            return Err(CallInstanceEmissionErrorV1::CalleeInlineAssembly);
        }
        Ok(())
    }
}

impl ScopedDeferredScalarViewV29<'_, '_, '_> {
    fn inline_callee_v30<'a>(
        &self,
        function: &'a Function,
        index: &CallSpliceIndexV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
        scratch: &mut usize,
    ) -> Result<ScopedInlineCalleeV30<'a>, CallInstanceEmissionErrorV1> {
        use CallInstanceEmissionErrorV1::CalleeInlineAssembly as Refused;
        self.check_ledger(budget)?;
        let DeferredPartsScopeV29::Container { map, container, .. } = self.scope else {
            return Err(Refused);
        };
        map.check_live_ledger_v1(budget)
            .map_err(|error| match error {
                InstanceCorrespondenceErrorV1::Resource(error) => error.into(),
                _ => Refused,
            })?;
        budget.charge_work(3)?;
        let seed = map
            .seeds
            .rows
            .get(self.inline_seed.ok_or(Refused)?)
            .ok_or(Refused)?;
        budget.charge_work(function.id.as_str().len())?;
        if seed.instance != container
            || seed.container != container
            || seed.function_name != function.id.as_str()
        {
            return Err(Refused);
        }
        let chain = seed.inline;
        let body = function.body.as_ref().ok_or(Refused)?;
        let mut count = 0usize;
        budget.charge_work(body.blocks.len())?;
        for block in &body.blocks {
            budget.charge_work(block.operations.len())?;
            for operation in &block.operations {
                if matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                    count = count.checked_add(1).ok_or_else(call_splice_arithmetic_v1)?;
                }
            }
        }
        if count != chain.count
            || (count == 0) != chain.first.is_none()
            || (count == 0) != chain.last.is_none()
        {
            return Err(Refused);
        }
        call_splice_charge_storage_v1(
            std::mem::size_of::<ScopedInlineCalleeV30<'_>>(),
            budget,
            scratch,
        )?;
        if count == 0 {
            return Ok(ScopedInlineCalleeV30 {
                function,
                ledger: self.ledger,
            });
        }
        call_splice_charge_storage_v1(
            argument_sum_v1(&[
                std::mem::size_of::<Vec<ScopedInlineActualV30<'_>>>(),
                std::mem::size_of::<[(ValueId, Option<ScalarType>); 2]>(),
            ])?,
            budget,
            scratch,
        )?;
        let mut actual = call_splice_vec_v1::<ScopedInlineActualV30<'_>>(count, budget, scratch)?;
        budget.charge_work(body.blocks.len())?;
        for block in &body.blocks {
            budget.charge_work(block.operations.len())?;
            for (ordinal, operation) in block.operations.iter().enumerate() {
                if matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                    actual.push(ScopedInlineActualV30 {
                        location: (
                            block.id,
                            u32::try_from(ordinal).map_err(|_| call_splice_arithmetic_v1())?,
                        ),
                        operation,
                        seen: false,
                    });
                }
            }
        }
        call_splice_sort_work_v1(actual.len(), budget)?;
        actual.sort_unstable_by_key(|row| row.location);
        budget.charge_work(actual.len())?;
        if actual
            .windows(2)
            .any(|pair| pair[0].location == pair[1].location)
        {
            return Err(Refused);
        }
        let mut next = chain.first;
        let mut last = None;
        // Follow only this container's prepaid source chain, never the global
        // seed/span rosters for each instruction or each callee.
        for _ in 0..chain.count {
            budget.charge_work(8)?;
            let ordinal = next.ok_or(Refused)?;
            let expected = map.inline.rows.get(ordinal).ok_or(Refused)?;
            let mapped = map.spans.rows.get(expected.span).ok_or(Refused)?;
            let [Some(segment), None] = mapped.segments else {
                return Err(Refused);
            };
            if mapped.instance != expected.instance
                || mapped.source != InstanceSpanSourceV1::Terminator(expected.source)
                || mapped.removed_call.is_some()
                || map.owner != Some(expected.source.correspondence_owner)
            {
                return Err(Refused);
            }
            let end = segment
                .first
                .checked_add(segment.count)
                .ok_or_else(call_splice_arithmetic_v1)?;
            budget.charge_work(call_splice_search_work_v1(actual.len()))?;
            let first = actual.partition_point(|row| row.location < (segment.block, segment.first));
            let row = actual.get_mut(first).ok_or(Refused)?;
            if row.seen || row.location.0 != segment.block || row.location.1 >= end {
                return Err(Refused);
            }
            let mut inputs = [(ValueId(0), None); 2];
            let mut used = 0usize;
            row.operation.kind.try_visit_operands(
                |value| -> Result<(), CallInstanceEmissionErrorV1> {
                    budget.charge_work(2)?;
                    let slot = inputs.get_mut(used).ok_or(Refused)?;
                    *slot = (value, index.value(value, budget)?.as_scalar());
                    used += 1;
                    Ok(())
                },
            )?;
            if used != expected.instruction.input_count() {
                return Err(Refused);
            }
            budget.charge_work(192)?;
            gfx942_inline_scalar_correspondence_v30::checked_physical_instruction(
                row.operation,
                expected.identity,
                expected.instruction,
                |value| {
                    inputs[..used]
                        .iter()
                        .find(|row| row.0 == value)
                        .and_then(|row| row.1)
                },
            )
            .ok_or(Refused)?;
            row.seen = true;
            budget.charge_work(1)?;
            if actual
                .get(first + 1)
                .is_some_and(|row| row.location.0 == segment.block && row.location.1 < end)
            {
                return Err(Refused);
            }
            last = Some(ordinal);
            next = expected.next;
        }
        budget.charge_work(count)?;
        if next.is_some() || last != chain.last || actual.iter().any(|row| !row.seen) {
            return Err(Refused);
        }
        // The enclosing splice owns and releases temporary census credit on
        // both success and failure, as for its other borrowed indices.
        Ok(ScopedInlineCalleeV30 {
            function,
            ledger: self.ledger,
        })
    }
}
