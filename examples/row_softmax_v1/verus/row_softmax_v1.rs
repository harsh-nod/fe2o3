use vstd::prelude::*;

verus! {

pub open spec fn row_elements_v1() -> nat { 64 }
pub open spec fn element_bytes_v1() -> nat { 4 }
pub open spec fn row_bytes_v1() -> nat { row_elements_v1() * element_bytes_v1() }

/// Abstract mathematical exponential. Verus supplies no implementation or
/// transcendental axioms for this symbol in V1.
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

/// This is the explicit unproved transcendental contract: callers must supply
/// positive real weights equal to the abstract exponential of the stable shift.
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

pub open spec fn finite_prefix_sum_v1(values: Seq<int>, end: nat) -> int
    recommends end <= values.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        finite_prefix_sum_v1(values, (end - 1) as nat) + values[(end - 1) as int]
    }
}

/// Exact real-number stable-softmax specification for one fixed row.
pub open spec fn stable_softmax_spec_v1(
    input: Seq<real>,
    maximum: real,
    weights: Seq<real>,
    output: Seq<real>,
) -> bool {
    &&& maximum_contract_v1(input, maximum)
    &&& exp_weights_contract_v1(input, maximum, weights)
    &&& fixed_row_v1(output)
    &&& forall |index: int| 0 <= index < row_elements_v1()
        ==> output[index] * prefix_sum_v1(weights, row_elements_v1()) == weights[index]
}

/// Finite sequential denominator-reduction state after `processed` weights.
pub open spec fn denominator_state_v1(
    weights: Seq<real>,
    processed: nat,
    accumulator: real,
) -> bool {
    &&& fixed_row_v1(weights)
    &&& processed <= row_elements_v1()
    &&& accumulator == prefix_sum_v1(weights, processed)
}

/// Exact finite normalization surrogate. Each positive integer weight is also
/// the output numerator and all outputs share their total as denominator.
/// This is not a refinement of real exponential or floating-point arithmetic.
pub open spec fn finite_normalization_state_v1(
    weights: Seq<int>,
    output_numerators: Seq<int>,
    denominator: int,
) -> bool {
    &&& weights.len() == row_elements_v1()
    &&& output_numerators.len() == row_elements_v1()
    &&& denominator == finite_prefix_sum_v1(weights, row_elements_v1())
    &&& forall |index: int| 0 <= index < row_elements_v1()
        ==> weights[index] > 0 && output_numerators[index] == weights[index]
}

pub open spec fn lane_input_index_v1(lane: nat) -> nat { lane }
pub open spec fn lane_scratch_index_v1(lane: nat) -> nat { lane }
pub open spec fn lane_output_index_v1(lane: nat) -> nat { lane }

pub open spec fn element_address_v1(base: int, index: nat) -> int {
    base + element_bytes_v1() * index
}

pub open spec fn row_region_fits_u64_v1(base: int) -> bool {
    0 <= base && base + row_bytes_v1() <= 0x1_0000_0000_0000_0000int
}

pub open spec fn separate_rows_v1(input_base: int, output_base: int) -> bool {
    input_base + row_bytes_v1() <= output_base
        || output_base + row_bytes_v1() <= input_base
}

pub proof fn fixed_row_is_nonempty_v1()
    ensures row_elements_v1() == 64, row_elements_v1() > 0,
{
}

pub proof fn active_lane_indices_are_in_bounds_v1(lane: nat)
    requires lane < row_elements_v1(),
    ensures
        lane_input_index_v1(lane) < row_elements_v1(),
        lane_scratch_index_v1(lane) < row_elements_v1(),
        lane_output_index_v1(lane) < row_elements_v1(),
{
}

pub proof fn distinct_lanes_own_distinct_scratch_and_output_v1(left: nat, right: nat)
    requires
        left < row_elements_v1(),
        right < row_elements_v1(),
        left != right,
    ensures
        lane_scratch_index_v1(left) != lane_scratch_index_v1(right),
        lane_output_index_v1(left) != lane_output_index_v1(right),
{
}

pub proof fn active_element_address_is_in_row_v1(base: int, lane: nat)
    requires
        row_region_fits_u64_v1(base),
        lane < row_elements_v1(),
    ensures
        base <= element_address_v1(base, lane),
        element_address_v1(base, lane) + element_bytes_v1() <= base + row_bytes_v1(),
        element_address_v1(base, lane) + element_bytes_v1()
            <= 0x1_0000_0000_0000_0000int,
{
}

