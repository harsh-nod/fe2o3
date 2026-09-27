//! Conservative lexical holder deaths, independent of the SSA being planned.
use super::*;

#[derive(Clone, Copy)]
struct Last {
    block: usize,
    statement: usize,
    pinned: bool,
}

pub(super) struct Schedule {
    locals: Vec<Last>,
    // Kahn's remaining indegrees conservatively pin cycles and their tails.
    indegrees: Vec<usize>,
}

pub(super) fn scratch_words(function: &SemanticFunctionDeclV1) -> Result<usize> {
    // Fixed walker/closure/Vec envelopes; Last is three logical words.
    64usize
        .checked_add(
            function
                .locals()
                .len()
                .checked_mul(3)
                .ok_or(Error::ResourceOverflow)?,
        )
        .and_then(|words| {
            function
                .blocks()
                .len()
                .checked_mul(2)
                .and_then(|blocks| words.checked_add(blocks))
        })
        .ok_or(Error::ResourceOverflow)
}

trait Visitor {
    fn work(&mut self, units: usize) -> Result<()>;
    fn local(&mut self, local: SemanticLocalIdV1) -> Result<()>;
}

fn place(visitor: &mut impl Visitor, place: &SemanticPlaceV1) -> Result<()> {
    visitor.local(place.local())?;
    for projection in place.projections() {
        visitor.work(1)?;
        if let SemanticProjectionKindV1::Index(local) = projection.kind() {
            visitor.local(local)?;
        }
    }
    Ok(())
}

fn operand(visitor: &mut impl Visitor, operand: &SemanticOperandV1) -> Result<()> {
    visitor.work(1)?;
    match operand {
        SemanticOperandV1::Copy(value) | SemanticOperandV1::Move(value) => place(visitor, value),
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

fn statement(visitor: &mut impl Visitor, statement: &SemanticStatementKindV1) -> Result<()> {
    visitor.work(1)?;
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            place(visitor, assignment.destination())?;
            assignment
                .value()
                .kind()
                .try_visit_operands(|value| operand(visitor, value))?;
            match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { place: value, .. }
                | SemanticRvalueKindV1::AddressOf { place: value, .. }
                | SemanticRvalueKindV1::Length(value)
                | SemanticRvalueKindV1::Discriminant(value) => place(visitor, value),
                SemanticRvalueKindV1::Load(load) => place(visitor, load.source()),
                SemanticRvalueKindV1::Use(_)
                | SemanticRvalueKindV1::Unary { .. }
                | SemanticRvalueKindV1::Binary { .. }
                | SemanticRvalueKindV1::CheckedBinary(_)
                | SemanticRvalueKindV1::UncheckedBinary(_)
                | SemanticRvalueKindV1::Cast { .. }
                | SemanticRvalueKindV1::Aggregate(_) => Ok(()),
            }
        }
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => visitor.local(*local),
        SemanticStatementKindV1::Deinitialize(value)
        | SemanticStatementKindV1::SetDiscriminant { place: value, .. } => place(visitor, value),
        SemanticStatementKindV1::Store(store) => {
            place(visitor, store.destination())?;
            operand(visitor, store.value())
        }
        SemanticStatementKindV1::AtomicRmw(value) => {
            place(visitor, value.destination())?;
            place(visitor, value.address())?;
            operand(visitor, value.value())
        }
        SemanticStatementKindV1::AtomicCompareExchange(value) => {
            place(visitor, value.destination())?;
            place(visitor, value.address())?;
            operand(visitor, value.expected())?;
            operand(visitor, value.replacement())
        }
        SemanticStatementKindV1::Assume(value) => operand(visitor, value),
        SemanticStatementKindV1::Nop => Ok(()),
    }
}

fn terminator(visitor: &mut impl Visitor, terminator: &SemanticTerminatorKindV1) -> Result<()> {
    visitor.work(1)?;
    match terminator {
        SemanticTerminatorKindV1::Call(call) => {
            for value in call.arguments() {
                operand(visitor, value)?;
            }
            if let Some(destination) = call.destination() {
                place(visitor, destination.place())?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for value in call.arguments() {
                operand(visitor, value)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => operand(visitor, discriminant),
        SemanticTerminatorKindV1::Drop { place: value, .. } => place(visitor, value),
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            operand(visitor, condition)?;
            partial_moves::visit_assert_operands_v1(message, &mut |value| operand(visitor, value))
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(()),
    }
}

struct Census<'a, M> {
    locals: &'a mut [Last],
    meter: &'a mut M,
    block: usize,
    statement: usize,
}
impl<M: BorrowWork> Visitor for Census<'_, M> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.meter.work(units)
    }
    fn local(&mut self, local: SemanticLocalIdV1) -> Result<()> {
        self.meter.work(4)?;
        let row = self
            .locals
            .get_mut(local.index() as usize)
            .ok_or(Error::ReplayMismatch)?;
        if row.block != usize::MAX && row.block != self.block {
            row.pinned = true;
        }
        row.block = self.block;
        row.statement = self.statement;
        row.pinned |= self.statement == usize::MAX;
        Ok(())
    }
}

