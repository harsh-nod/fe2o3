//! Independent source classifier for the genuine observation prefix.
//! Fixed-query candidates are deliberately an unsupported oracle boundary.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Event {
    Other,
    LiteralSkip,
    NonFixedBoundary,
    MalformedBounds,
    FixedOracleBoundary,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Step {
    pub block: usize,
    pub source_address: usize,
    pub event: Event,
}
const EMPTY: Step = Step {
    block: usize::MAX,
    source_address: 0,
    event: Event::Other,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Stop {
    End,
    NonFixedBoundary,
    MalformedBounds,
    FixedOracleBoundary,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Plan {
    pub rows: [Step; 32],
    pub len: usize,
    pub stop: Stop,
}
impl Plan {
    pub(super) const fn empty() -> Self {
        Self {
            rows: [EMPTY; 32],
            len: 0,
            stop: Stop::End,
        }
    }
}

// Exact original constant helper semantics, with only local symbol relocation.
#[derive(Clone, Copy)]
pub(super) enum FrozenConstantDefinition {
    Missing,
    Direct(u64),
    Alias(SemanticLocalIdV1),
    Invalid,
}
fn frozen_constant_definition(operand: &SemanticOperandV1) -> FrozenConstantDefinition {
    match operand {
        SemanticOperandV1::Constant(constant) => match constant.value() {
            SemanticConstantValueV1::Scalar(value) => u64::try_from(value.bits())
                .map(FrozenConstantDefinition::Direct)
                .unwrap_or(FrozenConstantDefinition::Invalid),
            _ => FrozenConstantDefinition::Invalid,
        },
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            FrozenConstantDefinition::Alias(place.local())
        }
        SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_) => {
            FrozenConstantDefinition::Invalid
        }
    }
}
fn frozen_constant_operand_value(
    operand: &SemanticOperandV1,
    constants: &[Option<u64>],
) -> Option<u64> {
    match frozen_constant_definition(operand) {
        FrozenConstantDefinition::Direct(value) => Some(value),
        FrozenConstantDefinition::Alias(local) => {
            constants.get(local.index() as usize).copied().flatten()
        }
        FrozenConstantDefinition::Missing | FrozenConstantDefinition::Invalid => None,
    }
}
fn classify(
    kind: &SemanticTerminatorKindV1,
    constants: &[Option<u64>],
    budget: &mut Budget<'_>,
) -> Q<Event> {
    budget.charge_work(1)?;
    match kind {
        SemanticTerminatorKindV1::Assert {
            expected,
            message: SemanticAssertMessageV1::BoundsCheck { length, index },
            unwind,
            ..
        } => {
            if !*expected || !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
                return Ok(Event::MalformedBounds);
            }
            budget.charge_work(1)?;
            if frozen_constant_operand_value(length, constants).is_some() && {
                budget.charge_work(1)?;
                frozen_constant_operand_value(index, constants).is_some()
            } {
                Ok(Event::LiteralSkip)
            } else if matches!(length, SemanticOperandV1::Constant(_)) {
                // Do not invoke the new lazy query as its own expected answer.
                Ok(Event::FixedOracleBoundary)
            } else {
                Ok(Event::NonFixedBoundary)
            }
        }
        SemanticTerminatorKindV1::SwitchInt { .. } => Ok(Event::NonFixedBoundary),
        _ => Ok(Event::Other),
    }
}
pub(super) fn plan(cfg: &NominalRootCfgSourceV1<'_>, budget: &mut Budget<'_>) -> Q<Plan> {
    let function = cfg.function();
    if function.blocks().len() > 32 {
        return Err(QueryError::Unavailable(
            "retirement oracle source exceeds fixed roster",
        ));
    }
    let mut plan = Plan::empty();
    for (block, source) in function.blocks().iter().enumerate() {
        let event = classify(
            source.terminator().kind(),
            cfg.source_tables().rich().constants(),
            budget,
        )?;
        plan.rows[plan.len] = Step {
            block,
            source_address: source as *const _ as usize,
            event,
        };
        plan.len += 1;
        plan.stop = match event {
            Event::NonFixedBoundary => Stop::NonFixedBoundary,
            Event::MalformedBounds => Stop::MalformedBounds,
            Event::FixedOracleBoundary => Stop::FixedOracleBoundary,
            _ => Stop::End,
        };
        if plan.stop != Stop::End {
            break;
        }
    }
    Ok(plan)
}

#[test]
fn source_oracle_empty_plan_has_no_claimed_visit_or_fixed_result() {
    let p = Plan::empty();
    assert_eq!(p.len, 0);
    assert_eq!(p.stop, Stop::End);
    assert!(
        p.rows
            .iter()
            .all(|s| s.block == usize::MAX && s.source_address == 0)
    );
}
#[test]
fn fixed_oracle_boundary_is_not_a_query_refusal_or_a_nonfixed_stage() {
    assert_ne!(Stop::FixedOracleBoundary, Stop::MalformedBounds);
    assert_ne!(Stop::FixedOracleBoundary, Stop::NonFixedBoundary);
    assert_ne!(Event::FixedOracleBoundary, Event::NonFixedBoundary);
}
