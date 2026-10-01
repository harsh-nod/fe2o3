//! Backward demands for independently modeled original value components.
//! This retains the source CFG and SSA reachability; it is not another SSA plan.

use super::{Error, Resource, Result, SourceSlots, Writer, add, vector};
use fe2o3_mir_model::{
    SsaBlockIdV1,
    semantic_mir_v1::{
        SemanticAssertMessageV1 as AssertMessage, SemanticEdgeRoleV1 as EdgeRole,
        SemanticFunctionDeclV1 as Function, SemanticFunctionIdV1 as FunctionId,
        SemanticLocalRoleV1 as LocalRole, SemanticOperandV1 as Operand, SemanticPlaceV1 as Place,
        SemanticRvalueKindV1 as Rvalue, SemanticStatementKindV1 as Statement,
        SemanticTerminatorKindV1 as Terminator, SemanticTypeShapeV1 as Shape,
    },
};
use std::{mem::size_of, ops::Range};

pub(super) struct ComponentDemandsV42<'a, 'view, 'source> {
    slots: &'a SourceSlots<'view, 'source>,
    function: FunctionId,
    locals: Vec<Range<usize>>,
    live: Vec<u64>,
    words: usize,
    blocks: usize,
    required: usize,
}

struct Edge {
    source: usize,
    target: usize,
    killed: Range<usize>,
}

struct Facts<'a, 'view, 'source, 'data> {
    slots: &'a SourceSlots<'view, 'source>,
    function: &'data Function,
    locals: &'data [Range<usize>],
    generated: &'data mut [u64],
    killed: &'data mut [u64],
}

fn mismatch() -> Error {
    Error::Statement("original aggregate component demand differs from its source CFG")
}

fn words_for(bits: usize) -> Result<usize> {
    Ok(add(bits, 63)? / 64)
}

