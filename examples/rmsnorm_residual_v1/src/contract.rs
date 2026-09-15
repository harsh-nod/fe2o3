//! Exact B3 shape, numerical, effect, and resource contracts.

/// Exact gfx942 processor name.
pub const GFX942_PROCESSOR_V1: &str = "gfx942";
/// Exact target-feature profile.
pub const GFX942_TARGET_FEATURES_V1: &str = "+wavefrontsize64,-xnack";
/// Qwen3 RMSNorm epsilon as exact `f32` bits (`1.0e-6_f32`).
pub const QWEN3_RMSNORM_EPSILON_BITS_V1: u32 = 0x3586_37bd;
/// One Wave64 owns each flattened row.
pub const RMSNORM_WAVE_LANES_V1: usize = 64;
/// Fixed wave reduction stages, in evaluation order.
pub const RMSNORM_REDUCTION_STRIDES_V1: [u8; 6] = [32, 16, 8, 4, 2, 1];
/// Largest flattened row count in the exact B3 matrix.
pub const MAX_B3_RMS_ROWS_V1: usize = 2_048;
/// Largest hidden width in the exact B3 matrix.
pub const MAX_B3_HIDDEN_SIZE_V1: usize = 4_096;
/// Largest element count in one exact B3 operator invocation.
pub const MAX_B3_RMS_ELEMENTS_V1: usize = MAX_B3_RMS_ROWS_V1 * MAX_B3_HIDDEN_SIZE_V1;

/// Exact Qwen3 role and hidden width.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3ModelRoleV1 {
    /// Qwen3-8B target with hidden width 4096.
    Target8B = 1,
    /// Qwen3-0.6B draft with hidden width 1024.
    Draft06B = 2,
}

impl Qwen3ModelRoleV1 {
    /// Returns the sole hidden width admitted for this role.
    pub const fn hidden_size(self) -> usize {
        match self {
            Self::Target8B => 4_096,
            Self::Draft06B => 1_024,
        }
    }

    /// Returns the canonical identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Closed M1 B3 workload bucket matrix.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum B3RmsNormBucketV1 {
    /// Prefill: one sequence with 128 active tokens.
    PrefillS1T128 = 1,
    /// Prefill: eight sequences with 128 active tokens each.
    PrefillS8T128 = 2,
    /// Prefill: one sequence with 512 active tokens.
    PrefillS1T512 = 3,
    /// Prefill: one sequence with 2048 active tokens.
    PrefillS1T2048 = 4,
    /// Decode: one sequence with one active token.
    DecodeS1 = 5,
    /// Decode: eight sequences with one active token each.
    DecodeS8 = 6,
    /// Decode: 32 sequences with one active token each.
    DecodeS32 = 7,
    /// Speculative decoding: one sequence and K=4.
    SpeculativeS1K4 = 8,
    /// Speculative decoding: eight sequences and K=4.
    SpeculativeS8K4 = 9,
    /// Speculative decoding: one sequence and K=8.
    SpeculativeS1K8 = 10,
    /// Speculative decoding: one sequence and K=16.
    SpeculativeS1K16 = 11,
}

/// All and only admitted B3 buckets, in stable identity order.
pub const B3_RMSNORM_BUCKETS_V1: [B3RmsNormBucketV1; 11] = [
    B3RmsNormBucketV1::PrefillS1T128,
    B3RmsNormBucketV1::PrefillS8T128,
    B3RmsNormBucketV1::PrefillS1T512,
    B3RmsNormBucketV1::PrefillS1T2048,
    B3RmsNormBucketV1::DecodeS1,
    B3RmsNormBucketV1::DecodeS8,
    B3RmsNormBucketV1::DecodeS32,
    B3RmsNormBucketV1::SpeculativeS1K4,
    B3RmsNormBucketV1::SpeculativeS8K4,
    B3RmsNormBucketV1::SpeculativeS1K8,
    B3RmsNormBucketV1::SpeculativeS1K16,
];

impl B3RmsNormBucketV1 {
    /// Returns the exact number of sequences in this bucket.
    pub const fn sequences(self) -> usize {
        match self {
            Self::PrefillS8T128 | Self::DecodeS8 | Self::SpeculativeS8K4 => 8,
            Self::DecodeS32 => 32,
            _ => 1,
        }
    }

