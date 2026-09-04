//! Exact B3 profile, numerical, effect, resource, and completion-binding contracts.

use std::collections::HashSet;

/// Exact shared Qwen3 vocabulary size.
pub const QWEN3_VOCABULARY_SIZE_V1: usize = 151_936;
/// Exact target hidden width.
pub const QWEN3_TARGET_HIDDEN_SIZE_V1: usize = 4_096;
/// Exact draft hidden width.
pub const QWEN3_DRAFT_HIDDEN_SIZE_V1: usize = 1_024;
/// Ferric maximum live request slots.
pub const M1_MAX_REQUESTS_V1: usize = 32;
/// Exact maximum speculative proposal K.
pub const M1_MAX_SPECULATIVE_K_V1: usize = 16;
/// Largest exact flattened B3 row count.
pub const MAX_LOGITS_ROWS_V1: u64 = 2_048;
/// Largest activation element count.
pub const MAX_LOGITS_ACTIVATION_ELEMENTS_V1: u64 = 8_388_608;
/// Largest LM-head weight element count.
pub const MAX_LOGITS_WEIGHT_ELEMENTS_V1: u64 = 622_329_856;
/// Largest logical FP32 logit count.
pub const MAX_LOGICAL_LOGITS_V1: u64 = 311_164_928;
/// Largest FP32 projection multiplication/addition count.
pub const MAX_LOGITS_PROJECTION_WORK_V1: u64 = 1_274_531_545_088;
/// Canonical serialized compact-record byte count.
pub const COMPACT_COMPLETION_RECORD_BYTES_V1: u64 = 96;

/// Exact target or draft model role.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum Qwen3LogitsRoleV1 {
    /// Qwen3-8B target.
    Target8B = 1,
    /// Qwen3-0.6B draft.
    Draft06B = 2,
}

impl Qwen3LogitsRoleV1 {
    /// Returns the exact hidden width.
    pub const fn hidden_size(self) -> usize {
        match self {
            Self::Target8B => QWEN3_TARGET_HIDDEN_SIZE_V1,
            Self::Draft06B => QWEN3_DRAFT_HIDDEN_SIZE_V1,
        }
    }

    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// B3 execution mode.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum B3LogitsModeV1 {
    /// Prompt prefill.
    Prefill = 1,
    /// Ordinary single-token decode.
    Decode = 2,
    /// Draft proposal or target verification.
    Speculative = 3,
}

/// Closed B3 bucket vocabulary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum B3LogitsBucketV1 {
    /// Prefill S1T128.
    PrefillS1T128 = 1,
    /// Prefill S8T128.
    PrefillS8T128 = 2,
    /// Prefill S1T512.
    PrefillS1T512 = 3,
    /// Prefill S1T2048.
    PrefillS1T2048 = 4,
    /// Decode S1C8192.
    DecodeS1C8192 = 5,
    /// Decode S8C8192.
    DecodeS8C8192 = 6,
    /// Decode S32C8192.
    DecodeS32C8192 = 7,
    /// Speculative S1K4C8192.
    SpeculativeS1K4C8192 = 8,
    /// Speculative S8K4C8192.
    SpeculativeS8K4C8192 = 9,
    /// Speculative S1K8C8192.
    SpeculativeS1K8C8192 = 10,
    /// Speculative S1K16C8192.
    SpeculativeS1K16C8192 = 11,
}

/// All and only B3 logits buckets.
pub const B3_LOGITS_BUCKETS_V1: [B3LogitsBucketV1; 11] = [
    B3LogitsBucketV1::PrefillS1T128,
    B3LogitsBucketV1::PrefillS8T128,
    B3LogitsBucketV1::PrefillS1T512,
    B3LogitsBucketV1::PrefillS1T2048,
    B3LogitsBucketV1::DecodeS1C8192,
    B3LogitsBucketV1::DecodeS8C8192,
    B3LogitsBucketV1::DecodeS32C8192,
    B3LogitsBucketV1::SpeculativeS1K4C8192,
    B3LogitsBucketV1::SpeculativeS8K4C8192,
    B3LogitsBucketV1::SpeculativeS1K8C8192,
    B3LogitsBucketV1::SpeculativeS1K16C8192,
];

