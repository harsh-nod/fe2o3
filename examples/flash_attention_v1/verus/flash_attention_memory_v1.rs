use vstd::prelude::*;

verus! {

pub open spec fn sequence_v1() -> nat { 8 }
pub open spec fn dimension_v1() -> nat { 16 }
pub open spec fn lanes_v1() -> nat { 64 }
pub open spec fn outputs_per_lane_v1() -> nat { 2 }
pub open spec fn tensor_elements_v1() -> nat { 128 }
pub open spec fn region_bytes_v1() -> nat { 512 }
pub open spec fn global_address_space_v1() -> nat { 1 }
pub open spec fn access_width_v1() -> nat { 4 }

pub open spec fn source_identity_v1() -> Seq<u64> {
    seq![
        0xa10fcfb5ebc3fc13u64, 0x19aa36a951d86f0fu64,
        0xdcdab9ec62708c89u64, 0xea10f3b5a51fb717u64,
    ]
}

pub open spec fn profile_identity_v1() -> Seq<u64> {
    seq![
        0x4dfe870bb76dd32bu64, 0x49144ee70ec4925eu64,
        0xab8677b7cbd1a1bfu64, 0xe99fa2294f85fec8u64,
    ]
}

pub open spec fn kernel_ir_identity_v1() -> Seq<u64> {
    seq![
        0x48bd8de911ebec55u64, 0x81709761a862c889u64,
        0xf47057c8086e0fecu64, 0x4c79d5cdb70bcfe9u64,
    ]
}

pub open spec fn descriptor_identity_v1() -> Seq<u64> {
    seq![
        0x03ae02a7bce06043u64, 0xaadf546d7504b20cu64,
        0xf9a2b1c772d98ca1u64, 0x20b9be3ec2a0a79au64,
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
        0xf993ef6952da81e5u64, 0x63100577b239770eu64,
        0x912cc5b56bf803bfu64, 0xce4e47436f726172u64,
    ]
}

/// Published G4 exact machine-body expectation. This proof does not claim that
/// the current upstream LLVM build reproduced it or bind a finalized payload.
pub open spec fn published_machine_body_identity_v1() -> Seq<u64> {
    seq![
        0x60e09278e2901a18u64, 0x67a5a187614a4d33u64,
        0xf12a45a733e266bfu64, 0x35b2693b85975d65u64,
    ]
}

/// SHA-256 of `fe2o3.flash_attention_v1.physical_machine_effect.gfx942.v1`.
pub open spec fn analyzer_profile_identity_v1() -> Seq<u64> {
    seq![
        0xa4ec224c4cd422a7u64, 0xf55a26a0ea7ac6f1u64,
        0x350de9b2ff0a02c8u64, 0xfc88ce9ba1d212b8u64,
    ]
}

pub open spec fn exact_evidence_identities_v1(
    source: Seq<u64>, profile: Seq<u64>, kernel_ir: Seq<u64>,
    descriptor: Seq<u64>, launch: Seq<u64>, effect: Seq<u64>,
    published_machine_body: Seq<u64>, analyzer_profile: Seq<u64>,
) -> bool {
    source == source_identity_v1()
        && profile == profile_identity_v1()
        && kernel_ir == kernel_ir_identity_v1()
        && descriptor == descriptor_identity_v1()
        && launch == launch_identity_v1()
        && effect == effect_identity_v1()
        && published_machine_body == published_machine_body_identity_v1()
        && analyzer_profile == analyzer_profile_identity_v1()
}

pub proof fn exact_evidence_identities_are_admitted_v1()
    ensures exact_evidence_identities_v1(
        source_identity_v1(), profile_identity_v1(), kernel_ir_identity_v1(),
        descriptor_identity_v1(), launch_identity_v1(), effect_identity_v1(),
        published_machine_body_identity_v1(), analyzer_profile_identity_v1(),
    ),
{
}

pub open spec fn tensor_index_v1(row: nat, column: nat) -> nat {
    row * dimension_v1() + column
}

pub open spec fn lane_output_index_v1(lane: nat, slot: nat) -> nat {
    lane * outputs_per_lane_v1() + slot
}

pub open spec fn lane_query_v1(lane: nat) -> nat { lane / 8 }

pub open spec fn lane_column_v1(lane: nat, slot: nat) -> nat {
    lane_output_index_v1(lane, slot) % dimension_v1()
}

pub open spec fn causal_key_v1(query: nat, key: nat) -> bool {
    query < sequence_v1() && key <= query
}

pub proof fn exact_fixed_extent_v1()
    ensures
        sequence_v1() * dimension_v1() == tensor_elements_v1(),
        lanes_v1() * outputs_per_lane_v1() == tensor_elements_v1(),
        tensor_elements_v1() * access_width_v1() == region_bytes_v1(),
{
}

pub proof fn lane_mapping_is_bounded_v1(lane: nat, slot: nat)
    requires lane < lanes_v1(), slot < outputs_per_lane_v1(),
    ensures
        lane_query_v1(lane) < sequence_v1(),
        lane_column_v1(lane, slot) < dimension_v1(),
        lane_output_index_v1(lane, slot) < tensor_elements_v1(),
        tensor_index_v1(lane_query_v1(lane), lane_column_v1(lane, slot))
            == lane_output_index_v1(lane, slot),
{
}

pub proof fn causal_key_row_is_bounded_v1(lane: nat, key: nat)
    requires lane < lanes_v1(), causal_key_v1(lane_query_v1(lane), key),
    ensures key < sequence_v1(), key <= lane_query_v1(lane),
{
    lane_mapping_is_bounded_v1(lane, 0);
}

pub proof fn qkv_reads_are_within_128_f32_v1(
    lane: nat, slot: nat, key: nat, feature: nat,
)
    requires
        lane < lanes_v1(), slot < outputs_per_lane_v1(),
        causal_key_v1(lane_query_v1(lane), key),
        feature < dimension_v1(),
    ensures
        tensor_index_v1(lane_query_v1(lane), feature) < tensor_elements_v1(),
        tensor_index_v1(key, feature) < tensor_elements_v1(),
        tensor_index_v1(key, lane_column_v1(lane, slot)) < tensor_elements_v1(),
{
    lane_mapping_is_bounded_v1(lane, slot);
    causal_key_row_is_bounded_v1(lane, key);
}

pub open spec fn byte_end_v1(base: nat, element: nat) -> nat {
    base + element * access_width_v1() + access_width_v1()
}

pub open spec fn region_is_admitted_v1(base: nat, bytes: nat) -> bool {
    bytes == region_bytes_v1()
        && base % access_width_v1() == 0
        && base + bytes <= 18446744073709551615
}

pub proof fn admitted_element_access_is_inside_region_v1(base: nat, bytes: nat, element: nat)
    requires region_is_admitted_v1(base, bytes), element < tensor_elements_v1(),
    ensures
        byte_end_v1(base, element) <= base + bytes,
        byte_end_v1(base, element) <= 18446744073709551615,
{
}

pub proof fn qkv_read_addresses_are_inside_regions_v1(
    q_base: nat, k_base: nat, v_base: nat, lane: nat, slot: nat, key: nat, feature: nat,
)
    requires
        region_is_admitted_v1(q_base, region_bytes_v1()),
        region_is_admitted_v1(k_base, region_bytes_v1()),
        region_is_admitted_v1(v_base, region_bytes_v1()),
        lane < lanes_v1(), slot < outputs_per_lane_v1(),
        causal_key_v1(lane_query_v1(lane), key), feature < dimension_v1(),
    ensures
        byte_end_v1(q_base, tensor_index_v1(lane_query_v1(lane), feature))
            <= q_base + region_bytes_v1(),
        byte_end_v1(k_base, tensor_index_v1(key, feature))
            <= k_base + region_bytes_v1(),
        byte_end_v1(v_base, tensor_index_v1(key, lane_column_v1(lane, slot)))
            <= v_base + region_bytes_v1(),
{
    qkv_reads_are_within_128_f32_v1(lane, slot, key, feature);
    admitted_element_access_is_inside_region_v1(
        q_base, region_bytes_v1(), tensor_index_v1(lane_query_v1(lane), feature));
    admitted_element_access_is_inside_region_v1(
        k_base, region_bytes_v1(), tensor_index_v1(key, feature));
    admitted_element_access_is_inside_region_v1(
        v_base, region_bytes_v1(), tensor_index_v1(key, lane_column_v1(lane, slot)));
}

pub proof fn assigned_output_write_is_inside_region_v1(
    output_base: nat, lane: nat, slot: nat,
)
    requires
        region_is_admitted_v1(output_base, region_bytes_v1()),
        lane < lanes_v1(), slot < outputs_per_lane_v1(),
    ensures
        lane_output_index_v1(lane, slot) < tensor_elements_v1(),
        byte_end_v1(output_base, lane_output_index_v1(lane, slot))
            <= output_base + region_bytes_v1(),
{
    lane_mapping_is_bounded_v1(lane, slot);
    admitted_element_access_is_inside_region_v1(
        output_base, region_bytes_v1(), lane_output_index_v1(lane, slot));
}

pub proof fn distinct_lane_slots_have_disjoint_output_writes_v1(
    left_lane: nat, left_slot: nat, right_lane: nat, right_slot: nat,
)
    requires
        left_lane < lanes_v1(), right_lane < lanes_v1(),
        left_slot < outputs_per_lane_v1(), right_slot < outputs_per_lane_v1(),
        left_lane != right_lane || left_slot != right_slot,
    ensures lane_output_index_v1(left_lane, left_slot)
        != lane_output_index_v1(right_lane, right_slot),
{
}

pub proof fn every_output_has_exact_lane_slot_owner_v1(output: nat)
    requires output < tensor_elements_v1(),
    ensures
        output / outputs_per_lane_v1() < lanes_v1(),
        output % outputs_per_lane_v1() < outputs_per_lane_v1(),
        lane_output_index_v1(
            output / outputs_per_lane_v1(), output % outputs_per_lane_v1(),
        ) == output,
{
}

pub open spec fn input_validation_phase_v1() -> nat { 0 }
pub open spec fn causal_recurrence_phase_v1() -> nat { 1 }
pub open spec fn output_commit_phase_v1() -> nat { 2 }

pub proof fn reads_precede_owned_output_commit_v1()
    ensures
        input_validation_phase_v1() < causal_recurrence_phase_v1(),
        causal_recurrence_phase_v1() < output_commit_phase_v1(),
{
}

pub proof fn exact_access_abi_v1()
    ensures global_address_space_v1() == 1, access_width_v1() == 4,
{
}

/// These false values keep all missing refinement and execution joins explicit.
pub open spec fn compiler_refinement_claimed_v1() -> bool { false }
pub open spec fn logical_address_refinement_claimed_v1() -> bool { false }
pub open spec fn isa_refinement_claimed_v1() -> bool { false }
pub open spec fn generalized_machine_memory_safety_claimed_v1() -> bool { false }
pub open spec fn generalized_gpu_race_freedom_claimed_v1() -> bool { false }
pub open spec fn gpu_execution_claimed_v1() -> bool { false }

pub proof fn assurance_boundary_is_conservative_v1()
    ensures
        !compiler_refinement_claimed_v1(),
        !logical_address_refinement_claimed_v1(),
        !isa_refinement_claimed_v1(),
        !generalized_machine_memory_safety_claimed_v1(),
        !generalized_gpu_race_freedom_claimed_v1(),
        !gpu_execution_claimed_v1(),
{
}

} // verus!