    /// Returns the exact active-token count per sequence for the model role.
    pub const fn active_tokens(self, role: Qwen3ModelRoleV1) -> usize {
        match self {
            Self::PrefillS1T128 | Self::PrefillS8T128 => 128,
            Self::PrefillS1T512 => 512,
            Self::PrefillS1T2048 => 2_048,
            Self::DecodeS1 | Self::DecodeS8 | Self::DecodeS32 => 1,
            Self::SpeculativeS1K4 | Self::SpeculativeS8K4 => match role {
                Qwen3ModelRoleV1::Target8B => 5,
                Qwen3ModelRoleV1::Draft06B => 4,
            },
            Self::SpeculativeS1K8 => match role {
                Qwen3ModelRoleV1::Target8B => 9,
                Qwen3ModelRoleV1::Draft06B => 8,
            },
            Self::SpeculativeS1K16 => match role {
                Qwen3ModelRoleV1::Target8B => 17,
                Qwen3ModelRoleV1::Draft06B => 16,
            },
        }
    }

    /// Returns the exact flattened row count for the role and bucket.
    pub const fn rows(self, role: Qwen3ModelRoleV1) -> usize {
        self.sequences() * self.active_tokens(role)
    }

    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Public inert profile record accepted only after exact validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RmsNormProfileDescriptorV1 {
    /// Exact Qwen3 role.
    pub role: Qwen3ModelRoleV1,
    /// Exact B3 workload bucket.
    pub bucket: B3RmsNormBucketV1,
    /// Number of independent sequences.
    pub sequences: usize,
    /// Number of active tokens per sequence.
    pub active_tokens: usize,
    /// Flattened row count (`sequences * active_tokens`).
    pub rows: usize,
    /// Qwen3 hidden width.
    pub hidden_size: usize,
}

impl RmsNormProfileDescriptorV1 {
    /// Constructs the canonical record for one role and closed B3 bucket.
    pub const fn canonical(role: Qwen3ModelRoleV1, bucket: B3RmsNormBucketV1) -> Self {
        Self {
            role,
            bucket,
            sequences: bucket.sequences(),
            active_tokens: bucket.active_tokens(role),
            rows: bucket.rows(role),
            hidden_size: role.hidden_size(),
        }
    }
}

/// Exact profile validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RmsNormProfileErrorV1 {
    /// Sequence count did not match the named B3 bucket.
    SequenceCount,
    /// Active-token count did not match the bucket and role.
    ActiveTokenCount,
    /// Flattened row count did not match the checked product.
    FlattenedRows,
    /// Hidden size did not match the Qwen3 role.
    HiddenSize,
    /// Checked resource arithmetic overflowed.
    ResourceArithmeticOverflow,
    /// A derived resource exceeded the bounded B3 ceiling.
    ResourceLimit,
}

/// Checked resource envelope for one structural candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RmsNormResourceContractV1 {
    /// Flattened row count and exact workgroup count.
    pub workgroups: usize,
    /// Exact number of Wave64 waves.
    pub waves: usize,
    /// Total activation elements per row-major activation buffer.
    pub activation_elements: usize,
    /// Total global bytes read by activation, residual, and shared weight.
    pub global_read_bytes: usize,
    /// Total global bytes written by normalized and residual outputs.
    pub global_write_bytes: usize,
    /// Logical temporary payload used by the transactional host reference.
    pub host_scratch_bytes: usize,
    /// Structural schedule uses no LDS allocation.
    pub lds_bytes_per_workgroup: usize,
    /// Exact threads per workgroup.
    pub threads_per_workgroup: usize,
}