impl B3LogitsBucketV1 {
    /// Returns the exact mode.
    pub const fn mode(self) -> B3LogitsModeV1 {
        match self {
            Self::PrefillS1T128
            | Self::PrefillS8T128
            | Self::PrefillS1T512
            | Self::PrefillS1T2048 => B3LogitsModeV1::Prefill,
            Self::DecodeS1C8192 | Self::DecodeS8C8192 | Self::DecodeS32C8192 => {
                B3LogitsModeV1::Decode
            }
            _ => B3LogitsModeV1::Speculative,
        }
    }

    /// Returns exact request count.
    pub const fn sequences(self) -> usize {
        match self {
            Self::PrefillS8T128 | Self::DecodeS8C8192 | Self::SpeculativeS8K4C8192 => 8,
            Self::DecodeS32C8192 => 32,
            _ => 1,
        }
    }

    /// Returns exact active tokens per request for a role.
    pub const fn active_tokens(self, role: Qwen3LogitsRoleV1) -> usize {
        match self {
            Self::PrefillS1T128 | Self::PrefillS8T128 => 128,
            Self::PrefillS1T512 => 512,
            Self::PrefillS1T2048 => 2_048,
            Self::DecodeS1C8192 | Self::DecodeS8C8192 | Self::DecodeS32C8192 => 1,
            Self::SpeculativeS1K4C8192 | Self::SpeculativeS8K4C8192 => match role {
                Qwen3LogitsRoleV1::Target8B => 5,
                Qwen3LogitsRoleV1::Draft06B => 4,
            },
            Self::SpeculativeS1K8C8192 => match role {
                Qwen3LogitsRoleV1::Target8B => 9,
                Qwen3LogitsRoleV1::Draft06B => 8,
            },
            Self::SpeculativeS1K16C8192 => match role {
                Qwen3LogitsRoleV1::Target8B => 17,
                Qwen3LogitsRoleV1::Draft06B => 16,
            },
        }
    }

    /// Returns exact speculative K, or zero outside speculation.
    pub const fn speculative_k(self) -> usize {
        match self {
            Self::SpeculativeS1K4C8192 | Self::SpeculativeS8K4C8192 => 4,
            Self::SpeculativeS1K8C8192 => 8,
            Self::SpeculativeS1K16C8192 => 16,
            _ => 0,
        }
    }

    /// Returns the stable identity tag.
    pub const fn identity_tag(self) -> u8 {
        self as u8
    }
}

/// Complete exact profile descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogitsProfileDescriptorV1 {
    /// Target/draft role.
    pub role: Qwen3LogitsRoleV1,
    /// Execution mode.
    pub mode: B3LogitsModeV1,
    /// Exact B3 bucket.
    pub bucket: B3LogitsBucketV1,
    /// Independent request count.
    pub sequences: usize,
    /// Active tokens per request.
    pub active_tokens: usize,
    /// Flattened active row count.
    pub rows: usize,
    /// Input/weight reduction width.
    pub hidden_size: usize,
    /// Exact output vocabulary width.
    pub vocabulary_size: usize,
    /// Exact speculative K, excluding the target's extra verification row.
    pub speculative_k: usize,
}

impl LogitsProfileDescriptorV1 {
    /// Constructs the canonical profile.
    pub const fn canonical(role: Qwen3LogitsRoleV1, bucket: B3LogitsBucketV1) -> Self {
        let sequences = bucket.sequences();
        let active_tokens = bucket.active_tokens(role);
        Self {
            role,
            mode: bucket.mode(),
            bucket,
            sequences,
            active_tokens,
            rows: sequences * active_tokens,
            hidden_size: role.hidden_size(),
            vocabulary_size: QWEN3_VOCABULARY_SIZE_V1,
            speculative_k: bucket.speculative_k(),
        }
    }
}

/// Profile validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogitsProfileErrorV1 {
    /// Mode did not match the named bucket.
    Mode,
    /// Sequence count differed.
    Sequences,
    /// Active token width differed.
    ActiveTokens,
    /// Flattened row count differed.
    Rows,
    /// Hidden width differed from role.
    HiddenSize,
    /// Vocabulary differed from 151936.
    VocabularySize,
    /// Speculative K differed or exceeded 16.
    SpeculativeK,
    /// Checked arithmetic overflowed.
    ArithmeticOverflow,
    /// Derived resources exceeded B3 bounds.
    ResourceLimit,
}

