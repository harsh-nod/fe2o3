// Expected-negative R44 mutation: final restore leaves a live certificate.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_certificate_remains_v1() -> bool { true }
pub proof fn mutated_restore_consumes_certificate_v1()
    ensures !mutated_certificate_remains_v1(),
{}
}
