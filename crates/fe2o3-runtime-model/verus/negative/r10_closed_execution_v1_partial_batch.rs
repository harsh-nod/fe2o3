use vstd::prelude::*;

verus! {

pub open spec fn mutated_published_count_v1(batch_len: nat, ready: bool) -> nat {
    if ready { batch_len } else if batch_len > 0 { 1 } else { 0 }
}

pub proof fn mutated_unready_batch_has_no_partial_publication_v1(batch_len: nat)
    requires batch_len > 1,
    ensures mutated_published_count_v1(batch_len, false) == 0,
{
}

} // verus!
