//! Reviewed structural correspondence from the exact attributed source to the CPU model.
//!
//! `syn` admits only the exact syntax tree encoded below, after removing Rust
//! documentation attributes. An executable abstract interpreter then evaluates
//! the admitted mask, physical-lane recurrence, inactive-zero, and ownership
//! rules against the existing independent CPU oracle. This is review evidence,
//! not a proof that Rust syntax or compiler execution refines the model.

use core::fmt;

use quote::ToTokens as _;
use sha2::{Digest as _, Sha256};

use crate::{
    CollectiveOutputV1, MAX_EXACT_INPUT_MAGNITUDE_V1, OracleErrorV1, WAVE64_LANES_V1,
    lane_is_active_v1, wave64_collectives_oracle_v1,
};

const ATTRIBUTED_SOURCE_BYTES_V2: &[u8] = include_bytes!("kernel.rs");
const CPU_ORACLE_BYTES_V2: &[u8] = include_bytes!("oracle.rs");
const CORRESPONDENCE_BYTES_V2: &[u8] = include_bytes!("source_model_correspondence.rs");
const BINDING_DOMAIN_V2: &[u8] = b"fe2o3.wave64.reviewed-source-cpu-correspondence.v2\0";

/// Exact non-authority boundary carried by every successful V2 review receipt.
pub const REVIEWED_SOURCE_CPU_CORRESPONDENCE_BOUNDARY_V2: &str = "reviewed exact-syntax structural correspondence from ordinary attributed Rust to an executable CPU source model;finite integral f32 corpus;active zero sign is abstracted;outer commit is recorded but Git-tree membership is not proven;proves_source_to_model_refinement=false;no MIR/compiler causality;no KIR/LLVM/ISA or GPU evidence;no generalized memory safety or race freedom;no parity authority";

// This independent source spelling is parsed by syn and compared as a complete
// syntax tree. Whitespace and non-doc comments are irrelevant; every attribute,
// type, expression, branch, call order, and write target remains significant.
const REVIEWED_ATTRIBUTED_SOURCE_SHAPE_V2: &str = r#"
#![allow(missing_docs)]

use fe2o3_device::{
    DisjointSlice, Gfx942Collectives, SubgroupTile, Wave64, WaveLane, kernel, thread,
};

pub const WAVE64_COLLECTIVES_WORKGROUP_V1: [u32; 3] = [64, 1, 1];

#[kernel(
    typed,
    namespace = "2863304ebf7f501a7f177c5b8f5a456261ee34760472727ba3f0205ccf5ce9cc",
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn wave64_collectives_v1(
    input: &[f32],
    active_mask: u64,
    mut reduction_output: DisjointSlice<f32>,
    mut inclusive_output: DisjointSlice<f32>,
    mut exclusive_output: DisjointSlice<f32>,
) {
    let lane = thread::index_1d().get();
    if lane >= 64
        || input.len() != 64
        || reduction_output.len() != 64
        || inclusive_output.len() != 64
        || exclusive_output.len() != 64
    {
        fe2o3_device::trap();
        return;
    }

    let active = active_mask & (1_u64 << lane) != 0;
    let contribution = if active { input[lane] } else { 0.0_f32 };

    let Some(lane_snapshot) = (unsafe { WaveLane::<Wave64>::from_raw(lane as u32) }) else {
        fe2o3_device::trap();
        return;
    };
    let wave = SubgroupTile::<64>::from_wave64_snapshot(&lane_snapshot);
    let context = unsafe { Gfx942Collectives::from_compiler() };

    let reduction = unsafe { wave.reduce_sum(&context, contribution) };
    let inclusive = unsafe { wave.inclusive_scan_sum(&context, contribution) };
    let exclusive = unsafe { wave.exclusive_scan_sum(&context, contribution) };

    let published_reduction = if active { reduction } else { 0.0 };
    let published_inclusive = if active { inclusive } else { 0.0 };
    let published_exclusive = if active { exclusive } else { 0.0 };

    if let Some(output) = unsafe { reduction_output.get_mut_at(lane) } {
        *output = published_reduction;
    }
    if let Some(output) = unsafe { inclusive_output.get_mut_at(lane) } {
        *output = published_inclusive;
    }
    if let Some(output) = unsafe { exclusive_output.get_mut_at(lane) } {
        *output = published_exclusive;
    }
}
"#;

