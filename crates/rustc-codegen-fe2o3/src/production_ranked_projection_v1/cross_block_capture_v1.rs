//! Narrow precision for an unchanged scalar captured before a split guard.
//! The caller still proves the numeric bound and the guard-to-use segment.

use super::*;

const MAX_CAPTURE_SEGMENT_BLOCKS: usize = 8;

impl SemanticAssertProofsV1<'_> {
    pub(super) fn exact_cross_block_guard_capture_v1(
        &mut self,
        captured: usize,
        source: usize,
        comparison: ScalarAssignmentSiteV1,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        self.charge(1)?;
        let Some(source_decl) = self.function.locals().get(source) else {
            return Ok(false);
        };
        let ty = source_decl.ty();
        if captured == source
            || self.unsigned_integer_bits(ty).is_none()
            || self
                .function
                .locals()
                .get(captured)
                .is_none_or(|l| l.ty() != ty)
            || self.definition_counts.get(captured).copied() != Some(1)
            || self.address_escaped.get(source).copied() != Some(false)
            || self.address_escaped.get(captured).copied() != Some(false)
        {
            return Ok(false);
        }
        let Some(capture) = self.assignments.get(captured).copied().flatten() else {
            return Ok(false);
        };
        if capture.block == comparison.block {
            return Ok(false);
        }
        let Some(comparison_block) = self.function.blocks().get(comparison.block) else {
            return Ok(false);
        };
        let Some(statement) = comparison_block.statements().get(comparison.statement) else {
            return Ok(false);
        };
        let SemanticStatementKindV1::Assign(condition) = statement.kind() else {
            return Ok(false);
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        } = condition.value().kind()
        else {
            return Ok(false);
        };
        if left.ty() != ty
            || right.ty() != ty
            || simple_operand_local(left).map(|l| l.index() as usize) != Some(captured)
            || matches!(right, SemanticOperandV1::Move(p) if p.local().index() as usize == source || p.local().index() as usize == captured)
            || condition.destination().ty() != condition.value().result_type()
            || !matches!(
                self.types
                    .get(condition.value().result_type().index() as usize)
                    .map(|t| t.shape()),
                Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
            )
        {
            return Ok(false);
        }
        let Some(capture_block) = self.function.blocks().get(capture.block) else {
            return Ok(false);
        };
        let Some(statement) = capture_block.statements().get(capture.statement) else {
            return Ok(false);
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(false);
        };
        // Moving the counter itself would not leave a live value for its
        // later increment. A Move of the captured alias in the comparison is
        // allowed, because comparison is the end of this checked segment.
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
        else {
            return Ok(false);
        };
        if !assignment.destination().projections().is_empty()
            || assignment.destination().local().index() as usize != captured
            || assignment.destination().ty() != ty
            || assignment.value().result_type() != ty
            || !place.projections().is_empty()
            || place.local().index() as usize != source
            || place.ty() != ty
            || !self.assignment_dominates_use(capture, comparison.block, comparison.statement)?
        {
            return Ok(false);
        }

        // Every dynamic comparison must pass the capture again. A unique
        // predecessor chain rejects joins and backedges which reuse a stale
        // captured value, rather than relying on static dominance alone.
        let mut segment = [usize::MAX; MAX_CAPTURE_SEGMENT_BLOCKS];
        let mut length = 0;
        let mut current = comparison.block;
        loop {
            self.charge(1)?;
            if length == segment.len()
                || segment[..length].contains(&current)
                || !self.graph.is_entry_reachable(current)
            {
                return Ok(false);
            }
            segment[length] = current;
            length += 1;
            let block = &self.function.blocks()[current];
            let start = if current == capture.block {
                capture.statement + 1
            } else {
                0
            };
            let end = if current == comparison.block {
                comparison.statement
            } else {
                block.statements().len()
            };
            if start > end {
                return Ok(false);
            }
            for statement in &block.statements()[start..end] {
                self.charge(1)?;
                let mut invalid = false;
                visit_statement_definition_places(statement.kind(), &mut |place| {
                    invalid |= matches!(local_definition_index(place), Some(l) if l == source || l == captured);
                });
                if invalid {
                    return Ok(false);
                }
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Borrow { .. }
                            | SemanticRvalueKindV1::AddressOf { .. }
                            | SemanticRvalueKindV1::Load(_) => return Ok(false),
                            _ => {}
                        }
                        assignment.value().kind().try_visit_operands(|operand| {
                            self.charge(1)?;
                            invalid |= matches!(operand, SemanticOperandV1::Move(p) if p.local().index() as usize == source || p.local().index() as usize == captured);
                            Ok::<_, ProductionRankedProjectionErrorV1>(())
                        })?;
                        if invalid {
                            return Ok(false);
                        }
                    }
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local)
                        if local.index() as usize != source
                            && local.index() as usize != captured => {}
                    SemanticStatementKindV1::Nop => {}
                    _ => return Ok(false),
                }
            }
            if current == capture.block {
                break;
            }
            if current == self.graph.entry() {
                return Ok(false);
            }
            let Some(predecessors) = self.graph.predecessors(current) else {
                return Ok(false);
            };
            let predecessor_count = predecessors.len();
            self.charge(predecessor_count)?;
            let Some(&[predecessor]) = self.graph.predecessors(current) else {
                return Ok(false);
            };
            match self.function.blocks()[predecessor].terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge)
                    if edge.target().index() as usize == current => {}
                SemanticTerminatorKindV1::Assert {
                    condition,
                    target,
                    unwind: SemanticUnwindActionV1::Unreachable,
                    ..
                } if target.target().index() as usize == current
                    && !matches!(condition, SemanticOperandV1::Move(p) if p.local().index() as usize == source || p.local().index() as usize == captured) =>
                    {}
                _ => return Ok(false),
            }
            current = predecessor;
        }

        // The existing graph is the normal-edge graph. Check the small
        // segment's incoming edges against complete source terminators too,
        // so an omitted cleanup or duplicate edge cannot supply another entry.
        let mut incoming = [0_u8; MAX_CAPTURE_SEGMENT_BLOCKS];
        for (from, block) in self.function.blocks().iter().enumerate() {
            self.charge(1)?;
            let mut invalid = false;
            block.terminator().kind().try_for_each_edge(|edge| {
                self.charge(length)?;
                if let Some(index) = segment[..length - 1]
                    .iter()
                    .position(|&b| b == edge.target().index() as usize)
                {
                    incoming[index] = incoming[index].saturating_add(1);
                    invalid |= from != segment[index + 1] || incoming[index] != 1;
                }
                Ok::<_, ProductionRankedProjectionErrorV1>(())
            })?;
            if invalid {
                return Ok(false);
            }
        }
        Ok(incoming[..length - 1].iter().all(|&n| n == 1))
    }
}

#[cfg(test)]
#[path = "cross_block_capture_v1/tests.rs"]
mod tests;
