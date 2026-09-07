// Expected-negative R48 mutation: an unaudited waiting owner is accepted as a timeout receipt.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_timeout_receipt_is_exact_v1(
    publication_exact: bool,
    wait_epoch: nat,
    _audit_loads: nat,
    _requests: nat,
) -> bool {
    publication_exact && wait_epoch > 0
}
pub proof fn unaudited_owner_cannot_retry_as_timeout_v1()
    ensures !mutated_timeout_receipt_is_exact_v1(true, 1, 0, 17),
{}
}