/// Exact content identities selected by the bounded correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCpuContentIdentitiesV2 {
    /// SHA-256 of exact checked-in `src/kernel.rs` bytes.
    pub attributed_source_sha256: [u8; 32],
    /// SHA-256 of exact checked-in `src/oracle.rs` bytes.
    pub cpu_oracle_sha256: [u8; 32],
    /// SHA-256 of this exact correspondence implementation.
    pub correspondence_sha256: [u8; 32],
}

/// Content identities bound to one externally selected outer Git commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCpuBindingV2 {
    /// Exact content identities included in the transcript.
    pub content: SourceCpuContentIdentitiesV2,
    /// Raw 20-byte Git object identity selected by the evidence producer.
    pub outer_commit: [u8; 20],
    /// Domain-separated SHA-256 of the content identities and outer commit.
    pub transcript_sha256: [u8; 32],
}

/// Exact abstract algorithm admitted by the syntax-tree collector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewedSourceAlgorithmV2 {
    lanes: usize,
    ordered_collectives: [CollectiveOutputV1; 3],
}

impl ReviewedSourceAlgorithmV2 {
    /// Exact physical lane count admitted by the source guard and launch attribute.
    pub const fn lanes(self) -> usize {
        self.lanes
    }

    /// Source order of reduction, inclusive scan, and exclusive scan calls.
    pub const fn ordered_collectives(self) -> [CollectiveOutputV1; 3] {
        self.ordered_collectives
    }

    /// Activity is selected by bit `lane` of the exact `u64` mask.
    pub const fn selects_mask_bit_at_physical_lane(self) -> bool {
        true
    }

    /// Scan recurrence order is increasing physical lane order.
    pub const fn uses_increasing_physical_lane_order(self) -> bool {
        true
    }

    /// Inactive contributions and all inactive publications are positive zero.
    pub const fn uses_inactive_positive_zero(self) -> bool {
        true
    }

    /// Each physical lane owns the same index in three distinct output views.
    pub const fn uses_same_lane_output_ownership(self) -> bool {
        true
    }
}

/// Three exact arrays produced by the executable attributed-source abstraction.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceCpuOutputsV2 {
    /// Masked reduction published by every active lane.
    pub reduction: [f32; WAVE64_LANES_V1],
    /// Increasing-physical-lane inclusive prefixes.
    pub inclusive: [f32; WAVE64_LANES_V1],
    /// Increasing-physical-lane exclusive prefixes.
    pub exclusive: [f32; WAVE64_LANES_V1],
}

impl SourceCpuOutputsV2 {
    fn values(&self, output: CollectiveOutputV1) -> &[f32; WAVE64_LANES_V1] {
        match output {
            CollectiveOutputV1::Reduction => &self.reduction,
            CollectiveOutputV1::Inclusive => &self.inclusive,
            CollectiveOutputV1::Exclusive => &self.exclusive,
        }
    }
}

/// Fail-closed syntax admission error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceStructureErrorV2 {
    /// Supplied bytes were not a Rust source file accepted by `syn`.
    InvalidRustSyntax,
    /// Parsed syntax differed from the complete independently encoded shape.
    NonCanonicalSyntaxTree,
}

impl fmt::Display for SourceStructureErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRustSyntax => "attributed source is not valid Rust syntax",
            Self::NonCanonicalSyntaxTree => "attributed source syntax tree is not exact",
        })
    }
}

impl std::error::Error for SourceStructureErrorV2 {}

/// First fail-closed error from the bounded structural correspondence.
#[derive(Clone, Debug, PartialEq)]
pub enum SourceCpuCorrespondenceErrorV2 {
    /// Caller-selected identities or transcript did not match exact contents.
    IdentityBinding,
    /// Exact checked-in attributed source did not match the reviewed syntax tree.
    Structure(SourceStructureErrorV2),
    /// Input was outside the exact CPU oracle corpus.
    Input(OracleErrorV1),
    /// Abstract source and CPU oracle values differed.
    SemanticValue {
        /// Output allocation containing the mismatch.
        output: CollectiveOutputV1,
        /// Physical output lane.
        lane: usize,
        /// Abstract-source binary32 bits.
        source_bits: u32,
        /// CPU-oracle binary32 bits.
        oracle_bits: u32,
    },
}

