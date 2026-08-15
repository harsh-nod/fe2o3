use vstd::prelude::*;

verus! {

pub open spec fn batches_v1() -> nat { 1 }
pub open spec fn heads_v1() -> nat { 1 }
pub open spec fn sequence_v1() -> nat { 8 }
pub open spec fn dimension_v1() -> nat { 16 }
pub open spec fn lanes_v1() -> nat { 64 }
pub open spec fn outputs_per_lane_v1() -> nat { 2 }
pub open spec fn tensor_elements_v1() -> nat { sequence_v1() * dimension_v1() }

/// SHA-256 limbs for the exact public B1/H1/N8/D16 profile namespace.
pub open spec fn profile_identity_v1() -> Seq<u64> {
    seq![
        0x4dfe870bb76dd32bu64,
        0x49144ee70ec4925eu64,
        0xab8677b7cbd1a1bfu64,
        0xe99fa2294f85fec8u64,
    ]
}

/// SHA-256 limbs for the exact public `src/kernel.rs` at this boundary.
pub open spec fn source_identity_v1() -> Seq<u64> {
    seq![
        0x2b00a64e43e69c41u64,
        0x6e70080e013edf90u64,
        0xe861fef94ee66441u64,
        0xda93d2c11b3e8f17u64,
    ]
}

/// SHA-256 limbs for this exact-rational mathematical model schema.
pub open spec fn model_schema_identity_v1() -> Seq<u64> {
    seq![
        0xf26a435e375adfebu64,
        0x1753dd7429870532u64,
        0xb90c88bbd46054b9u64,
        0x498c82408bcd062bu64,
    ]
}

pub open spec fn evidence_identity_admitted_v1(
    profile: Seq<u64>,
    source: Seq<u64>,
    model: Seq<u64>,
) -> bool {
    profile == profile_identity_v1()
        && source == source_identity_v1()
        && model == model_schema_identity_v1()
}

pub proof fn exact_evidence_identity_is_admitted_v1()
    ensures evidence_identity_admitted_v1(
        profile_identity_v1(),
        source_identity_v1(),
        model_schema_identity_v1(),
    ),
{
}

pub proof fn evidence_identity_substitution_fails_closed_v1(
    profile: Seq<u64>,
    source: Seq<u64>,
    model: Seq<u64>,
)
    requires evidence_identity_admitted_v1(profile, source, model),
    ensures
        profile == profile_identity_v1(),
        source == source_identity_v1(),
        model == model_schema_identity_v1(),
{
}

pub open spec fn exact_dimensions_v1(
    batches: nat,
    heads: nat,
    sequence: nat,
    dimension: nat,
    lanes: nat,
) -> bool {
    batches == batches_v1()
        && heads == heads_v1()
        && sequence == sequence_v1()
        && dimension == dimension_v1()
        && lanes == lanes_v1()
}

pub proof fn exact_profile_dimensions_and_extent_v1()
    ensures
        exact_dimensions_v1(1, 1, 8, 16, 64),
        tensor_elements_v1() == 128,
        lanes_v1() * outputs_per_lane_v1() == tensor_elements_v1(),
{
}

pub open spec fn causal_key_v1(query: nat, key: nat) -> bool {
    query < sequence_v1() && key <= query
}

pub open spec fn causal_prefix_len_v1(query: nat) -> nat {
    query + 1
}

pub proof fn causal_domain_is_nonempty_and_bounded_v1(query: nat)
    requires query < sequence_v1(),
    ensures
        causal_prefix_len_v1(query) > 0,
        causal_prefix_len_v1(query) <= sequence_v1(),
        causal_key_v1(query, 0),
        causal_key_v1(query, query),
{
}

pub proof fn future_keys_are_excluded_v1(query: nat, key: nat)
    requires query < sequence_v1(), query < key,
    ensures !causal_key_v1(query, key),
{
}

pub proof fn admitted_key_is_in_tensor_v1(query: nat, key: nat)
    requires causal_key_v1(query, key),
    ensures key < sequence_v1(), query < sequence_v1(),
{
}

pub open spec fn tensor_index_v1(row: nat, column: nat) -> nat {
    row * dimension_v1() + column
}

pub proof fn tensor_index_is_bounded_v1(row: nat, column: nat)
    requires row < sequence_v1(), column < dimension_v1(),
    ensures tensor_index_v1(row, column) < tensor_elements_v1(),
{
}

pub open spec fn lane_output_index_v1(lane: nat, slot: nat) -> nat {
    lane * outputs_per_lane_v1() + slot
}

pub open spec fn lane_query_v1(lane: nat) -> nat {
    lane_output_index_v1(lane, 0) / dimension_v1()
}

pub open spec fn lane_column_v1(lane: nat, slot: nat) -> nat {
    lane_output_index_v1(lane, slot) % dimension_v1()
}

pub proof fn lane_output_is_bounded_v1(lane: nat, slot: nat)
    requires lane < lanes_v1(), slot < outputs_per_lane_v1(),
    ensures lane_output_index_v1(lane, slot) < tensor_elements_v1(),
{
}

pub proof fn lane_coordinate_is_in_profile_v1(lane: nat, slot: nat)
    requires lane < lanes_v1(), slot < outputs_per_lane_v1(),
    ensures
        lane_query_v1(lane) < sequence_v1(),
        lane_column_v1(lane, slot) < dimension_v1(),
        tensor_index_v1(lane_query_v1(lane), lane_column_v1(lane, slot))
            == lane_output_index_v1(lane, slot),
{
    lane_output_is_bounded_v1(lane, slot);
}

pub proof fn causal_qkv_indices_are_bounded_v1(
    lane: nat,
    slot: nat,
    key: nat,
    feature: nat,
)
    requires
        lane < lanes_v1(),
        slot < outputs_per_lane_v1(),
        causal_key_v1(lane_query_v1(lane), key),
        feature < dimension_v1(),
    ensures
        tensor_index_v1(lane_query_v1(lane), feature) < tensor_elements_v1(),
        tensor_index_v1(key, feature) < tensor_elements_v1(),
        tensor_index_v1(key, lane_column_v1(lane, slot)) < tensor_elements_v1(),
{
    lane_coordinate_is_in_profile_v1(lane, slot);
    admitted_key_is_in_tensor_v1(lane_query_v1(lane), key);
    tensor_index_is_bounded_v1(lane_query_v1(lane), feature);
    tensor_index_is_bounded_v1(key, feature);
    tensor_index_is_bounded_v1(key, lane_column_v1(lane, slot));
}

pub proof fn distinct_lane_slots_have_distinct_outputs_v1(
    left_lane: nat,
    left_slot: nat,
    right_lane: nat,
    right_slot: nat,
)
    requires
        left_lane < lanes_v1(),
        right_lane < lanes_v1(),
        left_slot < outputs_per_lane_v1(),
        right_slot < outputs_per_lane_v1(),
        left_lane != right_lane || left_slot != right_slot,
    ensures lane_output_index_v1(left_lane, left_slot)
        != lane_output_index_v1(right_lane, right_slot),
{
}

pub proof fn every_output_has_exact_owner_v1(output: nat)
    requires output < tensor_elements_v1(),
    ensures
        output / outputs_per_lane_v1() < lanes_v1(),
        output % outputs_per_lane_v1() < outputs_per_lane_v1(),
        lane_output_index_v1(
            output / outputs_per_lane_v1(),
            output % outputs_per_lane_v1(),
        ) == output,
{
}

/// The sole transcendental abstraction. No exponential law or implementation
/// is supplied here; an admitted frame must provide exact positive weights.
pub uninterp spec fn exp_weight_v1(score_delta: int) -> int;

pub open spec fn prefix_max_v1(scores: Seq<int>, end: nat) -> int
    recommends 0 < end <= scores.len(),
    decreases end,
{
    if end <= 1 {
        scores[0]
    } else {
        let previous = prefix_max_v1(scores, (end - 1) as nat);
        let current = scores[(end - 1) as int];
        if current > previous { current } else { previous }
    }
}

pub open spec fn prefix_sum_v1(values: Seq<int>, end: nat) -> int
    recommends end <= values.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        prefix_sum_v1(values, (end - 1) as nat) + values[(end - 1) as int]
    }
}

