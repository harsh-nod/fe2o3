use vstd::prelude::*;
verus! {
pub enum DirectionV1 { D2h, H2d }
pub open spec fn mutated_next_valid_v1(previous: DirectionV1, next: DirectionV1) -> bool {
    previous != next
}
pub open spec fn mutated_first_repeated_h2d_step_v1() -> nat {
    if mutated_next_valid_v1(DirectionV1::H2d, DirectionV1::H2d) { 1 } else { 0 }
}
pub proof fn mutated_repeated_h2d_is_admitted_v1()
    ensures mutated_first_repeated_h2d_step_v1() == 1,
{}
}
