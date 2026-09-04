use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum OutcomeV1 { RuntimeError, Succeeded }

pub open spec fn mutated_observe_runtime_error_v1() -> OutcomeV1 {
    OutcomeV1::Succeeded
}

pub proof fn mutated_runtime_error_observation_is_exact_v1()
    ensures mutated_observe_runtime_error_v1() == OutcomeV1::RuntimeError,
{
}

}