fn product(left: usize, right: usize) -> Result<usize> {
    left.checked_mul(right)
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn zeros<T: Copy + Default>(count: usize, out: &mut Writer<'_, '_>) -> Result<Vec<T>> {
    let mut result = vector(count, out)?;
    out.budget.charge_work(count)?;
    result.resize(count, T::default());
    Ok(result)
}

fn word_mask(range: &Range<usize>, word: usize) -> u64 {
    let base = word * 64;
    let first = range.start.saturating_sub(base).min(64);
    let end = range.end.saturating_sub(base).min(64);
    if first >= end {
        0
    } else {
        (u64::MAX << first) & (u64::MAX >> (64 - end))
    }
}

impl<'a, 'view, 'source> ComponentDemandsV42<'a, 'view, 'source> {
    pub(super) fn derive(
        slots: &'a SourceSlots<'view, 'source>,
        function_id: FunctionId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.with_source_query_v42(out, |out| Self::derive_inner(slots, function_id, out))
    }

    fn derive_inner(
        slots: &'a SourceSlots<'view, 'source>,
        function_id: FunctionId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let source = slots.correspondence(out)?.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or_else(mismatch)?;
        let ssa = source
            .source_ssa(out.budget)?
            .plan_for_function(function_id)
            .ok_or_else(mismatch)?
            .plan();
        let blocks = function.blocks().len();
        let mut locals = vector(function.locals().len(), out)?;
        let mut leaves = 0;
        for local in function.locals() {
            out.budget.charge_work(2)?;
            let ty = semantic
                .types()
                .get(local.ty().index() as usize)
                .ok_or_else(mismatch)?;
            let count = if matches!(
                ty.shape(),
                Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. }
            ) {
                slots.aggregate_leaf_count(local.ty(), out)?.unwrap_or(0)
            } else {
                0
            };
            let end = add(leaves, count)?;
            locals.push(leaves..end);
            leaves = end;
        }
        let words = words_for(leaves)?;
        let cells = product(blocks, words)?;
        let mut live = zeros::<u64>(cells, out)?;
        let retained = out.budget.storage();
        let mut generated = zeros::<u64>(cells, out)?;
        let mut killed = zeros::<u64>(cells, out)?;
        let mut failure_killed = zeros::<u64>(words, out)?;
        let mut active = zeros::<bool>(blocks, out)?;
        let mut edge_count = 0;
        for (index, block) in function.blocks().iter().enumerate() {
            out.budget.charge_work(2)?;
            active[index] = ssa
                .live_in(SsaBlockIdV1::new(
                    u32::try_from(index).map_err(|_| Resource::Arithmetic)?,
                ))
                .is_some();
            if active[index] {
                edge_count = add(edge_count, block.terminator().kind().edge_count())?;
            }
        }
        let mut edges = vector(edge_count, out)?;
        let mut outgoing = vector(blocks, out)?;
        let mut predecessor_counts = zeros::<usize>(blocks, out)?;
        for (index, block) in function.blocks().iter().enumerate() {
            out.budget.charge_work(1)?;
            let first = edges.len();
            if active[index] {
                let start = product(index, words)?;
                let mut facts = Facts {
                    slots,
                    function,
                    locals: &locals,
                    generated: &mut generated[start..start + words],
                    killed: &mut killed[start..start + words],
                };
                for statement in block.statements() {
                    facts.statement(statement.kind(), out)?;
                }
                facts.terminator(block.terminator().kind(), &mut failure_killed, out)?;
                block.terminator().kind().try_for_each_edge(|edge| {
                    out.budget.charge_work(4)?;
                    let target = edge.target().index() as usize;
                    if active.get(target) != Some(&true) {
                        return Err(mismatch());
                    }
                    let killed = match block.terminator().kind() {
                        Terminator::Call(call) if edge.role() == EdgeRole::CallReturn => match call
                            .destination()
                        {
                            Some(destination) => facts.range(destination.place(), false, out)?,
                            None => return Err(mismatch()),
                        },
                        _ => 0..0,
                    };
                    predecessor_counts[target] = add(predecessor_counts[target], 1)?;
                    edges.push(Edge {
                        source: index,
                        target,
                        killed,
                    });
                    Ok::<_, Error>(())
                })?;
            }
            outgoing.push(first..edges.len());
        }
        if edges.len() != edge_count {
            return Err(mismatch());
        }
        let mut predecessor_offsets = vector(add(blocks, 1)?, out)?;
        predecessor_offsets.push(0);
        for &count in &predecessor_counts {
            out.budget.charge_work(1)?;
            predecessor_offsets.push(add(
                *predecessor_offsets.last().ok_or_else(mismatch)?,
                count,
            )?);
        }
        let mut predecessor_cursor = zeros::<usize>(blocks, out)?;
        let mut predecessors = zeros::<usize>(edge_count, out)?;
        for edge in &edges {
            out.budget.charge_work(3)?;
            let at = add(
                predecessor_offsets[edge.target],
                predecessor_cursor[edge.target],
            )?;
            predecessors[at] = edge.source;
            predecessor_cursor[edge.target] = add(predecessor_cursor[edge.target], 1)?;
        }
        let mut pending = zeros::<bool>(blocks, out)?;
        let mut worklist = vector(blocks, out)?;
        for index in 0..blocks {
            out.budget.charge_work(1)?;
            if active[index] {
                pending[index] = true;
                worklist.push(index);
            }
        }
        while let Some(block) = worklist.pop() {
            out.budget.charge_work(2)?;
            pending[block] = false;
            let first = product(block, words)?;
            let mut changed = false;
            for word in 0..words {
                out.budget.charge_work(3)?;
                let mut next = 0;
                for edge in &edges[outgoing[block].clone()] {
                    out.budget.charge_work(3)?;
                    next |=
                        live[product(edge.target, words)? + word] & !word_mask(&edge.killed, word);
                }
                next = generated[first + word] | (next & !killed[first + word]);
                if next != live[first + word] {
                    if next & live[first + word] != live[first + word] {
                        return Err(mismatch());
                    }
                    live[first + word] = next;
                    changed = true;
                }
            }
            if changed {
                for &parent in
                    &predecessors[predecessor_offsets[block]..predecessor_offsets[block + 1]]
                {
                    out.budget.charge_work(2)?;
                    if !pending[parent] {
                        if worklist.len() == worklist.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        worklist.push(parent);
                        pending[parent] = true;
                    }
                }
            }
        }
        drop((
            generated,
            killed,
            failure_killed,
            active,
            edges,
            outgoing,
            predecessor_counts,
            predecessor_offsets,
            predecessor_cursor,
            predecessors,
            pending,
            worklist,
        ));
        let released = out
            .budget
            .storage()
            .checked_sub(retained)
            .ok_or(Resource::Accounting)?;
        out.budget.release_storage(released)?;
        Ok(Self {
            slots,
            function: function_id,
            locals,
            live,
            words,
            blocks,
            required: retained,
        })
    }

    pub(super) fn leaf_required(
        &self,
        function: FunctionId,
        block: usize,
        local: usize,
        leaf: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        self.slots.with_source_query_v42(out, |out| {
            self.leaf_required_inner(function, block, local, leaf, out)
        })
    }

    fn leaf_required_inner(
        &self,
        function: FunctionId,
        block: usize,
        local: usize,
        leaf: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        if out.budget.storage() < self.required {
            return Err(Resource::Accounting.into());
        }
        out.budget.charge_work(4)?;
        let range = self.locals.get(local).ok_or_else(mismatch)?;
        if function != self.function || block >= self.blocks || leaf >= range.len() {
            return Err(mismatch());
        }
        let bit = add(range.start, leaf)?;
        Ok(self.live[product(block, self.words)? + bit / 64] & (1_u64 << (bit % 64)) != 0)
    }
}

