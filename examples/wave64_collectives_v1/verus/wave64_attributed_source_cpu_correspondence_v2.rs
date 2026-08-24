use vstd::prelude::*;

verus! {

/// Four big-endian words of one SHA-256 content identity.
pub struct Digest256V2 {
    pub word0: nat,
    pub word1: nat,
    pub word2: nat,
    pub word3: nat,
}

/// Five big-endian words of the reviewed public-base Git commit identity.
pub struct GitCommit160V2 {
    pub word0: nat,
    pub word1: nat,
    pub word2: nat,
    pub word3: nat,
    pub word4: nat,
}

pub open spec fn attributed_source_identity_v2() -> Digest256V2 {
    Digest256V2 {
        word0: 0x7c6ead1e7c01a61a,
        word1: 0x8f31a010c9e8cb9b,
        word2: 0xd1c21a905ba61e9d,
        word3: 0x90c6c077c748ffd4,
    }
}

pub open spec fn cpu_oracle_identity_v2() -> Digest256V2 {
    Digest256V2 {
        word0: 0x837aae894e5c04da,
        word1: 0x4b598e45f344f2e5,
        word2: 0xdf0aa8bc6155acf0,
        word3: 0xbf05809ecd86d407,
    }
}

pub open spec fn reviewed_correspondence_identity_v2() -> Digest256V2 {
    Digest256V2 {
        word0: 0xd1c8630a5e534fe5,
        word1: 0x59db0b669ca55a6f,
        word2: 0x9dda5454a50d57fe,
        word3: 0xb67eb3b969941e87,
    }
}

pub open spec fn reviewed_outer_public_base_commit_v2() -> GitCommit160V2 {
    GitCommit160V2 {
        word0: 0xb8daeb2b,
        word1: 0xc953924a,
        word2: 0x42454282,
        word3: 0x0bed566e,
        word4: 0x52d57290,
    }
}

pub open spec fn wave64_lanes_v2() -> nat { 64 }
pub open spec fn reduction_kind_v2() -> nat { 0 }
pub open spec fn inclusive_kind_v2() -> nat { 1 }
pub open spec fn exclusive_kind_v2() -> nat { 2 }

pub struct ReviewedAttributedSourceV2 {
    pub source_identity: Digest256V2,
    pub correspondence_identity: Digest256V2,
    pub outer_public_base: GitCommit160V2,
    pub lanes: nat,
    pub mask_bits: nat,
    pub increasing_physical_lane_order: bool,
    pub inactive_contribution_is_zero: bool,
    pub inactive_publication_is_zero: bool,
    pub same_lane_output_owner: bool,
}

pub struct CpuSourceModelV2 {
    pub oracle_identity: Digest256V2,
    pub correspondence_identity: Digest256V2,
    pub outer_public_base: GitCommit160V2,
    pub lanes: nat,
    pub mask_bits: nat,
    pub increasing_physical_lane_order: bool,
    pub inactive_contribution_is_zero: bool,
    pub inactive_publication_is_zero: bool,
    pub same_lane_output_owner: bool,
}

pub open spec fn exact_reviewed_attributed_source_v2() -> ReviewedAttributedSourceV2 {
    ReviewedAttributedSourceV2 {
        source_identity: attributed_source_identity_v2(),
        correspondence_identity: reviewed_correspondence_identity_v2(),
        outer_public_base: reviewed_outer_public_base_commit_v2(),
        lanes: 64,
        mask_bits: 64,
        increasing_physical_lane_order: true,
        inactive_contribution_is_zero: true,
        inactive_publication_is_zero: true,
        same_lane_output_owner: true,
    }
}

pub open spec fn exact_cpu_source_model_v2() -> CpuSourceModelV2 {
    CpuSourceModelV2 {
        oracle_identity: cpu_oracle_identity_v2(),
        correspondence_identity: reviewed_correspondence_identity_v2(),
        outer_public_base: reviewed_outer_public_base_commit_v2(),
        lanes: 64,
        mask_bits: 64,
        increasing_physical_lane_order: true,
        inactive_contribution_is_zero: true,
        inactive_publication_is_zero: true,
        same_lane_output_owner: true,
    }
}

/// Identity- and profile-bound reviewed structural relation. The outer commit
/// is selected data; this predicate does not prove Git-tree membership.
pub open spec fn reviewed_source_to_cpu_profile_v2(
    source: ReviewedAttributedSourceV2,
    cpu: CpuSourceModelV2,
) -> bool {
    &&& source.source_identity == attributed_source_identity_v2()
    &&& cpu.oracle_identity == cpu_oracle_identity_v2()
    &&& source.correspondence_identity == reviewed_correspondence_identity_v2()
    &&& cpu.correspondence_identity == reviewed_correspondence_identity_v2()
    &&& source.outer_public_base == reviewed_outer_public_base_commit_v2()
    &&& cpu.outer_public_base == reviewed_outer_public_base_commit_v2()
    &&& source.lanes == cpu.lanes == wave64_lanes_v2()
    &&& source.mask_bits == cpu.mask_bits == 64
    &&& source.increasing_physical_lane_order
    &&& cpu.increasing_physical_lane_order
    &&& source.inactive_contribution_is_zero
    &&& cpu.inactive_contribution_is_zero
    &&& source.inactive_publication_is_zero
    &&& cpu.inactive_publication_is_zero
    &&& source.same_lane_output_owner
    &&& cpu.same_lane_output_owner
}

pub proof fn exact_reviewed_profiles_bind_source_oracle_correspondence_and_outer_commit_v2()
    ensures reviewed_source_to_cpu_profile_v2(
        exact_reviewed_attributed_source_v2(),
        exact_cpu_source_model_v2(),
    ),
{
}

pub open spec fn power_of_two_v2(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 { 1 } else { 2 * power_of_two_v2((exponent - 1) as nat) }
}

pub open spec fn mask_prefix_value_v2(active: Seq<bool>, end: nat) -> nat
    recommends end <= active.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        let lane = (end - 1) as nat;
        mask_prefix_value_v2(active, lane)
            + if active[lane as int] { power_of_two_v2(lane) } else { 0 }
    }
}

