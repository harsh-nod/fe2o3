//! Bounded structural correspondence for the canonical row-softmax source.
//!
//! This module admits only the complete reviewed syntax tree in `kernel.rs`.
//! It then compares an operation trace derived from that admitted shape with a
//! separately encoded lane-zero, three-loop abstract model. The comparison is
//! review evidence only: it does not assign semantics to Rust, floating-point
//! arithmetic, `DeviceMath`, OCML, LLVM, or an executing GPU.

use core::fmt;

use quote::ToTokens as _;
use sha2::{Digest as _, Sha256};

use crate::ROW_ELEMENTS_V1;

const ATTRIBUTED_SOURCE_BYTES_V1: &[u8] = include_bytes!("kernel.rs");
const ABSTRACT_MODEL_BYTES_V1: &[u8] = include_bytes!("source_model_correspondence.rs");
const VERUS_MODEL_BYTES_V1: &[u8] = include_bytes!("../verus/row_softmax_v1.rs");
const MEMORY_PRECONDITIONS_BYTES_V1: &[u8] =
    b"input-f32-elements=64;output-disjoint-f32-elements=64";
const BINDING_DOMAIN_V1: &[u8] = b"fe2o3.row-softmax.reviewed-ordinary-source.v1\0";

/// Exact non-authority boundary carried by every successful review receipt.
pub const REVIEWED_ROW_SOFTMAX_SOURCE_BOUNDARY_V1: &str = "ordinary example-owned #[kernel] Rust source with exact AST structural admission;reviewed lane0-only three-loop zero-barrier abstract operation model conditional on authenticated exact 64-element input and output preconditions;content, precondition profile, and caller-selected outer commit are transcript-bound;runtime precondition satisfaction unproved;proves_source_to_model_refinement=false;exp_f32/IEEE/OCML semantics unproved;Rust operational semantics unproved;no MIR/compiler/KIR/LLVM/ISA/GPU causality;no generalized memory safety or race freedom;no parity authority";

// This spelling is independent of the compiled source. `syn` compares the
// complete syntax tree, so every non-documentation attribute, type, expression,
// call, operand, branch, loop, and write target is significant.
const REVIEWED_ATTRIBUTED_SOURCE_SHAPE_V1: &str = r#"
#![allow(non_upper_case_globals)]

use fe2o3_device::{DeviceMath, DisjointSlice, GridExclusive, kernel, thread};

const ROW_ELEMENTS: usize = 64;

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(64, 64, 64))
)]
pub fn row_softmax_v1(input: &[f32], mut output: DisjointSlice<f32, GridExclusive>) {
    if let Some(leader) = thread::grid_leader() {
        let mut maximum = f32::NEG_INFINITY;
        let mut index = 0_usize;
        while index < ROW_ELEMENTS {
            let value = input[index];
            if value > maximum {
                maximum = value;
            }
            index += 1;
        }

        let math = DeviceMath::current();
        let mut denominator = 0.0_f32;
        index = 0;
        while index < ROW_ELEMENTS {
            denominator += math.exp_f32(input[index] - maximum);
            index += 1;
        }

        index = 0;
        while index < ROW_ELEMENTS {
            let probability = math.exp_f32(input[index] - maximum) / denominator;
            if let Some(slot) = output.get_mut_exclusive(&leader, index) {
                *slot = probability;
            }
            index += 1;
        }
    }
}
"#;

/// One of the three ordered loops in the admitted source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowSoftmaxPhaseV1 {
    /// Sequential maximum selection over all 64 inputs.
    Maximum,
    /// Sequential accumulation of 64 abstract exponential results.
    Denominator,
    /// Sequential recomputation, division, and publication of 64 outputs.
    Output,
}

