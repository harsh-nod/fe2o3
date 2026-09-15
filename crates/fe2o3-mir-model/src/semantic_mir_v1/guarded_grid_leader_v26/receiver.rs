//! Closed original receiver transfer; no selection of a same-typed Grid.
use super::*;
use crate::semantic_option_dominance::{SemanticOptionDominanceV1, SemanticOptionProducerV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Receiver {
    pub grid_option: SemanticLocalIdV1,
    pub grid: SemanticLocalIdV1,
    pub receiver: SemanticLocalIdV1,
    pub borrow_block: SemanticBlockIdV1,
    pub borrow_statement: u32,
}

fn fail() -> SemanticMirErrorV1 {
    SemanticMirErrorV1::InvalidFunctionAbi
}
fn charge(work: &mut u64, amount: usize) -> Result<(), SemanticMirErrorV1> {
    *work = work.checked_sub(amount as u64).ok_or_else(fail)?;
    Ok(())
}
fn local(operand: &SemanticOperandV1, ty: SemanticTypeIdV1) -> Option<SemanticLocalIdV1> {
    match operand {
        SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
            if p.ty() == ty && p.projections().is_empty() =>
        {
            Some(p.local())
        }
        _ => None,
    }
}
fn call<'a>(
    f: &'a SemanticFunctionDeclV1,
    b: SemanticBlockIdV1,
    callee: SemanticFunctionIdV1,
    callables: &[SemanticCallableDeclV1],
) -> Result<&'a SemanticDirectCallV1, SemanticMirErrorV1> {
    let Some(SemanticTerminatorKindV1::Call(c)) = f
        .blocks()
        .get(b.index() as usize)
        .map(|b| b.terminator().kind())
    else {
        return Err(fail());
    };
    if callables.get(c.callee().index() as usize) != Some(&SemanticCallableDeclV1::defined(callee))
        || c.unwind() != SemanticUnwindActionV1::Unreachable
    {
        return Err(fail());
    }
    Ok(c)
}

