use vstd::prelude::*;

verus! {

pub open spec fn fe2o3_pointwise_equal_v1(actual: Seq<int>, reference: Seq<int>) -> bool {
    &&& actual.len() == reference.len()
    &&& forall|i: int| 0 <= i < actual.len() ==> #[trigger] actual[i] == #[trigger] reference[i]
}

pub proof fn fe2o3_exact_total_output_v1(actual: Seq<int>, reference: Seq<int>)
    requires fe2o3_pointwise_equal_v1(actual, reference),
    ensures actual == reference,
{
    assert(actual =~= reference);
}

pub open spec fn fe2o3_finite_recurrence_v1(actual: Seq<int>, reference: Seq<int>) -> bool {
    &&& actual.len() == reference.len()
    &&& actual.len() > 0
    &&& actual[0] == reference[0]
    &&& forall|i: int| 0 <= i < actual.len() - 1
        && #[trigger] actual[i] == #[trigger] reference[i]
        ==> actual[i + 1] == reference[i + 1]
}

proof fn fe2o3_recurrence_prefix_v1(actual: Seq<int>, reference: Seq<int>, end: nat)
    requires fe2o3_finite_recurrence_v1(actual, reference), end < actual.len(),
    ensures actual[end as int] == reference[end as int],
    decreases end,
{
    if end > 0 {
        fe2o3_recurrence_prefix_v1(actual, reference, (end - 1) as nat);
        assert(0 <= end - 1 < actual.len() - 1);
    }
}

pub proof fn fe2o3_finite_recurrence_refinement_v1(
    actual: Seq<int>,
    reference: Seq<int>,
)
    requires fe2o3_finite_recurrence_v1(actual, reference),
    ensures actual == reference,
{
    assert forall|i: int| 0 <= i < actual.len() implies
        #[trigger] actual[i] == #[trigger] reference[i] by {
        fe2o3_recurrence_prefix_v1(actual, reference, i as nat);
    }
    assert(actual =~= reference);
}

} // verus!

fn main() {}