impl Facts<'_, '_, '_, '_> {
    fn range(&self, place: &Place, read: bool, out: &mut Writer<'_, '_>) -> Result<Range<usize>> {
        out.budget.charge_work(3)?;
        let local = place.local().index() as usize;
        let range = self.locals.get(local).ok_or_else(mismatch)?;
        if range.is_empty() {
            return Ok(0..0);
        }
        let ty = self.function.locals().get(local).ok_or_else(mismatch)?.ty();
        match self
            .slots
            .aggregate_component_range(ty, place.projections(), out)?
        {
            Some((selected, result_type)) => {
                if result_type != place.ty() || selected.end > range.len() {
                    return Err(mismatch());
                }
                Ok(add(range.start, selected.start)?..add(range.start, selected.end)?)
            }
            // Dynamic/address projections cannot prove an exact overwrite.
            // Requiring all enclosing components is conservative for reads.
            None if read => Ok(range.clone()),
            None => Ok(0..0),
        }
    }

    fn mark(&mut self, range: Range<usize>, read: bool, out: &mut Writer<'_, '_>) -> Result<()> {
        if range.is_empty() {
            return Ok(());
        }
        for word in range.start / 64..words_for(range.end)? {
            out.budget.charge_work(3)?;
            let mask = word_mask(&range, word);
            if read {
                self.generated[word] |= mask & !self.killed[word];
            } else {
                self.killed[word] |= mask;
            }
        }
        Ok(())
    }

    fn place(&mut self, place: &Place, read: bool, out: &mut Writer<'_, '_>) -> Result<()> {
        let range = self.range(place, read, out)?;
        self.mark(range, read, out)
    }

    fn operand(&mut self, operand: &Operand, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        match operand {
            Operand::Copy(place) => self.place(place, true, out),
            Operand::Move(place) => {
                self.place(place, true, out)?;
                self.place(place, false, out)
            }
            Operand::Constant(_) => Ok(()),
        }
    }

    fn statement(&mut self, statement: &Statement, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        match statement {
            Statement::Assign(assignment) => {
                let value = assignment.value().kind();
                value.try_visit_operands(|operand| self.operand(operand, out))?;
                match value {
                    Rvalue::Borrow { place, .. }
                    | Rvalue::AddressOf { place, .. }
                    | Rvalue::Length(place)
                    | Rvalue::Discriminant(place) => self.place(place, true, out)?,
                    Rvalue::Load(load) => self.place(load.source(), true, out)?,
                    Rvalue::Use(_)
                    | Rvalue::Unary { .. }
                    | Rvalue::Binary { .. }
                    | Rvalue::CheckedBinary(_)
                    | Rvalue::UncheckedBinary(_)
                    | Rvalue::Cast { .. }
                    | Rvalue::Aggregate(_) => (),
                }
                self.place(assignment.destination(), false, out)
            }
            Statement::Store(store) => {
                self.place(store.destination(), true, out)?;
                self.operand(store.value(), out)
            }
            Statement::AtomicRmw(operation) => {
                self.place(operation.address(), true, out)?;
                self.operand(operation.value(), out)?;
                self.place(operation.destination(), false, out)
            }
            Statement::AtomicCompareExchange(operation) => {
                self.place(operation.address(), true, out)?;
                self.operand(operation.expected(), out)?;
                self.operand(operation.replacement(), out)?;
                self.place(operation.destination(), false, out)
            }
            Statement::SetDiscriminant { place, .. } => self.place(place, true, out),
            Statement::Deinitialize(place) => self.place(place, false, out),
            Statement::StorageLive(local) | Statement::StorageDead(local) => self.mark(
                self.locals
                    .get(local.index() as usize)
                    .ok_or_else(mismatch)?
                    .clone(),
                false,
                out,
            ),
            Statement::Assume(operand) => self.operand(operand, out),
            Statement::Nop => Ok(()),
        }
    }

