use vstd::prelude::*;

verus! {

pub open spec fn source_exclusive_end_v2(lane: nat) -> nat { lane }
pub open spec fn mutated_cpu_exclusive_end_v2(lane: nat) -> nat { lane + 1 }

pub proof fn mutated_cpu_exclusive_uses_same_physical_prefix_v2(lane: nat)
    requires lane < 64,
    ensures source_exclusive_end_v2(lane) == mutated_cpu_exclusive_end_v2(lane),
{
}

}
