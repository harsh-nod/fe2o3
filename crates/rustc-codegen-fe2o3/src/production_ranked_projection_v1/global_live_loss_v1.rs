//! Read-only observations of the existing final replay of converged entries.
//! Block/statement order is NOT execution order, reaching definitions, or proof.
use super::*;

const MAX_AGGREGATES: usize = 16;
const MAX_REMOVALS: usize = 8;
const MAX_INPUTS: usize = 4;
const MAX_PROJECTIONS: usize = 4;
const MAX_WORK: usize = 131_072;

#[derive(Debug, Default)]
#[allow(dead_code)] // Only rendered on the existing Global rejection.
pub(super) struct Trace {
    function: Option<SemanticFunctionIdentityV1>,
    // Independent diagnostic logical operations; never the analysis work ledger.
    work: usize,
    work_truncated: bool,
    input_roster_truncated: bool,
    foreign_function: bool,
    last_assignment_examined: Option<(usize, usize)>,
    multi_reference_aggregates_in_final_replay_order: [Option<Event>; MAX_AGGREGATES],
    aggregates_truncated: bool,
    untransferred_input_removals_in_final_replay_order: [Option<Event>; MAX_REMOVALS],
    removals_truncated: bool,
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

impl Custody {
    fn retained(self) -> bool {
        matches!(
            self,
            Self::Global { .. } | Self::Capture | Self::ConditionalCapture
        )
    }
}

fn custody(value: Option<&ProjectedCapabilityValueV1>) -> Custody {
    match value {
        None => Custody::Absent,
        Some(ProjectedCapabilityValueV1::Invalid) => Custody::Invalid,
        Some(
            ProjectedCapabilityValueV1::CapturedGlobal(_)
            | ProjectedCapabilityValueV1::CapturedGlobalProduct(_)
            | ProjectedCapabilityValueV1::ExclusiveGlobalCapture(_),
        ) => Custody::Capture,
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

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
struct Input {
    ordinal: usize,
    local: u32,
    ty: u32,
    mode: &'static str,
    reference_typed_not_authority: bool,
    projection_count: usize,
    projections: [Option<(SemanticProjectionKindV1, u32)>; MAX_PROJECTIONS],
    // Base-local facts at this statement, not a resolved projected-place proof.
    base_before: Custody,
    base_after: Custody,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(super) struct Event {
    block: usize,
    statement: usize,
    source: SemanticSourceProvenanceV1,
    kind: &'static str,
    destination: u32,
    destination_ty: u32,
    destination_projection_count: usize,
    computed_origin_before_consumption: Custody,
    destination_after: Custody,
    initialized_scalar_copy_preserved: bool,
    inputs_truncated: bool,
    inputs: [Option<Input>; MAX_INPUTS],
}

impl Trace {
    fn charge(&mut self, amount: usize) -> bool {
        if let Some(work) = self
            .work
            .checked_add(amount)
            .filter(|work| *work <= MAX_WORK)
        {
            self.work = work;
            true
        } else {
            self.work_truncated = true;
            false
        }
    }

    pub(super) fn before(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        block: usize,
        statement: usize,
        state: &ProjectedCapabilityStateV1,
    ) -> Option<Event> {
        if self.work_truncated || !self.charge(1) {
            return None;
        }
        if self
            .function
            .is_some_and(|identity| identity != function.identity())
        {
            self.foreign_function = true;
            return None;
        }
        self.function = Some(function.identity());
        let value = function.blocks().get(block)?.statements().get(statement)?;
        let SemanticStatementKindV1::Assign(assignment) = value.kind() else {
            return None;
        };
        self.last_assignment_examined = Some((block, statement));
        let kind = match assignment.value().kind() {
            SemanticRvalueKindV1::Aggregate(_) => "Aggregate",
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_)) => "Copy",
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(_)) => "Move",
            SemanticRvalueKindV1::Borrow { .. } => "Borrow",
            SemanticRvalueKindV1::AddressOf { .. } => "AddressOf",
            _ => "OtherAssignment",
        };
        let mut event = Event {
            block,
            statement,
            source: value.source(),
            kind,
            destination: assignment.destination().local().index(),
            destination_ty: assignment.destination().ty().index(),
            destination_projection_count: assignment.destination().projections().len(),
            computed_origin_before_consumption: Custody::Absent,
            destination_after: Custody::Absent,
            initialized_scalar_copy_preserved: false,
            inputs_truncated: false,
            inputs: [None; MAX_INPUTS],
        };
        let mut ordinal = 0;
        let mut add = |place: Option<&SemanticPlaceV1>, mode| {
            if ordinal == MAX_INPUTS || !self.charge(1 + MAX_PROJECTIONS) {
                event.inputs_truncated = true;
                return Err(());
            }
            if let Some(place) = place {
                let mut projections = [None; MAX_PROJECTIONS];
                for (slot, projection) in projections.iter_mut().zip(place.projections()) {
                    *slot = Some((projection.kind(), projection.result_type().index()));
                }
                event.inputs_truncated |= place.projections().len() > MAX_PROJECTIONS;
                let before = custody(state.get(&(place.local().index() as usize)));
                event.inputs[ordinal] = Some(Input {
                    ordinal,
                    local: place.local().index(),
                    ty: place.ty().index(),
                    mode,
                    reference_typed_not_authority: matches!(types.get(place.ty().index() as usize)
                        .map(SemanticTypeDeclV1::shape), Some(SemanticTypeShapeV1::Pointer(pointer))
                        if pointer.kind() == SemanticPointerKindV1::Reference),
                    projection_count: place.projections().len(),
                    projections,
                    base_before: before,
                    base_after: before,
                });
            }
            ordinal += 1;
            Ok(())
        };
        match assignment.value().kind() {
            SemanticRvalueKindV1::Borrow { kind, place } => {
                let _ = add(
                    Some(place),
                    match kind {
                        SemanticBorrowKindV1::Shared => "SharedBorrow",
                        SemanticBorrowKindV1::Mutable => "MutableBorrow",
                        SemanticBorrowKindV1::Fake => "FakeBorrow",
                    },
                );
            }
            SemanticRvalueKindV1::AddressOf { place, .. } => {
                let _ = add(Some(place), "AddressOf");
            }
            other => {
                let _ = other.try_visit_operands(|operand| match operand {
                    SemanticOperandV1::Copy(place) => add(Some(place), "Copy"),
                    SemanticOperandV1::Move(place) => add(Some(place), "Move"),
                    SemanticOperandV1::Constant(_) => add(None, "Constant"),
                });
            }
        }
        self.input_roster_truncated |= event.inputs_truncated;
        Some(event)
    }

    pub(super) fn after(
        &mut self,
        before: Option<Event>,
        computed_origin: Option<&ProjectedCapabilityValueV1>,
        initialized_scalar_copy_preserved: bool,
        state: &ProjectedCapabilityStateV1,
    ) {
        let Some(mut event) = before else {
            return;
        };
        if !self.charge(1 + MAX_INPUTS) {
            return;
        }
        event.computed_origin_before_consumption = custody(computed_origin);
        event.destination_after = custody(state.get(&(event.destination as usize)));
        event.initialized_scalar_copy_preserved = initialized_scalar_copy_preserved;
        for input in event.inputs.iter_mut().flatten() {
            input.base_after = custody(state.get(&(input.local as usize)));
        }
        // Type selection is only a diagnostic filter; missing facts stay missing.
        if event.kind == "Aggregate"
            && event
                .inputs
                .iter()
                .flatten()
                .filter(|input| input.reference_typed_not_authority)
                .count()
                >= 2
        {
            if let Some(slot) = self
                .multi_reference_aggregates_in_final_replay_order
                .iter_mut()
                .find(|slot| slot.is_none())
            {
                *slot = Some(event);
            } else {
                self.aggregates_truncated = true;
            }
        }
        if !event.computed_origin_before_consumption.retained()
            && event
                .inputs
                .iter()
                .flatten()
                .any(|input| input.base_before.retained() && !input.base_after.retained())
        {
            if let Some(slot) = self
                .untransferred_input_removals_in_final_replay_order
                .iter_mut()
                .find(|slot| slot.is_none())
            {
                *slot = Some(event);
            } else {
                self.removals_truncated = true;
            }
        }
    }
}

pub(super) fn attach(
    mut error: ProductionRankedProjectionErrorV1,
    trace: &mut Trace,
) -> ProductionRankedProjectionErrorV1 {
    if let ProductionRankedProjectionErrorV1::GlobalAccess {
        observation: Some(observation),
        ..
    } = &mut error
    {
        observation.attach_live_replay(std::mem::take(trace));
    }
    error
}

#[cfg(test)]
#[path = "global_live_loss_v1/tests.rs"]
mod tests;