fn checked_resource_contract(
    descriptor: RmsNormProfileDescriptorV1,
) -> Result<RmsNormResourceContractV1, RmsNormProfileErrorV1> {
    let elements = descriptor
        .rows
        .checked_mul(descriptor.hidden_size)
        .ok_or(RmsNormProfileErrorV1::ResourceArithmeticOverflow)?;
    if descriptor.rows > MAX_B3_RMS_ROWS_V1 || elements > MAX_B3_RMS_ELEMENTS_V1 {
        return Err(RmsNormProfileErrorV1::ResourceLimit);
    }
    let activation_bytes = elements
        .checked_mul(2)
        .ok_or(RmsNormProfileErrorV1::ResourceArithmeticOverflow)?;
    let global_read_bytes = activation_bytes
        .checked_mul(3)
        .ok_or(RmsNormProfileErrorV1::ResourceArithmeticOverflow)?;
    let global_write_bytes = activation_bytes
        .checked_mul(2)
        .ok_or(RmsNormProfileErrorV1::ResourceArithmeticOverflow)?;
    let host_scratch_bytes = activation_bytes
        .checked_mul(2)
        .and_then(|bytes| {
            descriptor
                .hidden_size
                .checked_mul(4)
                .and_then(|row_bytes| bytes.checked_add(row_bytes))
        })
        .ok_or(RmsNormProfileErrorV1::ResourceArithmeticOverflow)?;
    Ok(RmsNormResourceContractV1 {
        workgroups: descriptor.rows,
        waves: descriptor.rows,
        activation_elements: elements,
        global_read_bytes,
        global_write_bytes,
        host_scratch_bytes,
        lds_bytes_per_workgroup: 0,
        threads_per_workgroup: RMSNORM_WAVE_LANES_V1,
    })
}

/// Validated inert exact-profile value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedRmsNormProfileV1 {
    descriptor: RmsNormProfileDescriptorV1,
    resources: RmsNormResourceContractV1,
}

impl ValidatedRmsNormProfileV1 {
    /// Returns the exact validated profile record.
    pub const fn descriptor(self) -> RmsNormProfileDescriptorV1 {
        self.descriptor
    }

    /// Returns the checked structural resource envelope.
    pub const fn resources(self) -> RmsNormResourceContractV1 {
        self.resources
    }
}

/// Validates all axes of one exact role-and-bucket profile.
pub fn validate_rmsnorm_profile_v1(
    descriptor: RmsNormProfileDescriptorV1,
) -> Result<ValidatedRmsNormProfileV1, RmsNormProfileErrorV1> {
    let expected = RmsNormProfileDescriptorV1::canonical(descriptor.role, descriptor.bucket);
    if descriptor.sequences != expected.sequences {
        return Err(RmsNormProfileErrorV1::SequenceCount);
    }
    if descriptor.active_tokens != expected.active_tokens {
        return Err(RmsNormProfileErrorV1::ActiveTokenCount);
    }
    if descriptor.rows != expected.rows {
        return Err(RmsNormProfileErrorV1::FlattenedRows);
    }
    if descriptor.hidden_size != expected.hidden_size {
        return Err(RmsNormProfileErrorV1::HiddenSize);
    }
    let resources = checked_resource_contract(descriptor)?;
    Ok(ValidatedRmsNormProfileV1 {
        descriptor,
        resources,
    })
}

/// Residual-add evaluation rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ResidualAddPolicyV1 {
    /// Decode each BF16 operand exactly and add once in `f32`.
    Bf16OperandsFp32Add = 1,
    /// Deliberately unsupported BF16 addition, retained for hostile tests.
    Bf16Add = 2,
}

/// Sum-of-squares evaluation order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SquareReductionPolicyV1 {
    /// Per-lane ascending stride-64 FP32 sums and fixed halving Wave64 tree.
    Wave64StrideAscendingFixedTreeFp32 = 1,
    /// Deliberately unsupported sequential reduction.
    SequentialFp32 = 2,
}

/// Reciprocal-root evaluation rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ReciprocalRootPolicyV1 {
    /// FP32 mean, epsilon add, square root, then reciprocal.
    Fp32MeanEpsilonSqrtReciprocal = 1,
    /// Deliberately unsupported approximate reciprocal square root.
    ApproximateRsqrt = 2,
}

/// Normalized output multiplication order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ScaleOrderPolicyV1 {
    /// `(residual_sum * reciprocal_rms) * weight`, left-associated in FP32.
    ResidualThenReciprocalThenWeight = 1,
    /// Deliberately unsupported weight-first order.
    WeightFirst = 2,
}

/// Output storage conversion rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum OutputCastPolicyV1 {
    /// BF16 round-to-nearest, ties-to-even after finite FP32 evaluation.
    Bf16RoundToNearestTiesEven = 1,
    /// Deliberately unsupported truncating cast.
    Bf16Truncate = 2,
}

