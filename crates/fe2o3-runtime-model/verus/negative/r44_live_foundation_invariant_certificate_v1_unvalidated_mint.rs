// Expected-negative R44 mutation: unvalidated transfer mints a certificate.
use vstd::prelude::*;
verus! {
pub open spec fn validation_passed_v1() -> bool { false }
pub proof fn mutated_unvalidated_transfer_mints_certificate_v1()
    ensures validation_passed_v1(),
{}
}
