// Expected-negative R45 mutation: a cross-session source is same-session.
use vstd::prelude::*;
verus! {
pub open spec fn source_session_v1() -> nat { 23 }
pub open spec fn target_session_v1() -> nat { 29 }
pub proof fn mutated_cross_session_source_is_exact_v1()
    ensures source_session_v1() == target_session_v1(),
{}
}