pub open spec fn weighted_prefix_sum_v1(
    weights: Seq<int>,
    values: Seq<int>,
    end: nat,
) -> int
    recommends end <= weights.len() && weights.len() == values.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        weighted_prefix_sum_v1(weights, values, (end - 1) as nat)
            + values[(end - 1) as int] * weights[(end - 1) as int]
    }
}

pub proof fn prefix_sum_recurrence_v1(values: Seq<int>, end: nat)
    requires end < values.len(),
    ensures prefix_sum_v1(values, end + 1)
        == prefix_sum_v1(values, end) + values[end as int],
{
}

pub proof fn weighted_prefix_sum_recurrence_v1(
    weights: Seq<int>,
    values: Seq<int>,
    end: nat,
)
    requires weights.len() == values.len(), end < weights.len(),
    ensures weighted_prefix_sum_v1(weights, values, end + 1)
        == weighted_prefix_sum_v1(weights, values, end)
            + values[end as int] * weights[end as int],
{
}

pub open spec fn exponential_frame_v1(
    scores: Seq<int>,
    maximum: int,
    weights: Seq<int>,
) -> bool {
    scores.len() > 0
        && scores.len() == weights.len()
        && maximum == prefix_max_v1(scores, scores.len())
        && forall |index: int| 0 <= index < scores.len() ==>
            #[trigger] weights[index] == exp_weight_v1(scores[index] - maximum)
                && weights[index] > 0
}

