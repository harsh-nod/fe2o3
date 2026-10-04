use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Active, Retained, Released }

pub open spec fn mutated_releasable_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Retained
}

pub proof fn mutated_batch_retention_blocks_mapping_release_v1()
    ensures !mutated_releasable_v1(PhaseV1::Retained),
{
}

} // verus!
