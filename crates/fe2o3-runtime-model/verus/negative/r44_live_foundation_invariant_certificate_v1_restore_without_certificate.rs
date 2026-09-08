// Expected-negative R44 mutation: restore without a valid certificate yields session authority.
use vstd::prelude::*;
verus! {
// Mutation: restore gates only on current validation and ignores certificate
// custody.
pub open spec fn mutated_restore_returns_session_v1(
    _certificate_present: bool,
    validation_current: bool,
) -> bool {
    validation_current
}
pub proof fn mutated_restore_without_certificate_yields_no_session_v1()
    ensures !mutated_restore_returns_session_v1(false, true),
{}
}
