// Expected-negative R44 mutation: a cross-session loan reclaims custody.
use vstd::prelude::*;
verus! {
pub open spec fn expected_session_v1() -> nat { 43 }
pub open spec fn loan_session_v1() -> nat { 44 }
pub proof fn mutated_cross_session_reclaim_is_exact_v1()
    ensures loan_session_v1() == expected_session_v1(),
{}
}