/// Exact source spelling `active_mask & (1_u64 << lane) != 0`, abstracted as
/// a sequence and bound to all 64 bits of one concrete mask.
pub open spec fn attributed_source_mask_selection_v2(active: Seq<bool>, mask_bits: u64) -> bool {
    active.len() == wave64_lanes_v2()
        && mask_bits as nat == mask_prefix_value_v2(active, wave64_lanes_v2())
}

pub open spec fn cpu_oracle_mask_selection_v2(active: Seq<bool>, mask_bits: u64) -> bool {
    active.len() == wave64_lanes_v2()
        && mask_bits as nat == mask_prefix_value_v2(active, wave64_lanes_v2())
}

pub proof fn source_and_cpu_select_the_same_active_mask_v2(
    active: Seq<bool>,
    mask_bits: u64,
)
    ensures attributed_source_mask_selection_v2(active, mask_bits)
        == cpu_oracle_mask_selection_v2(active, mask_bits),
{
}

pub open spec fn exact_finite_integral_f32_corpus_v2(input: Seq<int>) -> bool {
    input.len() == wave64_lanes_v2()
        && forall |lane: int| 0 <= lane < wave64_lanes_v2() ==>
            -1024 <= #[trigger] input[lane] <= 1024
}

pub open spec fn attributed_source_contribution_v2(
    input: Seq<int>, active: Seq<bool>, lane: nat,
) -> int
    recommends input.len() == 64, active.len() == 64, lane < 64,
{
    if active[lane as int] { input[lane as int] } else { 0 }
}

pub open spec fn cpu_oracle_contribution_v2(
    input: Seq<int>, active: Seq<bool>, lane: nat,
) -> int
    recommends input.len() == 64, active.len() == 64, lane < 64,
{
    if active[lane as int] { input[lane as int] } else { 0 }
}

pub proof fn inactive_and_active_contributions_match_v2(
    input: Seq<int>, active: Seq<bool>, lane: nat,
)
    requires exact_finite_integral_f32_corpus_v2(input), active.len() == 64, lane < 64,
    ensures attributed_source_contribution_v2(input, active, lane)
        == cpu_oracle_contribution_v2(input, active, lane),
{
}

pub open spec fn attributed_source_prefix_end_v2(kind: nat, output_lane: nat) -> nat {
    if kind == reduction_kind_v2() {
        64
    } else if kind == inclusive_kind_v2() {
        output_lane + 1
    } else {
        output_lane
    }
}

pub open spec fn cpu_oracle_prefix_end_v2(kind: nat, output_lane: nat) -> nat {
    if kind == reduction_kind_v2() {
        64
    } else if kind == inclusive_kind_v2() {
        output_lane + 1
    } else {
        output_lane
    }
}

pub proof fn source_and_cpu_choose_same_physical_lane_prefix_v2(kind: nat, output_lane: nat)
    requires output_lane < 64,
        kind == reduction_kind_v2() || kind == inclusive_kind_v2() || kind == exclusive_kind_v2(),
    ensures attributed_source_prefix_end_v2(kind, output_lane)
        == cpu_oracle_prefix_end_v2(kind, output_lane),
{
}

pub open spec fn attributed_source_recurrence_v2(
    input: Seq<int>, active: Seq<bool>, end: nat,
) -> int
    recommends input.len() == 64, active.len() == 64, end <= 64,
    decreases end,
{
    if end == 0 {
        0
    } else {
        let lane = (end - 1) as nat;
        attributed_source_recurrence_v2(input, active, lane)
            + attributed_source_contribution_v2(input, active, lane)
    }
}

pub open spec fn cpu_oracle_recurrence_v2(
    input: Seq<int>, active: Seq<bool>, end: nat,
) -> int
    recommends input.len() == 64, active.len() == 64, end <= 64,
    decreases end,
{
    if end == 0 {
        0
    } else {
        let lane = (end - 1) as nat;
        cpu_oracle_recurrence_v2(input, active, lane)
            + cpu_oracle_contribution_v2(input, active, lane)
    }
}

