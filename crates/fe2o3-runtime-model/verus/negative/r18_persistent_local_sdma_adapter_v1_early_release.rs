use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Published, Settled, Quarantined }
pub open spec fn mutated_can_release_v1(phase: PhaseV1) -> bool {
    phase != PhaseV1::Quarantined
}
pub proof fn mutated_published_use_blocks_native_release_v1()
    ensures !mutated_can_release_v1(PhaseV1::Published),
{}
}
