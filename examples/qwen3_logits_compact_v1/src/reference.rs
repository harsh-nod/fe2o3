//! Streaming FP32 projection/argmax reference and independent oracles.

use crate::{
    Bf16V1, CompactBatchBindingV1, CompactBatchExpectationV1, CompactBindingErrorV1,
    LogitsPlanIdentityV1, LogitsStructuralIdentityV1, QWEN3_VOCABULARY_SIZE_V1,
    StructuralLogitsCandidateV1, validate_compact_batch_binding_v1,
};

/// Random-access exact BF16 activation and LM-head weight source.
///
/// Implementations may stream, map, or procedurally expose data; the host
/// model never requires a duplicate full model image.
pub trait Bf16ProjectionSourceV1 {
    /// Declared flattened activation element count.
    fn activation_elements(&self) -> usize;
    /// Declared flattened weight element count.
    fn weight_elements(&self) -> usize;
    /// Reads one activation from `[row][hidden]`.
    fn activation(&self, index: usize) -> Option<Bf16V1>;
    /// Reads one weight from `[token_id][hidden]`.
    fn weight(&self, index: usize) -> Option<Bf16V1>;
}

/// Borrowed slice implementation of the exact projection source.
#[derive(Clone, Copy, Debug)]
pub struct Bf16SliceProjectionSourceV1<'a> {
    /// Contiguous `[row][hidden]` activation.
    pub activation: &'a [Bf16V1],
    /// Contiguous `[token_id][hidden]` LM-head weight.
    pub weight: &'a [Bf16V1],
}

impl Bf16ProjectionSourceV1 for Bf16SliceProjectionSourceV1<'_> {
    fn activation_elements(&self) -> usize {
        self.activation.len()
    }

    fn weight_elements(&self) -> usize {
        self.weight.len()
    }

    fn activation(&self, index: usize) -> Option<Bf16V1> {
        self.activation.get(index).copied()
    }

    fn weight(&self, index: usize) -> Option<Bf16V1> {
        self.weight.get(index).copied()
    }
}

/// Tensor named by a reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogitsTensorV1 {
    /// Activation source.
    Activation,
    /// LM-head weight source.
    Weight,
    /// Provider logits.
    Logits,
    /// Compact output records.
    CompactOutput,
}

/// FP32 projection stage that failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogitsArithmeticStageV1 {
    /// BF16-decoded multiplication.
    Product,
    /// Ascending-hidden FP32 sum.
    Accumulation,
}

/// Fail-closed reference error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogitsReferenceErrorV1 {
    /// Runtime identity binding failed.
    Binding(CompactBindingErrorV1),
    /// Source/provider/output extent differed.
    WrongLength {
        /// Affected tensor.
        tensor: LogitsTensorV1,
        /// Exact expected count.
        expected: usize,
        /// Observed count.
        actual: usize,
    },
    /// Requested row/token was outside candidate bounds.
    CoordinateOutOfRange,
    /// A source did not return a declared in-bounds element.
    MissingSourceElement {
        /// Affected source.
        tensor: LogitsTensorV1,
        /// Flattened element index.
        index: usize,
    },
    /// A logically read BF16 input was NaN or infinity.
    NonFiniteInput {
        /// Affected source.
        tensor: LogitsTensorV1,
        /// Flattened element index.
        index: usize,
    },
    /// Projection produced a nonfinite intermediate.
    NonFiniteIntermediate {
        /// Row coordinate.
        row: usize,
        /// Token ID coordinate.
        token_id: usize,
        /// Hidden feature coordinate.
        feature: usize,
        /// Failed stage.
        stage: LogitsArithmeticStageV1,
    },
    /// Provider returned NaN or infinity.
    NonFiniteLogit {
        /// Row coordinate.
        row: usize,
        /// Token ID coordinate.
        token_id: usize,
    },
    /// Checked indexing overflowed.
    ArithmeticOverflow,
    /// Bounded staging allocation failed.
    AllocationFailure,
}

impl From<CompactBindingErrorV1> for LogitsReferenceErrorV1 {
    fn from(value: CompactBindingErrorV1) -> Self {
        Self::Binding(value)
    }
}

fn exact_source_lengths(
    candidate: StructuralLogitsCandidateV1,
) -> Result<(usize, usize), LogitsReferenceErrorV1> {
    Ok((
        usize::try_from(candidate.resources().activation_elements)
            .map_err(|_| LogitsReferenceErrorV1::ArithmeticOverflow)?,
        usize::try_from(candidate.resources().weight_elements)
            .map_err(|_| LogitsReferenceErrorV1::ArithmeticOverflow)?,
    ))
}

