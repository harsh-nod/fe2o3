//! Logical visits for the additional lane audit, including unknown-use scans.
use super::*;

fn place(p: &SemanticPlaceV1, budget: &mut Budget) -> Result<(), ProductionSemanticSsaErrorV1> {
    // One visit here and one in the unchanged all-use invalidator.
    budget.charge(2usize.saturating_mul(p.projections().len().saturating_add(1)))
}
fn operand(op: &SemanticOperandV1, budget: &mut Budget) -> Result<(), ProductionSemanticSsaErrorV1> {
    match op {
        SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) => place(p, budget),
        SemanticOperandV1::Constant(_) => budget.charge(2),
    }
}
pub(super) fn statement(source: &SemanticStatementKindV1, budget: &mut Budget) -> Result<(), ProductionSemanticSsaErrorV1> {
    match source {
        SemanticStatementKindV1::Assign(a) => {
            place(a.destination(), budget)?;
            a.value().kind().try_visit_operands(|op| operand(op, budget))?;
            match a.value().kind() {
                SemanticRvalueKindV1::Borrow {place:p,..} | SemanticRvalueKindV1::AddressOf {place:p,..}
                | SemanticRvalueKindV1::Length(p) | SemanticRvalueKindV1::Discriminant(p) => place(p, budget)?,
                SemanticRvalueKindV1::Load(load) => place(load.source(), budget)?,
                SemanticRvalueKindV1::Use(_) | SemanticRvalueKindV1::Unary {..}
                | SemanticRvalueKindV1::Binary {..} | SemanticRvalueKindV1::CheckedBinary(_)
                | SemanticRvalueKindV1::UncheckedBinary(_) | SemanticRvalueKindV1::Cast {..}
                | SemanticRvalueKindV1::Aggregate(_) => {},
            }
        }
        SemanticStatementKindV1::Store(store) => {place(store.destination(), budget)?; operand(store.value(), budget)?;}
        SemanticStatementKindV1::AtomicRmw(op) => {place(op.destination(), budget)?; place(op.address(), budget)?; operand(op.value(), budget)?;}
        SemanticStatementKindV1::AtomicCompareExchange(op) => {
            place(op.destination(), budget)?; place(op.address(), budget)?;
            operand(op.expected(), budget)?; operand(op.replacement(), budget)?;
        }
        SemanticStatementKindV1::SetDiscriminant {place:p,..} | SemanticStatementKindV1::Deinitialize(p) => place(p, budget)?,
        SemanticStatementKindV1::Assume(op) => operand(op, budget)?,
        SemanticStatementKindV1::StorageLive(_) | SemanticStatementKindV1::StorageDead(_) | SemanticStatementKindV1::Nop => {},
    }
    Ok(())
}
pub(super) fn terminator(source: &SemanticTerminatorKindV1, budget: &mut Budget) -> Result<(), ProductionSemanticSsaErrorV1> {
    match source {
        SemanticTerminatorKindV1::Call(call) => {
            if let Some(destination)=call.destination() {place(destination.place(), budget)?;}
            for op in call.arguments() {operand(op, budget)?;}
        }
        SemanticTerminatorKindV1::TailCall(call) => {for op in call.arguments() {operand(op, budget)?;}}
        SemanticTerminatorKindV1::SwitchInt {discriminant,..} => operand(discriminant, budget)?,
        SemanticTerminatorKindV1::Drop {place:p,..} => place(p, budget)?,
        SemanticTerminatorKindV1::Assert {condition,message,..} => {
            operand(condition, budget)?;
            match message {
                SemanticAssertMessageV1::BoundsCheck {length,index} => {operand(length, budget)?; operand(index, budget)?;}
                SemanticAssertMessageV1::Overflow {left,right,..} => {operand(left, budget)?; operand(right, budget)?;}
                SemanticAssertMessageV1::DivisionByZero(op) | SemanticAssertMessageV1::RemainderByZero(op) => operand(op, budget)?,
                SemanticAssertMessageV1::MisalignedPointerDereference {required_alignment,found_alignment} => {
                    operand(required_alignment, budget)?; operand(found_alignment, budget)?;
                }
                SemanticAssertMessageV1::NullPointerDereference | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => {},
            }
        }
        SemanticTerminatorKindV1::Goto(_) | SemanticTerminatorKindV1::FalseEdge {..} | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume | SemanticTerminatorKindV1::UnwindTerminate | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {},
    }
    Ok(())
}
