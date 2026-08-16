use vstd::prelude::*;

verus! {

pub open spec fn tokens_v1() -> nat { 8 }
pub open spec fn experts_v1() -> nat { 4 }
pub open spec fn top_k_v1() -> nat { 2 }
pub open spec fn capacity_v1() -> nat { 4 }
pub open spec fn routes_v1() -> nat { 16 }
pub open spec fn logits_v1() -> nat { 32 }
pub open spec fn offsets_v1() -> nat { 5 }
pub open spec fn output_buffers_v1() -> nat { 7 }
pub open spec fn access_width_v1() -> nat { 4 }
pub open spec fn global_address_space_v1() -> nat { 1 }
pub open spec fn drop_route_v1() -> nat { 4294967295 }

pub open spec fn source_identity_v1() -> Seq<u64> {
    seq![
        0xb77016caa0c3708eu64, 0x420e583712e65e4eu64,
        0x6428db7b4feafd8du64, 0x0a1d4bdc475ef6ffu64,
    ]
}

pub open spec fn profile_identity_v1() -> Seq<u64> {
    seq![
        0x4180ef61545684e6u64, 0x46bd5227333e7514u64,
        0xd22a2d379d7d6573u64, 0x97df4d41f7a192d1u64,
    ]
}

pub open spec fn kernel_ir_identity_v1() -> Seq<u64> {
    seq![
        0x3dfa5db917624031u64, 0x06e7d3a1581700b1u64,
        0xd03282f5dd157277u64, 0x61e5cc42c63731b2u64,
    ]
}

pub open spec fn descriptor_identity_v1() -> Seq<u64> {
    seq![
        0x7852334c9d38cd45u64, 0x44c5353776505543u64,
        0x44e8e59de2dc822fu64, 0x4f2492dfea998743u64,
    ]
}

pub open spec fn launch_identity_v1() -> Seq<u64> {
    seq![
        0x100bc49f34627485u64, 0xa959b7201a238bbfu64,
        0x8421df800d7f1028u64, 0xbbfff6bd8c51edd1u64,
    ]
}

pub open spec fn effect_identity_v1() -> Seq<u64> {
    seq![
        0x496368f70c211b00u64, 0x1417fb904622971du64,
        0x008ca24442beaef3u64, 0xe4c6c175b4f5f6bau64,
    ]
}

pub open spec fn routing_identity_v1() -> Seq<u64> {
    seq![
        0xa94a13c1ad0ac149u64, 0x8e1c6cc63416dc1cu64,
        0xda2f7c14c5e4c1c4u64, 0x22e354820fc09315u64,
    ]
}

/// Published exact-machine expectation. It is data for this finite model,
/// not a machine-code refinement or artifact-authority claim.
pub open spec fn published_machine_body_identity_v1() -> Seq<u64> {
    seq![
        0x4728028b85cc3ff4u64, 0x07190de6a70b9c84u64,
        0x4437e9f92fc587e0u64, 0x614940be898346cfu64,
    ]
}

/// SHA-256 of `fe2o3.moe_top2_v1.physical_machine_effect.gfx942.v1`.
pub open spec fn analyzer_profile_identity_v1() -> Seq<u64> {
    seq![
        0x40bea576eb92b0a1u64, 0x96914bf544f3770du64,
        0x2b0757e379e5342cu64, 0x956cb200b4454051u64,
    ]
}

pub open spec fn exact_evidence_identities_v1(
    source: Seq<u64>, profile: Seq<u64>, kernel_ir: Seq<u64>,
    descriptor: Seq<u64>, launch: Seq<u64>, effect: Seq<u64>,
    routing: Seq<u64>, published_machine_body: Seq<u64>,
    analyzer_profile: Seq<u64>,
) -> bool {
    source == source_identity_v1()
        && profile == profile_identity_v1()
        && kernel_ir == kernel_ir_identity_v1()
        && descriptor == descriptor_identity_v1()
        && launch == launch_identity_v1()
        && effect == effect_identity_v1()
        && routing == routing_identity_v1()
        && published_machine_body == published_machine_body_identity_v1()
        && analyzer_profile == analyzer_profile_identity_v1()
}

pub proof fn exact_evidence_identities_are_admitted_v1()
    ensures exact_evidence_identities_v1(
        source_identity_v1(), profile_identity_v1(), kernel_ir_identity_v1(),
        descriptor_identity_v1(), launch_identity_v1(), effect_identity_v1(),
        routing_identity_v1(), published_machine_body_identity_v1(),
        analyzer_profile_identity_v1(),
    ),
{
}

/// Buffer tags are logits=0, top2=1, requested=2, admitted=3, offsets=4,
/// route_slots=5, permutation=6, and inverse=7.
pub open spec fn buffer_elements_v1(buffer: nat) -> nat {
    if buffer == 0 { logits_v1() }
    else if buffer == 1 || buffer == 5 || buffer == 6 || buffer == 7 { routes_v1() }
    else if buffer == 2 || buffer == 3 { experts_v1() }
    else if buffer == 4 { offsets_v1() }
    else { 0 }
}

pub open spec fn buffer_bytes_v1(buffer: nat) -> nat {
    buffer_elements_v1(buffer) * access_width_v1()
}

pub proof fn exact_eight_buffer_extents_v1()
    ensures
        buffer_elements_v1(0) == 32, buffer_bytes_v1(0) == 128,
        buffer_elements_v1(1) == 16, buffer_bytes_v1(1) == 64,
        buffer_elements_v1(2) == 4, buffer_bytes_v1(2) == 16,
        buffer_elements_v1(3) == 4, buffer_bytes_v1(3) == 16,
        buffer_elements_v1(4) == 5, buffer_bytes_v1(4) == 20,
        buffer_elements_v1(5) == 16, buffer_bytes_v1(5) == 64,
        buffer_elements_v1(6) == 16, buffer_bytes_v1(6) == 64,
        buffer_elements_v1(7) == 16, buffer_bytes_v1(7) == 64,
{
}

pub open spec fn logit_index_v1(token: nat, expert: nat) -> nat {
    token * experts_v1() + expert
}

pub proof fn token_logit_index_is_bounded_v1(token: nat, expert: nat)
    requires token < tokens_v1(), expert < experts_v1(),
    ensures logit_index_v1(token, expert) < logits_v1(),
{
}

pub open spec fn route_id_v1(token: nat, rank: nat) -> nat {
    token * top_k_v1() + rank
}

pub proof fn token_rank_route_id_is_bounded_v1(token: nat, rank: nat)
    requires token < tokens_v1(), rank < top_k_v1(),
    ensures route_id_v1(token, rank) < routes_v1(),
{
}

pub proof fn expert_count_and_offset_indices_are_bounded_v1(expert: nat)
    requires expert < experts_v1(),
    ensures
        expert < buffer_elements_v1(2),
        expert < buffer_elements_v1(3),
        expert < buffer_elements_v1(4),
        expert + 1 < buffer_elements_v1(4),
{
}

pub open spec fn valid_route_value_v1(value: nat) -> bool {
    value < routes_v1() || value == drop_route_v1()
}

pub proof fn route_slot_permutation_and_inverse_values_are_bounded_v1(value: nat)
    requires valid_route_value_v1(value),
    ensures value < routes_v1() || value == 4294967295,
{
}

pub open spec fn accepted_slot_v1(offset: nat, stable_rank: nat) -> nat {
    offset + stable_rank
}

pub proof fn accepted_route_slot_is_bounded_v1(offset: nat, stable_rank: nat)
    requires
        offset <= routes_v1(), stable_rank < capacity_v1(),
        offset + stable_rank < routes_v1(),
    ensures accepted_slot_v1(offset, stable_rank) < buffer_elements_v1(5),
{
}

pub open spec fn region_is_admitted_v1(base: nat, bytes: nat, buffer: nat) -> bool {
    buffer < 8
        && bytes == buffer_bytes_v1(buffer)
        && base % access_width_v1() == 0
        && base + bytes <= 18446744073709551615
}

pub open spec fn byte_end_v1(base: nat, element: nat) -> nat {
    base + element * access_width_v1() + access_width_v1()
}

pub proof fn admitted_buffer_access_is_inside_region_v1(
    base: nat, bytes: nat, buffer: nat, element: nat,
)
    requires
        region_is_admitted_v1(base, bytes, buffer),
        element < buffer_elements_v1(buffer),
    ensures
        byte_end_v1(base, element) <= base + bytes,
        byte_end_v1(base, element) <= 18446744073709551615,
{
}

pub proof fn every_exact_abi_access_is_in_bounds_v1(
    base: nat, buffer: nat, element: nat,
)
    requires
        buffer < 8,
        region_is_admitted_v1(base, buffer_bytes_v1(buffer), buffer),
        element < buffer_elements_v1(buffer),
    ensures byte_end_v1(base, element) <= base + buffer_bytes_v1(buffer),
{
    admitted_buffer_access_is_inside_region_v1(
        base, buffer_bytes_v1(buffer), buffer, element,
    );
}

pub open spec fn regions_disjoint_v1(
    left_base: nat, left_bytes: nat, right_base: nat, right_bytes: nat,
) -> bool {
    left_base + left_bytes <= right_base || right_base + right_bytes <= left_base
}

pub proof fn pairwise_disjoint_regions_have_distinct_element_addresses_v1(
    left_base: nat, left_buffer: nat, left_index: nat,
    right_base: nat, right_buffer: nat, right_index: nat,
)
    requires
        left_buffer < 8, right_buffer < 8,
        region_is_admitted_v1(left_base, buffer_bytes_v1(left_buffer), left_buffer),
        region_is_admitted_v1(right_base, buffer_bytes_v1(right_buffer), right_buffer),
        regions_disjoint_v1(
            left_base, buffer_bytes_v1(left_buffer),
            right_base, buffer_bytes_v1(right_buffer),
        ),
        left_index < buffer_elements_v1(left_buffer),
        right_index < buffer_elements_v1(right_buffer),
    ensures
        left_base + left_index * access_width_v1()
            != right_base + right_index * access_width_v1(),
{
    admitted_buffer_access_is_inside_region_v1(
        left_base, buffer_bytes_v1(left_buffer), left_buffer, left_index,
    );
    admitted_buffer_access_is_inside_region_v1(
        right_base, buffer_bytes_v1(right_buffer), right_buffer, right_index,
    );
}

pub open spec fn write_owner_key_v1(output_buffer: nat, index: nat) -> nat {
    (output_buffer - 1) as nat * 32 + index
}

pub proof fn every_output_element_has_lane_zero_owner_v1(
    output_buffer: nat, index: nat,
)
    requires
        1 <= output_buffer < 8,
        index < buffer_elements_v1(output_buffer),
    ensures
        index < 32,
        write_owner_key_v1(output_buffer, index) < output_buffers_v1() * 32,
{
}

pub proof fn distinct_output_elements_have_distinct_write_owners_v1(
    left_buffer: nat, left_index: nat, right_buffer: nat, right_index: nat,
)
    requires
        1 <= left_buffer < 8, 1 <= right_buffer < 8,
        left_index < buffer_elements_v1(left_buffer),
        right_index < buffer_elements_v1(right_buffer),
        left_buffer != right_buffer || left_index != right_index,
    ensures write_owner_key_v1(left_buffer, left_index)
        != write_owner_key_v1(right_buffer, right_index),
{
}

pub proof fn no_duplicate_external_write_ownership_v1(
    left_buffer: nat, left_index: nat, right_buffer: nat, right_index: nat,
)
    requires
        1 <= left_buffer < 8, 1 <= right_buffer < 8,
        left_index < buffer_elements_v1(left_buffer),
        right_index < buffer_elements_v1(right_buffer),
        write_owner_key_v1(left_buffer, left_index)
            == write_owner_key_v1(right_buffer, right_index),
    ensures left_buffer == right_buffer, left_index == right_index,
{
    if left_buffer != right_buffer || left_index != right_index {
        distinct_output_elements_have_distinct_write_owners_v1(
            left_buffer, left_index, right_buffer, right_index,
        );
    }
}

pub open spec fn input_validation_phase_v1() -> nat { 0 }
pub open spec fn top2_selection_phase_v1() -> nat { 1 }
pub open spec fn requested_count_phase_v1() -> nat { 2 }
pub open spec fn capacity_clamp_phase_v1() -> nat { 3 }
pub open spec fn exclusive_scan_phase_v1() -> nat { 4 }
pub open spec fn sentinel_initialization_phase_v1() -> nat { 5 }
pub open spec fn stable_rank_phase_v1() -> nat { 6 }
pub open spec fn slot_assignment_phase_v1() -> nat { 7 }
pub open spec fn permutation_inverse_phase_v1() -> nat { 8 }
pub open spec fn output_commit_phase_v1() -> nat { 9 }

pub proof fn stable_routing_phases_precede_output_commit_v1()
    ensures
        input_validation_phase_v1() < top2_selection_phase_v1(),
        top2_selection_phase_v1() < requested_count_phase_v1(),
        requested_count_phase_v1() < capacity_clamp_phase_v1(),
        capacity_clamp_phase_v1() < exclusive_scan_phase_v1(),
        exclusive_scan_phase_v1() < sentinel_initialization_phase_v1(),
        sentinel_initialization_phase_v1() < stable_rank_phase_v1(),
        stable_rank_phase_v1() < slot_assignment_phase_v1(),
        slot_assignment_phase_v1() < permutation_inverse_phase_v1(),
        permutation_inverse_phase_v1() < output_commit_phase_v1(),
{
}

pub proof fn exact_access_abi_v1()
    ensures global_address_space_v1() == 1, access_width_v1() == 4,
{
}

pub open spec fn compiler_refinement_claimed_v1() -> bool { false }
pub open spec fn kernel_ir_refinement_claimed_v1() -> bool { false }
pub open spec fn logical_address_refinement_claimed_v1() -> bool { false }
pub open spec fn isa_refinement_claimed_v1() -> bool { false }
pub open spec fn artifact_authority_claimed_v1() -> bool { false }
pub open spec fn generalized_machine_memory_safety_claimed_v1() -> bool { false }
pub open spec fn generalized_gpu_race_freedom_claimed_v1() -> bool { false }
pub open spec fn gpu_execution_claimed_v1() -> bool { false }

pub proof fn assurance_boundary_is_conservative_v1()
    ensures
        !compiler_refinement_claimed_v1(),
        !kernel_ir_refinement_claimed_v1(),
        !logical_address_refinement_claimed_v1(),
        !isa_refinement_claimed_v1(),
        !artifact_authority_claimed_v1(),
        !generalized_machine_memory_safety_claimed_v1(),
        !generalized_gpu_race_freedom_claimed_v1(),
        !gpu_execution_claimed_v1(),
{
}

} // verus!
