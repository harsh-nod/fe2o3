use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Active, Quarantined }

pub open spec fn mutated_releasable_v1(phase: PhaseV1) -> bool {
    phase == PhaseV1::Quarantined
}

pub proof fn mutated_indeterminate_batch_blocks_mapping_release_v1()
    ensures !mutated_releasable_v1(PhaseV1::Quarantined),
{
}

} // verus!
