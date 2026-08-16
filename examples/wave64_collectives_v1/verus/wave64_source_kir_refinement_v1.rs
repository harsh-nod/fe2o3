use vstd::prelude::*;

verus! {

/// Four big-endian words of one pinned SHA-256 identity.
pub struct Digest256V1 {
    pub word0: nat,
    pub word1: nat,
    pub word2: nat,
    pub word3: nat,
}

/// SHA-256 of the exact checked-in attributed `src/kernel.rs`.
pub open spec fn attributed_source_identity_v1() -> Digest256V1 {
    Digest256V1 {
        word0: 0x01ac1365b0fdfe91,
        word1: 0xcdc8f7cf6a14ae5a,
        word2: 0xcbea41528103ec3d,
        word3: 0xe5fe6d895261625e,
    }
}

/// SHA-256 of the exact checked-in Wave64 semantic Kernel-IR schema source.
pub open spec fn kernel_ir_schema_identity_v1() -> Digest256V1 {
    Digest256V1 {
        word0: 0x382fcf4c8733e55d,
        word1: 0xcacaf8b25691a270,
        word2: 0xa9adcf68912a679c,
        word3: 0x6ea848fee62f84be,
    }
}

pub open spec fn wave64_lanes_v1() -> nat { 64 }
pub open spec fn reduction_kind_v1() -> nat { 0 }
pub open spec fn inclusive_kind_v1() -> nat { 1 }
pub open spec fn exclusive_kind_v1() -> nat { 2 }

pub open spec fn power_of_two_v1(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 {
        1
    } else {
        2 * power_of_two_v1((exponent - 1) as nat)
    }
}

pub open spec fn mask_prefix_value_v1(active: Seq<bool>, end: nat) -> nat
    recommends end <= active.len(),
    decreases end,
{
    if end == 0 {
        0
    } else {
        let lane = (end - 1) as nat;
        mask_prefix_value_v1(active, lane)
            + if active[lane as int] { power_of_two_v1(lane) } else { 0 }
    }
}

/// Binds all logical activity predicates to one concrete `u64` value.
pub open spec fn explicit_wave64_mask_v1(active: Seq<bool>, mask_bits: u64) -> bool {
    active.len() == wave64_lanes_v1()
        && mask_bits as nat == mask_prefix_value_v1(active, wave64_lanes_v1())
}

/// Integer abstraction of finite integral binary32 inputs in `[-1024, 1024]`.
pub open spec fn finite_f32_value_model_v1(input: Seq<int>) -> bool {
    input.len() == wave64_lanes_v1()
        && forall |lane: int| 0 <= lane < wave64_lanes_v1() ==>
            -1024 <= #[trigger] input[lane] <= 1024
}

/// Exact source-model fields selected by the refinement relation.
pub struct SourceModelProfileV1 {
    pub source_identity: Digest256V1,
    pub lanes: nat,
    pub mask_bits: nat,
    pub finite_integral_f32_abs_bound: nat,
    pub output_allocations: nat,
}

/// Exact canonical semantic Kernel-IR fields selected by the relation.
pub struct KernelIrProfileV1 {
    pub schema_identity: Digest256V1,
    pub target_gfx942_xnack_minus: bool,
    pub code_object_version: nat,
    pub wave_width: nat,
    pub workgroup_width: nat,
    pub mask_bits: nat,
    pub finite_integral_f32_abs_bound: nat,
    pub ordered_collective0: nat,
    pub ordered_collective1: nat,
    pub ordered_collective2: nat,
    pub output_allocations: nat,
}

pub open spec fn exact_source_model_profile_v1() -> SourceModelProfileV1 {
    SourceModelProfileV1 {
        source_identity: attributed_source_identity_v1(),
        lanes: 64,
        mask_bits: 64,
        finite_integral_f32_abs_bound: 1024,
        output_allocations: 3,
    }
}

pub open spec fn exact_kernel_ir_profile_v1() -> KernelIrProfileV1 {
    KernelIrProfileV1 {
        schema_identity: kernel_ir_schema_identity_v1(),
        target_gfx942_xnack_minus: true,
        code_object_version: 6,
        wave_width: 64,
        workgroup_width: 64,
        mask_bits: 64,
        finite_integral_f32_abs_bound: 1024,
        ordered_collective0: reduction_kind_v1(),
        ordered_collective1: inclusive_kind_v1(),
        ordered_collective2: exclusive_kind_v1(),
        output_allocations: 3,
    }
}

/// Identity- and profile-bound source-model-to-semantic-KIR relation.
pub open spec fn exact_source_model_to_kernel_ir_profile_v1(
    source: SourceModelProfileV1,
    kir: KernelIrProfileV1,
) -> bool {
    &&& source.source_identity == attributed_source_identity_v1()
    &&& kir.schema_identity == kernel_ir_schema_identity_v1()
    &&& source.lanes == kir.wave_width
    &&& source.lanes == kir.workgroup_width
    &&& source.mask_bits == kir.mask_bits
    &&& source.finite_integral_f32_abs_bound == kir.finite_integral_f32_abs_bound
    &&& source.output_allocations == kir.output_allocations
    &&& kir.target_gfx942_xnack_minus
    &&& kir.code_object_version == 6
    &&& kir.ordered_collective0 == reduction_kind_v1()
    &&& kir.ordered_collective1 == inclusive_kind_v1()
    &&& kir.ordered_collective2 == exclusive_kind_v1()
}

pub proof fn exact_profiles_have_the_bound_identity_and_shape_v1()
    ensures exact_source_model_to_kernel_ir_profile_v1(
        exact_source_model_profile_v1(),
        exact_kernel_ir_profile_v1(),
    ),
{
}

/// Source-model contributor predicate before numerical summation.
pub open spec fn source_contributes_v1(
    active: Seq<bool>,
    output_lane: nat,
    contributor: nat,
    kind: nat,
) -> bool
    recommends output_lane < 64, contributor < 64, active.len() == 64,
{
    active[output_lane as int] && active[contributor as int]
        && if kind == reduction_kind_v1() {
            true
        } else if kind == inclusive_kind_v1() {
            contributor <= output_lane
        } else {
            contributor < output_lane
        }
}

/// Prefix end selected by each ordered canonical Kernel-IR collective.
pub open spec fn kernel_ir_prefix_end_v1(kind: nat, output_lane: nat) -> nat {
    if kind == reduction_kind_v1() {
        64
    } else if kind == inclusive_kind_v1() {
        output_lane + 1
    } else {
        output_lane
    }
}

/// Canonical semantic Kernel-IR contributor predicate.
pub open spec fn kernel_ir_contributes_v1(
    active: Seq<bool>,
    output_lane: nat,
    contributor: nat,
    kind: nat,
) -> bool
    recommends output_lane < 64, contributor < 64, active.len() == 64,
{
    active[output_lane as int] && active[contributor as int]
        && contributor < kernel_ir_prefix_end_v1(kind, output_lane)
}

/// An arbitrary lane/contributor pair proves all `2^64` masks through the
/// explicit activity sequence, rather than testing a finite mask sample.
pub proof fn source_and_kernel_ir_contributors_are_equal_v1(
    active: Seq<bool>,
    mask_bits: u64,
    output_lane: nat,
    contributor: nat,
    kind: nat,
)
    requires
        explicit_wave64_mask_v1(active, mask_bits),
        output_lane < wave64_lanes_v1(),
        contributor < wave64_lanes_v1(),
        kind == reduction_kind_v1()
            || kind == inclusive_kind_v1()
            || kind == exclusive_kind_v1(),
    ensures source_contributes_v1(active, output_lane, contributor, kind)
        == kernel_ir_contributes_v1(active, output_lane, contributor, kind),
{
    if kind == reduction_kind_v1() {
        assert(contributor < kernel_ir_prefix_end_v1(kind, output_lane));
    } else if kind == inclusive_kind_v1() {
        assert((contributor <= output_lane) == (contributor < output_lane + 1));
    }
}

pub open spec fn source_owner_v1(lane: nat) -> nat { lane }
pub open spec fn kernel_ir_owner_v1(lane: nat) -> nat { lane }

pub proof fn source_and_kernel_ir_ownership_is_identical_and_injective_v1(
    left: nat,
    right: nat,
)
    requires left < wave64_lanes_v1(), right < wave64_lanes_v1(), left != right,
    ensures
        source_owner_v1(left) == kernel_ir_owner_v1(left),
        source_owner_v1(right) == kernel_ir_owner_v1(right),
        source_owner_v1(left) != source_owner_v1(right),
        kernel_ir_owner_v1(left) != kernel_ir_owner_v1(right),
{
}

pub open spec fn source_prefix_value_v1(
    input: Seq<int>,
    active: Seq<bool>,
    output_lane: nat,
    kind: nat,
    end: nat,
) -> int
    recommends end <= 64, input.len() == 64, active.len() == 64,
    decreases end,
{
    if end == 0 {
        0
    } else {
        let contributor = (end - 1) as nat;
        source_prefix_value_v1(input, active, output_lane, kind, contributor)
            + if source_contributes_v1(active, output_lane, contributor, kind) {
                input[contributor as int]
            } else {
                0
            }
    }
}

pub open spec fn kernel_ir_prefix_value_v1(
    input: Seq<int>,
    active: Seq<bool>,
    output_lane: nat,
    kind: nat,
    end: nat,
) -> int
    recommends end <= 64, input.len() == 64, active.len() == 64,
    decreases end,
{
    if end == 0 {
        0
    } else {
        let contributor = (end - 1) as nat;
        kernel_ir_prefix_value_v1(input, active, output_lane, kind, contributor)
            + if kernel_ir_contributes_v1(active, output_lane, contributor, kind) {
                input[contributor as int]
            } else {
                0
            }
    }
}

pub proof fn source_and_kernel_ir_prefix_values_are_equal_v1(
    input: Seq<int>,
    active: Seq<bool>,
    mask_bits: u64,
    output_lane: nat,
    kind: nat,
    end: nat,
)
    requires
        finite_f32_value_model_v1(input),
        explicit_wave64_mask_v1(active, mask_bits),
        output_lane < wave64_lanes_v1(),
        kind == reduction_kind_v1()
            || kind == inclusive_kind_v1()
            || kind == exclusive_kind_v1(),
        end <= wave64_lanes_v1(),
    ensures source_prefix_value_v1(input, active, output_lane, kind, end)
        == kernel_ir_prefix_value_v1(input, active, output_lane, kind, end),
    decreases end,
{
    if end > 0 {
        let contributor = (end - 1) as nat;
        source_and_kernel_ir_prefix_values_are_equal_v1(
            input, active, mask_bits, output_lane, kind, contributor,
        );
        source_and_kernel_ir_contributors_are_equal_v1(
            active, mask_bits, output_lane, contributor, kind,
        );
    }
}

pub proof fn exact_masked_reduction_and_scans_refine_semantic_kernel_ir_v1(
    input: Seq<int>,
    active: Seq<bool>,
    mask_bits: u64,
    output_lane: nat,
    kind: nat,
)
    requires
        finite_f32_value_model_v1(input),
        explicit_wave64_mask_v1(active, mask_bits),
        output_lane < wave64_lanes_v1(),
        kind == reduction_kind_v1()
            || kind == inclusive_kind_v1()
            || kind == exclusive_kind_v1(),
    ensures
        exact_source_model_to_kernel_ir_profile_v1(
            exact_source_model_profile_v1(),
            exact_kernel_ir_profile_v1(),
        ),
        source_prefix_value_v1(input, active, output_lane, kind, 64)
            == kernel_ir_prefix_value_v1(input, active, output_lane, kind, 64),
        source_owner_v1(output_lane) == kernel_ir_owner_v1(output_lane),
{
    exact_profiles_have_the_bound_identity_and_shape_v1();
    source_and_kernel_ir_prefix_values_are_equal_v1(
        input, active, mask_bits, output_lane, kind, 64,
    );
}

/// Missing joins remain explicit and false in the formal evidence.
pub open spec fn source_to_model_refinement_claimed_v1() -> bool { false }
pub open spec fn compiler_causality_claimed_v1() -> bool { false }
pub open spec fn llvm_isa_refinement_claimed_v1() -> bool { false }
pub open spec fn protected_execution_authority_claimed_v1() -> bool { false }
pub open spec fn generalized_safety_claimed_v1() -> bool { false }
pub open spec fn parity_promotion_claimed_v1() -> bool { false }

pub proof fn refinement_boundary_grants_no_adjacent_authority_v1()
    ensures
        !source_to_model_refinement_claimed_v1(),
        !compiler_causality_claimed_v1(),
        !llvm_isa_refinement_claimed_v1(),
        !protected_execution_authority_claimed_v1(),
        !generalized_safety_claimed_v1(),
        !parity_promotion_claimed_v1(),
{
}

} // verus!
