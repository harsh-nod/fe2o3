use vstd::prelude::*;

verus! {

pub open spec fn source_active_v2(active: Seq<bool>, lane: nat) -> bool {
    active[lane as int]
}

pub open spec fn mutated_cpu_active_v2(active: Seq<bool>, lane: nat) -> bool {
    !active[lane as int]
}

pub proof fn mutated_cpu_mask_selection_matches_source_v2(active: Seq<bool>, lane: nat)
    requires active.len() == 64, lane < 64,
    ensures source_active_v2(active, lane) == mutated_cpu_active_v2(active, lane),
{
}

}