/// Checked exact logical resource contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogitsResourceContractV1 {
    /// Flattened rows.
    pub rows: u64,
    /// BF16 activation elements.
    pub activation_elements: u64,
    /// BF16 LM-head weight elements.
    pub weight_elements: u64,
    /// Logical FP32 logits evaluated by projection plus argmax.
    pub logical_logits: u64,
    /// FP32 multiplications.
    pub fp32_multiplications: u64,
    /// FP32 additions.
    pub fp32_additions: u64,
    /// Strict-greater argmax comparisons after each row's initial token.
    pub argmax_comparisons: u64,
    /// Compact output records.
    pub compact_records: u64,
    /// Canonical compact output bytes.
    pub compact_output_bytes: u64,
    /// Full logical logits bytes; the streaming reference does not allocate it.
    pub avoided_logits_staging_bytes: u64,
}

fn resources(
    profile: LogitsProfileDescriptorV1,
) -> Result<LogitsResourceContractV1, LogitsProfileErrorV1> {
    let rows = u64::try_from(profile.rows).map_err(|_| LogitsProfileErrorV1::ArithmeticOverflow)?;
    let hidden =
        u64::try_from(profile.hidden_size).map_err(|_| LogitsProfileErrorV1::ArithmeticOverflow)?;
    let vocabulary = u64::try_from(profile.vocabulary_size)
        .map_err(|_| LogitsProfileErrorV1::ArithmeticOverflow)?;
    let activation_elements = rows
        .checked_mul(hidden)
        .ok_or(LogitsProfileErrorV1::ArithmeticOverflow)?;
    let weight_elements = vocabulary
        .checked_mul(hidden)
        .ok_or(LogitsProfileErrorV1::ArithmeticOverflow)?;
    let logical_logits = rows
        .checked_mul(vocabulary)
        .ok_or(LogitsProfileErrorV1::ArithmeticOverflow)?;
    let work = logical_logits
        .checked_mul(hidden)
        .ok_or(LogitsProfileErrorV1::ArithmeticOverflow)?;
    let compact_output_bytes = rows
        .checked_mul(COMPACT_COMPLETION_RECORD_BYTES_V1)
        .ok_or(LogitsProfileErrorV1::ArithmeticOverflow)?;
    let avoided_logits_staging_bytes = logical_logits
        .checked_mul(4)
        .ok_or(LogitsProfileErrorV1::ArithmeticOverflow)?;
    Ok(LogitsResourceContractV1 {
        rows,
        activation_elements,
        weight_elements,
        logical_logits,
        fp32_multiplications: work,
        fp32_additions: work,
        argmax_comparisons: logical_logits
            .checked_sub(rows)
            .ok_or(LogitsProfileErrorV1::ArithmeticOverflow)?,
        compact_records: rows,
        compact_output_bytes,
        avoided_logits_staging_bytes,
    })
}

/// Validated inert profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedLogitsProfileV1 {
    descriptor: LogitsProfileDescriptorV1,
    resources: LogitsResourceContractV1,
}

impl ValidatedLogitsProfileV1 {
    /// Returns descriptor.
    pub const fn descriptor(self) -> LogitsProfileDescriptorV1 {
        self.descriptor
    }

    /// Returns checked resources.
    pub const fn resources(self) -> LogitsResourceContractV1 {
        self.resources
    }
}

/// Validates every profile and resource field.
pub fn validate_logits_profile_v1(
    profile: LogitsProfileDescriptorV1,
) -> Result<ValidatedLogitsProfileV1, LogitsProfileErrorV1> {
    let exact = LogitsProfileDescriptorV1::canonical(profile.role, profile.bucket);
    if profile.mode != exact.mode {
        return Err(LogitsProfileErrorV1::Mode);
    }
    if profile.sequences != exact.sequences {
        return Err(LogitsProfileErrorV1::Sequences);
    }
    if profile.active_tokens != exact.active_tokens {
        return Err(LogitsProfileErrorV1::ActiveTokens);
    }
    if profile.rows != exact.rows {
        return Err(LogitsProfileErrorV1::Rows);
    }
    if profile.hidden_size != exact.hidden_size {
        return Err(LogitsProfileErrorV1::HiddenSize);
    }
    if profile.vocabulary_size != QWEN3_VOCABULARY_SIZE_V1 {
        return Err(LogitsProfileErrorV1::VocabularySize);
    }
    if profile.speculative_k != exact.speculative_k
        || profile.speculative_k > M1_MAX_SPECULATIVE_K_V1
    {
        return Err(LogitsProfileErrorV1::SpeculativeK);
    }
    let resources = resources(profile)?;
    if resources.rows > MAX_LOGITS_ROWS_V1
        || resources.activation_elements > MAX_LOGITS_ACTIVATION_ELEMENTS_V1
        || resources.weight_elements > MAX_LOGITS_WEIGHT_ELEMENTS_V1
        || resources.logical_logits > MAX_LOGICAL_LOGITS_V1
        || resources.fp32_multiplications > MAX_LOGITS_PROJECTION_WORK_V1
    {
        return Err(LogitsProfileErrorV1::ResourceLimit);
    }
    Ok(ValidatedLogitsProfileV1 {
        descriptor: profile,
        resources,
    })
}

