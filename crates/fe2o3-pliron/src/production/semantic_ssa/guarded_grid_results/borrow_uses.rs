//! An erased local may be reconstructed for shared borrows only. In particular
//! no later borrow may recreate a value consumed by an original Move or Drop.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticUnwindActionV1;

pub(super) fn only_shared(
    function: &SemanticFunctionDeclV1,
    owner: SemanticLocalIdV1,
    charge: &mut impl FnMut(usize, usize) -> Result<(), ProductionSemanticSsaErrorV1>,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    shared_uses(function, owner, false, charge)
}

/// A structural custody check only. The existing SSA planner must still prove
/// that the single original definition reaches every borrowed use. This mode
/// is never used to recreate an erased owner or initialize a capability.
pub(super) fn initialized_shared(
    function: &SemanticFunctionDeclV1,
    owner: SemanticLocalIdV1,
    charge: &mut impl FnMut(usize, usize) -> Result<(), ProductionSemanticSsaErrorV1>,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    shared_uses(function, owner, true, charge)
}

fn shared_uses(
    function: &SemanticFunctionDeclV1,
    owner: SemanticLocalIdV1,
    initialized: bool,
    charge: &mut impl FnMut(usize, usize) -> Result<(), ProductionSemanticSsaErrorV1>,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    charge(function.locals().len(), 0)?;
    if !function
        .locals()
        .get(owner.index() as usize)
        .is_some_and(|l| l.role() == SemanticLocalRoleV1::Temporary)
    {
        return Ok(false);
    }
    let owner_type = function.locals()[owner.index() as usize].ty();
    let mut definitions = 0_usize;
    let mut define = |p: &SemanticPlaceV1| {
        if !initialized || definitions != 0 || !p.projections().is_empty() || p.ty() != owner_type {
            false
        } else {
            definitions += 1;
            true
        }
    };
    let place = |p: &SemanticPlaceV1| p.local() == owner;
    let operand = |o: &SemanticOperandV1| matches!(o,SemanticOperandV1::Copy(p)|SemanticOperandV1::Move(p) if place(p));
    for block in function.blocks() {
        charge(1 + block.statements().len(), 0)?;
        for statement in block.statements() {
            let bad = match statement.kind() {
                SemanticStatementKindV1::Assign(a) => {
                    if place(a.destination())
                        && (a.value().result_type() != owner_type || !define(a.destination()))
                    {
                        return Ok(false);
                    }
                    let mut found = false;
                    a.value().kind().try_visit_operands(|o| {
                        charge(1, 0)?;
                        found |= operand(o);
                        Ok::<(), ProductionSemanticSsaErrorV1>(())
                    })?;
                    found
                        || match a.value().kind() {
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: p,
                            } => place(p) && !p.projections().is_empty(),
                            SemanticRvalueKindV1::Borrow { place: p, .. }
                            | SemanticRvalueKindV1::AddressOf { place: p, .. }
                            | SemanticRvalueKindV1::Length(p)
                            | SemanticRvalueKindV1::Discriminant(p) => place(p),
                            SemanticRvalueKindV1::Load(l) => place(l.source()),
                            SemanticRvalueKindV1::Use(_)
                            | SemanticRvalueKindV1::Unary { .. }
                            | SemanticRvalueKindV1::Cast { .. }
                            | SemanticRvalueKindV1::Binary { .. }
                            | SemanticRvalueKindV1::CheckedBinary(_)
                            | SemanticRvalueKindV1::UncheckedBinary(_)
                            | SemanticRvalueKindV1::Aggregate(_) => false,
                        }
                }
                SemanticStatementKindV1::Store(s) => place(s.destination()) || operand(s.value()),
                SemanticStatementKindV1::AtomicRmw(a) => {
                    place(a.destination()) || place(a.address()) || operand(a.value())
                }
                SemanticStatementKindV1::AtomicCompareExchange(a) => {
                    place(a.destination())
                        || place(a.address())
                        || operand(a.expected())
                        || operand(a.replacement())
                }
                SemanticStatementKindV1::SetDiscriminant { place: p, .. }
                | SemanticStatementKindV1::Deinitialize(p) => place(p),
                SemanticStatementKindV1::StorageLive(l)
                | SemanticStatementKindV1::StorageDead(l) => *l == owner,
                SemanticStatementKindV1::Assume(o) => operand(o),
                SemanticStatementKindV1::Nop => false,
            };
            if bad {
                return Ok(false);
            }
        }
        let bad = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(c) => {
                charge(c.arguments().len(), 0)?;
                c.arguments().iter().any(operand)
                    || c.destination().is_some_and(|d| place(d.place())
                        && (c.unwind() != SemanticUnwindActionV1::Unreachable
                            || d.edge().role() != SemanticEdgeRoleV1::CallReturn
                            || !define(d.place())))
            }
            SemanticTerminatorKindV1::TailCall(c) => {
                charge(c.arguments().len(), 0)?;
                c.arguments().iter().any(operand)
            }
            SemanticTerminatorKindV1::Drop { place: p, .. } => place(p),
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => operand(discriminant),
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                operand(condition)
                    || match message {
                        SemanticAssertMessageV1::BoundsCheck { length, index } => {
                            operand(length) || operand(index)
                        }
                        SemanticAssertMessageV1::Overflow { left, right, .. } => {
                            operand(left) || operand(right)
                        }
                        SemanticAssertMessageV1::DivisionByZero(o)
                        | SemanticAssertMessageV1::RemainderByZero(o) => operand(o),
                        SemanticAssertMessageV1::MisalignedPointerDereference {
                            required_alignment,
                            found_alignment,
                        } => operand(required_alignment) || operand(found_alignment),
                        SemanticAssertMessageV1::NullPointerDereference
                        | SemanticAssertMessageV1::ResumedAfterReturn
                        | SemanticAssertMessageV1::ResumedAfterPanic => false,
                    }
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => false,
        };
        if bad {
            return Ok(false);
        }
    }
    Ok(!initialized || definitions == 1)
}

#[cfg(test)]
#[path = "borrow_uses/tests.rs"]
mod tests;