impl fmt::Display for SourceCpuCorrespondenceErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityBinding => {
                formatter.write_str("source/CPU identity binding is not exact")
            }
            Self::Structure(error) => write!(formatter, "{error}"),
            Self::Input(error) => write!(formatter, "source model rejected input: {error}"),
            Self::SemanticValue { output, lane, .. } => {
                write!(
                    formatter,
                    "{output} source/CPU value differs at lane {lane}"
                )
            }
        }
    }
}

impl std::error::Error for SourceCpuCorrespondenceErrorV2 {}

/// Inert result of one exact structural and executable-model comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCpuCorrespondenceReceiptV2 {
    binding: SourceCpuBindingV2,
    active_mask: u64,
    checked_outputs: u32,
}

impl SourceCpuCorrespondenceReceiptV2 {
    /// Exact content/outer-commit transcript checked for this observation.
    pub const fn binding(self) -> SourceCpuBindingV2 {
        self.binding
    }

    /// Logical activity mask used by this concrete observation.
    pub const fn active_mask(self) -> u64 {
        self.active_mask
    }

    /// Number of concrete output lanes compared with the CPU oracle.
    pub const fn checked_outputs(self) -> u32 {
        self.checked_outputs
    }

    /// This evidence is explicitly a reviewed structural correspondence.
    pub const fn is_reviewed_structural_correspondence(self) -> bool {
        true
    }

    /// Syntax-tree review plus model comparison is not semantic refinement.
    pub const fn proves_source_to_model_refinement(self) -> bool {
        false
    }

    /// Recording a commit does not itself prove Git-tree membership.
    pub const fn proves_outer_commit_contains_content(self) -> bool {
        false
    }

    /// No MIR or compiler causality is established here.
    pub const fn proves_compiler_causality(self) -> bool {
        false
    }

    /// No KIR, LLVM, ISA, or GPU observation is joined here.
    pub const fn proves_machine_refinement_or_execution(self) -> bool {
        false
    }

    /// This exact algorithm check is not generalized memory or race safety.
    pub const fn proves_generalized_safety(self) -> bool {
        false
    }

    /// This inert evidence cannot promote a parity row.
    pub const fn grants_parity_promotion(self) -> bool {
        false
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn append_field(transcript: &mut Sha256, bytes: &[u8]) {
    transcript.update((bytes.len() as u64).to_be_bytes());
    transcript.update(bytes);
}

/// Returns the exact three content identities used by this build.
pub fn exact_source_cpu_content_identities_v2() -> SourceCpuContentIdentitiesV2 {
    SourceCpuContentIdentitiesV2 {
        attributed_source_sha256: sha256(ATTRIBUTED_SOURCE_BYTES_V2),
        cpu_oracle_sha256: sha256(CPU_ORACLE_BYTES_V2),
        correspondence_sha256: sha256(CORRESPONDENCE_BYTES_V2),
    }
}

/// Domain-separates exact content identities with an outer Git commit.
///
/// This records a commit supplied by the evidence producer. It deliberately
/// does not inspect a repository or prove that the commit contains the bytes.
pub fn bind_source_cpu_content_to_outer_commit_v2(outer_commit: [u8; 20]) -> SourceCpuBindingV2 {
    let content = exact_source_cpu_content_identities_v2();
    let mut transcript = Sha256::new();
    transcript.update(BINDING_DOMAIN_V2);
    append_field(&mut transcript, &content.attributed_source_sha256);
    append_field(&mut transcript, &content.cpu_oracle_sha256);
    append_field(&mut transcript, &content.correspondence_sha256);
    append_field(&mut transcript, &outer_commit);
    SourceCpuBindingV2 {
        content,
        outer_commit,
        transcript_sha256: transcript.finalize().into(),
    }
}

fn without_doc_attributes(mut syntax: syn::File) -> syn::File {
    syntax
        .attrs
        .retain(|attribute| !attribute.path().is_ident("doc"));
    for item in &mut syntax.items {
        let attributes = match item {
            syn::Item::Const(item) => &mut item.attrs,
            syn::Item::Fn(item) => &mut item.attrs,
            syn::Item::Use(item) => &mut item.attrs,
            _ => continue,
        };
        attributes.retain(|attribute| !attribute.path().is_ident("doc"));
    }
    syntax
}

fn canonical_syntax(source: &str) -> Result<String, SourceStructureErrorV2> {
    let syntax = syn::parse_file(source).map_err(|_| SourceStructureErrorV2::InvalidRustSyntax)?;
    Ok(without_doc_attributes(syntax)
        .into_token_stream()
        .to_string())
}

/// Parses and admits only the complete independently encoded kernel syntax tree.
///
/// The returned algorithm is an inert abstract description. This function is
/// intentionally public so hostile tests can demonstrate rejection after
/// recomputing any mutation's digest.
pub fn collect_reviewed_source_algorithm_v2(
    source: &str,
) -> Result<ReviewedSourceAlgorithmV2, SourceStructureErrorV2> {
    let actual = canonical_syntax(source)?;
    let reviewed = canonical_syntax(REVIEWED_ATTRIBUTED_SOURCE_SHAPE_V2)
        .expect("reviewed attributed source shape must parse");
    if actual != reviewed {
        return Err(SourceStructureErrorV2::NonCanonicalSyntaxTree);
    }
    Ok(ReviewedSourceAlgorithmV2 {
        lanes: WAVE64_LANES_V1,
        ordered_collectives: [
            CollectiveOutputV1::Reduction,
            CollectiveOutputV1::Inclusive,
            CollectiveOutputV1::Exclusive,
        ],
    })
}

fn validate_abstract_input(input: &[f32]) -> Result<(), OracleErrorV1> {
    if input.len() != WAVE64_LANES_V1 {
        return Err(OracleErrorV1::WrongInputLength {
            actual: input.len(),
        });
    }
    for (lane, value) in input.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(OracleErrorV1::NonFiniteInput { lane });
        }
        if value.fract() != 0.0 || value.abs() > MAX_EXACT_INPUT_MAGNITUDE_V1 {
            return Err(OracleErrorV1::OutsideExactCorpus { lane, value });
        }
    }
    Ok(())
}