pub proof fn increasing_lane_recurrences_are_equal_v2(
    input: Seq<int>, active: Seq<bool>, end: nat,
)
    requires exact_finite_integral_f32_corpus_v2(input), active.len() == 64, end <= 64,
    ensures attributed_source_recurrence_v2(input, active, end)
        == cpu_oracle_recurrence_v2(input, active, end),
    decreases end,
{
    if end > 0 {
        let lane = (end - 1) as nat;
        increasing_lane_recurrences_are_equal_v2(input, active, lane);
        inactive_and_active_contributions_match_v2(input, active, lane);
    }
}

pub open spec fn attributed_source_publication_v2(
    input: Seq<int>, active: Seq<bool>, output_lane: nat, kind: nat,
) -> int
    recommends input.len() == 64, active.len() == 64, output_lane < 64,
{
    if active[output_lane as int] {
        attributed_source_recurrence_v2(
            input, active, attributed_source_prefix_end_v2(kind, output_lane),
        )
    } else {
        0
    }
}

pub open spec fn cpu_oracle_publication_v2(
    input: Seq<int>, active: Seq<bool>, output_lane: nat, kind: nat,
) -> int
    recommends input.len() == 64, active.len() == 64, output_lane < 64,
{
    if active[output_lane as int] {
        cpu_oracle_recurrence_v2(
            input, active, cpu_oracle_prefix_end_v2(kind, output_lane),
        )
    } else {
        0
    }
}

pub proof fn reduction_inclusive_exclusive_and_inactive_publications_match_v2(
    input: Seq<int>, active: Seq<bool>, mask_bits: u64, output_lane: nat, kind: nat,
)
    requires
        exact_finite_integral_f32_corpus_v2(input),
        attributed_source_mask_selection_v2(active, mask_bits),
        output_lane < 64,
        kind == reduction_kind_v2() || kind == inclusive_kind_v2() || kind == exclusive_kind_v2(),
    ensures attributed_source_publication_v2(input, active, output_lane, kind)
        == cpu_oracle_publication_v2(input, active, output_lane, kind),
{
    source_and_cpu_choose_same_physical_lane_prefix_v2(kind, output_lane);
    increasing_lane_recurrences_are_equal_v2(
        input, active, attributed_source_prefix_end_v2(kind, output_lane),
    );
}

pub open spec fn attributed_source_owner_v2(lane: nat) -> nat { lane }
pub open spec fn cpu_oracle_owner_v2(lane: nat) -> nat { lane }

pub proof fn source_and_cpu_same_lane_ownership_is_equal_and_injective_v2(
    left: nat, right: nat,
)
    requires left < 64, right < 64, left != right,
    ensures
        attributed_source_owner_v2(left) == cpu_oracle_owner_v2(left),
        attributed_source_owner_v2(right) == cpu_oracle_owner_v2(right),
        attributed_source_owner_v2(left) != attributed_source_owner_v2(right),
        cpu_oracle_owner_v2(left) != cpu_oracle_owner_v2(right),
{
}

/// Top-level reviewed correspondence for one arbitrary mask, output lane, and
/// collective kind. Rust-side syn admission is outside this theorem.
pub proof fn exact_attributed_source_algorithm_corresponds_to_cpu_oracle_v2(
    input: Seq<int>, active: Seq<bool>, mask_bits: u64, output_lane: nat, kind: nat,
)
    requires
        exact_finite_integral_f32_corpus_v2(input),
        attributed_source_mask_selection_v2(active, mask_bits),
        output_lane < 64,
        kind == reduction_kind_v2() || kind == inclusive_kind_v2() || kind == exclusive_kind_v2(),
    ensures
        reviewed_source_to_cpu_profile_v2(
            exact_reviewed_attributed_source_v2(), exact_cpu_source_model_v2(),
        ),
        attributed_source_publication_v2(input, active, output_lane, kind)
            == cpu_oracle_publication_v2(input, active, output_lane, kind),
        attributed_source_owner_v2(output_lane) == cpu_oracle_owner_v2(output_lane),
{
    exact_reviewed_profiles_bind_source_oracle_correspondence_and_outer_commit_v2();
    reduction_inclusive_exclusive_and_inactive_publications_match_v2(
        input, active, mask_bits, output_lane, kind,
    );
}

/// Missing semantic and machine joins remain explicit and false.
pub open spec fn proves_source_to_model_refinement_v2() -> bool { false }
pub open spec fn proves_outer_commit_membership_v2() -> bool { false }
pub open spec fn proves_mir_or_compiler_causality_v2() -> bool { false }
pub open spec fn proves_kir_llvm_isa_or_gpu_v2() -> bool { false }
pub open spec fn proves_generalized_memory_or_race_safety_v2() -> bool { false }
pub open spec fn grants_parity_promotion_v2() -> bool { false }

pub proof fn reviewed_correspondence_grants_no_adjacent_authority_v2()
    ensures
        !proves_source_to_model_refinement_v2(),
        !proves_outer_commit_membership_v2(),
        !proves_mir_or_compiler_causality_v2(),
        !proves_kir_llvm_isa_or_gpu_v2(),
        !proves_generalized_memory_or_race_safety_v2(),
        !grants_parity_promotion_v2(),
{
}

} // verus!
