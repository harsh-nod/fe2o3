use vstd::prelude::*;
verus! {
pub enum DirectionV1 { DeviceToHost, HostToDevice }
pub open spec fn mutated_engine_admitted_v1(direction: DirectionV1, engine: nat) -> bool {
    engine < 2
}
pub proof fn mutated_d2h_engine_one_is_rejected_v1()
    ensures !mutated_engine_admitted_v1(DirectionV1::DeviceToHost, 1),
{}
}
