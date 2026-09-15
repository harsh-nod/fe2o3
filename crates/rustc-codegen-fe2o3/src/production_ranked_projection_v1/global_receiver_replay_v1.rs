//! Test-only, opt-in observations of the real capability transfer and meet.
//! These snapshots never supply facts or alter an analysis result.
use super::*;
use std::cell::RefCell;

const MAX_RECORDS: usize = 128;
const MAX_WORK: usize = 131_072;
const MAX_LOCALS: usize = 5;

#[derive(Default)]
struct Trace {
    work: usize,
    records: usize,
    truncated: bool,
    visit: usize,
    function: Option<SemanticFunctionIdentityV1>,
    phase: &'static str,
}

thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}

struct Scope;

impl Drop for Scope {
    fn drop(&mut self) {
        TRACE.with(|slot| {
            if let Some(trace) = slot.borrow_mut().take() {
                eprintln!(
                    "receiver-replay end records={} work={} truncated={}",
                    trace.records, trace.work, trace.truncated
                );
            }
        });
    }
}

pub(crate) fn observe<T>(run: impl FnOnce() -> T) -> T {
    TRACE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none(), "receiver replay cannot nest");
        *slot = Some(Trace::default());
    });
    let _scope = Scope;
    run()
}

fn charge(amount: usize) -> bool {
    TRACE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(trace) = slot.as_mut() else {
            return false;
        };
        if trace.truncated {
            return false;
        }
        match trace
            .work
            .checked_add(amount)
            .filter(|work| *work <= MAX_WORK)
        {
            Some(work) => {
                trace.work = work;
                true
            }
            None => {
                trace.truncated = true;
                false
            }
        }
    })
}

pub(super) fn begin_block(function: &SemanticFunctionDeclV1, phase: &'static str) {
    if !charge(1) {
        return;
    }
    TRACE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let trace = slot.as_mut().expect("enabled trace");
        trace.visit += 1;
        trace.function = Some(function.identity());
        trace.phase = phase;
    });
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fact {
    Absent,
    Invalid,
    Global(ProjectedGlobalViewV1),
    Other,
}

fn fact(value: Option<&ProjectedCapabilityValueV1>) -> Fact {
    match value {
        None => Fact::Absent,
        Some(ProjectedCapabilityValueV1::Invalid) => Fact::Invalid,
        Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view))) => {
            Fact::Global(*view)
        }
        Some(_) => Fact::Other,
    }
}

#[derive(Debug)]
pub(super) struct Before {
    kind: &'static str,
    block: usize,
    statement: Option<usize>,
    target: Option<usize>,
    callee: Option<u32>,
    receiver_ownership: Option<SemanticSourceArgumentOwnershipV1>,
    source: SemanticSourceProvenanceV1,
    locals: [Option<(u32, Fact)>; MAX_LOCALS],
    incoming: [Option<Fact>; MAX_LOCALS],
    prefix_truncated: bool,
}

impl Before {
    fn new(
        kind: &'static str,
        block: usize,
        statement: Option<usize>,
        source: SemanticSourceProvenanceV1,
    ) -> Option<Self> {
        charge(1).then_some(Self {
            kind,
            block,
            statement,
            source,
            target: None,
            callee: None,
            receiver_ownership: None,
            locals: [None; MAX_LOCALS],
            incoming: [None; MAX_LOCALS],
            prefix_truncated: false,
        })
    }

    fn local(&mut self, local: u32, state: &ProjectedCapabilityStateV1) -> Result<(), ()> {
        if !charge(1 + MAX_LOCALS) {
            self.prefix_truncated = true;
            return Err(());
        }
        if self.locals.iter().flatten().any(|row| row.0 == local) {
            return Ok(());
        }
        let Some(slot) = self.locals.iter_mut().find(|slot| slot.is_none()) else {
            self.prefix_truncated = true;
            return Err(());
        };
        *slot = Some((local, fact(state.get(&(local as usize)))));
        Ok(())
    }

    fn place(
        &mut self,
        place: &SemanticPlaceV1,
        state: &ProjectedCapabilityStateV1,
    ) -> Result<(), ()> {
        // Base-local custody is observed. Projected places are never resolved here.
        self.local(place.local().index(), state)
    }
}

pub(super) fn assignment(
    function: &SemanticFunctionDeclV1,
    block: usize,
    statement: usize,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    state: &ProjectedCapabilityStateV1,
) -> Option<Before> {
    let kind = match assignment.value().kind() {
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_)) => "Copy",
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(_)) => "Move",
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            ..
        } => "MutableBorrow",
        SemanticRvalueKindV1::Borrow { .. } => "OtherBorrow",
        SemanticRvalueKindV1::AddressOf { .. } => "AddressOf",
        _ => "Assignment",
    };
    let mut before = Before::new(
        kind,
        block,
        Some(statement),
        function.blocks()[block].statements()[statement].source(),
    )?;
    let _ = before.place(assignment.destination(), state);
    match assignment.value().kind() {
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => {
            let _ = before.place(place, state);
        }
        value => {
            let _ = value.try_visit_operands(|operand| {
                if !charge(1) {
                    before.prefix_truncated = true;
                    return Err(());
                }
                match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                        before.place(place, state)
                    }
                    SemanticOperandV1::Constant(_) => Ok(()),
                }
            });
        }
    }
    Some(before)
}