/// Returns source coordinates only. The importer separately authenticates
/// Context::grid/current_branded and the root's actual nominal context brand.
pub(super) fn observe<'a>(
    f: &'a SemanticFunctionDeclV1,
    getter: SemanticFunctionIdV1,
    source: SemanticGuardedGridLeaderSourceV1,
    types: SemanticGuardedGridLeaderTypesV1,
    callables: &[SemanticCallableDeclV1],
    work: &mut u64,
) -> Result<Receiver, SemanticMirErrorV1> {
    if f.locals().len() > 4096 || f.blocks().len() > 4096 {
        return Err(fail());
    }
    let total = f
        .blocks()
        .iter()
        .try_fold(f.locals().len(), |n, b| {
            n.checked_add(b.statements().len() + 1)
        })
        .ok_or_else(fail)?;
    charge(work, total.checked_mul(8).ok_or_else(fail)?)?;
    // The fixed statement allowance does not cover variable operand rosters.
    // Precharge their complete length before either allocation or the custody
    // scan; each operand is compared with at most two tracked locals.
    for block in f.blocks() {
        for statement in block.statements() {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() {
                    charge(
                        work,
                        aggregate.operands().len().checked_mul(2).ok_or_else(fail)?,
                    )?;
                }
            }
        }
        let arguments = match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => call.arguments().len(),
            SemanticTerminatorKindV1::TailCall(call) => call.arguments().len(),
            _ => 0,
        };
        charge(work, arguments.checked_mul(2).ok_or_else(fail)?)?;
    }
    let leader_call = call(f, source.call_block, getter, callables)?;
    let grid_call = call(f, source.grid_call_block, source.grid_getter, callables)?;
    let [argument] = leader_call.arguments() else {
        return Err(fail());
    };
    let receiver = local(argument, types.grid_reference).ok_or_else(fail)?;
    let [context] = grid_call.arguments() else {
        return Err(fail());
    };
    local(context, types.context_reference).ok_or_else(fail)?;
    let destination = grid_call.destination().ok_or_else(fail)?;
    if destination.place().ty() != types.grid_option
        || !destination.place().projections().is_empty()
        || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
    {
        return Err(fail());
    }
    let grid_option = destination.place().local();
    let mut definitions = vec![0_u8; f.locals().len()];
    let mut assignments = vec![None; f.locals().len()];
    let mut define = |place: &SemanticPlaceV1,
                      site: Option<(SemanticBlockIdV1, u32, &'a SemanticAssignmentV1)>|
     -> Result<(), SemanticMirErrorV1> {
        let slot = definitions
            .get_mut(place.local().index() as usize)
            .ok_or_else(fail)?;
        *slot = slot.saturating_add(if place.projections().is_empty() { 1 } else { 2 });
        assignments[place.local().index() as usize] = site;
        Ok(())
    };
    for (bi, b) in f.blocks().iter().enumerate() {
        for (si, s) in b.statements().iter().enumerate() {
            match s.kind() {
                SemanticStatementKindV1::Assign(a) => {
                    define(
                        a.destination(),
                        Some((SemanticBlockIdV1::from_index(bi as u32), si as u32, a)),
                    )?;
                    if let SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place,
                    }
                    | SemanticRvalueKindV1::AddressOf { place, .. } = a.value().kind()
                    {
                        define(place, None)?;
                    }
                }
                SemanticStatementKindV1::Store(s) => define(s.destination(), None)?,
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => define(place, None)?,
                SemanticStatementKindV1::StorageLive(l)
                | SemanticStatementKindV1::StorageDead(l) => {
                    let place = SemanticPlaceV1::new(
                        *l,
                        vec![],
                        f.locals().get(l.index() as usize).ok_or_else(fail)?.ty(),
                    )?;
                    define(&place, None)?;
                }
                SemanticStatementKindV1::AtomicRmw(a) => define(a.destination(), None)?,
                SemanticStatementKindV1::AtomicCompareExchange(a) => define(a.destination(), None)?,
                SemanticStatementKindV1::Assume(_) | SemanticStatementKindV1::Nop => {}
            }
        }
        if let SemanticTerminatorKindV1::Call(c) = b.terminator().kind() {
            if let Some(d) = c.destination() {
                define(d.place(), None)?;
            }
        }
    }
    drop(define);
    let exact = |l: SemanticLocalIdV1| -> Result<(SemanticBlockIdV1,u32,&SemanticAssignmentV1),SemanticMirErrorV1> {
        if definitions.get(l.index() as usize) != Some(&1) { return Err(fail()); }
        assignments[l.index() as usize].ok_or_else(fail)
    };
    let (borrow_block, borrow_statement, borrow) = exact(receiver)?;
    let SemanticRvalueKindV1::Borrow {
        kind: SemanticBorrowKindV1::Shared,
        place,
    } = borrow.value().kind()
    else {
        return Err(fail());
    };
    if borrow.value().result_type() != types.grid_reference
        || !place.projections().is_empty()
        || place.ty() != types.grid
    {
        return Err(fail());
    }
    let grid = place.local();
    let (extract_block, _, extract) = exact(grid)?;
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(payload)) = extract.value().kind() else {
        return Err(fail());
    };
    let [downcast, field] = payload.projections() else {
        return Err(fail());
    };
    if extract.value().result_type() != types.grid
        || payload.local() != grid_option
        || payload.ty() != types.grid
        || downcast.kind() != SemanticProjectionKindV1::Downcast(1)
        || downcast.result_type() != types.grid_option
        || field.kind() != SemanticProjectionKindV1::Field(0)
        || field.result_type() != types.grid
        || definitions.get(grid_option.index() as usize) != Some(&1)
    {
        return Err(fail());
    }
    // A block-local extraction/borrow/call keeps the linear value version exact;
    // the existing Option engine supplies its complete Some-edge dominance.
    if extract_block != borrow_block || borrow_block != source.call_block {
        return Err(fail());
    }
    let (_, extract_statement, _) = exact(grid)?;
    if extract_statement >= borrow_statement {
        return Err(fail());
    }
    let dominance = SemanticOptionDominanceV1::analyze(
        f,
        &[SemanticOptionProducerV1::new(
            grid_option,
            destination.edge().target(),
        )],
    )
    .map_err(|_| fail())?;
    charge(work, dominance.work_units())?;
    if !dominance
        .availability(grid_option)
        .is_some_and(|a| dominance.allows(a, source.call_block))
    {
        return Err(fail());
    }
    // No competing Move/call use may consume this Grid or its borrow. The only
    // allowed direct uses are the observed transfer and shared receiver call.
    let tracked = [grid, receiver];
    let used = |o: &SemanticOperandV1| operand_mentions(o, &tracked);
    for (bi, b) in f.blocks().iter().enumerate() {
        for (si, s) in b.statements().iter().enumerate() {
            let bad = match s.kind() {
                SemanticStatementKindV1::Assign(a) => {
                    let permitted = (bi as u32, si as u32)
                        == (extract_block.index(), extract_statement)
                        || (bi as u32, si as u32) == (borrow_block.index(), borrow_statement);
                    !permitted && mentions(a.value().kind(), &tracked)
                }
                SemanticStatementKindV1::Store(s) => {
                    tracked.contains(&s.destination().local()) || used(s.value())
                }
                SemanticStatementKindV1::Assume(o) => used(o),
                SemanticStatementKindV1::AtomicRmw(a) => {
                    tracked.contains(&a.address().local()) || used(a.value())
                }
                SemanticStatementKindV1::AtomicCompareExchange(a) => {
                    tracked.contains(&a.address().local())
                        || used(a.expected())
                        || used(a.replacement())
                }
                // The complete definition inventory above rejects every lifetime
                // or write to a tracked local, including projected writes.
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Deinitialize(_)
                | SemanticStatementKindV1::SetDiscriminant { .. }
                | SemanticStatementKindV1::Nop => false,
            };
            if bad {
                return Err(fail());
            }
        }
        let bad = match b.terminator().kind() {
            SemanticTerminatorKindV1::Call(c) => c.arguments().iter().any(|o| {
                operand_mentions(o, &[grid])
                    || (operand_mentions(o, &[receiver]) && bi as u32 != source.call_block.index())
            }),
            SemanticTerminatorKindV1::TailCall(c) => c.arguments().iter().any(used),
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => used(discriminant),
            SemanticTerminatorKindV1::Drop { place, .. } => tracked.contains(&place.local()),
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => used(condition) || assert_mentions(message, &tracked),
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => false,
        };
        if bad {
            return Err(fail());
        }
    }
    Ok(Receiver {
        grid_option,
        grid,
        receiver,
        borrow_block,
        borrow_statement,
    })
}