/// Executes the exact admitted mask, recurrence, inactivity, and ownership model.
///
/// Mathematical integer accumulation is exact for the admitted binary32
/// corpus. Active zero sign is therefore abstracted; inactive publication is
/// still constructed as bit-exact positive zero.
pub fn interpret_reviewed_source_algorithm_v2(
    algorithm: ReviewedSourceAlgorithmV2,
    input: &[f32],
    active_mask: u64,
) -> Result<SourceCpuOutputsV2, OracleErrorV1> {
    validate_abstract_input(input)?;
    debug_assert_eq!(algorithm.lanes, WAVE64_LANES_V1);

    let contributions: [i64; WAVE64_LANES_V1] = core::array::from_fn(|lane| {
        if lane_is_active_v1(active_mask, lane) {
            input[lane] as i64
        } else {
            0
        }
    });
    let reduction_value: i64 = contributions.iter().sum();
    let mut running = 0_i64;
    let mut inclusive_values = [0_i64; WAVE64_LANES_V1];
    let mut exclusive_values = [0_i64; WAVE64_LANES_V1];
    for lane in 0..WAVE64_LANES_V1 {
        exclusive_values[lane] = running;
        running += contributions[lane];
        inclusive_values[lane] = running;
    }

    let publish = |values: &[i64; WAVE64_LANES_V1]| {
        core::array::from_fn(|lane| {
            if lane_is_active_v1(active_mask, lane) {
                values[lane] as f32
            } else {
                0.0_f32
            }
        })
    };
    let reductions = [reduction_value; WAVE64_LANES_V1];
    Ok(SourceCpuOutputsV2 {
        reduction: publish(&reductions),
        inclusive: publish(&inclusive_values),
        exclusive: publish(&exclusive_values),
    })
}

