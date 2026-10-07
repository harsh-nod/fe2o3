// Expected-negative R56 mutation: the indeterminate/untouched second native is
// substituted with the already confirmed first native.
use vstd::prelude::*;
verus! {
pub open spec fn first_native_v1(cursor: nat) -> nat { cursor % 2 }
pub open spec fn mutated_second_native_v1(cursor: nat) -> nat { cursor % 2 }
pub proof fn mutated_partial_coordinates_are_rejected_v1(cursor: nat)
    ensures mutated_second_native_v1(cursor) != first_native_v1(cursor),
{}
}
fn main() {}
