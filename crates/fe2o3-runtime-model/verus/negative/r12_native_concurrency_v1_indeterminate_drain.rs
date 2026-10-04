use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 {
    Released,
    Indeterminate,
}

pub open spec fn mutated_drain_allowed_v1(_phase: PhaseV1) -> bool {
    true
}

pub proof fn mutated_indeterminate_state_blocks_drain_v1()
    ensures !mutated_drain_allowed_v1(PhaseV1::Indeterminate),
{
}

}
