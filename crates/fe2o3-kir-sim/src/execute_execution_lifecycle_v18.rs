//! Affine lifecycle state only; no tile distribution, collective, or launch authority.
//! The legacy debug DTO cannot represent these tokens: after issuance the
//! entire value stack is explicitly NotCaptured, including ordinary bindings.
//! Memory records and snapshots remain available; no token is disguised as data.
use super::*;
use fe2o3_kernel_ir::{ExecutionOperationV15 as Op, ExecutionRoleV15 as Role};

#[cfg(test)]
#[path = "execute_execution_lifecycle_v18_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Context { borrowed: Option<ValueId> },
    Workgroup { context: ValueId },
}

// These cells live in the existing preflight-sized per-frame SSA map. A single
// checked generation distinguishes successive uses of one loop-carried scope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Token {
    invocation: SimulationInvocationV1,
    function: usize,
    generation: u64,
    producer: ValueId,
    state: State,
}

impl Token {
    pub(super) fn ty(self) -> Type {
        Type::Execution(match self.state {
            State::Context { .. } => Role::Context,
            State::Workgroup { .. } => Role::Workgroup,
        })
    }

    fn matches_owner(self, invocation: SimulationInvocationV1, function: usize) -> bool {
        self.invocation == invocation && self.function == function
    }
}

pub(crate) fn supports_operation(operation: &Operation) -> bool {
    match &operation.kind {
        OperationKind::Execution(Op::ContextIssue) => {
            matches!(operation.results.as_slice(), [value] if value.ty == Type::Execution(Role::Context))
        }
        OperationKind::Execution(Op::WorkgroupDerive { .. }) => {
            matches!(operation.results.as_slice(), [value] if value.ty == Type::Execution(Role::Workgroup))
        }
        OperationKind::Execution(Op::ScopeEnd { discarded, .. }) => {
            discarded.is_empty() && operation.results.is_empty()
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Invalid,
    SsaLimit,
}

fn token(
    values: &RuntimeValues<'_>,
    id: ValueId,
    invocation: SimulationInvocationV1,
    function: usize,
) -> Result<Token, Failure> {
    match values.get_ref(&id) {
        Some(RuntimeValue::Execution(value))
            if value.producer == id && value.matches_owner(invocation, function) =>
        {
            Ok(*value)
        }
        _ => Err(Failure::Invalid),
    }
}

fn transition(
    values: &mut RuntimeValues<'_>,
    operation: &Operation,
    invocation: SimulationInvocationV1,
    function: usize,
    role: FunctionRole,
    max_values: usize,
) -> Result<(), Failure> {
    if role != FunctionRole::KernelEntry || !supports_operation(operation) {
        return Err(Failure::Invalid);
    }
    let OperationKind::Execution(kind) = &operation.kind else {
        return Err(Failure::Invalid);
    };
    let result = operation.results.first().map(|value| value.id);
    if let Some(id) = result {
        if values.contains_key(&id) {
            return Err(Failure::Invalid);
        }
        if values.len() >= max_values {
            return Err(Failure::SsaLimit);
        }
    }
    match kind {
        Op::ContextIssue => {
            let id = result.ok_or(Failure::Invalid)?;
            values
                .try_insert(
                    id,
                    RuntimeValue::Execution(Token {
                        invocation,
                        function,
                        generation: 0,
                        producer: id,
                        state: State::Context { borrowed: None },
                    }),
                )
                .map_err(|()| Failure::Invalid)?;
        }
        Op::WorkgroupDerive { context } => {
            let id = result.ok_or(Failure::Invalid)?;
            let mut source = token(values, *context, invocation, function)?;
            if source.state != (State::Context { borrowed: None }) {
                return Err(Failure::Invalid);
            }
            source.generation = source.generation.checked_add(1).ok_or(Failure::Invalid)?;
            source.state = State::Context { borrowed: Some(id) };
            let workgroup = Token {
                producer: id,
                state: State::Workgroup { context: *context },
                ..source
            };
            values
                .try_insert(*context, RuntimeValue::Execution(source))
                .map_err(|()| Failure::Invalid)?;
            values
                .try_insert(id, RuntimeValue::Execution(workgroup))
                .map_err(|()| Failure::Invalid)?;
        }
        Op::ScopeEnd { workgroup, .. } => {
            let group = token(values, *workgroup, invocation, function)?;
            let State::Workgroup { context } = group.state else {
                return Err(Failure::Invalid);
            };
            let mut source = token(values, context, invocation, function)?;
            if source.state
                != (State::Context {
                    borrowed: Some(*workgroup),
                })
                || source.generation != group.generation
            {
                return Err(Failure::Invalid);
            }
            source.state = State::Context { borrowed: None };
            values.remove(workgroup);
            values
                .try_insert(context, RuntimeValue::Execution(source))
                .map_err(|()| Failure::Invalid)?;
        }
        _ => return Err(Failure::Invalid),
    }
    Ok(())
}

pub(super) fn execute_and_bind(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    frame: &mut RuntimeFrame<'_>,
    operation: &Operation,
    site: CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    let invalid = || {
        engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "V18 affine lifecycle owner or transition",
            ),
        )
    };
    let invocation = engine.invocation.ok_or_else(invalid)?;
    transition(
        &mut frame.values,
        operation,
        invocation,
        frame.function_index,
        frame.function.role,
        engine.limits.max_ssa_values,
    )
    .map_err(|failure| match failure {
        Failure::Invalid => invalid(),
        Failure::SsaLimit => engine.at(
            site,
            SimulationExecutionErrorKindV1::SsaValueLimit {
                limit: engine.limits.max_ssa_values,
            },
        ),
    })
}