pub proof fn separate_input_and_output_accesses_do_not_alias_v1(
    input_base: int,
    output_base: int,
    reader: nat,
    writer: nat,
)
    requires
        row_region_fits_u64_v1(input_base),
        row_region_fits_u64_v1(output_base),
        separate_rows_v1(input_base, output_base),
        reader < row_elements_v1(),
        writer < row_elements_v1(),
    ensures
        element_address_v1(input_base, reader)
            != element_address_v1(output_base, writer),
{
    active_element_address_is_in_row_v1(input_base, reader);
    active_element_address_is_in_row_v1(output_base, writer);
}

pub proof fn distinct_output_writes_do_not_race_v1(base: int, left: nat, right: nat)
    requires
        row_region_fits_u64_v1(base),
        left < row_elements_v1(),
        right < row_elements_v1(),
        left != right,
    ensures
        element_address_v1(base, lane_output_index_v1(left))
            != element_address_v1(base, lane_output_index_v1(right)),
{
}

pub proof fn distinct_scratch_writes_do_not_race_v1(base: int, left: nat, right: nat)
    requires
        row_region_fits_u64_v1(base),
        left < row_elements_v1(),
        right < row_elements_v1(),
        left != right,
    ensures
        element_address_v1(base, lane_scratch_index_v1(left))
            != element_address_v1(base, lane_scratch_index_v1(right)),
{
}

pub proof fn maximum_stable_shift_is_nonpositive_v1(
    input: Seq<real>,
    maximum: real,
    lane: nat,
)
    requires
        maximum_contract_v1(input, maximum),
        lane < row_elements_v1(),
    ensures input[lane as int] - maximum <= 0real,
{
}

pub proof fn denominator_reduction_step_preserves_state_v1(
    weights: Seq<real>,
    processed: nat,
    accumulator: real,
)
    requires
        denominator_state_v1(weights, processed, accumulator),
        processed < row_elements_v1(),
    ensures denominator_state_v1(
        weights,
        processed + 1,
        accumulator + weights[processed as int],
    ),
{
}

pub proof fn positive_prefix_has_positive_sum_v1(weights: Seq<real>, end: nat)
    requires
        fixed_row_v1(weights),
        0 < end <= row_elements_v1(),
        forall |index: int| 0 <= index < row_elements_v1() ==> weights[index] > 0real,
    ensures prefix_sum_v1(weights, end) > 0real,
    decreases end,
{
    if end == 1 {
        assert(prefix_sum_v1(weights, 1) == weights[0]);
    } else {
        positive_prefix_has_positive_sum_v1(weights, (end - 1) as nat);
    }
}

pub proof fn exp_contract_gives_positive_denominator_v1(
    input: Seq<real>,
    maximum: real,
    weights: Seq<real>,
)
    requires exp_weights_contract_v1(input, maximum, weights),
    ensures prefix_sum_v1(weights, row_elements_v1()) > 0real,
{
    positive_prefix_has_positive_sum_v1(weights, row_elements_v1());
}

proof fn equal_finite_prefixes_v1(
    weights: Seq<int>,
    output_numerators: Seq<int>,
    end: nat,
)
    requires
        weights.len() == row_elements_v1(),
        output_numerators.len() == row_elements_v1(),
        end <= row_elements_v1(),
        forall |index: int| 0 <= index < row_elements_v1()
            ==> output_numerators[index] == weights[index],
    ensures finite_prefix_sum_v1(output_numerators, end) == finite_prefix_sum_v1(weights, end),
    decreases end,
{
    if end > 0 {
        equal_finite_prefixes_v1(weights, output_numerators, (end - 1) as nat);
        let output_element = output_numerators[(end - 1) as int];
        let weight_element = weights[(end - 1) as int];
        assert(output_element == weight_element);
    }
}

pub proof fn finite_normalization_output_numerator_is_positive_v1(
    weights: Seq<int>,
    output_numerators: Seq<int>,
    denominator: int,
    lane: nat,
)
    requires
        finite_normalization_state_v1(weights, output_numerators, denominator),
        lane < row_elements_v1(),
    ensures output_numerators[lane as int] > 0,
{
}

pub proof fn finite_normalization_numerators_sum_to_denominator_v1(
    weights: Seq<int>,
    output_numerators: Seq<int>,
    denominator: int,
)
    requires finite_normalization_state_v1(weights, output_numerators, denominator),
    ensures finite_prefix_sum_v1(output_numerators, row_elements_v1()) == denominator,
{
    equal_finite_prefixes_v1(weights, output_numerators, row_elements_v1());
}

} // verus!
