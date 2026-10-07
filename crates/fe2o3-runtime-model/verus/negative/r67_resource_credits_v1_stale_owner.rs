// Expected negative: nonzero record identity substitutes for exact ownership.
use vstd::prelude::*;
verus! {
pub open spec fn owner_admitted_v1(actual: u64, expected: u64) -> bool {
    actual != 0
}
pub proof fn mutated_stale_owner_v1(actual: u64, expected: u64)
    requires owner_admitted_v1(actual, expected),
    ensures actual == expected,
{}
}