/// Complete numerical record. It describes evaluation order, not machine proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RmsNormNumericalPolicyV1 {
    /// Exact epsilon bits.
    pub epsilon_bits: u32,
    /// Residual-add rule.
    pub residual_add: ResidualAddPolicyV1,
    /// Square-reduction rule.
    pub square_reduction: SquareReductionPolicyV1,
    /// Reciprocal-root rule.
    pub reciprocal_root: ReciprocalRootPolicyV1,
    /// Normalized-output multiplication order.
    pub scale_order: ScaleOrderPolicyV1,
    /// Both outputs' storage conversion.
    pub output_cast: OutputCastPolicyV1,
    /// Whether every physical input and intermediate must be finite.
    pub reject_non_finite: bool,
}

impl RmsNormNumericalPolicyV1 {
    /// Returns the only admitted numerical policy.
    pub const fn exact() -> Self {
        Self {
            epsilon_bits: QWEN3_RMSNORM_EPSILON_BITS_V1,
            residual_add: ResidualAddPolicyV1::Bf16OperandsFp32Add,
            square_reduction: SquareReductionPolicyV1::Wave64StrideAscendingFixedTreeFp32,
            reciprocal_root: ReciprocalRootPolicyV1::Fp32MeanEpsilonSqrtReciprocal,
            scale_order: ScaleOrderPolicyV1::ResidualThenReciprocalThenWeight,
            output_cast: OutputCastPolicyV1::Bf16RoundToNearestTiesEven,
            reject_non_finite: true,
        }
    }
}

/// Numerical-policy mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericalPolicyErrorV1 {
    /// Epsilon differed from the exact Qwen3 value.
    Epsilon,
    /// Residual-add rule differed.
    ResidualAdd,
    /// Square-reduction order differed.
    SquareReduction,
    /// Reciprocal-root rule differed.
    ReciprocalRoot,
    /// Scale order differed.
    ScaleOrder,
    /// Output cast differed.
    OutputCast,
    /// Non-finite inputs or intermediates were not rejected.
    NonFinitePolicy,
}

/// Validates every independent numerical-policy axis.
pub fn validate_numerical_policy_v1(
    policy: RmsNormNumericalPolicyV1,
) -> Result<(), NumericalPolicyErrorV1> {
    let exact = RmsNormNumericalPolicyV1::exact();
    if policy.epsilon_bits != exact.epsilon_bits {
        return Err(NumericalPolicyErrorV1::Epsilon);
    }
    if policy.residual_add != exact.residual_add {
        return Err(NumericalPolicyErrorV1::ResidualAdd);
    }
    if policy.square_reduction != exact.square_reduction {
        return Err(NumericalPolicyErrorV1::SquareReduction);
    }
    if policy.reciprocal_root != exact.reciprocal_root {
        return Err(NumericalPolicyErrorV1::ReciprocalRoot);
    }
    if policy.scale_order != exact.scale_order {
        return Err(NumericalPolicyErrorV1::ScaleOrder);
    }
    if policy.output_cast != exact.output_cast {
        return Err(NumericalPolicyErrorV1::OutputCast);
    }
    if !policy.reject_non_finite {
        return Err(NumericalPolicyErrorV1::NonFinitePolicy);
    }
    Ok(())
}

/// Explicit memory, initialization, alias, race, and publication contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RmsNormEffectContractV1 {
    /// Activation, residual, and weight are read-only initialized BF16.
    pub initialized_read_buffers: u8,
    /// Normalized and residual results are write-only BF16 outputs.
    pub write_buffers: u8,
    /// Read/read aliasing is permitted.
    pub read_only_inputs_may_alias: bool,
    /// Each output must be disjoint from every other buffer.
    pub writable_outputs_are_disjoint: bool,
    /// Every output element has exactly one row/lane owner.
    pub output_mapping_is_total_and_injective: bool,
    /// Every lane executes the same six reduction collectives.
    pub wave_collectives_are_convergent: bool,
    /// Results publish only after complete validation and evaluation.
    pub output_commit_is_transactional: bool,
    /// Every address is derived by checked row-major indexing.
    pub accesses_are_bounded: bool,
}

impl RmsNormEffectContractV1 {
    /// Returns the only admitted effect contract.
    pub const fn exact() -> Self {
        Self {
            initialized_read_buffers: 3,
            write_buffers: 2,
            read_only_inputs_may_alias: true,
            writable_outputs_are_disjoint: true,
            output_mapping_is_total_and_injective: true,
            wave_collectives_are_convergent: true,
            output_commit_is_transactional: true,
            accesses_are_bounded: true,
        }
    }
}