pub open spec fn online_state_v1(
    scores: Seq<int>,
    values: Seq<int>,
    weights: Seq<int>,
    maximum: int,
    denominator: int,
    numerator: int,
) -> bool {
    scores.len() > 0
        && scores.len() <= sequence_v1()
        && scores.len() == values.len()
        && exponential_frame_v1(scores, maximum, weights)
        && denominator == prefix_sum_v1(weights, weights.len())
        && numerator == weighted_prefix_sum_v1(weights, values, weights.len())
}

pub proof fn initial_online_state_is_exact_v1(score: int, value: int)
    requires exp_weight_v1(0) > 0,
    ensures online_state_v1(
        seq![score],
        seq![value],
        seq![exp_weight_v1(0)],
        score,
        exp_weight_v1(0),
        value * exp_weight_v1(0),
    ),
{
    assert(seq![score].len() == 1);
    assert(seq![exp_weight_v1(0)].len() == 1);
    assert(prefix_max_v1(seq![score], 1) == score);
    assert forall |index: int| 0 <= index < seq![score].len() implies
        #[trigger] seq![exp_weight_v1(0)][index]
            == exp_weight_v1(seq![score][index] - score)
            && seq![exp_weight_v1(0)][index] > 0 by {
        assert(index == 0);
    }
    prefix_sum_recurrence_v1(seq![exp_weight_v1(0)], 0);
    weighted_prefix_sum_recurrence_v1(
        seq![exp_weight_v1(0)],
        seq![value],
        0,
    );
}

pub open spec fn next_maximum_v1(old_maximum: int, next_score: int) -> int {
    if next_score > old_maximum { next_score } else { old_maximum }
}

pub proof fn maximum_frame_update_bounds_both_v1(old_maximum: int, next_score: int)
    ensures
        next_maximum_v1(old_maximum, next_score) >= old_maximum,
        next_maximum_v1(old_maximum, next_score) >= next_score,
        next_maximum_v1(old_maximum, next_score) == old_maximum
            || next_maximum_v1(old_maximum, next_score) == next_score,
{
}

pub proof fn positive_prefix_has_positive_sum_v1(weights: Seq<int>, end: nat)
    requires
        0 < end <= weights.len(),
        forall |index: int| 0 <= index < weights.len() ==> weights[index] > 0,
    ensures prefix_sum_v1(weights, end) > 0,
    decreases end,
{
    if end == 1 {
        prefix_sum_recurrence_v1(weights, 0);
    } else {
        positive_prefix_has_positive_sum_v1(weights, (end - 1) as nat);
        assert(weights[(end - 1) as int] > 0);
        prefix_sum_recurrence_v1(weights, (end - 1) as nat);
    }
}

pub proof fn online_denominator_is_nonzero_v1(
    scores: Seq<int>,
    values: Seq<int>,
    weights: Seq<int>,
    maximum: int,
    denominator: int,
    numerator: int,
)
    requires online_state_v1(scores, values, weights, maximum, denominator, numerator),
    ensures denominator > 0,
{
    positive_prefix_has_positive_sum_v1(weights, weights.len());
}

