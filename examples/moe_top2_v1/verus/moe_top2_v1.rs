use vstd::prelude::*;

verus! {

pub open spec fn tokens_v1() -> nat { 8 }
pub open spec fn experts_v1() -> nat { 4 }
pub open spec fn top_k_v1() -> nat { 2 }
pub open spec fn capacity_v1() -> nat { 4 }
pub open spec fn routes_v1() -> nat { tokens_v1() * top_k_v1() }
pub open spec fn score_elements_v1() -> nat { tokens_v1() * experts_v1() }
pub open spec fn drop_route_v1() -> nat { routes_v1() }

/// SHA-256 limbs for the exact public T8/E4/K2/C4 profile namespace.
pub open spec fn profile_identity_v1() -> Seq<u64> {
    seq![
        0x4180ef61545684e6u64,
        0x46bd5227333e7514u64,
        0xd22a2d379d7d6573u64,
        0x97df4d41f7a192d1u64,
    ]
}

/// SHA-256 limbs for the exact public `src/kernel.rs` at the proof boundary.
pub open spec fn source_identity_v1() -> Seq<u64> {
    seq![
        0xb77016caa0c3708eu64,
        0x420e583712e65e4eu64,
        0x6428db7b4feafd8du64,
        0x0a1d4bdc475ef6ffu64,
    ]
}

/// SHA-256 limbs for this mathematical model's reviewed schema identity.
pub open spec fn model_schema_identity_v1() -> Seq<u64> {
    seq![
        0xf8543b2709377789u64,
        0x0dd0d1fab0767924u64,
        0x21c1d3c64df6571cu64,
        0x83c91b3ffa361da7u64,
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

/// Scores are mathematical integers. This is not an IEEE-754 refinement.
pub open spec fn score_index_v1(token: nat, expert: nat) -> nat {
    token * experts_v1() + expert
}

pub open spec fn candidate_precedes_v1(
    scores: Seq<int>,
    token: nat,
    left: nat,
    right: nat,
) -> bool
    recommends
        scores.len() == score_elements_v1(),
        token < tokens_v1(),
        left < experts_v1(),
        right < experts_v1(),
{
    let left_score = scores[score_index_v1(token, left) as int];
    let right_score = scores[score_index_v1(token, right) as int];
    left_score > right_score || (left_score == right_score && left < right)
}

pub open spec fn top2_pair_v1(
    scores: Seq<int>,
    token: nat,
    first: nat,
    second: nat,
) -> bool {
    scores.len() == score_elements_v1()
        && token < tokens_v1()
        && first < experts_v1()
        && second < experts_v1()
        && first != second
        && candidate_precedes_v1(scores, token, first, second)
        && forall |candidate: nat|
            candidate < experts_v1() && candidate != first ==>
                #[trigger] candidate_precedes_v1(scores, token, first, candidate)
        && forall |candidate: nat|
            candidate < experts_v1() && candidate != first && candidate != second ==>
                #[trigger] candidate_precedes_v1(scores, token, second, candidate)
}

pub proof fn lower_expert_id_breaks_equal_score_ties_v1(
    scores: Seq<int>,
    token: nat,
    lower: nat,
    higher: nat,
)
    requires
        scores.len() == score_elements_v1(),
        token < tokens_v1(),
        lower < higher,
        higher < experts_v1(),
        scores[score_index_v1(token, lower) as int]
            == scores[score_index_v1(token, higher) as int],
    ensures
        candidate_precedes_v1(scores, token, lower, higher),
        !candidate_precedes_v1(scores, token, higher, lower),
{
}

pub proof fn precedence_is_asymmetric_v1(
    scores: Seq<int>,
    token: nat,
    left: nat,
    right: nat,
)
    requires
        scores.len() == score_elements_v1(),
        token < tokens_v1(),
        left < experts_v1(),
        right < experts_v1(),
        left != right,
    ensures candidate_precedes_v1(scores, token, left, right)
        == !candidate_precedes_v1(scores, token, right, left),
{
}

pub proof fn admitted_top2_has_range_distinctness_and_order_v1(
    scores: Seq<int>,
    token: nat,
    first: nat,
    second: nat,
)
    requires top2_pair_v1(scores, token, first, second),
    ensures
        first < 4,
        second < 4,
        first != second,
        candidate_precedes_v1(scores, token, first, second),
{
}

pub proof fn exact_top2_pair_is_deterministic_v1(
    scores: Seq<int>,
    token: nat,
    first: nat,
    second: nat,
    other_first: nat,
    other_second: nat,
)
    requires
        top2_pair_v1(scores, token, first, second),
        top2_pair_v1(scores, token, other_first, other_second),
    ensures first == other_first, second == other_second,
{
    if first != other_first {
        assert(candidate_precedes_v1(scores, token, first, other_first));
        assert(candidate_precedes_v1(scores, token, other_first, first));
        precedence_is_asymmetric_v1(scores, token, first, other_first);
    }
    if second != other_second {
        assert(first == other_first);
        assert(candidate_precedes_v1(scores, token, second, other_second));
        assert(candidate_precedes_v1(scores, token, other_second, second));
        precedence_is_asymmetric_v1(scores, token, second, other_second);
    }
}

pub open spec fn route_id_v1(token: nat, rank: nat) -> nat {
    token * top_k_v1() + rank
}

pub open spec fn exact_top2_selection_v1(scores: Seq<int>, selected: Seq<nat>) -> bool {
    scores.len() == score_elements_v1()
        && selected.len() == routes_v1()
        && (forall |token: nat| token < tokens_v1() ==>
            #[trigger] top2_pair_v1(
                scores,
                token,
                selected[route_id_v1(token, 0) as int],
                selected[route_id_v1(token, 1) as int],
            ))
}

pub proof fn exact_selection_has_two_ordered_distinct_experts_v1(
    scores: Seq<int>,
    selected: Seq<nat>,
    token: nat,
)
    requires exact_top2_selection_v1(scores, selected), token < tokens_v1(),
    ensures
        selected[route_id_v1(token, 0) as int] < experts_v1(),
        selected[route_id_v1(token, 1) as int] < experts_v1(),
        selected[route_id_v1(token, 0) as int]
            != selected[route_id_v1(token, 1) as int],
        candidate_precedes_v1(
            scores,
            token,
            selected[route_id_v1(token, 0) as int],
            selected[route_id_v1(token, 1) as int],
        ),
{
    assert(top2_pair_v1(
        scores,
        token,
        selected[route_id_v1(token, 0) as int],
        selected[route_id_v1(token, 1) as int],
    ));
}

pub open spec fn valid_selected_v1(selected: Seq<nat>) -> bool {
    selected.len() == routes_v1()
        && (forall |route: int| 0 <= route < routes_v1() ==>
            #[trigger] selected[route] < experts_v1())
}

pub open spec fn requested_prefix_v1(
    selected: Seq<nat>,
    expert: nat,
    end: nat,
) -> nat
    recommends end <= selected.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        let route = (end - 1) as nat;
        requested_prefix_v1(selected, expert, route)
            + if selected[route as int] == expert { 1nat } else { 0nat }
    }
}

pub open spec fn requested_count_v1(selected: Seq<nat>, expert: nat) -> nat {
    requested_prefix_v1(selected, expert, routes_v1())
}

pub open spec fn admitted_count_v1(selected: Seq<nat>, expert: nat) -> nat {
    if requested_count_v1(selected, expert) < capacity_v1() {
        requested_count_v1(selected, expert)
    } else {
        capacity_v1()
    }
}

pub proof fn requested_prefix_recurrence_v1(
    selected: Seq<nat>,
    expert: nat,
    end: nat,
)
    requires selected.len() == routes_v1(), end < routes_v1(),
    ensures requested_prefix_v1(selected, expert, end + 1)
        == requested_prefix_v1(selected, expert, end)
            + if selected[end as int] == expert { 1nat } else { 0nat },
{
}

pub proof fn requested_prefix_is_bounded_v1(
    selected: Seq<nat>,
    expert: nat,
    end: nat,
)
    requires selected.len() == routes_v1(), end <= routes_v1(),
    ensures requested_prefix_v1(selected, expert, end) <= end,
    decreases end,
{
    if end > 0 {
        requested_prefix_is_bounded_v1(selected, expert, (end - 1) as nat);
    }
}

pub proof fn requested_prefix_is_monotonic_v1(
    selected: Seq<nat>,
    expert: nat,
    left: nat,
    right: nat,
)
    requires selected.len() == routes_v1(), left <= right, right <= routes_v1(),
    ensures requested_prefix_v1(selected, expert, left)
        <= requested_prefix_v1(selected, expert, right),
    decreases right - left,
{
    if left < right {
        let previous = (right - 1) as nat;
        requested_prefix_is_monotonic_v1(selected, expert, left, previous);
        assert(requested_prefix_v1(selected, expert, right)
            >= requested_prefix_v1(selected, expert, previous));
    }
}

pub proof fn admitted_count_relates_request_and_capacity_v1(
    selected: Seq<nat>,
    expert: nat,
)
    requires valid_selected_v1(selected), expert < experts_v1(),
    ensures
        admitted_count_v1(selected, expert) <= requested_count_v1(selected, expert),
        admitted_count_v1(selected, expert) <= capacity_v1(),
        admitted_count_v1(selected, expert)
            == if requested_count_v1(selected, expert) < capacity_v1() {
                requested_count_v1(selected, expert)
            } else {
                capacity_v1()
            },
{
}

pub open spec fn expert_offset_v1(selected: Seq<nat>, expert: nat) -> nat
    decreases expert,
{
    if expert == 0 {
        0
    } else {
        expert_offset_v1(selected, (expert - 1) as nat)
            + admitted_count_v1(selected, (expert - 1) as nat)
    }
}

pub proof fn exclusive_scan_offset_recurrence_v1(
    selected: Seq<nat>,
    expert: nat,
)
    requires valid_selected_v1(selected), expert < experts_v1(),
    ensures expert_offset_v1(selected, expert + 1)
        == expert_offset_v1(selected, expert) + admitted_count_v1(selected, expert),
{
}

pub proof fn expert_offset_has_capacity_bound_v1(
    selected: Seq<nat>,
    expert: nat,
)
    requires valid_selected_v1(selected), expert <= experts_v1(),
    ensures
        expert_offset_v1(selected, expert) <= expert * capacity_v1(),
        expert_offset_v1(selected, expert) <= routes_v1(),
    decreases expert,
{
    if expert > 0 {
        let previous = (expert - 1) as nat;
        expert_offset_has_capacity_bound_v1(selected, previous);
        admitted_count_relates_request_and_capacity_v1(selected, previous);
    }
}

pub proof fn expert_offsets_are_monotonic_v1(
    selected: Seq<nat>,
    left: nat,
    right: nat,
)
    requires valid_selected_v1(selected), left <= right, right <= experts_v1(),
    ensures expert_offset_v1(selected, left) <= expert_offset_v1(selected, right),
    decreases right - left,
{
    if left < right {
        let previous = (right - 1) as nat;
        expert_offsets_are_monotonic_v1(selected, left, previous);
        assert(expert_offset_v1(selected, right)
            == expert_offset_v1(selected, previous)
                + admitted_count_v1(selected, previous));
    }
}

pub proof fn exclusive_scan_total_is_bounded_v1(selected: Seq<nat>)
    requires valid_selected_v1(selected),
    ensures expert_offset_v1(selected, experts_v1()) <= routes_v1(),
{
    expert_offset_has_capacity_bound_v1(selected, experts_v1());
}

pub open spec fn stable_rank_v1(selected: Seq<nat>, route: nat) -> nat
    recommends route < selected.len(),
{
    requested_prefix_v1(selected, selected[route as int], route)
}

pub open spec fn route_is_accepted_v1(selected: Seq<nat>, route: nat) -> bool {
    valid_selected_v1(selected)
        && route < routes_v1()
        && stable_rank_v1(selected, route) < capacity_v1()
}

pub open spec fn route_slot_v1(selected: Seq<nat>, route: nat) -> nat {
    if route_is_accepted_v1(selected, route) {
        expert_offset_v1(selected, selected[route as int]) + stable_rank_v1(selected, route)
    } else {
        drop_route_v1()
    }
}

proof fn stable_rank_is_below_requested_count_v1(selected: Seq<nat>, route: nat)
    requires valid_selected_v1(selected), route < routes_v1(),
    ensures stable_rank_v1(selected, route) < requested_count_v1(
        selected,
        selected[route as int],
    ),
{
    let expert = selected[route as int];
    requested_prefix_recurrence_v1(selected, expert, route);
    requested_prefix_is_monotonic_v1(selected, expert, route + 1, routes_v1());
}

proof fn accepted_rank_is_below_admitted_count_v1(selected: Seq<nat>, route: nat)
    requires route_is_accepted_v1(selected, route),
    ensures stable_rank_v1(selected, route)
        < admitted_count_v1(selected, selected[route as int]),
{
    let expert = selected[route as int];
    stable_rank_is_below_requested_count_v1(selected, route);
    admitted_count_relates_request_and_capacity_v1(selected, expert);
}

pub proof fn stable_prefix_acceptance_and_drop_v1(selected: Seq<nat>, route: nat)
    requires valid_selected_v1(selected), route < routes_v1(),
    ensures
        stable_rank_v1(selected, route) < capacity_v1()
            ==> route_is_accepted_v1(selected, route),
        stable_rank_v1(selected, route) >= capacity_v1()
            ==> !route_is_accepted_v1(selected, route)
                && route_slot_v1(selected, route) == drop_route_v1(),
{
}

pub proof fn accepted_route_slot_is_in_bounds_v1(selected: Seq<nat>, route: nat)
    requires route_is_accepted_v1(selected, route),
    ensures
        route_slot_v1(selected, route) < expert_offset_v1(selected, experts_v1()),
        route_slot_v1(selected, route) < routes_v1(),
{
    let expert = selected[route as int];
    accepted_rank_is_below_admitted_count_v1(selected, route);
    exclusive_scan_offset_recurrence_v1(selected, expert);
    expert_offsets_are_monotonic_v1(selected, expert + 1, experts_v1());
    exclusive_scan_total_is_bounded_v1(selected);
}

proof fn equal_expert_routes_have_ordered_stable_ranks_v1(
    selected: Seq<nat>,
    earlier: nat,
    later: nat,
)
    requires
        valid_selected_v1(selected),
        earlier < later,
        later < routes_v1(),
        selected[earlier as int] == selected[later as int],
    ensures stable_rank_v1(selected, earlier) < stable_rank_v1(selected, later),
{
    let expert = selected[earlier as int];
    requested_prefix_recurrence_v1(selected, expert, earlier);
    requested_prefix_is_monotonic_v1(selected, expert, earlier + 1, later);
}

pub proof fn accepted_route_slots_are_unique_v1(
    selected: Seq<nat>,
    left: nat,
    right: nat,
)
    requires
        route_is_accepted_v1(selected, left),
        route_is_accepted_v1(selected, right),
        left != right,
    ensures route_slot_v1(selected, left) != route_slot_v1(selected, right),
{
    let left_expert = selected[left as int];
    let right_expert = selected[right as int];
    if left_expert == right_expert {
        if left < right {
            equal_expert_routes_have_ordered_stable_ranks_v1(selected, left, right);
        } else {
            equal_expert_routes_have_ordered_stable_ranks_v1(selected, right, left);
        }
    } else if left_expert < right_expert {
        accepted_rank_is_below_admitted_count_v1(selected, left);
        exclusive_scan_offset_recurrence_v1(selected, left_expert);
        expert_offsets_are_monotonic_v1(selected, left_expert + 1, right_expert);
    } else {
        accepted_rank_is_below_admitted_count_v1(selected, right);
        exclusive_scan_offset_recurrence_v1(selected, right_expert);
        expert_offsets_are_monotonic_v1(selected, right_expert + 1, left_expert);
    }
}

pub open spec fn exact_routing_output_v1(
    selected: Seq<nat>,
    requested: Seq<nat>,
    admitted: Seq<nat>,
    offsets: Seq<nat>,
    slots: Seq<nat>,
    permutation: Seq<nat>,
    inverse: Seq<nat>,
) -> bool {
    valid_selected_v1(selected)
        && requested.len() == experts_v1()
        && admitted.len() == experts_v1()
        && offsets.len() == experts_v1() + 1
        && slots.len() == routes_v1()
        && permutation.len() == routes_v1()
        && inverse.len() == routes_v1()
        && (forall |expert: int| 0 <= expert < experts_v1() ==>
            requested[expert] == requested_count_v1(selected, expert as nat)
                && admitted[expert] == admitted_count_v1(selected, expert as nat))
        && (forall |expert: int| 0 <= expert <= experts_v1() ==>
            offsets[expert] == expert_offset_v1(selected, expert as nat))
        && (forall |route: int| 0 <= route < routes_v1() ==>
            #[trigger] slots[route] == route_slot_v1(selected, route as nat)
                && inverse[route] == route_slot_v1(selected, route as nat)
                && (route_is_accepted_v1(selected, route as nat) ==>
                    permutation[route_slot_v1(selected, route as nat) as int] == route as nat))
        && (forall |slot: int|
            expert_offset_v1(selected, experts_v1()) <= slot < routes_v1() ==>
                #[trigger] permutation[slot] == drop_route_v1())
}

pub open spec fn exact_routing_state_v1(
    scores: Seq<int>,
    selected: Seq<nat>,
    requested: Seq<nat>,
    admitted: Seq<nat>,
    offsets: Seq<nat>,
    slots: Seq<nat>,
    permutation: Seq<nat>,
    inverse: Seq<nat>,
) -> bool {
    exact_top2_selection_v1(scores, selected)
        && exact_routing_output_v1(
            selected, requested, admitted, offsets, slots, permutation, inverse,
        )
}

pub proof fn output_counts_capacity_and_scan_are_exact_v1(
    selected: Seq<nat>,
    requested: Seq<nat>,
    admitted: Seq<nat>,
    offsets: Seq<nat>,
    slots: Seq<nat>,
    permutation: Seq<nat>,
    inverse: Seq<nat>,
    expert: nat,
)
    requires
        exact_routing_output_v1(
            selected, requested, admitted, offsets, slots, permutation, inverse,
        ),
        expert < experts_v1(),
    ensures
        requested[expert as int] == requested_count_v1(selected, expert),
        admitted[expert as int] <= requested[expert as int],
        admitted[expert as int] <= capacity_v1(),
        offsets[(expert + 1) as int]
            == offsets[expert as int] + admitted[expert as int],
        offsets[experts_v1() as int] <= routes_v1(),
{
    assert(requested[expert as int] == requested_count_v1(selected, expert));
    assert(admitted[expert as int] == admitted_count_v1(selected, expert));
    assert(offsets[expert as int] == expert_offset_v1(selected, expert));
    assert(offsets[(expert + 1) as int] == expert_offset_v1(selected, expert + 1));
    assert(offsets[experts_v1() as int]
        == expert_offset_v1(selected, experts_v1()));
    admitted_count_relates_request_and_capacity_v1(selected, expert);
    exclusive_scan_offset_recurrence_v1(selected, expert);
    exclusive_scan_total_is_bounded_v1(selected);
}

pub proof fn exact_routing_state_joins_selection_counts_and_packing_v1(
    scores: Seq<int>,
    selected: Seq<nat>,
    requested: Seq<nat>,
    admitted: Seq<nat>,
    offsets: Seq<nat>,
    slots: Seq<nat>,
    permutation: Seq<nat>,
    inverse: Seq<nat>,
    token: nat,
    expert: nat,
)
    requires
        exact_routing_state_v1(
            scores, selected, requested, admitted, offsets, slots, permutation, inverse,
        ),
        token < tokens_v1(),
        expert < experts_v1(),
    ensures
        selected[route_id_v1(token, 0) as int] < experts_v1(),
        selected[route_id_v1(token, 1) as int] < experts_v1(),
        selected[route_id_v1(token, 0) as int]
            != selected[route_id_v1(token, 1) as int],
        candidate_precedes_v1(
            scores,
            token,
            selected[route_id_v1(token, 0) as int],
            selected[route_id_v1(token, 1) as int],
        ),
        requested[expert as int] == requested_count_v1(selected, expert),
        admitted[expert as int] <= requested[expert as int],
        admitted[expert as int] <= capacity_v1(),
        offsets[experts_v1() as int] <= routes_v1(),
{
    exact_selection_has_two_ordered_distinct_experts_v1(scores, selected, token);
    output_counts_capacity_and_scan_are_exact_v1(
        selected, requested, admitted, offsets, slots, permutation, inverse, expert,
    );
}

pub proof fn accepted_permutation_inverse_round_trip_v1(
    selected: Seq<nat>,
    requested: Seq<nat>,
    admitted: Seq<nat>,
    offsets: Seq<nat>,
    slots: Seq<nat>,
    permutation: Seq<nat>,
    inverse: Seq<nat>,
    route: nat,
)
    requires
        exact_routing_output_v1(
            selected, requested, admitted, offsets, slots, permutation, inverse,
        ),
        route_is_accepted_v1(selected, route),
    ensures
        slots[route as int] < routes_v1(),
        inverse[route as int] == slots[route as int],
        permutation[inverse[route as int] as int] == route,
{
    accepted_route_slot_is_in_bounds_v1(selected, route);
    assert(route < routes_v1());
    assert(slots[route as int] == route_slot_v1(selected, route));
    assert(inverse[route as int] == route_slot_v1(selected, route));
    assert(permutation[route_slot_v1(selected, route) as int] == route);
}

pub proof fn dropped_routes_and_permutation_tail_are_sentinels_v1(
    selected: Seq<nat>,
    requested: Seq<nat>,
    admitted: Seq<nat>,
    offsets: Seq<nat>,
    slots: Seq<nat>,
    permutation: Seq<nat>,
    inverse: Seq<nat>,
    route: nat,
    tail_slot: nat,
)
    requires
        exact_routing_output_v1(
            selected, requested, admitted, offsets, slots, permutation, inverse,
        ),
        route < routes_v1(),
        !route_is_accepted_v1(selected, route),
        expert_offset_v1(selected, experts_v1()) <= tail_slot,
        tail_slot < routes_v1(),
    ensures
        slots[route as int] == drop_route_v1(),
        inverse[route as int] == drop_route_v1(),
        permutation[tail_slot as int] == drop_route_v1(),
{
    assert(slots[route as int] == route_slot_v1(selected, route));
    assert(inverse[route as int] == route_slot_v1(selected, route));
    assert(route_slot_v1(selected, route) == drop_route_v1());
    assert(permutation[tail_slot as int] == drop_route_v1());
}

pub open spec fn ieee_f32_refinement_claimed_v1() -> bool { false }
pub open spec fn rust_source_refinement_claimed_v1() -> bool { false }
pub open spec fn compiler_refinement_claimed_v1() -> bool { false }
pub open spec fn machine_safety_claimed_v1() -> bool { false }
pub open spec fn gpu_result_claimed_v1() -> bool { false }

pub proof fn assurance_boundary_is_explicit_v1()
    ensures
        !ieee_f32_refinement_claimed_v1(),
        !rust_source_refinement_claimed_v1(),
        !compiler_refinement_claimed_v1(),
        !machine_safety_claimed_v1(),
        !gpu_result_claimed_v1(),
{
}

} // verus!