/// Exact projection/argmax numerical policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogitsNumericalPolicyV1 {
    /// Activation storage is BF16.
    pub activation_bf16: bool,
    /// Weight storage is BF16.
    pub weight_bf16: bool,
    /// LM-head has no bias.
    pub bias_absent: bool,
    /// Hidden features are multiplied/added ascending in FP32.
    pub ascending_hidden_separate_fp32_mul_add: bool,
    /// FMA contraction is outside this source model.
    pub contraction_disabled: bool,
    /// Every input/logit/intermediate must be finite.
    pub reject_non_finite: bool,
    /// Token IDs are scanned ascending.
    pub token_ids_ascending: bool,
    /// Winner changes only for strict greater comparison.
    pub replace_only_on_strict_greater: bool,
    /// Equal maxima select lowest token ID.
    pub lowest_token_id_tie_break: bool,
}

impl LogitsNumericalPolicyV1 {
    /// Returns the only admitted numerical policy.
    pub const fn exact() -> Self {
        Self {
            activation_bf16: true,
            weight_bf16: true,
            bias_absent: true,
            ascending_hidden_separate_fp32_mul_add: true,
            contraction_disabled: true,
            reject_non_finite: true,
            token_ids_ascending: true,
            replace_only_on_strict_greater: true,
            lowest_token_id_tie_break: true,
        }
    }
}

/// Numerical-policy validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogitsNumericalErrorV1 {
    /// At least one numerical or tie-break field differed.
    NonCanonical,
}

/// Validates the complete numerical policy.
pub fn validate_logits_numerical_policy_v1(
    policy: LogitsNumericalPolicyV1,
) -> Result<(), LogitsNumericalErrorV1> {
    if policy != LogitsNumericalPolicyV1::exact() {
        return Err(LogitsNumericalErrorV1::NonCanonical);
    }
    Ok(())
}

/// Exact logical effect, alias, race, and publication contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LogitsEffectContractV1 {
    /// Activation and weight sources are initialized read-only.
    pub inputs_initialized_read_only: bool,
    /// No full logits staging is required.
    pub streamed_logit_consumption: bool,
    /// Each row/token projection has one logical evaluator.
    pub unique_logit_coordinates: bool,
    /// Compact records have one writer per row.
    pub unique_compact_record_writers: bool,
    /// Output is staged separately and published after complete success.
    pub transactional_output: bool,
    /// Output must not alias input storage.
    pub output_disjoint_from_inputs: bool,
    /// Host model uses no atomics.
    pub atomics: u8,
    /// Host model uses no barriers.
    pub barriers: u8,
}

impl LogitsEffectContractV1 {
    /// Returns the only admitted effect contract.
    pub const fn exact() -> Self {
        Self {
            inputs_initialized_read_only: true,
            streamed_logit_consumption: true,
            unique_logit_coordinates: true,
            unique_compact_record_writers: true,
            transactional_output: true,
            output_disjoint_from_inputs: true,
            atomics: 0,
            barriers: 0,
        }
    }
}

/// Effect-contract validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogitsEffectErrorV1 {
    /// At least one effect, alias, race, or publication field differed.
    NonCanonical,
}

/// Validates the complete effect contract.
pub fn validate_logits_effect_contract_v1(
    effects: LogitsEffectContractV1,
) -> Result<(), LogitsEffectErrorV1> {
    if effects != LogitsEffectContractV1::exact() {
        return Err(LogitsEffectErrorV1::NonCanonical);
    }
    Ok(())
}

