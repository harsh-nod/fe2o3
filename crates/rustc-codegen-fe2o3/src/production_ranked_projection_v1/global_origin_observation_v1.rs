//! Rejection-only custody observations. Definition rows are a bounded syntactic
//! dependency roster, NOT reaching definitions, dominance, or historical state.
use super::*;

const MAX_LOCALS: usize = 16;
const MAX_ROWS: usize = 32;
const MAX_INPUTS: usize = 4;
const MAX_PROJECTIONS: usize = 4;
const MAX_WORK: usize = 131_072;

#[derive(Debug)]
#[allow(dead_code)] // Rendered in the existing rejection, never consumed as proof.
pub(super) struct Observation {
    work: usize,
    truncated: bool,
    receiver: u32,
    receiver_state_before_consumption: Custody,
    private_flow: Option<private_scalar_capture_v1::flow_observation::Observation>,
    live_final_replay_not_execution_order: Option<global_live_loss_v1::Trace>,
    syntactic_definitions_not_dominance: Vec<Row>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Custody {
    Absent,
    Invalid,
    Global {
        view: u32,
        allocation: u64,
        borrow: Option<SemanticBorrowKindV1>,
    },
    Capture,
    ConditionalCapture,
    Other,
}

fn custody(state: &ProjectedCapabilityStateV1, local: u32) -> Custody {
    match state.get(&(local as usize)) {
        None => Custody::Absent,
        Some(ProjectedCapabilityValueV1::Invalid) => Custody::Invalid,
        Some(ProjectedCapabilityValueV1::CapturedGlobal(_)) => Custody::Capture,
        Some(ProjectedCapabilityValueV1::GlobalEnum(_)) => Custody::ConditionalCapture,
        Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::GlobalView(view))) => {
            Custody::Global {
                view: view.view.index(),
                allocation: view.allocation.allocation_origin,
                borrow: view.borrow,
            }
        }
        Some(_) => Custody::Other,
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct Row {
    local: u32,
    block: usize,
    statement: Option<usize>,
    source: SemanticSourceProvenanceV1,
    kind: &'static str,
    destination_projection_count: usize,
    callee: Option<u32>,
    inputs: [Option<Input>; MAX_INPUTS],
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct Input {
    local: u32,
    ty: u32,
    mode: &'static str,
    // This is the current state at the failed access, NOT at the definition row.
    custody_at_failed_access: Custody,
    projection_count: usize,
    projections: [Option<(SemanticProjectionKindV1, u32)>; MAX_PROJECTIONS],
}

impl Observation {
    pub(super) fn attach_live_replay(&mut self, trace: global_live_loss_v1::Trace) {
        self.live_final_replay_not_execution_order = Some(trace);
    }

    pub(super) fn collect(
        function: &SemanticFunctionDeclV1,
        state: &ProjectedCapabilityStateV1,
        receiver: u32,
    ) -> Self {
        Self::with_budget(function, state, receiver, MAX_WORK)
    }

    fn with_budget(
        function: &SemanticFunctionDeclV1,
        state: &ProjectedCapabilityStateV1,
        receiver: u32,
        remaining: usize,
    ) -> Self {
        let mut result = Self {
            work: 0,
            truncated: false,
            receiver,
            receiver_state_before_consumption: custody(state, receiver),
            private_flow: None,
            live_final_replay_not_execution_order: None,
            syntactic_definitions_not_dominance: Vec::new(),
        };
        if receiver as usize >= function.locals().len() {
            result.truncated = true;
            return result;
        }
        let mut pending = Vec::with_capacity(MAX_LOCALS);
        pending.push(receiver);
        let mut next = 0;
        'walk: while next < pending.len() {
            let local = pending[next];
            next += 1;
            for (block, body) in function.blocks().iter().enumerate() {
                if !result.charge(remaining) {
                    break 'walk;
                }
                for (statement, value) in body.statements().iter().enumerate() {
                    if !result.charge(remaining) {
                        break 'walk;
                    }
                    let mut row = Row {
                        local,
                        block,
                        statement: Some(statement),
                        source: value.source(),
                        kind: "",
                        destination_projection_count: 0,
                        callee: None,
                        inputs: [None; MAX_INPUTS],
                    };
                    match value.kind() {
                        SemanticStatementKindV1::Assign(assignment)
                            if assignment.destination().local().index() == local =>
                        {
                            row.destination_projection_count =
                                assignment.destination().projections().len();
                            row.kind = match assignment.value().kind() {
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_)) => "Copy",
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(_)) => "Move",
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(_)) => {
                                    "Constant"
                                }
                                SemanticRvalueKindV1::Borrow { .. } => "Borrow",
                                SemanticRvalueKindV1::Aggregate(_) => "Aggregate",
                                _ => "OtherRvalue",
                            };
                            match assignment.value().kind() {
                                SemanticRvalueKindV1::Borrow { kind, place } => result.input(
                                    &mut row,
                                    state,
                                    place,
                                    match kind {
                                        SemanticBorrowKindV1::Shared => "SharedBorrow",
                                        SemanticBorrowKindV1::Mutable => "MutableBorrow",
                                        SemanticBorrowKindV1::Fake => "FakeBorrow",
                                    },
                                ),
                                SemanticRvalueKindV1::AddressOf { place, .. } => {
                                    result.input(&mut row, state, place, "AddressOf")
                                }
                                SemanticRvalueKindV1::Load(load) => {
                                    result.input(&mut row, state, load.source(), "Load")
                                }
                                SemanticRvalueKindV1::Length(place)
                                | SemanticRvalueKindV1::Discriminant(place) => {
                                    result.input(&mut row, state, place, "Metadata")
                                }
                                _ => {}
                            }
                            if assignment
                                .value()
                                .kind()
                                .try_visit_operands(|operand| {
                                    if !result.charge(remaining) {
                                        return Err(());
                                    }
                                    result.operand(&mut row, state, operand);
                                    Ok(())
                                })
                                .is_err()
                            {
                                break 'walk;
                            }
                        }
                        SemanticStatementKindV1::StorageLive(id) if id.index() == local => {
                            row.kind = "StorageLive"
                        }
                        SemanticStatementKindV1::StorageDead(id) if id.index() == local => {
                            row.kind = "StorageDead"
                        }
                        SemanticStatementKindV1::Deinitialize(place)
                            if place.local().index() == local =>
                        {
                            row.kind = "Deinitialize"
                        }
                        SemanticStatementKindV1::SetDiscriminant { place, .. }
                            if place.local().index() == local =>
                        {
                            row.kind = "SetDiscriminant"
                        }
                        _ => continue,
                    }
                    if !result.row(row, &mut pending) {
                        break 'walk;
                    }
                }
                if !result.charge(remaining) {
                    break 'walk;
                }
                if let SemanticTerminatorKindV1::Call(call) = body.terminator().kind()
                    && let Some(destination) = call.destination()
                    && destination.place().local().index() == local
                {
                    let mut row = Row {
                        local,
                        block,
                        statement: None,
                        source: body.terminator().source(),
                        kind: "CallResult",
                        destination_projection_count: destination.place().projections().len(),
                        callee: Some(call.callee().index()),
                        inputs: [None; MAX_INPUTS],
                    };
                    for operand in call.arguments() {
                        if !result.charge(remaining) {
                            break 'walk;
                        }
                        result.operand(&mut row, state, operand);
                    }
                    if !result.row(row, &mut pending) {
                        break 'walk;
                    }
                }
            }
        }
        result
    }

    fn charge(&mut self, limit: usize) -> bool {
        if self.work == limit {
            self.truncated = true;
            false
        } else {
            self.work += 1;
            true
        }
    }

    fn operand(
        &mut self,
        row: &mut Row,
        state: &ProjectedCapabilityStateV1,
        operand: &SemanticOperandV1,
    ) {
        match operand {
            SemanticOperandV1::Copy(place) => self.input(row, state, place, "Copy"),
            SemanticOperandV1::Move(place) => self.input(row, state, place, "Move"),
            SemanticOperandV1::Constant(_) => {}
        }
    }

    fn input(
        &mut self,
        row: &mut Row,
        state: &ProjectedCapabilityStateV1,
        place: &SemanticPlaceV1,
        mode: &'static str,
    ) {
        let Some(slot) = row.inputs.iter_mut().find(|slot| slot.is_none()) else {
            self.truncated = true;
            return;
        };
        let mut projections = [None; MAX_PROJECTIONS];
        for (slot, projection) in projections.iter_mut().zip(place.projections()) {
            *slot = Some((projection.kind(), projection.result_type().index()));
        }
        self.truncated |= place.projections().len() > MAX_PROJECTIONS;
        *slot = Some(Input {
            local: place.local().index(),
            ty: place.ty().index(),
            mode,
            custody_at_failed_access: custody(state, place.local().index()),
            projection_count: place.projections().len(),
            projections,
        });
    }

    fn row(&mut self, row: Row, pending: &mut Vec<u32>) -> bool {
        if self.syntactic_definitions_not_dominance.len() == MAX_ROWS {
            self.truncated = true;
            return false;
        }
        for input in row.inputs.iter().flatten() {
            if !pending.contains(&input.local) {
                if pending.len() == MAX_LOCALS {
                    self.truncated = true;
                } else {
                    pending.push(input.local);
                }
            }
        }
        self.syntactic_definitions_not_dominance.push(row);
        true
    }
}

pub(super) fn attach_private_flow(
    mut error: ProductionRankedProjectionErrorV1,
    function: &SemanticFunctionDeclV1,
    reads: Option<&private_scalar_capture_v1::PrivateScalarReads<'_>>,
) -> ProductionRankedProjectionErrorV1 {
    if let ProductionRankedProjectionErrorV1::GlobalAccess {
        observation: Some(observation),
        ..
    } = &mut error
    {
        observation.private_flow = reads.and_then(|reads| reads.observation_for(function));
    }
    error
}

#[cfg(test)]
#[path = "global_origin_observation_v1/tests.rs"]
mod tests;
