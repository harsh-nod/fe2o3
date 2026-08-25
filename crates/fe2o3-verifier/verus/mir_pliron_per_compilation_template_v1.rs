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

pub open spec fn fe2o3_inverse_permutation_v1(
    mapping: Seq<nat>,
    inverse: Seq<nat>,
) -> bool {
    &&& mapping.len() == inverse.len()
    &&& forall|i: int| 0 <= i < mapping.len() ==> {
        &&& #[trigger] mapping[i] < mapping.len()
        &&& inverse[mapping[i] as int] == i
    }
}

pub proof fn fe2o3_permutation_injective_v1(
    mapping: Seq<nat>,
    inverse: Seq<nat>,
    left: nat,
    right: nat,
)
    requires
        fe2o3_inverse_permutation_v1(mapping, inverse),
        left < mapping.len(),
        right < mapping.len(),
        mapping[left as int] == mapping[right as int],
    ensures left == right,
{
    assert(inverse[mapping[left as int] as int] == left);
    assert(inverse[mapping[right as int] as int] == right);
}

} // verus!

fn main() {}