/// Stable plan identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LogitsPlanIdentityV1(pub [u8; 32]);

impl LogitsPlanIdentityV1 {
    /// Returns whether at least one byte is nonzero.
    pub fn is_present(self) -> bool {
        self.0.iter().any(|byte| *byte != 0)
    }
}

/// Ferric-compatible request slot/generation identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompactRequestIdentityV1 {
    /// Slot below 32.
    pub slot: u32,
    /// Nonzero generation.
    pub generation: u32,
}

/// Runtime batch identity fields carried into compact records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactBatchBindingV1 {
    /// Plan identity, equal to the admitted candidate plan.
    pub plan_identity: LogitsPlanIdentityV1,
    /// Nonzero submission/completion epoch.
    pub epoch: u64,
    /// Exact speculative K for this bucket.
    pub speculative_k: usize,
    /// Requests in canonical batch order.
    pub requests: Vec<CompactRequestIdentityV1>,
}

/// Independently supplied expected runtime identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompactBatchExpectationV1 {
    /// Exact expected epoch.
    pub epoch: u64,
    /// Exact expected requests in batch order.
    pub requests: Vec<CompactRequestIdentityV1>,
}

/// Runtime binding validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompactBindingErrorV1 {
    /// Plan identity was absent.
    MissingPlanIdentity,
    /// Plan identity differed from candidate.
    StalePlanIdentity,
    /// Epoch was zero.
    MissingEpoch,
    /// Epoch differed from independent expectation.
    StaleEpoch,
    /// Speculative K differed or exceeded 16.
    SpeculativeK,
    /// Request count differed from exact sequences.
    RequestCount,
    /// Request slot was outside Ferric's 32-slot bound.
    RequestSlot,
    /// Request generation was zero.
    MissingRequestGeneration,
    /// Request slots were duplicated.
    DuplicateRequestSlot,
    /// Request identity/order differed from expectation.
    StaleRequest,
    /// Bounded validation allocation failed.
    AllocationFailure,
}

/// Validates runtime identities against candidate and independent expectation.
pub fn validate_compact_batch_binding_v1(
    profile: ValidatedLogitsProfileV1,
    candidate_plan: LogitsPlanIdentityV1,
    binding: &CompactBatchBindingV1,
    expected: &CompactBatchExpectationV1,
) -> Result<(), CompactBindingErrorV1> {
    if !binding.plan_identity.is_present() || !candidate_plan.is_present() {
        return Err(CompactBindingErrorV1::MissingPlanIdentity);
    }
    if binding.plan_identity != candidate_plan {
        return Err(CompactBindingErrorV1::StalePlanIdentity);
    }
    if binding.epoch == 0 || expected.epoch == 0 {
        return Err(CompactBindingErrorV1::MissingEpoch);
    }
    if binding.epoch != expected.epoch {
        return Err(CompactBindingErrorV1::StaleEpoch);
    }
    let descriptor = profile.descriptor();
    if binding.speculative_k != descriptor.speculative_k
        || binding.speculative_k > M1_MAX_SPECULATIVE_K_V1
    {
        return Err(CompactBindingErrorV1::SpeculativeK);
    }
    if binding.requests.len() != descriptor.sequences
        || expected.requests.len() != descriptor.sequences
    {
        return Err(CompactBindingErrorV1::RequestCount);
    }
    let mut slots = HashSet::new();
    slots
        .try_reserve(binding.requests.len())
        .map_err(|_| CompactBindingErrorV1::AllocationFailure)?;
    for (actual, expected_request) in binding.requests.iter().zip(&expected.requests) {
        if usize::try_from(actual.slot).map_or(true, |slot| slot >= M1_MAX_REQUESTS_V1) {
            return Err(CompactBindingErrorV1::RequestSlot);
        }
        if actual.generation == 0 {
            return Err(CompactBindingErrorV1::MissingRequestGeneration);
        }
        if !slots.insert(actual.slot) {
            return Err(CompactBindingErrorV1::DuplicateRequestSlot);
        }
        if actual != expected_request {
            return Err(CompactBindingErrorV1::StaleRequest);
        }
    }
    Ok(())
}