/// One abstract source operation whose order and operands are reviewed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowSoftmaxAbstractOperationV1 {
    /// Read the one-dimensional physical thread index.
    ObservePhysicalLane,
    /// Select execution exactly when the observed lane equals zero.
    SelectLaneZero {
        /// Physical lane being selected or rejected.
        lane: usize,
        /// Whether this lane enters the three-loop body.
        selected: bool,
    },
    /// Read one input element during the named phase.
    ReadInput {
        /// Ordered loop containing the read.
        phase: RowSoftmaxPhaseV1,
        /// Exact input index operand.
        index: usize,
    },
    /// Compare an input with, and conditionally select, the running maximum.
    CompareAndSelectMaximum {
        /// Exact input index supplying the compared value.
        index: usize,
    },
    /// Acquire the compiler-supplied abstract device-math capability.
    AcquireDeviceMath,
    /// Subtract the selected maximum from one indexed input.
    SubtractMaximum {
        /// Denominator or output loop containing the subtraction.
        phase: RowSoftmaxPhaseV1,
        /// Exact input index operand.
        index: usize,
    },
    /// Invoke the abstract `exp_f32` operation on the preceding subtraction.
    InvokeAbstractExp {
        /// Denominator or output loop containing the call.
        phase: RowSoftmaxPhaseV1,
        /// Exact input index feeding the call.
        index: usize,
    },
    /// Add one abstract exponential result to the sequential denominator.
    AccumulateDenominator {
        /// Exact input index whose result is accumulated.
        index: usize,
    },
    /// Divide one abstract exponential result by the completed denominator.
    DivideByDenominator {
        /// Exact input index whose result is divided.
        index: usize,
    },
    /// Write one output element through the disjoint output view.
    WriteOutput {
        /// Sole physical lane owning the write.
        owner_lane: usize,
        /// Exact output index operand.
        index: usize,
    },
}

/// Exact abstract algorithm admitted by the syntax-tree collector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReviewedRowSoftmaxAlgorithmV1 {
    row_elements: usize,
    participating_lane: usize,
    phases: [RowSoftmaxPhaseV1; 3],
}

impl ReviewedRowSoftmaxAlgorithmV1 {
    /// Exact fixed width admitted by the constant, attributes, and loop tests.
    pub const fn row_elements(self) -> usize {
        self.row_elements
    }

    /// Sole physical lane admitted by the source branch.
    pub const fn participating_lane(self) -> usize {
        self.participating_lane
    }

    /// Exact source order of maximum, denominator, and output loops.
    pub const fn phases(self) -> [RowSoftmaxPhaseV1; 3] {
        self.phases
    }

    /// The admitted source contains no barrier operation.
    pub const fn barrier_count(self) -> usize {
        0
    }
}

/// Ordered operations emitted by one physical lane in the abstract model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowSoftmaxAbstractTraceV1 {
    lane: usize,
    operations: Vec<RowSoftmaxAbstractOperationV1>,
}

impl RowSoftmaxAbstractTraceV1 {
    /// Physical lane represented by this trace.
    pub const fn lane(&self) -> usize {
        self.lane
    }

    /// Exact ordered abstract operations for this lane.
    pub fn operations(&self) -> &[RowSoftmaxAbstractOperationV1] {
        &self.operations
    }

    /// Number of abstract input reads in the trace.
    pub fn input_reads(&self) -> usize {
        self.operations
            .iter()
            .filter(|operation| {
                matches!(operation, RowSoftmaxAbstractOperationV1::ReadInput { .. })
            })
            .count()
    }

    /// Number of abstract exponential invocations in the trace.
    pub fn abstract_exp_calls(&self) -> usize {
        self.operations
            .iter()
            .filter(|operation| {
                matches!(
                    operation,
                    RowSoftmaxAbstractOperationV1::InvokeAbstractExp { .. }
                )
            })
            .count()
    }

    /// Exact output indices written by this physical lane, in source order.
    pub fn output_writes(&self) -> Vec<usize> {
        self.operations
            .iter()
            .filter_map(|operation| match operation {
                RowSoftmaxAbstractOperationV1::WriteOutput { index, .. } => Some(*index),
                _ => None,
            })
            .collect()
    }

    /// The trace contains no barrier operation by construction.
    pub const fn barrier_count(&self) -> usize {
        0
    }
}

/// Exact checked-in content identities selected by this evidence layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxSourceContentIdentitiesV1 {
    /// SHA-256 of the sole canonical ordinary kernel source.
    pub attributed_source_sha256: [u8; 32],
    /// SHA-256 of this independently reviewed abstract-model implementation.
    pub abstract_model_sha256: [u8; 32],
    /// SHA-256 of the pre-existing Verus mathematical model.
    pub verus_model_sha256: [u8; 32],
    /// SHA-256 of the exact input/output-length precondition profile.
    pub memory_preconditions_sha256: [u8; 32],
}

/// Exact content identities bound to one externally selected Git commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxSourceBindingV1 {
    /// Exact content identities included in the transcript.
    pub content: RowSoftmaxSourceContentIdentitiesV1,
    /// Raw 20-byte Git object identity selected by the caller.
    pub outer_commit: [u8; 20],
    /// Domain-separated SHA-256 of exact content identities and outer commit.
    pub transcript_sha256: [u8; 32],
}