    fn message(&mut self, message: &AssertMessage, out: &mut Writer<'_, '_>) -> Result<()> {
        match message {
            AssertMessage::BoundsCheck {
                length: left,
                index: right,
            }
            | AssertMessage::Overflow { left, right, .. }
            | AssertMessage::MisalignedPointerDereference {
                required_alignment: left,
                found_alignment: right,
            } => {
                self.operand(left, out)?;
                self.operand(right, out)
            }
            AssertMessage::DivisionByZero(operand) | AssertMessage::RemainderByZero(operand) => {
                self.operand(operand, out)
            }
            AssertMessage::NullPointerDereference
            | AssertMessage::ResumedAfterReturn
            | AssertMessage::ResumedAfterPanic => Ok(()),
        }
    }

    fn terminator(
        &mut self,
        terminator: &Terminator,
        failure_killed: &mut [u64],
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        match terminator {
            Terminator::SwitchInt { discriminant, .. } => self.operand(discriminant, out),
            Terminator::Call(call) => {
                for operand in call.arguments() {
                    self.operand(operand, out)?;
                }
                Ok(())
            }
            Terminator::TailCall(call) => {
                for operand in call.arguments() {
                    self.operand(operand, out)?;
                }
                Ok(())
            }
            Terminator::Assert {
                condition, message, ..
            } => {
                self.operand(condition, out)?;
                out.budget.charge_work(product(self.killed.len(), 2)?)?;
                failure_killed.copy_from_slice(self.killed);
                self.message(message, out)?;
                // Failure-only Moves contribute demand but never kill a value
                // transported to the successful continuation.
                self.killed.copy_from_slice(failure_killed);
                Ok(())
            }
            Terminator::Drop { place, .. } => {
                self.place(place, true, out)?;
                self.place(place, false, out)
            }
            Terminator::Return => {
                for (local, declaration) in self.function.locals().iter().enumerate() {
                    out.budget.charge_work(1)?;
                    if declaration.role() == LocalRole::Return {
                        self.mark(self.locals[local].clone(), true, out)?;
                    }
                }
                Ok(())
            }
            Terminator::Goto(_)
            | Terminator::FalseEdge { .. }
            | Terminator::UnwindResume
            | Terminator::UnwindTerminate
            | Terminator::Abort
            | Terminator::Unreachable => Ok(()),
        }
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<ComponentDemandsV42<'_, '_, '_>>()
        + h::<Facts<'_, '_, '_, '_>>()
        + h::<Edge>()
        + h::<Range<usize>>()
        + 5 * h::<Vec<u64>>()
        + 6 * h::<Vec<usize>>()
        + 2 * h::<Vec<bool>>()
        + 2 * h::<Vec<Range<usize>>>()
        + h::<Vec<Edge>>()
        + 32 * size_of::<usize>()
        + 16 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_component_demand_word_masks_preserve_exact_prefix_boundaries() {
        for range in [0..0, 0..1, 0..64, 1..64, 63..65, 64..128, 65..127, 127..129] {
            for word in 0..3 {
                let mut expected = 0_u64;
                for bit in 0..64 {
                    if range.contains(&(word * 64 + bit)) {
                        expected |= 1_u64 << bit;
                    }
                }
                assert_eq!(word_mask(&range, word), expected);
            }
        }
    }

    #[test]
    fn original_component_demand_header_census_is_independent_of_retained_struct_layout() {
        type Retained<'a, 'v, 's> = (
            &'a SourceSlots<'v, 's>,
            FunctionId,
            Vec<Range<usize>>,
            Vec<u64>,
            usize,
            usize,
            usize,
        );
        type EdgeFields = (usize, usize, Range<usize>);
        type FactFields<'a, 'v, 's, 'd> = (
            &'a SourceSlots<'v, 's>,
            &'d Function,
            &'d [Range<usize>],
            &'d mut [u64],
            &'d mut [u64],
        );
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(
            size_of::<ComponentDemandsV42<'_, '_, '_>>(),
            size_of::<Retained<'_, '_, '_>>()
        );
        assert_eq!(size_of::<Edge>(), size_of::<EdgeFields>());
        assert_eq!(
            size_of::<Facts<'_, '_, '_, '_>>(),
            size_of::<FactFields<'_, '_, '_, '_>>()
        );
        assert_eq!(
            headers(),
            h::<Retained<'_, '_, '_>>()
                + h::<FactFields<'_, '_, '_, '_>>()
                + h::<EdgeFields>()
                + h::<Range<usize>>()
                + 5 * h::<Vec<u64>>()
                + 6 * h::<Vec<usize>>()
                + 2 * h::<Vec<bool>>()
                + 2 * h::<Vec<Range<usize>>>()
                + h::<Vec<EdgeFields>>()
                + 32 * size_of::<usize>()
                + 16 * size_of::<&()>()
        );
    }
}
