use vstd::prelude::*;

verus! {

pub proof fn mutated_closed_destination_ranges_overlap_v1()
    ensures !(16 <= 16 && 16 <= 16),
{
}

}
