// Expected-negative R44 mutation: a substituted certificate changes state.
use vstd::prelude::*;
verus! {
pub open spec fn state_before_v1() -> nat { 43 }
pub open spec fn mutated_state_after_v1() -> nat { 44 }
pub proof fn mutated_certificate_substitution_preserves_state_v1()
    ensures mutated_state_after_v1() == state_before_v1(),
{}
}
