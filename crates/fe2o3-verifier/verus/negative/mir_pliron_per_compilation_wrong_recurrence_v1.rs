use vstd::prelude::*;

verus! {

pub proof fn wrong_recurrence_does_not_refine_v1(
    actual: Seq<int>,
    reference: Seq<int>,
)
    requires
        actual.len() == reference.len(),
        actual.len() > 1,
        actual[0] == reference[0],
        forall|i: int| 0 <= i < actual.len() - 1 ==> #[trigger] actual[i + 1] == actual[i] + 2,
        forall|i: int| 0 <= i < reference.len() - 1 ==> #[trigger] reference[i + 1] == reference[i] + 1,
    ensures
        actual == reference,
{
}

} // verus!

fn main() {}
