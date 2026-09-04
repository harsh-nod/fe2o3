use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum StatusV1 { Succeeded, Failed }

pub open spec fn mutated_observe_status_v1(observed: StatusV1) -> StatusV1 {
    StatusV1::Succeeded
}

pub proof fn mutated_terminal_observation_is_exact_v1()
    ensures mutated_observe_status_v1(StatusV1::Failed) == StatusV1::Failed,
{
}

}
