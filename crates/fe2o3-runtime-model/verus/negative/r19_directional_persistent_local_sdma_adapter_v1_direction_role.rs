use vstd::prelude::*;
verus! {
pub enum DirectionV1 { D2h, H2d }
pub open spec fn mutated_direction_valid_v1(direction: DirectionV1, engine: nat) -> bool {
    engine < 2
}
pub proof fn mutated_h2d_engine_zero_is_rejected_v1()
    ensures !mutated_direction_valid_v1(DirectionV1::H2d, 0),
{}
}