impl Schedule {
    pub(super) fn new(
        function: &SemanticFunctionDeclV1,
        meter: &mut impl BorrowWork,
    ) -> Result<Self> {
        meter.work(
            function
                .locals()
                .len()
                .checked_add(
                    function
                        .blocks()
                        .len()
                        .checked_mul(2)
                        .ok_or(Error::ResourceOverflow)?,
                )
                .ok_or(Error::ResourceOverflow)?,
        )?;
        let locals = function
            .locals()
            .iter()
            .map(|local| Last {
                block: usize::MAX,
                statement: usize::MAX,
                pinned: local.role() == SemanticLocalRoleV1::Return,
            })
            .collect();
        let mut result = Self {
            locals,
            indegrees: vec![0usize; function.blocks().len()],
        };
        let mut ready = Vec::with_capacity(function.blocks().len());
        for (block, body) in function.blocks().iter().enumerate() {
            meter.work(1)?;
            let mut census = Census {
                locals: &mut result.locals,
                meter,
                block,
                statement: 0,
            };
            for (index, row) in body.statements().iter().enumerate() {
                census.statement = index;
                statement(&mut census, row.kind())?;
            }
            census.statement = usize::MAX;
            terminator(&mut census, body.terminator().kind())?;
            body.terminator().kind().try_for_each_edge(|edge| {
                meter.work(3)?;
                let degree = result
                    .indegrees
                    .get_mut(edge.target().index() as usize)
                    .ok_or(Error::ReplayMismatch)?;
                *degree = degree.checked_add(1).ok_or(Error::ResourceOverflow)?;
                Ok::<(), Error>(())
            })?;
        }
        for (block, degree) in result.indegrees.iter().enumerate() {
            meter.work(1)?;
            if *degree == 0 {
                ready.push(block);
            }
        }
        while let Some(block) = ready.pop() {
            meter.work(1)?;
            function.blocks()[block]
                .terminator()
                .kind()
                .try_for_each_edge(|edge| {
                    meter.work(3)?;
                    let target = edge.target().index() as usize;
                    let degree = result
                        .indegrees
                        .get_mut(target)
                        .ok_or(Error::ReplayMismatch)?;
                    *degree = degree.checked_sub(1).ok_or(Error::ReplayMismatch)?;
                    if *degree == 0 {
                        ready.push(target);
                    }
                    Ok::<(), Error>(())
                })?;
        }
        Ok(result)
    }

    pub(super) fn completed<M: BorrowWork, O: ReadObserver>(
        &self,
        analysis: &mut Analysis<'_, '_, M, O>,
        site: SemanticTransparentBorrowSiteV1,
        source: &SemanticStatementKindV1,
    ) -> Result<()> {
        analysis.meter.work(1)?;
        if self.indegrees.get(site.block as usize) != Some(&0) {
            return Ok(());
        }
        statement(
            &mut Expire {
                schedule: self,
                analysis,
                site,
            },
            source,
        )
    }
}

struct Expire<'a, 'f, 'm, M: BorrowWork, O: ReadObserver> {
    schedule: &'a Schedule,
    analysis: &'a mut Analysis<'f, 'm, M, O>,
    site: SemanticTransparentBorrowSiteV1,
}
impl<M: BorrowWork, O: ReadObserver> Visitor for Expire<'_, '_, '_, M, O> {
    fn work(&mut self, units: usize) -> Result<()> {
        self.analysis.meter.work(units)
    }
    fn local(&mut self, local: SemanticLocalIdV1) -> Result<()> {
        self.analysis.meter.work(4)?;
        let row = self
            .schedule
            .locals
            .get(local.index() as usize)
            .ok_or(Error::ReplayMismatch)?;
        if row.pinned
            || row.block != self.site.block as usize
            || row.statement != self.site.statement as usize
        {
            return Ok(());
        }
        self.analysis.tree()?;
        if let Some(aliases) = self.analysis.holders.remove(&local.index()) {
            // End only logical holders, never source storage or referent values.
            self.analysis.discard(aliases, false)?;
        }
        Ok(())
    }
}
