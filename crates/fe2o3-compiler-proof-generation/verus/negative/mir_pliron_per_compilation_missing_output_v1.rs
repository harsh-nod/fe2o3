use vstd::prelude::*;

verus! {

pub proof fn missing_output_is_not_total_v1(actual: Seq<int>, reference: Seq<int>)
    requires
        actual.len() + 1 == reference.len(),
        forall|i: int| 0 <= i < actual.len() ==> actual[i] == reference[i],
    ensures
        actual == reference,
{
}

} // verus!

fn main() {}