/// Fail-closed structural-admission error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowSoftmaxSourceStructureErrorV1 {
    /// Supplied bytes were not a Rust source file accepted by `syn`.
    InvalidRustSyntax,
    /// Parsed syntax differed from the complete independently encoded shape.
    NonCanonicalSyntaxTree,
}

impl fmt::Display for RowSoftmaxSourceStructureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRustSyntax => "row-softmax source is not valid Rust syntax",
            Self::NonCanonicalSyntaxTree => "row-softmax source syntax tree is not exact",
        })
    }
}

impl std::error::Error for RowSoftmaxSourceStructureErrorV1 {}

/// First failure from exact source/model correspondence review.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RowSoftmaxSourceCorrespondenceErrorV1 {
    /// Caller-supplied content identities or transcript were not exact.
    IdentityBinding,
    /// Exact checked-in source failed structural admission.
    Structure(RowSoftmaxSourceStructureErrorV1),
    /// Source-derived and independently encoded operation traces differed.
    AbstractTrace {
        /// Physical lane whose reviewed traces differed.
        lane: usize,
    },
}

impl fmt::Display for RowSoftmaxSourceCorrespondenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IdentityBinding => formatter.write_str("row-softmax source binding is not exact"),
            Self::Structure(error) => write!(formatter, "{error}"),
            Self::AbstractTrace { lane } => {
                write!(
                    formatter,
                    "row-softmax abstract trace differs for lane {lane}"
                )
            }
        }
    }
}

impl std::error::Error for RowSoftmaxSourceCorrespondenceErrorV1 {}

/// Inert result of one exact structural and abstract-model comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowSoftmaxSourceCorrespondenceReceiptV1 {
    binding: RowSoftmaxSourceBindingV1,
    checked_physical_lanes: u32,
    checked_abstract_operations: u32,
}

impl RowSoftmaxSourceCorrespondenceReceiptV1 {
    /// Exact content/outer-commit transcript checked for this receipt.
    pub const fn binding(self) -> RowSoftmaxSourceBindingV1 {
        self.binding
    }

    /// Number of physical lane schedules compared.
    pub const fn checked_physical_lanes(self) -> u32 {
        self.checked_physical_lanes
    }

    /// Number of ordered abstract operations compared across all lanes.
    pub const fn checked_abstract_operations(self) -> u32 {
        self.checked_abstract_operations
    }

    /// Exact input length required before the abstract trace applies.
    pub const fn required_input_elements(self) -> u32 {
        64
    }

    /// Exact output length required before the abstract trace applies.
    pub const fn required_output_elements(self) -> u32 {
        64
    }

    /// The trace is conditional on, and its transcript authenticates, those lengths.
    pub const fn authenticates_exact_memory_preconditions(self) -> bool {
        true
    }

    /// This evidence does not observe the runtime slices that reach the kernel.
    pub const fn proves_runtime_memory_preconditions(self) -> bool {
        false
    }

    /// The canonical kernel is ordinary, example-owned attributed Rust source.
    pub const fn has_single_canonical_ordinary_source(self) -> bool {
        true
    }

    /// Exact AST review and trace comparison do not prove Rust semantics.
    pub const fn proves_source_to_model_refinement(self) -> bool {
        false
    }

    /// Abstract calls do not establish exponential or floating-point semantics.
    pub const fn proves_exp_ieee_or_ocml_semantics(self) -> bool {
        false
    }

    /// Recording an outer commit does not prove Git-tree membership.
    pub const fn proves_outer_commit_contains_content(self) -> bool {
        false
    }

    /// No compiler or machine causality is established by this receipt.
    pub const fn proves_compiler_or_gpu_causality(self) -> bool {
        false
    }

    /// Fixed ownership traces are not generalized memory or race safety.
    pub const fn proves_generalized_memory_or_race_safety(self) -> bool {
        false
    }

    /// This inert evidence cannot promote a parity row.
    pub const fn grants_parity_promotion(self) -> bool {
        false
    }
}

fn canonical_syntax(source: &str) -> Result<String, RowSoftmaxSourceStructureErrorV1> {
    syn::parse_file(source)
        .map(|syntax| syntax.into_token_stream().to_string())
        .map_err(|_| RowSoftmaxSourceStructureErrorV1::InvalidRustSyntax)
}

