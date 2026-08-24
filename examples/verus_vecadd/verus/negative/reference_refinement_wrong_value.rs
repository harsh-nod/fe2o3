use vstd::prelude::*;

verus! {

pub struct Write {
    pub coordinate: int,
    pub value: int,
}

pub open spec fn mutated_contract(writes: Seq<Write>, reference: Seq<int>) -> bool {
    writes.len() == reference.len()
        && forall|i: int| 0 <= i < writes.len() ==> {
            &&& writes[i].coordinate == i
            &&& writes[i].value == reference[i] + 1
        }
}

pub proof fn mutated_reference_value_is_accepted(
    writes: Seq<Write>,
    reference: Seq<int>,
)
    requires mutated_contract(writes, reference), reference.len() > 0,
    ensures writes[0].value == reference[0],
{
}

} // verus!
