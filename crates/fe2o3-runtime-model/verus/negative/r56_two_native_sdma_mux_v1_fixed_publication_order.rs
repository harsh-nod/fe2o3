// Expected-negative R56 mutation: publication order is fixed [0,1] instead of
// beginning at the logical cursor parity.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_first_native_v1(cursor: nat) -> nat { 0 }
pub proof fn mutated_fixed_publication_order_is_rejected_v1()
    ensures mutated_first_native_v1(1) == 1nat % 2,
{}
}
fn main() {}
