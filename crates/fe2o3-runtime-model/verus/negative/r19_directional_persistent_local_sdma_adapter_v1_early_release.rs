use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Published, Idle }
pub open spec fn mutated_can_release_v1(phase: PhaseV1) -> bool { true }
pub proof fn mutated_published_custody_blocks_release_v1()
    ensures !mutated_can_release_v1(PhaseV1::Published),
{}
}