fn validate_source<S: Bf16ProjectionSourceV1>(
    candidate: StructuralLogitsCandidateV1,
    source: &S,
) -> Result<(), LogitsReferenceErrorV1> {
    let (activation, weight) = exact_source_lengths(candidate)?;
    if source.activation_elements() != activation {
        return Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::Activation,
            expected: activation,
            actual: source.activation_elements(),
        });
    }
    if source.weight_elements() != weight {
        return Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::Weight,
            expected: weight,
            actual: source.weight_elements(),
        });
    }
    Ok(())
}

/// Evaluates one exact BF16/FP32 vocabulary projection coordinate.
pub fn qwen3_project_logit_v1<S: Bf16ProjectionSourceV1>(
    candidate: StructuralLogitsCandidateV1,
    source: &S,
    row: usize,
    token_id: usize,
) -> Result<f32, LogitsReferenceErrorV1> {
    validate_source(candidate, source)?;
    let profile = candidate.profile().descriptor();
    if row >= profile.rows || token_id >= profile.vocabulary_size {
        return Err(LogitsReferenceErrorV1::CoordinateOutOfRange);
    }
    let activation_start = row
        .checked_mul(profile.hidden_size)
        .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
    let weight_start = token_id
        .checked_mul(profile.hidden_size)
        .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
    let mut accumulator = 0.0_f32;
    for feature in 0..profile.hidden_size {
        let activation_index = activation_start
            .checked_add(feature)
            .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
        let weight_index = weight_start
            .checked_add(feature)
            .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
        let activation = source.activation(activation_index).ok_or(
            LogitsReferenceErrorV1::MissingSourceElement {
                tensor: LogitsTensorV1::Activation,
                index: activation_index,
            },
        )?;
        let weight =
            source
                .weight(weight_index)
                .ok_or(LogitsReferenceErrorV1::MissingSourceElement {
                    tensor: LogitsTensorV1::Weight,
                    index: weight_index,
                })?;
        if !activation.is_finite() {
            return Err(LogitsReferenceErrorV1::NonFiniteInput {
                tensor: LogitsTensorV1::Activation,
                index: activation_index,
            });
        }
        if !weight.is_finite() {
            return Err(LogitsReferenceErrorV1::NonFiniteInput {
                tensor: LogitsTensorV1::Weight,
                index: weight_index,
            });
        }
        let product = activation.to_f32() * weight.to_f32();
        if !product.is_finite() {
            return Err(LogitsReferenceErrorV1::NonFiniteIntermediate {
                row,
                token_id,
                feature,
                stage: LogitsArithmeticStageV1::Product,
            });
        }
        accumulator += product;
        if !accumulator.is_finite() {
            return Err(LogitsReferenceErrorV1::NonFiniteIntermediate {
                row,
                token_id,
                feature,
                stage: LogitsArithmeticStageV1::Accumulation,
            });
        }
    }
    Ok(accumulator)
}

/// Independent idealized F64 projection oracle for one coordinate.
pub fn qwen3_project_logit_f64_oracle_v1<S: Bf16ProjectionSourceV1>(
    candidate: StructuralLogitsCandidateV1,
    source: &S,
    row: usize,
    token_id: usize,
) -> Result<f64, LogitsReferenceErrorV1> {
    validate_source(candidate, source)?;
    let profile = candidate.profile().descriptor();
    if row >= profile.rows || token_id >= profile.vocabulary_size {
        return Err(LogitsReferenceErrorV1::CoordinateOutOfRange);
    }
    let mut terms = Vec::new();
    terms
        .try_reserve_exact(profile.hidden_size)
        .map_err(|_| LogitsReferenceErrorV1::AllocationFailure)?;
    for feature in 0..profile.hidden_size {
        let activation_index = row
            .checked_mul(profile.hidden_size)
            .and_then(|base| base.checked_add(feature))
            .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
        let weight_index = token_id
            .checked_mul(profile.hidden_size)
            .and_then(|base| base.checked_add(feature))
            .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
        let activation = source.activation(activation_index).ok_or(
            LogitsReferenceErrorV1::MissingSourceElement {
                tensor: LogitsTensorV1::Activation,
                index: activation_index,
            },
        )?;
        let weight =
            source
                .weight(weight_index)
                .ok_or(LogitsReferenceErrorV1::MissingSourceElement {
                    tensor: LogitsTensorV1::Weight,
                    index: weight_index,
                })?;
        if !activation.is_finite() {
            return Err(LogitsReferenceErrorV1::NonFiniteInput {
                tensor: LogitsTensorV1::Activation,
                index: activation_index,
            });
        }
        if !weight.is_finite() {
            return Err(LogitsReferenceErrorV1::NonFiniteInput {
                tensor: LogitsTensorV1::Weight,
                index: weight_index,
            });
        }
        terms.push(f64::from(activation.to_f32()) * f64::from(weight.to_f32()));
    }
    Ok(terms.into_iter().sum())
}

