// Expected-negative R57 mutation: quarantine releases its owners.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_quarantine_release_is_allowed_v1() -> bool { true }
pub proof fn mutated_quarantine_release_is_rejected_v1()
    ensures !mutated_quarantine_release_is_allowed_v1(),
{}
}
fn main() {}