/// Independent effect-contract mismatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectContractErrorV1 {
    /// Initialized read inventory changed.
    ReadInventory,
    /// Output-write inventory changed.
    WriteInventory,
    /// Read-only alias rule changed.
    ReadAliasPolicy,
    /// Writable disjointness was removed.
    OutputAliasPolicy,
    /// Total injective ownership was removed.
    OutputOwnership,
    /// Collective convergence was removed.
    Convergence,
    /// Transactional publication was removed.
    TransactionalCommit,
    /// Bounds checking was removed.
    Bounds,
}

/// Validates every independent effect-contract axis.
pub fn validate_effect_contract_v1(
    contract: RmsNormEffectContractV1,
) -> Result<(), EffectContractErrorV1> {
    let exact = RmsNormEffectContractV1::exact();
    if contract.initialized_read_buffers != exact.initialized_read_buffers {
        return Err(EffectContractErrorV1::ReadInventory);
    }
    if contract.write_buffers != exact.write_buffers {
        return Err(EffectContractErrorV1::WriteInventory);
    }
    if contract.read_only_inputs_may_alias != exact.read_only_inputs_may_alias {
        return Err(EffectContractErrorV1::ReadAliasPolicy);
    }
    if !contract.writable_outputs_are_disjoint {
        return Err(EffectContractErrorV1::OutputAliasPolicy);
    }
    if !contract.output_mapping_is_total_and_injective {
        return Err(EffectContractErrorV1::OutputOwnership);
    }
    if !contract.wave_collectives_are_convergent {
        return Err(EffectContractErrorV1::Convergence);
    }
    if !contract.output_commit_is_transactional {
        return Err(EffectContractErrorV1::TransactionalCommit);
    }
    if !contract.accesses_are_bounded {
        return Err(EffectContractErrorV1::Bounds);
    }
    Ok(())
}

/// One independent proof/property obligation named by this foundation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RmsNormObligationV1 {
    /// Valid allocation provenance for every effect.
    MemorySafe = 1,
    /// Bounds for every global access.
    BoundsSafe = 2,
    /// Every read observes initialized data.
    Initialized = 3,
    /// Writable buffers are disjoint from all other buffers.
    AliasDisjoint = 4,
    /// Conflicting cross-lane/workgroup effects are excluded.
    RaceFree = 5,
    /// Every lane reaches every wave collective.
    BarrierConvergent = 6,
    /// The row/lane output map is total and injective.
    OutputRegionInjective = 7,
    /// The schedule implements RMSNorm plus residual semantics.
    FunctionalRefinement = 8,
    /// BF16/FP32 behavior obeys the named numerical policy.
    NumericalContract = 9,
    /// Exact resource use remains within the B3 envelope.
    ResourceBounded = 10,
    /// Source-to-machine correspondence, deliberately unsupported here.
    MachineRefinementBoundary = 11,
}

/// Properties specified independently by the host-only foundation.
pub const RMSNORM_FOUNDATION_OBLIGATIONS_V1: [RmsNormObligationV1; 10] = [
    RmsNormObligationV1::MemorySafe,
    RmsNormObligationV1::BoundsSafe,
    RmsNormObligationV1::Initialized,
    RmsNormObligationV1::AliasDisjoint,
    RmsNormObligationV1::RaceFree,
    RmsNormObligationV1::BarrierConvergent,
    RmsNormObligationV1::OutputRegionInjective,
    RmsNormObligationV1::FunctionalRefinement,
    RmsNormObligationV1::NumericalContract,
    RmsNormObligationV1::ResourceBounded,
];

/// Returns a checked row-major element index.
pub fn rmsnorm_element_index_v1(
    profile: ValidatedRmsNormProfileV1,
    row: usize,
    column: usize,
) -> Option<usize> {
    let descriptor = profile.descriptor();
    if row >= descriptor.rows || column >= descriptor.hidden_size {
        return None;
    }
    row.checked_mul(descriptor.hidden_size)?.checked_add(column)
}

/// Returns whether the named lane owns the named output column.
pub const fn rmsnorm_lane_owns_column_v1(lane: usize, column: usize) -> bool {
    lane < RMSNORM_WAVE_LANES_V1 && column % RMSNORM_WAVE_LANES_V1 == lane
}