/// Bounded source of FP32 logits consumed by the compact argmax stage.
pub trait LogitProviderV1 {
    /// Declared flattened row count.
    fn rows(&self) -> usize;
    /// Declared vocabulary width.
    fn vocabulary_size(&self) -> usize;
    /// Evaluates one logit.
    fn logit(&self, row: usize, token_id: usize) -> Result<f32, LogitsReferenceErrorV1>;
}

/// Concrete composition of exact BF16 projection with streaming argmax.
#[derive(Clone, Copy, Debug)]
pub struct ProjectedLogitProviderV1<'a, S> {
    /// Admitted structural candidate.
    pub candidate: StructuralLogitsCandidateV1,
    /// Exact BF16 activation/weight source.
    pub source: &'a S,
}

impl<S: Bf16ProjectionSourceV1> LogitProviderV1 for ProjectedLogitProviderV1<'_, S> {
    fn rows(&self) -> usize {
        self.candidate.profile().descriptor().rows
    }

    fn vocabulary_size(&self) -> usize {
        self.candidate.profile().descriptor().vocabulary_size
    }

    fn logit(&self, row: usize, token_id: usize) -> Result<f32, LogitsReferenceErrorV1> {
        qwen3_project_logit_v1(self.candidate, self.source, row, token_id)
    }
}

/// Canonical compact output for one active row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactCompletionRecordV1 {
    /// Record schema version.
    pub schema_version: u16,
    /// Model role.
    pub role: crate::Qwen3LogitsRoleV1,
    /// Exact B3 bucket.
    pub bucket: crate::B3LogitsBucketV1,
    /// Request slot/generation.
    pub request: crate::CompactRequestIdentityV1,
    /// Exact batch epoch.
    pub epoch: u64,
    /// Exact generated-plan identity.
    pub plan_identity: LogitsPlanIdentityV1,
    /// Exact structural candidate identity.
    pub candidate_identity: LogitsStructuralIdentityV1,
    /// Flattened canonical row ordinal.
    pub row: u32,
    /// Active token coordinate within this request.
    pub local_token: u32,
    /// Lowest token ID attaining maximum FP32 logit.
    pub token_id: u32,
}

/// Successful transactional evaluation summary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompactReferenceStateV1 {
    /// Published record count.
    pub records: usize,
    /// Exact strict-greater comparisons after each row's initial token.
    pub comparisons: u64,
}

fn allocate_records(
    count: usize,
) -> Result<Vec<CompactCompletionRecordV1>, LogitsReferenceErrorV1> {
    let mut records = Vec::new();
    records
        .try_reserve_exact(count)
        .map_err(|_| LogitsReferenceErrorV1::AllocationFailure)?;
    Ok(records)
}

/// Applies lowest-ID argmax to a bounded provider and publishes compact records
/// transactionally.
///
/// This is the provider-only argmax/record model. Use
/// [`qwen3_logits_argmax_compact_reference_v1`] for the complete exact BF16
/// projection composition.
pub fn qwen3_argmax_compact_from_provider_reference_v1<P: LogitProviderV1>(
    candidate: StructuralLogitsCandidateV1,
    binding: &CompactBatchBindingV1,
    expected: &CompactBatchExpectationV1,
    provider: &P,
    output: &mut [CompactCompletionRecordV1],
) -> Result<CompactReferenceStateV1, LogitsReferenceErrorV1> {
    validate_compact_batch_binding_v1(
        candidate.profile(),
        candidate.plan_identity(),
        binding,
        expected,
    )?;
    let profile = candidate.profile().descriptor();
    if provider.rows() != profile.rows {
        return Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::Logits,
            expected: profile.rows,
            actual: provider.rows(),
        });
    }
    if provider.vocabulary_size() != profile.vocabulary_size {
        return Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::Logits,
            expected: profile.vocabulary_size,
            actual: provider.vocabulary_size(),
        });
    }
    if output.len() != profile.rows {
        return Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::CompactOutput,
            expected: profile.rows,
            actual: output.len(),
        });
    }

    let mut staged = allocate_records(profile.rows)?;
    let mut comparisons = 0_u64;
    for row in 0..profile.rows {
        let mut winning_token = 0_usize;
        let mut winning_logit = provider.logit(row, 0)?;
        if !winning_logit.is_finite() {
            return Err(LogitsReferenceErrorV1::NonFiniteLogit { row, token_id: 0 });
        }
        for token_id in 1..profile.vocabulary_size {
            let logit = provider.logit(row, token_id)?;
            if !logit.is_finite() {
                return Err(LogitsReferenceErrorV1::NonFiniteLogit { row, token_id });
            }
            if logit > winning_logit {
                winning_logit = logit;
                winning_token = token_id;
            }
            comparisons = comparisons
                .checked_add(1)
                .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
        }
        let request_index = row
            .checked_div(profile.active_tokens)
            .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
        let local_token = row
            .checked_rem(profile.active_tokens)
            .ok_or(LogitsReferenceErrorV1::ArithmeticOverflow)?;
        staged.push(CompactCompletionRecordV1 {
            schema_version: 1,
            role: profile.role,
            bucket: profile.bucket,
            request: *binding
                .requests
                .get(request_index)
                .ok_or(LogitsReferenceErrorV1::CoordinateOutOfRange)?,
            epoch: binding.epoch,
            plan_identity: binding.plan_identity,
            candidate_identity: candidate.candidate_identity(),
            row: u32::try_from(row).map_err(|_| LogitsReferenceErrorV1::ArithmeticOverflow)?,
            local_token: u32::try_from(local_token)
                .map_err(|_| LogitsReferenceErrorV1::ArithmeticOverflow)?,
            token_id: u32::try_from(winning_token)
                .map_err(|_| LogitsReferenceErrorV1::ArithmeticOverflow)?,
        });
    }
    output.copy_from_slice(&staged);
    Ok(CompactReferenceStateV1 {
        records: staged.len(),
        comparisons,
    })
}

