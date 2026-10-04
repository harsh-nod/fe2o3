use vstd::prelude::*;

verus! {

pub open spec fn row_elements_v1() -> nat { 64 }

pub uninterp spec fn exp_real_v1(value: real) -> real;

pub open spec fn fixed_row_v1(values: Seq<real>) -> bool {
    values.len() == row_elements_v1()
}

pub open spec fn maximum_contract_v1(input: Seq<real>, maximum: real) -> bool {
    &&& fixed_row_v1(input)
    &&& forall |index: int| 0 <= index < row_elements_v1()
        ==> input[index] <= maximum
    &&& exists |index: int| 0 <= index < row_elements_v1()
        && input[index] == maximum
}

pub open spec fn exp_weights_contract_v1(
    input: Seq<real>,
    maximum: real,
    weights: Seq<real>,
) -> bool {
    &&& fixed_row_v1(input)
    &&& fixed_row_v1(weights)
    &&& forall |index: int| 0 <= index < row_elements_v1()
        ==> weights[index] == exp_real_v1(input[index] - maximum)
            && weights[index] > 0real
}

pub open spec fn prefix_sum_v1(values: Seq<real>, end: nat) -> real
    recommends end <= values.len(),
    decreases end,
{
    if end == 0 {
        0real
    } else {
        prefix_sum_v1(values, (end - 1) as nat) + values[(end - 1) as int]
    }
}

/// Mutation of the actual stable-softmax formula: every output incorrectly
/// receives lane zero's numerator instead of its own lane's numerator.
pub open spec fn mutated_stable_softmax_spec_v1(
    input: Seq<real>,
    maximum: real,
    weights: Seq<real>,
    output: Seq<real>,
) -> bool {
    &&& maximum_contract_v1(input, maximum)
    &&& exp_weights_contract_v1(input, maximum, weights)
    &&& fixed_row_v1(output)
    &&& forall |index: int| 0 <= index < row_elements_v1()
        ==> output[index] * prefix_sum_v1(weights, row_elements_v1()) == weights[0]
}

pub proof fn mutated_stable_softmax_spec_preserves_lane_numerator_correspondence_v1(
    input: Seq<real>,
    maximum: real,
    weights: Seq<real>,
    output: Seq<real>,
    lane: nat,
)
    requires
        mutated_stable_softmax_spec_v1(input, maximum, weights, output),
        lane < row_elements_v1(),
    ensures output[lane as int] * prefix_sum_v1(weights, row_elements_v1())
        == weights[lane as int],
{
}

} // verus!
