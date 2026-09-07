// Expected-negative R44 mutation: distinct foundations validate as exact.
use vstd::prelude::*;
verus! {
pub open spec fn expected_foundation_v1() -> nat { 43 }
pub open spec fn substituted_foundation_v1() -> nat { 44 }
pub proof fn mutated_substituted_transfer_is_exact_v1()
    ensures substituted_foundation_v1() == expected_foundation_v1(),
{}
}