/// This is the explicit, conditional exponential frame bridge. In addition to
/// both frame contracts, callers provide the real-exponential rescaling law as
/// pointwise integer equalities. Aggregate rescaling premises are kept in the
/// online step because this model deliberately proves no exponential algebra.
pub open spec fn frame_rescaling_contract_v1(
    old_scores: Seq<int>,
    old_weights: Seq<int>,
    old_maximum: int,
    next_score: int,
    next_weights: Seq<int>,
    next_maximum: int,
    previous_scale: int,
    current_weight: int,
) -> bool {
    exponential_frame_v1(old_scores, old_maximum, old_weights)
        && exponential_frame_v1(old_scores.push(next_score), next_maximum, next_weights)
        && next_maximum == if next_score > old_maximum { next_score } else { old_maximum }
        && previous_scale == exp_weight_v1(old_maximum - next_maximum)
        && current_weight == exp_weight_v1(next_score - next_maximum)
        && previous_scale > 0
        && current_weight > 0
        && forall |index: int| 0 <= index < old_weights.len() ==>
            #[trigger] next_weights[index] == old_weights[index] * previous_scale
        && next_weights[old_weights.len() as int] == current_weight
}

pub open spec fn online_step_contract_v1(
    old_scores: Seq<int>,
    old_values: Seq<int>,
    old_weights: Seq<int>,
    old_maximum: int,
    old_denominator: int,
    old_numerator: int,
    next_score: int,
    next_value: int,
    next_weights: Seq<int>,
    next_maximum: int,
    previous_scale: int,
    current_weight: int,
    next_denominator: int,
    next_numerator: int,
) -> bool {
    online_state_v1(
        old_scores,
        old_values,
        old_weights,
        old_maximum,
        old_denominator,
        old_numerator,
    )
        && old_scores.len() < sequence_v1()
        && frame_rescaling_contract_v1(
            old_scores,
            old_weights,
            old_maximum,
            next_score,
            next_weights,
            next_maximum,
            previous_scale,
            current_weight,
        )
        && prefix_sum_v1(next_weights, old_weights.len())
            == old_denominator * previous_scale
        && weighted_prefix_sum_v1(
            next_weights,
            old_values.push(next_value),
            old_weights.len(),
        ) == old_numerator * previous_scale
        && next_denominator == old_denominator * previous_scale + current_weight
        && next_numerator == old_numerator * previous_scale + next_value * current_weight
}

pub proof fn online_step_preserves_sum_and_numerator_v1(
    old_scores: Seq<int>,
    old_values: Seq<int>,
    old_weights: Seq<int>,
    old_maximum: int,
    old_denominator: int,
    old_numerator: int,
    next_score: int,
    next_value: int,
    next_weights: Seq<int>,
    next_maximum: int,
    previous_scale: int,
    current_weight: int,
    next_denominator: int,
    next_numerator: int,
)
    requires online_step_contract_v1(
        old_scores,
        old_values,
        old_weights,
        old_maximum,
        old_denominator,
        old_numerator,
        next_score,
        next_value,
        next_weights,
        next_maximum,
        previous_scale,
        current_weight,
        next_denominator,
        next_numerator,
    ),
    ensures online_state_v1(
        old_scores.push(next_score),
        old_values.push(next_value),
        next_weights,
        next_maximum,
        next_denominator,
        next_numerator,
    ),
{
    assert(next_weights.len() == old_weights.len() + 1);
    assert(old_values.push(next_value).len() == next_weights.len());
    prefix_sum_recurrence_v1(next_weights, old_weights.len());
    weighted_prefix_sum_recurrence_v1(
        next_weights,
        old_values.push(next_value),
        old_weights.len(),
    );
    assert(old_denominator == prefix_sum_v1(old_weights, old_weights.len()));
    assert(old_numerator
        == weighted_prefix_sum_v1(old_weights, old_values, old_weights.len()));
    assert(next_weights[old_weights.len() as int] == current_weight);
    assert(next_denominator == old_denominator * previous_scale + current_weight);
    assert(next_numerator == old_numerator * previous_scale + next_value * current_weight);
    assert(prefix_sum_v1(next_weights, old_weights.len())
        == old_denominator * previous_scale);
    assert(weighted_prefix_sum_v1(
        next_weights,
        old_values.push(next_value),
        old_weights.len(),
    ) == old_numerator * previous_scale);
    assert(next_denominator == prefix_sum_v1(next_weights, next_weights.len()));
    assert(next_numerator == weighted_prefix_sum_v1(
        next_weights,
        old_values.push(next_value),
        next_weights.len(),
    ));
}

