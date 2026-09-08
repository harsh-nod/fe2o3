// Expected-negative R60 mutation: any wait identity is accepted for the predecessor.
use vstd::prelude::*;

verus! {

pub open spec fn mutated_wait_binding_v1(expected: nat, observed: nat) -> bool {
    expected > 0 && observed > 0
}

pub proof fn mutated_foreign_wait_for_prior_is_rejected_v1()
    ensures !mutated_wait_binding_v1(1, 2),
{
}

}