fn operand_mentions(o: &SemanticOperandV1, locals: &[SemanticLocalIdV1]) -> bool {
    matches!(o,SemanticOperandV1::Copy(p)|SemanticOperandV1::Move(p) if locals.contains(&p.local()))
}
fn assert_mentions(message: &SemanticAssertMessageV1, locals: &[SemanticLocalIdV1]) -> bool {
    let used = |o| operand_mentions(o, locals);
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => used(length) || used(index),
        SemanticAssertMessageV1::Overflow { left, right, .. } => used(left) || used(right),
        SemanticAssertMessageV1::DivisionByZero(o)
        | SemanticAssertMessageV1::RemainderByZero(o) => used(o),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => used(required_alignment) || used(found_alignment),
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => false,
    }
}
fn mentions(value: &SemanticRvalueKindV1, locals: &[SemanticLocalIdV1]) -> bool {
    let operand = |o: &SemanticOperandV1| operand_mentions(o, locals);
    match value {
        SemanticRvalueKindV1::Use(o)
        | SemanticRvalueKindV1::Unary { operand: o, .. }
        | SemanticRvalueKindV1::Cast { operand: o, .. } => operand(o),
        SemanticRvalueKindV1::Binary { left, right, .. } => operand(left) || operand(right),
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => locals.contains(&place.local()),
        SemanticRvalueKindV1::Aggregate(a) => a.operands().iter().any(operand),
        SemanticRvalueKindV1::CheckedBinary(b) => operand(b.left()) || operand(b.right()),
        SemanticRvalueKindV1::UncheckedBinary(b) => operand(b.left()) || operand(b.right()),
        SemanticRvalueKindV1::Load(l) => locals.contains(&l.source().local()),
    }
}