pub(super) fn lifetime(
    function: &SemanticFunctionDeclV1,
    block: usize,
    statement: usize,
    local: SemanticLocalIdV1,
    state: &ProjectedCapabilityStateV1,
) -> Option<Before> {
    let statement_value = &function.blocks()[block].statements()[statement];
    let kind = if matches!(
        statement_value.kind(),
        SemanticStatementKindV1::StorageLive(_)
    ) {
        "StorageLive"
    } else {
        "StorageDead"
    };
    let mut before = Before::new(kind, block, Some(statement), statement_value.source())?;
    let _ = before.local(local.index(), state);
    Some(before)
}

pub(super) fn terminal(
    function: &SemanticFunctionDeclV1,
    block: usize,
    call: &SemanticDirectCallV1,
    callable: &SemanticCallableDeclV1,
    state: &ProjectedCapabilityStateV1,
) -> Option<Before> {
    let mut before = Before::new(
        "GlobalTerminal",
        block,
        None,
        function.blocks()[block].terminator().source(),
    )?;
    before.callee = Some(call.callee().index());
    if let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = callable {
        before.receiver_ownership = binding.abi().source_argument_ownership().first().copied();
    }
    for operand in call.arguments().iter().take(MAX_LOCALS) {
        if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand {
            let _ = before.place(place, state);
        }
    }
    before.prefix_truncated |= call.arguments().len() > MAX_LOCALS;
    Some(before)
}

pub(super) fn meet(
    function: &SemanticFunctionDeclV1,
    block: usize,
    target: usize,
    current: &ProjectedCapabilityStateV1,
    incoming: &ProjectedCapabilityStateV1,
) -> Option<Before> {
    let mut before = Before::new(
        "Meet",
        block,
        None,
        function.blocks()[block].terminator().source(),
    )?;
    before.target = Some(target);
    // Retain the lowest changed Global keys, independently of HashMap order.
    for (local, value) in current.iter().chain(incoming.iter()) {
        if !charge(1 + MAX_LOCALS) {
            before.prefix_truncated = true;
            break;
        }
        if !matches!(fact(Some(value)), Fact::Global(_))
            || current.get(local) == incoming.get(local)
        {
            continue;
        }
        let Ok(local) = u32::try_from(*local) else {
            before.prefix_truncated = true;
            continue;
        };
        if before.locals.iter().flatten().any(|row| row.0 == local) {
            continue;
        }
        let slot = before
            .locals
            .iter()
            .position(Option::is_none)
            .unwrap_or_else(|| {
                before.prefix_truncated = true;
                MAX_LOCALS - 1
            });
        if before.locals[slot].is_none_or(|row| local < row.0) {
            before.locals[slot] = Some((local, fact(current.get(&(local as usize)))));
            before
                .locals
                .sort_by_key(|row| row.map(|row| row.0).unwrap_or(u32::MAX));
        }
    }
    for (row, incoming_slot) in before.locals.iter().zip(&mut before.incoming) {
        if let Some((local, _)) = row {
            *incoming_slot = Some(fact(incoming.get(&(*local as usize))));
        }
    }
    Some(before)
}

pub(super) fn after(
    before: Option<Before>,
    computed: Option<&ProjectedCapabilityValueV1>,
    state: &ProjectedCapabilityStateV1,
) {
    let Some(before) = before else { return };
    if !charge(1 + MAX_LOCALS) {
        return;
    }
    let after = before
        .locals
        .map(|row| row.map(|(local, _)| fact(state.get(&(local as usize)))));
    let changed_global = before.locals.iter().zip(after).any(|(row, after)| {
        row.is_some_and(|(_, prior)| {
            after.is_some_and(|after| {
                prior != after
                    && (matches!(prior, Fact::Global(_)) || matches!(after, Fact::Global(_)))
            })
        })
    });
    let changed_meet_inputs = before.kind == "Meet" && before.locals.iter().any(Option::is_some);
    if !changed_global
        && !changed_meet_inputs
        && before.kind != "GlobalTerminal"
        && !before.prefix_truncated
    {
        return;
    }
    TRACE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let trace = slot.as_mut().expect("enabled trace");
        if trace.records == MAX_RECORDS { trace.truncated = true; return }
        trace.records += 1;
        eprintln!("receiver-replay record={} phase={} visit={} function={:?} event={before:?} computed={:?} after={after:?}",
            trace.records, trace.phase, trace.visit, trace.function, fact(computed));
    });
}

#[cfg(test)]
#[path = "global_receiver_replay_v1/tests.rs"]
mod tests;
