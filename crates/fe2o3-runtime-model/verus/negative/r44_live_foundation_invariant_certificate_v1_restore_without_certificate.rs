// Expected-negative R44 mutation: restore without a valid certificate yields session authority.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_session_authority_v1() -> bool { true }
pub proof fn mutated_restore_without_certificate_yields_no_session_v1()
    ensures !mutated_session_authority_v1(),
{}
}