pub open spec fn causal_reference_v1(
    query: nat,
    scores: Seq<int>,
    values: Seq<int>,
    weights: Seq<int>,
    maximum: int,
    denominator: int,
    numerator: int,
) -> bool {
    query < sequence_v1()
        && scores.len() == causal_prefix_len_v1(query)
        && online_state_v1(scores, values, weights, maximum, denominator, numerator)
}

pub open spec fn exact_rational_output_v1(
    numerator: int,
    denominator: int,
    reference_numerator: int,
    reference_denominator: int,
) -> bool {
    denominator > 0
        && reference_denominator > 0
        && numerator == reference_numerator
        && denominator == reference_denominator
}

pub proof fn online_state_matches_causal_reference_v1(
    query: nat,
    scores: Seq<int>,
    values: Seq<int>,
    weights: Seq<int>,
    maximum: int,
    denominator: int,
    numerator: int,
)
    requires causal_reference_v1(
        query,
        scores,
        values,
        weights,
        maximum,
        denominator,
        numerator,
    ),
    ensures
        denominator == prefix_sum_v1(weights, causal_prefix_len_v1(query)),
        numerator == weighted_prefix_sum_v1(
            weights,
            values,
            causal_prefix_len_v1(query),
        ),
        exact_rational_output_v1(
            numerator,
            denominator,
            weighted_prefix_sum_v1(weights, values, causal_prefix_len_v1(query)),
            prefix_sum_v1(weights, causal_prefix_len_v1(query)),
        ),
{
    online_denominator_is_nonzero_v1(
        scores,
        values,
        weights,
        maximum,
        denominator,
        numerator,
    );
}

pub proof fn exact_profile_output_cell_is_owned_and_bounded_v1(
    lane: nat,
    slot: nat,
    key: nat,
    feature: nat,
)
    requires
        lane < lanes_v1(),
        slot < outputs_per_lane_v1(),
        causal_key_v1(lane_query_v1(lane), key),
        feature < dimension_v1(),
    ensures
        lane_output_index_v1(lane, slot) < tensor_elements_v1(),
        tensor_index_v1(lane_query_v1(lane), feature) < tensor_elements_v1(),
        tensor_index_v1(key, feature) < tensor_elements_v1(),
        tensor_index_v1(key, lane_column_v1(lane, slot)) < tensor_elements_v1(),
{
    lane_output_is_bounded_v1(lane, slot);
    causal_qkv_indices_are_bounded_v1(lane, slot, key, feature);
}

pub open spec fn exponential_numerical_law_claimed_v1() -> bool { false }
pub open spec fn ieee_f32_refinement_claimed_v1() -> bool { false }
pub open spec fn ocml_refinement_claimed_v1() -> bool { false }
pub open spec fn rust_source_refinement_claimed_v1() -> bool { false }
pub open spec fn compiler_kir_refinement_claimed_v1() -> bool { false }
pub open spec fn llvm_isa_refinement_claimed_v1() -> bool { false }
pub open spec fn machine_safety_claimed_v1() -> bool { false }
pub open spec fn data_race_freedom_claimed_v1() -> bool { false }
pub open spec fn gpu_result_claimed_v1() -> bool { false }

pub proof fn assurance_boundary_is_explicit_v1()
    ensures
        !exponential_numerical_law_claimed_v1(),
        !ieee_f32_refinement_claimed_v1(),
        !ocml_refinement_claimed_v1(),
        !rust_source_refinement_claimed_v1(),
        !compiler_kir_refinement_claimed_v1(),
        !llvm_isa_refinement_claimed_v1(),
        !machine_safety_claimed_v1(),
        !data_race_freedom_claimed_v1(),
        !gpu_result_claimed_v1(),
{
}

}
