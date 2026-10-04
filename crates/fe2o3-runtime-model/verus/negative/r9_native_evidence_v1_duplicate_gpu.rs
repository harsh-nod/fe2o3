use vstd::prelude::*;

verus! {

pub open spec fn mutated_canonical_gpu_ids_v1(left: nat, right: nat) -> bool {
    left > 0 && left <= right
}

pub proof fn mutated_canonical_gpu_ids_are_unique_v1(gpu_id: nat)
    requires gpu_id > 0, mutated_canonical_gpu_ids_v1(gpu_id, gpu_id),
    ensures gpu_id != gpu_id,
{
}

} // verus!