/// Admits only the complete reviewed kernel syntax tree.
///
/// Whitespace and comments are not represented by `syn`; every executable
/// token, attribute, type, call, operand, branch, loop, and write is exact.
pub fn collect_reviewed_row_softmax_algorithm_v1(
    source: &str,
) -> Result<ReviewedRowSoftmaxAlgorithmV1, RowSoftmaxSourceStructureErrorV1> {
    let actual = canonical_syntax(source)?;
    let reviewed = canonical_syntax(REVIEWED_ATTRIBUTED_SOURCE_SHAPE_V1)
        .expect("reviewed row-softmax source shape must parse");
    if actual != reviewed {
        return Err(RowSoftmaxSourceStructureErrorV1::NonCanonicalSyntaxTree);
    }
    Ok(ReviewedRowSoftmaxAlgorithmV1 {
        row_elements: ROW_ELEMENTS_V1,
        participating_lane: 0,
        phases: [
            RowSoftmaxPhaseV1::Maximum,
            RowSoftmaxPhaseV1::Denominator,
            RowSoftmaxPhaseV1::Output,
        ],
    })
}

fn push_loop_operation_trace(
    operations: &mut Vec<RowSoftmaxAbstractOperationV1>,
    phase: RowSoftmaxPhaseV1,
    row_elements: usize,
) {
    for index in 0..row_elements {
        operations.push(RowSoftmaxAbstractOperationV1::ReadInput { phase, index });
        match phase {
            RowSoftmaxPhaseV1::Maximum => {
                operations.push(RowSoftmaxAbstractOperationV1::CompareAndSelectMaximum { index });
            }
            RowSoftmaxPhaseV1::Denominator => {
                operations.push(RowSoftmaxAbstractOperationV1::SubtractMaximum { phase, index });
                operations.push(RowSoftmaxAbstractOperationV1::InvokeAbstractExp { phase, index });
                operations.push(RowSoftmaxAbstractOperationV1::AccumulateDenominator { index });
            }
            RowSoftmaxPhaseV1::Output => {
                operations.push(RowSoftmaxAbstractOperationV1::SubtractMaximum { phase, index });
                operations.push(RowSoftmaxAbstractOperationV1::InvokeAbstractExp { phase, index });
                operations.push(RowSoftmaxAbstractOperationV1::DivideByDenominator { index });
                operations.push(RowSoftmaxAbstractOperationV1::WriteOutput {
                    owner_lane: 0,
                    index,
                });
            }
        }
    }
}

/// Interprets the admitted source algorithm as an ordered abstract trace.
pub fn interpret_reviewed_row_softmax_source_v1(
    algorithm: ReviewedRowSoftmaxAlgorithmV1,
    lane: usize,
) -> RowSoftmaxAbstractTraceV1 {
    let mut operations = vec![
        RowSoftmaxAbstractOperationV1::ObservePhysicalLane,
        RowSoftmaxAbstractOperationV1::SelectLaneZero {
            lane,
            selected: lane == algorithm.participating_lane,
        },
    ];
    if lane == algorithm.participating_lane {
        push_loop_operation_trace(&mut operations, algorithm.phases[0], algorithm.row_elements);
        operations.push(RowSoftmaxAbstractOperationV1::AcquireDeviceMath);
        push_loop_operation_trace(&mut operations, algorithm.phases[1], algorithm.row_elements);
        push_loop_operation_trace(&mut operations, algorithm.phases[2], algorithm.row_elements);
    }
    RowSoftmaxAbstractTraceV1 { lane, operations }
}

