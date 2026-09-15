// Included by the CSR child. Only full-block states enter the existing queue;
// the initial statement prefix is a distinct state and may be revisited in full.

enum BindingPrefixV1 {
    Rejected,
    Found,
    Continue,
}

impl CsrQueryV1<'_, '_> {
    fn binding_prefix(
        &mut self,
        local: usize,
        ty: SemanticTypeIdV1,
        expected: Option<(usize, usize)>,
        block_index: usize,
        before: usize,
    ) -> Result<BindingPrefixV1, Error> {
        self.work.charge(1)?;
        if self.reachable[block_index] == 0 {
            return Ok(BindingPrefixV1::Rejected);
        }
        let block = &self.source.blocks()[block_index];
        for statement_index in (0..before).rev() {
            self.work.charge(1)?;
            match block.statements()[statement_index].kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if !assignment.destination().projections().is_empty() {
                        return Ok(BindingPrefixV1::Rejected);
                    }
                    if assignment.destination().local().index() as usize == local {
                        return Ok(
                            if expected == Some((block_index, statement_index))
                                && assignment.destination().ty() == ty
                                && assignment.value().result_type() == ty
                            {
                                BindingPrefixV1::Found
                            } else {
                                BindingPrefixV1::Rejected
                            },
                        );
                    }
                    let mut moved = false;
                    assignment.value().kind().try_visit_operands(|operand| {
                        self.work.charge(1)?;
                        moved |= matches!(operand, SemanticOperandV1::Move(place)
                            if place.local().index() as usize == local);
                        Ok::<_, Error>(())
                    })?;
                    if moved {
                        return Ok(BindingPrefixV1::Rejected);
                    }
                }
                SemanticStatementKindV1::Store(_)
                | SemanticStatementKindV1::AtomicRmw(_)
                | SemanticStatementKindV1::AtomicCompareExchange(_) => {
                    return Ok(BindingPrefixV1::Rejected);
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    if !place.projections().is_empty() || place.local().index() as usize == local {
                        return Ok(BindingPrefixV1::Rejected);
                    }
                }
                SemanticStatementKindV1::StorageLive(id)
                | SemanticStatementKindV1::StorageDead(id) => {
                    if id.index() as usize == local {
                        return Ok(BindingPrefixV1::Rejected);
                    }
                }
                SemanticStatementKindV1::Assume(SemanticOperandV1::Move(place))
                    if place.local().index() as usize == local =>
                {
                    return Ok(BindingPrefixV1::Rejected);
                }
                SemanticStatementKindV1::Assume(_) | SemanticStatementKindV1::Nop => {}
            }
        }
        Ok(BindingPrefixV1::Continue)
    }

    pub(super) fn exact_binding_reaches(
        &mut self,
        local: usize,
        ty: SemanticTypeIdV1,
        expected: Option<(usize, usize)>,
        use_site: (usize, usize),
    ) -> Result<bool, Error> {
        self.work.charge(1)?;
        if local >= self.source.locals().len()
            || self
                .source
                .blocks()
                .get(use_site.0)
                .is_none_or(|block| use_site.1 > block.statements().len())
        {
            return Ok(false);
        }
        let generation = self.workspace.begin(&mut self.work)?;
        let mut current = Some(use_site);
        if use_site.1 == self.source.blocks()[use_site.0].statements().len() {
            self.workspace.seen[use_site.0] = generation;
        }
        let mut found = false;
        while let Some((block_index, before)) = current.take().or_else(|| {
            self.workspace
                .pending
                .pop()
                .map(|block| (block, self.source.blocks()[block].statements().len()))
        }) {
            match self.binding_prefix(local, ty, expected, block_index, before)? {
                BindingPrefixV1::Rejected => return Ok(false),
                BindingPrefixV1::Found => {
                    found = true;
                    continue;
                }
                BindingPrefixV1::Continue => {}
            }
            if block_index == self.source.entry().index() as usize {
                if expected.is_some() {
                    return Ok(false);
                }
                found = true;
            }
            let start = self.predecessor_offsets[block_index];
            let end = self.predecessor_offsets[block_index + 1];
            for index in start..end {
                self.work.charge(1)?;
                let predecessor = self.predecessors[index];
                if self.reachable[predecessor] == 0 {
                    continue;
                }
                // A terminator kill still rejects even if this full-block state
                // was already enqueued. Do not turn visited-state reuse into a waiver.
                let killed = match self.source.blocks()[predecessor].terminator().kind() {
                    SemanticTerminatorKindV1::Call(_)
                    | SemanticTerminatorKindV1::TailCall(_)
                    | SemanticTerminatorKindV1::Drop { .. } => true,
                    SemanticTerminatorKindV1::SwitchInt { discriminant, .. }
                    | SemanticTerminatorKindV1::Assert {
                        condition: discriminant,
                        ..
                    } => {
                        matches!(discriminant, SemanticOperandV1::Move(place)
                            if place.local().index() as usize == local)
                    }
                    _ => false,
                };
                if killed {
                    return Ok(false);
                }
                if self.workspace.seen[predecessor] != generation {
                    self.work.charge(1)?;
                    self.workspace.seen[predecessor] = generation;
                    self.workspace.pending.push(predecessor);
                }
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
impl LosslessCsrV1<'_> {
    pub(super) fn binding_allocation_identity(&self) -> [(*const usize, usize, usize); 8] {
        [
            &self.successor_offsets,
            &self.successors,
            &self.predecessor_offsets,
            &self.predecessors,
            &self.reachable,
            &self.workspace.seen,
            &self.workspace.region,
            &self.workspace.pending,
        ]
        .map(|values| (values.as_ptr(), values.len(), values.capacity()))
    }
}
