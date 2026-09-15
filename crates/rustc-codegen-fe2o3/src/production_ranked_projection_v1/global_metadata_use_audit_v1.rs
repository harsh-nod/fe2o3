use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticAssertMessageV1, SemanticAssignmentV1};

type Result<T> = std::result::Result<T, Rejected>;

enum Rejected {
    Observed,
    Resource(ProductionRankedProjectionErrorV1),
}

struct Audit<'a> {
    selected: [SemanticLocalIdV1; 2],
    work: &'a mut usize,
}

impl Audit<'_> {
    fn charge(&mut self, amount: usize) -> Result<()> {
        charge_capability_dataflow_work_v1(self.work, amount).map_err(Rejected::Resource)
    }

    fn place(&mut self, place: &SemanticPlaceV1) -> Result<()> {
        self.charge(place.projections().len() + 1)?;
        if self.selected.contains(&place.local()) || place.projections().iter().any(|projection| matches!(projection.kind(), SemanticProjectionKindV1::Index(local) if self.selected.contains(&local))) { return Err(Rejected::Observed); }
        Ok(())
    }
    fn operand(&mut self, operand: &SemanticOperandV1) -> Result<()> {
        self.charge(1)?;
        match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => self.place(place),
            SemanticOperandV1::Constant(_) => Ok(()),
        }
    }
    fn statement(&mut self, kind: &SemanticStatementKindV1) -> Result<()> {
        self.charge(1)?;
        match kind {
            SemanticStatementKindV1::Assign(assignment) => {
                self.place(assignment.destination())?;
                assignment
                    .value()
                    .kind()
                    .try_visit_operands(|operand| self.operand(operand))?;
                match assignment.value().kind() {
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
            SemanticStatementKindV1::Deinitialize(place)
            | SemanticStatementKindV1::SetDiscriminant { place, .. } => self.place(place),
            SemanticStatementKindV1::Store(store) => {
                self.place(store.destination())?;
                self.operand(store.value())
            }
            SemanticStatementKindV1::AtomicRmw(atomic) => {
                self.place(atomic.destination())?;
                self.place(atomic.address())?;
                self.operand(atomic.value())
            }
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                self.place(atomic.destination())?;
                self.place(atomic.address())?;
                self.operand(atomic.expected())?;
                self.operand(atomic.replacement())
            }
            SemanticStatementKindV1::Assume(operand) => self.operand(operand),
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => Ok(()),
        }
    }
    fn terminator(&mut self, kind: &SemanticTerminatorKindV1) -> Result<()> {
        self.charge(1)?;
        match kind {
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    self.operand(operand)?;
                }
                if let Some(destination) = call.destination() {
                    self.place(destination.place())?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    self.operand(operand)?;
                }
                Ok(())
            }
            SemanticTerminatorKindV1::Drop { place, .. } => self.place(place),
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => self.operand(discriminant),
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                self.operand(condition)?;
                match message {
                    SemanticAssertMessageV1::BoundsCheck {
                        length: left,
                        index: right,
                    }
                    | SemanticAssertMessageV1::Overflow { left, right, .. }
                    | SemanticAssertMessageV1::MisalignedPointerDereference {
                        required_alignment: left,
                        found_alignment: right,
                    } => {
                        self.operand(left)?;
                        self.operand(right)
                    }
                    SemanticAssertMessageV1::DivisionByZero(operand)
                    | SemanticAssertMessageV1::RemainderByZero(operand) => self.operand(operand),
                    SemanticAssertMessageV1::NullPointerDereference
                    | SemanticAssertMessageV1::ResumedAfterReturn
                    | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
                }
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
}

pub(super) fn closed(
    function: &SemanticFunctionDeclV1,
    allowed: [&SemanticAssignmentV1; 3],
    selected: [SemanticLocalIdV1; 2],
    work: &mut usize,
) -> std::result::Result<bool, ProductionRankedProjectionErrorV1> {
    let mut audit = Audit { selected, work };
    let mut found = [false; 3];
    let checked = (|| {
        for data in function.blocks() {
            audit.charge(1)?;
            for value in data.statements() {
                audit.charge(1)?;
                if let SemanticStatementKindV1::Assign(assignment) = value.kind() {
                    if let Some(index) = allowed
                        .iter()
                        .position(|expected| std::ptr::eq(*expected, assignment))
                    {
                        if std::mem::replace(&mut found[index], true) {
                            return Err(Rejected::Observed);
                        }
                        continue;
                    }
                }
                audit.statement(value.kind())?;
            }
            audit.terminator(data.terminator().kind())?;
        }
        if !found.into_iter().all(|found| found) {
            return Err(Rejected::Observed);
        }
        Ok(())
    })();
    match checked {
        Ok(()) => Ok(true),
        Err(Rejected::Observed) => Ok(false),
        Err(Rejected::Resource(error)) => Err(error),
    }
}

#[cfg(test)]
mod tests;
