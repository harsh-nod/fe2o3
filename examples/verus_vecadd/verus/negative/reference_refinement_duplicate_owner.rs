use vstd::prelude::*;

verus! {

pub struct Write {
    pub coordinate: int,
}

pub proof fn mutated_duplicate_owner_is_injective(writes: Seq<Write>)
    requires
        writes.len() == 2,
        writes[0].coordinate == 0,
        writes[1].coordinate == 0,
    ensures
        forall|left: int, right: int|
            0 <= left < writes.len()
                && 0 <= right < writes.len()
                && writes[left].coordinate == writes[right].coordinate
                ==> left == right,
{
}

} // verus!
