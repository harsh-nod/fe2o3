use vstd::prelude::*;

verus! {

pub open spec fn mutated_published_epoch_v1(epoch: nat) -> nat {
    epoch + 1
}

pub proof fn mutated_ready_publication_retains_epoch_v1(epoch: nat)
    ensures mutated_published_epoch_v1(epoch) == epoch,
{
}

} // verus!
