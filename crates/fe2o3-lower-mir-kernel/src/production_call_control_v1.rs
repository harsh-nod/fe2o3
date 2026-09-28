// Original-source reachability, before emission. A missing normal return is a
// control fact, never a value, an unwind proof, or evidence that a loop terminates.
struct ProductionControlBlockV1 {
    reachable: bool,
    call: Option<usize>,
    next: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionCallControlV1 {
    Unreachable,
    MayReturn,
    NoNormalReturn,
}

impl ProductionCallInstancePlanV1<'_> {
    pub(crate) fn instance_reachable(&self, instance: ProductionCallInstanceIdV1) -> Option<bool> {
        Some(self.instance(instance)?.reachable)
    }

    // This is conservative may-return, not proof that every execution returns.
    pub(crate) fn instance_may_return(&self, instance: ProductionCallInstanceIdV1) -> Option<bool> {
        Some(self.instance(instance)?.may_return)
    }

    pub(crate) fn block_reachable(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: SemanticBlockIdV1,
    ) -> Option<bool> {
        let row = self.instance(instance)?;
        let index = row
            .control_blocks
            .start
            .checked_add(block.index() as usize)?;
        if !row.control_blocks.contains(&index) {
            return None;
        }
        Some(row.reachable && self.control_blocks.values.get(index)?.reachable)
    }

    pub(crate) fn call_control(
        &self,
        occurrence: ProductionCallOccurrenceV1,
    ) -> Option<ProductionCallControlV1> {
        let row = self.instance(occurrence.caller)?;
        let index = row
            .control_blocks
            .start
            .checked_add(occurrence.block.index() as usize)?;
        if !row.control_blocks.contains(&index) {
            return None;
        }
        let block = self.control_blocks.values.get(index)?;
        let call = self.calls.values.get(block.call?)?;
        if call.occurrence != occurrence {
            return None;
        }
        Some(if !row.reachable || !block.reachable {
            ProductionCallControlV1::Unreachable
        } else if call.source.destination().is_none()
            || call
                .child
                .is_some_and(|child| !self.instances.values[child.index()].may_return)
        {
            ProductionCallControlV1::NoNormalReturn
        } else {
            ProductionCallControlV1::MayReturn
        })
    }
}

fn build_control(
    plan: &mut ProductionCallInstancePlanV1<'_>,
    budget: &mut Budget<'_>,
    storage: &mut usize,
) -> Result<(), Error> {
    for instance in 0..plan.instances.values.len() {
        budget.charge_work(2)?;
        let start = plan.control_blocks.values.len();
        let count = plan.instances.values[instance].declaration.blocks().len();
        for _ in 0..count {
            plan.control_blocks.push(
                ProductionControlBlockV1 {
                    reachable: false,
                    call: None,
                    next: None,
                },
                budget,
                storage,
            )?;
        }
        plan.instances.values[instance].control_blocks = start..plan.control_blocks.values.len();
        for call_index in plan.instances.values[instance].calls.clone() {
            budget.charge_work(4)?;
            let call = &plan.calls.values[call_index];
            let block = call.occurrence.block.index() as usize;
            if call.occurrence.caller.index() != instance || block >= count {
                return Err(Error::Source);
            }
            if let Some(child) = call.child {
                let child_row = plan.instance(child).ok_or(Error::Source)?;
                if child.index() <= instance || child_row.incoming != Some(call_index) {
                    return Err(Error::Source);
                }
            }
            if plan.control_blocks.values[start + block]
                .call
                .replace(call_index)
                .is_some()
            {
                return Err(Error::Source);
            }
        }
    }

    // Every child follows its parent in the admitted nonrecursive instance tree.
    // Thus a reverse pass settles each child's may-return before its caller.
    for instance in (0..plan.instances.values.len()).rev() {
        budget.charge_work(4)?;
        let row = &plan.instances.values[instance];
        let declaration = row.declaration;
        let ssa = row.ssa;
        let range = row.control_blocks.clone();
        let entry = declaration.entry().index() as usize;
        if entry >= range.len() || !ssa.plan().is_reachable(SsaBlockIdV1::new(entry as u32)) {
            return Err(Error::Source);
        }
        let first = range.start + entry;
        plan.control_blocks.values[first].reachable = true;
        let mut pending = Some(first);
        while let Some(index) = pending {
            budget.charge_work(4)?;
            pending = plan.control_blocks.values[index].next.take();
            let block = index - range.start;
            let terminator = declaration.blocks()[block].terminator().kind();
            if matches!(terminator, SemanticTerminatorKindV1::Return) {
                plan.instances.values[instance].may_return = true;
            }
            let no_normal_return = if let SemanticTerminatorKindV1::Call(source) = terminator {
                let call_index = plan.control_blocks.values[index]
                    .call
                    .ok_or(Error::Source)?;
                let call = &plan.calls.values[call_index];
                if !std::ptr::eq(source, call.source) {
                    return Err(Error::Source);
                }
                call.child
                    .is_some_and(|child| !plan.instances.values[child.index()].may_return)
            } else {
                false
            };
            terminator.try_for_each_edge(|edge| {
                budget.charge_work(3)?;
                // Unwinding is not normal return. Its cleanup edge is retained
                // even when the callee has no reachable Return terminator.
                if no_normal_return && edge.role() == SemanticEdgeRoleV1::CallReturn {
                    return Ok(());
                }
                let target = edge.target().index() as usize;
                if target >= range.len()
                    || !ssa.plan().is_reachable(SsaBlockIdV1::new(target as u32))
                {
                    return Err(Error::Source);
                }
                let target = range.start + target;
                let target_row = &mut plan.control_blocks.values[target];
                if !target_row.reachable {
                    target_row.reachable = true;
                    target_row.next = pending;
                    pending = Some(target);
                }
                Ok(())
            })?;
        }
    }

    // Local reachability does not make an uncalled instance live. Propagate
    // actual root reachability in the opposite, parent-before-child order.
    plan.instances
        .values
        .first_mut()
        .ok_or(Error::Source)?
        .reachable = true;
    for instance in 0..plan.instances.values.len() {
        budget.charge_work(2)?;
        let row = &plan.instances.values[instance];
        if !row.reachable {
            continue;
        }
        let start = row.control_blocks.start;
        for call_index in row.calls.clone() {
            budget.charge_work(3)?;
            let call = &plan.calls.values[call_index];
            if let Some(child) = call.child
                && plan.control_blocks.values[start + call.occurrence.block.index() as usize]
                    .reachable
            {
                plan.instances.values[child.index()].reachable = true;
            }
        }
    }
    Ok(())
}
