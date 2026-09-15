//! Necessary local-coordinate footprint only. A hit always runs the full audit.
//! Include dynamic Index locals, matching the common reference invalidator.
use super::*;

pub(super) struct Relevance {
    words: Vec<u64>,
}

impl Relevance {
    #[cfg(test)]
    pub(super) fn capacity_words(&self) -> usize {
        self.words.capacity()
    }

    pub(super) fn new(
        locals: usize,
        tracked: impl Iterator<Item = u32>,
        budget: &mut Budget,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        budget.charge(3)?;
        let mut words = Vec::new();
        for _ in 0..locals.div_ceil(64) {
            push(&mut words, 0, budget)?;
        }
        for local in tracked {
            budget.charge(2)?;
            if local as usize >= locals {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            words[local as usize / 64] |= 1u64 << (local % 64);
        }
        Ok(Self { words })
    }

    pub(super) fn statement(
        &self,
        source: &SemanticStatementKindV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        result(
            Probe {
                words: &self.words,
                budget,
            }
            .statement(source),
        )
    }

    pub(super) fn terminator(
        &self,
        source: &SemanticTerminatorKindV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        result(
            Probe {
                words: &self.words,
                budget,
            }
            .terminator(source),
        )
    }
}

enum Stop {
    Hit,
    Work(ProductionSemanticSsaErrorV1),
}

fn result(value: Result<(), Stop>) -> Result<bool, ProductionSemanticSsaErrorV1> {
    match value {
        Ok(()) => Ok(false),
        Err(Stop::Hit) => Ok(true),
        Err(Stop::Work(error)) => Err(error),
    }
}

struct Probe<'a, 'b> {
    words: &'a [u64],
    budget: &'b mut Budget,
}

impl Probe<'_, '_> {
    fn work(&mut self) -> Result<(), Stop> {
        self.budget.charge(1).map_err(Stop::Work)
    }

    fn local(&mut self, local: SemanticLocalIdV1) -> Result<(), Stop> {
        self.work()?;
        if self
            .words
            .get(local.index() as usize / 64)
            .is_some_and(|word| word & (1u64 << (local.index() % 64)) != 0)
        {
            Err(Stop::Hit)
        } else {
            Ok(())
        }
    }

    fn place(&mut self, place: &SemanticPlaceV1) -> Result<(), Stop> {
        self.local(place.local())?;
        for projection in place.projections() {
            self.work()?;
            if let SemanticProjectionKindV1::Index(local) = projection.kind() {
                self.local(local)?;
            }
        }
        Ok(())
    }

    fn operand(&mut self, operand: &SemanticOperandV1) -> Result<(), Stop> {
        self.work()?;
        match operand {
            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => self.place(p),
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }

    fn rvalue(&mut self, value: &SemanticRvalueKindV1) -> Result<(), Stop> {
        self.work()?;
        value.try_visit_operands(|operand| self.operand(operand))?;
        match value {
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. }
            | SemanticRvalueKindV1::Length(place)
            | SemanticRvalueKindV1::Discriminant(place) => self.place(place),
            SemanticRvalueKindV1::Load(load) => self.place(load.source()),
            SemanticRvalueKindV1::Use(_)
            | SemanticRvalueKindV1::Unary { .. }
            | SemanticRvalueKindV1::Binary { .. }
            | SemanticRvalueKindV1::CheckedBinary(_)
            | SemanticRvalueKindV1::UncheckedBinary(_)
            | SemanticRvalueKindV1::Cast { .. }
            | SemanticRvalueKindV1::Aggregate(_) => Ok(()),
        }
    }

    fn statement(&mut self, source: &SemanticStatementKindV1) -> Result<(), Stop> {
        self.work()?;
        match source {
            SemanticStatementKindV1::Assign(a) => {
                self.place(a.destination())?;
                self.rvalue(a.value().kind())
            }
            SemanticStatementKindV1::Store(store) => {
                self.place(store.destination())?;
                self.operand(store.value())
            }
            SemanticStatementKindV1::AtomicRmw(op) => {
                self.place(op.destination())?;
                self.place(op.address())?;
                self.operand(op.value())
            }
            SemanticStatementKindV1::AtomicCompareExchange(op) => {
                self.place(op.destination())?;
                self.place(op.address())?;
                self.operand(op.expected())?;
                self.operand(op.replacement())
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => self.place(place),
            SemanticStatementKindV1::Assume(operand) => self.operand(operand),
            // The existing escape audit ignores these. Original planner Kill
            // events and the lowerer's owner/loan lifetime checks still apply.
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => Ok(()),
        }
    }

    fn terminator(&mut self, source: &SemanticTerminatorKindV1) -> Result<(), Stop> {
        self.work()?;
        match source {
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(destination) = call.destination() {
                    self.place(destination.place())?;
                }
                for operand in call.arguments() {
                    self.operand(operand)?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    self.operand(operand)?;
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                self.operand(discriminant)?
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.place(place)?,
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.operand(condition)?;
                self.message(message)?;
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
        Ok(())
    }

    fn message(&mut self, message: &SemanticAssertMessageV1) -> Result<(), Stop> {
        self.work()?;
        match message {
            SemanticAssertMessageV1::BoundsCheck { length, index } => {
                self.operand(length)?;
                self.operand(index)
            }
            SemanticAssertMessageV1::Overflow { left, right, .. } => {
                self.operand(left)?;
                self.operand(right)
            }
            SemanticAssertMessageV1::DivisionByZero(op)
            | SemanticAssertMessageV1::RemainderByZero(op) => self.operand(op),
            SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment,
                found_alignment,
            } => {
                self.operand(required_alignment)?;
                self.operand(found_alignment)
            }
            SemanticAssertMessageV1::NullPointerDereference
            | SemanticAssertMessageV1::ResumedAfterReturn
            | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
        }
    }
}