fn compare_with_oracle(
    source: &SourceCpuOutputsV2,
    oracle: &SourceCpuOutputsV2,
    active_mask: u64,
) -> Result<(), SourceCpuCorrespondenceErrorV2> {
    for output in [
        CollectiveOutputV1::Reduction,
        CollectiveOutputV1::Inclusive,
        CollectiveOutputV1::Exclusive,
    ] {
        let source_values = source.values(output);
        let oracle_values = oracle.values(output);
        for lane in 0..WAVE64_LANES_V1 {
            let equal = if lane_is_active_v1(active_mask, lane) {
                source_values[lane] == oracle_values[lane]
            } else {
                source_values[lane].to_bits() == 0.0_f32.to_bits()
                    && oracle_values[lane].to_bits() == 0.0_f32.to_bits()
            };
            if !equal {
                return Err(SourceCpuCorrespondenceErrorV2::SemanticValue {
                    output,
                    lane,
                    source_bits: source_values[lane].to_bits(),
                    oracle_bits: oracle_values[lane].to_bits(),
                });
            }
        }
    }
    Ok(())
}

/// Reviews exact attributed syntax and compares its abstract interpretation to the CPU oracle.
pub fn verify_reviewed_source_to_cpu_correspondence_v2(
    input: &[f32],
    active_mask: u64,
    binding: SourceCpuBindingV2,
) -> Result<SourceCpuCorrespondenceReceiptV2, SourceCpuCorrespondenceErrorV2> {
    let exact_binding = bind_source_cpu_content_to_outer_commit_v2(binding.outer_commit);
    if binding != exact_binding {
        return Err(SourceCpuCorrespondenceErrorV2::IdentityBinding);
    }
    let source_text = core::str::from_utf8(ATTRIBUTED_SOURCE_BYTES_V2)
        .map_err(|_| SourceStructureErrorV2::InvalidRustSyntax)
        .map_err(SourceCpuCorrespondenceErrorV2::Structure)?;
    let algorithm = collect_reviewed_source_algorithm_v2(source_text)
        .map_err(SourceCpuCorrespondenceErrorV2::Structure)?;
    let source = interpret_reviewed_source_algorithm_v2(algorithm, input, active_mask)
        .map_err(SourceCpuCorrespondenceErrorV2::Input)?;

    let mut reduction = [f32::NAN; WAVE64_LANES_V1];
    let mut inclusive = [f32::NAN; WAVE64_LANES_V1];
    let mut exclusive = [f32::NAN; WAVE64_LANES_V1];
    wave64_collectives_oracle_v1(
        input,
        active_mask,
        &mut reduction,
        &mut inclusive,
        &mut exclusive,
    )
    .map_err(SourceCpuCorrespondenceErrorV2::Input)?;
    let oracle = SourceCpuOutputsV2 {
        reduction,
        inclusive,
        exclusive,
    };
    compare_with_oracle(&source, &oracle, active_mask)?;

    Ok(SourceCpuCorrespondenceReceiptV2 {
        binding,
        active_mask,
        checked_outputs: (3 * WAVE64_LANES_V1) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_detects_each_output_family() {
        let exact = SourceCpuOutputsV2 {
            reduction: [0.0; WAVE64_LANES_V1],
            inclusive: [0.0; WAVE64_LANES_V1],
            exclusive: [0.0; WAVE64_LANES_V1],
        };
        for output in [
            CollectiveOutputV1::Reduction,
            CollectiveOutputV1::Inclusive,
            CollectiveOutputV1::Exclusive,
        ] {
            let mut hostile = exact.clone();
            match output {
                CollectiveOutputV1::Reduction => hostile.reduction[9] = 1.0,
                CollectiveOutputV1::Inclusive => hostile.inclusive[9] = 1.0,
                CollectiveOutputV1::Exclusive => hostile.exclusive[9] = 1.0,
            }
            assert!(matches!(
                compare_with_oracle(&hostile, &exact, 1_u64 << 9),
                Err(SourceCpuCorrespondenceErrorV2::SemanticValue {
                    output: actual,
                    lane: 9,
                    ..
                }) if actual == output
            ));
        }
    }

    #[test]
    fn inactive_negative_zero_is_not_accepted() {
        let exact = SourceCpuOutputsV2 {
            reduction: [0.0; WAVE64_LANES_V1],
            inclusive: [0.0; WAVE64_LANES_V1],
            exclusive: [0.0; WAVE64_LANES_V1],
        };
        let mut hostile = exact.clone();
        hostile.exclusive[63] = -0.0;
        assert!(matches!(
            compare_with_oracle(&hostile, &exact, 0),
            Err(SourceCpuCorrespondenceErrorV2::SemanticValue {
                output: CollectiveOutputV1::Exclusive,
                lane: 63,
                ..
            })
        ));
    }
}