/// Independently encodes the reviewed lane-zero, three-loop operation model.
pub fn reviewed_row_softmax_abstract_model_v1(lane: usize) -> RowSoftmaxAbstractTraceV1 {
    let mut operations = vec![
        RowSoftmaxAbstractOperationV1::ObservePhysicalLane,
        RowSoftmaxAbstractOperationV1::SelectLaneZero {
            lane,
            selected: lane == 0,
        },
    ];
    if lane == 0 {
        for index in 0..64 {
            operations.push(RowSoftmaxAbstractOperationV1::ReadInput {
                phase: RowSoftmaxPhaseV1::Maximum,
                index,
            });
            operations.push(RowSoftmaxAbstractOperationV1::CompareAndSelectMaximum { index });
        }
        operations.push(RowSoftmaxAbstractOperationV1::AcquireDeviceMath);
        for index in 0..64 {
            operations.push(RowSoftmaxAbstractOperationV1::ReadInput {
                phase: RowSoftmaxPhaseV1::Denominator,
                index,
            });
            operations.push(RowSoftmaxAbstractOperationV1::SubtractMaximum {
                phase: RowSoftmaxPhaseV1::Denominator,
                index,
            });
            operations.push(RowSoftmaxAbstractOperationV1::InvokeAbstractExp {
                phase: RowSoftmaxPhaseV1::Denominator,
                index,
            });
            operations.push(RowSoftmaxAbstractOperationV1::AccumulateDenominator { index });
        }
        for index in 0..64 {
            operations.push(RowSoftmaxAbstractOperationV1::ReadInput {
                phase: RowSoftmaxPhaseV1::Output,
                index,
            });
            operations.push(RowSoftmaxAbstractOperationV1::SubtractMaximum {
                phase: RowSoftmaxPhaseV1::Output,
                index,
            });
            operations.push(RowSoftmaxAbstractOperationV1::InvokeAbstractExp {
                phase: RowSoftmaxPhaseV1::Output,
                index,
            });
            operations.push(RowSoftmaxAbstractOperationV1::DivideByDenominator { index });
            operations.push(RowSoftmaxAbstractOperationV1::WriteOutput {
                owner_lane: 0,
                index,
            });
        }
    }
    RowSoftmaxAbstractTraceV1 { lane, operations }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn append_field(transcript: &mut Sha256, bytes: &[u8]) {
    transcript.update((bytes.len() as u64).to_be_bytes());
    transcript.update(bytes);
}

/// Returns exact identities for source, abstract model, and Verus model.
pub fn exact_row_softmax_source_content_identities_v1() -> RowSoftmaxSourceContentIdentitiesV1 {
    RowSoftmaxSourceContentIdentitiesV1 {
        attributed_source_sha256: sha256(ATTRIBUTED_SOURCE_BYTES_V1),
        abstract_model_sha256: sha256(ABSTRACT_MODEL_BYTES_V1),
        verus_model_sha256: sha256(VERUS_MODEL_BYTES_V1),
        memory_preconditions_sha256: sha256(MEMORY_PRECONDITIONS_BYTES_V1),
    }
}

/// Binds exact checked-in contents to one caller-selected outer Git commit.
///
/// This function records the supplied commit; it does not inspect a repository
/// or prove that the corresponding tree contains these bytes.
pub fn bind_row_softmax_source_content_to_outer_commit_v1(
    outer_commit: [u8; 20],
) -> RowSoftmaxSourceBindingV1 {
    let content = exact_row_softmax_source_content_identities_v1();
    let mut transcript = Sha256::new();
    transcript.update(BINDING_DOMAIN_V1);
    append_field(&mut transcript, &content.attributed_source_sha256);
    append_field(&mut transcript, &content.abstract_model_sha256);
    append_field(&mut transcript, &content.verus_model_sha256);
    append_field(&mut transcript, &content.memory_preconditions_sha256);
    append_field(&mut transcript, &outer_commit);
    RowSoftmaxSourceBindingV1 {
        content,
        outer_commit,
        transcript_sha256: transcript.finalize().into(),
    }
}

/// Compares all 64 physical-lane traces after exact source admission.
pub fn verify_reviewed_row_softmax_source_correspondence_v1(
    binding: RowSoftmaxSourceBindingV1,
) -> Result<RowSoftmaxSourceCorrespondenceReceiptV1, RowSoftmaxSourceCorrespondenceErrorV1> {
    if binding != bind_row_softmax_source_content_to_outer_commit_v1(binding.outer_commit) {
        return Err(RowSoftmaxSourceCorrespondenceErrorV1::IdentityBinding);
    }
    let source = core::str::from_utf8(ATTRIBUTED_SOURCE_BYTES_V1)
        .map_err(|_| RowSoftmaxSourceStructureErrorV1::InvalidRustSyntax)
        .map_err(RowSoftmaxSourceCorrespondenceErrorV1::Structure)?;
    let algorithm = collect_reviewed_row_softmax_algorithm_v1(source)
        .map_err(RowSoftmaxSourceCorrespondenceErrorV1::Structure)?;

    let mut checked_abstract_operations = 0_u32;
    for lane in 0..64 {
        let source_trace = interpret_reviewed_row_softmax_source_v1(algorithm, lane);
        let model_trace = reviewed_row_softmax_abstract_model_v1(lane);
        if source_trace != model_trace {
            return Err(RowSoftmaxSourceCorrespondenceErrorV1::AbstractTrace { lane });
        }
        checked_abstract_operations = checked_abstract_operations
            .checked_add(source_trace.operations.len() as u32)
            .expect("bounded row-softmax trace count fits u32");
    }

    Ok(RowSoftmaxSourceCorrespondenceReceiptV1 {
        binding,
        checked_physical_lanes: 64,
        checked_abstract_operations,
    })
}
