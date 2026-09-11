//! Observation classes, not authority to decode, retry, or dispose native owners.

use crate::{
    RuntimeCompletionFailureV1, RuntimeCompletionStatusV1, RuntimeErrorV1, RuntimeValidationErrorV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::async_engine) enum CompletionClassV1 {
    Pending,
    Rejected,
    SuccessCandidate,
    Failed(RuntimeCompletionFailureV1),
    QuiescentWithoutResult,
    QuiescentError,
    Terminal,
    ObservationError,
}

pub(in crate::async_engine) fn classify_completion_v1<E>(
    observation: &Result<RuntimeCompletionStatusV1, RuntimeErrorV1<E>>,
) -> CompletionClassV1 {
    use CompletionClassV1 as Class;
    match observation {
        Ok(RuntimeCompletionStatusV1::Pending) => Class::Pending,
        Ok(RuntimeCompletionStatusV1::Succeeded) => Class::SuccessCandidate,
        Ok(RuntimeCompletionStatusV1::Failed(reason)) => Class::Failed(*reason),
        Ok(RuntimeCompletionStatusV1::QuiescentWithoutResult) => Class::QuiescentWithoutResult,
        Err(RuntimeErrorV1::BackendRejected(_)) => Class::Rejected,
        Err(RuntimeErrorV1::BackendQuiescent(_)) => Class::QuiescentError,
        Err(
            RuntimeErrorV1::BackendTerminal(_)
            | RuntimeErrorV1::BackendProtocol(_)
            | RuntimeErrorV1::Validation(RuntimeValidationErrorV1::ContextTerminal),
        ) => Class::Terminal,
        Err(RuntimeErrorV1::Validation(_)) => Class::ObservationError,
    }
}