/// Encodes one compact record into its canonical 96-byte little-endian form.
pub fn encode_compact_completion_record_v1(record: CompactCompletionRecordV1) -> [u8; 96] {
    let mut encoded = [0_u8; 96];
    encoded[0..2].copy_from_slice(&record.schema_version.to_le_bytes());
    encoded[2] = record.role as u8;
    encoded[3] = record.bucket as u8;
    encoded[4..8].copy_from_slice(&record.request.slot.to_le_bytes());
    encoded[8..12].copy_from_slice(&record.request.generation.to_le_bytes());
    encoded[12..20].copy_from_slice(&record.epoch.to_le_bytes());
    encoded[20..52].copy_from_slice(&record.plan_identity.0);
    encoded[52..84].copy_from_slice(&record.candidate_identity.bytes());
    encoded[84..88].copy_from_slice(&record.row.to_le_bytes());
    encoded[88..92].copy_from_slice(&record.local_token.to_le_bytes());
    encoded[92..96].copy_from_slice(&record.token_id.to_le_bytes());
    encoded
}

/// Streams every exact BF16 projection logit through lowest-ID argmax and
/// publishes compact records transactionally.
///
/// Unlike the provider-only model, this entry point cannot receive caller-
/// asserted FP32 logits: every value is derived through the exact activation
/// and LM-head BF16 source contract.
pub fn qwen3_logits_argmax_compact_reference_v1<S: Bf16ProjectionSourceV1>(
    candidate: StructuralLogitsCandidateV1,
    binding: &CompactBatchBindingV1,
    expected: &CompactBatchExpectationV1,
    source: &S,
    output: &mut [CompactCompletionRecordV1],
) -> Result<CompactReferenceStateV1, LogitsReferenceErrorV1> {
    validate_source(candidate, source)?;
    let provider = ProjectedLogitProviderV1 { candidate, source };
    qwen3_argmax_compact_from_provider_reference_v1(candidate, binding, expected, &provider, output)
}

/// Independent two-pass lowest-token-ID argmax oracle over one exact row.
pub fn independent_lowest_token_argmax_v1(logits: &[f32]) -> Result<u32, LogitsReferenceErrorV1> {
    if logits.len() != QWEN3_VOCABULARY_SIZE_V1 {
        return Err(LogitsReferenceErrorV1::WrongLength {
            tensor: LogitsTensorV1::Logits,
            expected: QWEN3_VOCABULARY_SIZE_V1,
            actual: logits.len(),
        });
    }
    if let Some((token_id, _)) = logits
        .iter()
        .enumerate()
        .find(|(_, logit)| !logit.is_finite())
    {
        return Err(LogitsReferenceErrorV1::NonFiniteLogit { row: 0, token_id });
    }
    let maximum = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let token_id = logits
        .iter()
        .position(|logit| *logit == maximum)
        .ok_or(LogitsReferenceErrorV1::CoordinateOutOfRange)?;
    u32::try_from(token_id).map_err(|_| LogitsReferenceErrorV1::ArithmeticOverflow)
}
