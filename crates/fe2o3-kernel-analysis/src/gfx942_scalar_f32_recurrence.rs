//! Bounded scalar-GEMM binary32 recurrence-step analysis for authenticated gfx942 traces.
//!
//! This layer closes two deliberately narrow issue #214 prerequisites. It checks that one exact
//! `V_MUL_F32_e32_vi` result is the unique provenance root of one exact
//! `V_ADD_F32_e32_vi` operand, and that no fused floating-point definition reaches either add
//! operand through the closed `V_MOV_B32_e32` register-copy profile. It also supplies deterministic
//! executable candidate semantics for the required separate multiply then add step through the
//! workspace-pinned LLVM APFloat port. This does not establish general machine provenance or the
//! loop-carried recurrence across machine CFG backedges.
//!
//! The checked artifact remains inert. In particular, this module does not prove AMDGPU opcode
//! semantics, MODE/denormal behavior, NaN conformance of gfx942, KIR/LLVM-to-machine simulation,
//! memory addressing, EXEC behavior, or hardware refinement. It cannot mint Worker V3 evidence.

use crate::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1,
    AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1, Gfx942InstructionRegisterFactsV1,
    Gfx942MachineDataflowErrorV1, Gfx942MachineDataflowV1, Gfx942ReachingDefinitionV1,
    Gfx942RegisterFactsErrorV1, Gfx942RegisterUnitV1, PhysicalMachineBranchKindV1,
    PhysicalMachineInstructionTraceV1, PhysicalMachineMemoryAccessV1,
};
use rustc_apfloat::ieee::Single;
use rustc_apfloat::{Float, Round, Status};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, error::Error, fmt};

pub const GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-RECURRENCE-STEP-CANDIDATE/V1\0";
pub const GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_VERSION_V1: u16 = 1;
pub const MAX_GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_BYTES_V1: usize = 1024;
pub const MAX_GFX942_SCALAR_F32_RECURRENCE_ITERATIONS_V1: usize = 1_048_576;

const ARTIFACT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-RECURRENCE-STEP-CANDIDATE-IDENTITY/V1\0";
const NUMERIC_MODEL_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/GFX942-SCALAR-F32-NUMERIC-MODEL/V1\0";
const NUMERIC_MODEL_DESCRIPTION_V1: &[u8] =
    b"rustc_apfloat-0.2.3+llvm-462a31f5a5ab;binary32;round-nearest-ties-even;separate-mul-add;exceptions-retained";
const TARGET_TAG_GFX942_XNACK_MINUS_COV6: u8 = 1;
const POLICY_TAG_SEPARATE_MUL_ADD_RNE_PRESERVE_SUBNORMALS: u8 = 1;
const CHECKED_FACTS_V1: u16 = (1 << 0) | (1 << 1) | (1 << 2) | (1 << 3);

/// Exception-status bits reported by the pinned APFloat executable model.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942ScalarF32StatusV1(u8);

impl Gfx942ScalarF32StatusV1 {
    pub const fn bits(self) -> u8 {
        self.0
    }

    pub const fn invalid_operation(self) -> bool {
        self.0 & Status::INVALID_OP.bits() != 0
    }

    pub const fn divide_by_zero(self) -> bool {
        self.0 & Status::DIV_BY_ZERO.bits() != 0
    }

    pub const fn overflow(self) -> bool {
        self.0 & Status::OVERFLOW.bits() != 0
    }

    pub const fn underflow(self) -> bool {
        self.0 & Status::UNDERFLOW.bits() != 0
    }

    pub const fn inexact(self) -> bool {
        self.0 & Status::INEXACT.bits() != 0
    }

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl From<Status> for Gfx942ScalarF32StatusV1 {
    fn from(status: Status) -> Self {
        Self(status.bits())
    }
}

/// One executable candidate-semantics step with separate binary32 rounding points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ScalarF32RecurrenceStepV1 {
    product_bits: u32,
    accumulator_bits: u32,
    multiply_status: Gfx942ScalarF32StatusV1,
    add_status: Gfx942ScalarF32StatusV1,
}

impl Gfx942ScalarF32RecurrenceStepV1 {
    pub const fn product_bits(self) -> u32 {
        self.product_bits
    }

    pub const fn accumulator_bits(self) -> u32 {
        self.accumulator_bits
    }

    pub const fn multiply_status(self) -> Gfx942ScalarF32StatusV1 {
        self.multiply_status
    }

    pub const fn add_status(self) -> Gfx942ScalarF32StatusV1 {
        self.add_status
    }

    pub const fn combined_status(self) -> Gfx942ScalarF32StatusV1 {
        self.multiply_status.union(self.add_status)
    }
}

/// Bounded result of the scalar dot-product candidate semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942ScalarF32DotProductV1 {
    accumulator_bits: u32,
    iterations: u64,
    status: Gfx942ScalarF32StatusV1,
}

impl Gfx942ScalarF32DotProductV1 {
    pub const fn accumulator_bits(self) -> u32 {
        self.accumulator_bits
    }

    pub const fn iterations(self) -> u64 {
        self.iterations
    }

    pub const fn status(self) -> Gfx942ScalarF32StatusV1 {
        self.status
    }
}

/// Executes one issue #214 recurrence step with distinct multiplication and addition.
///
/// The returned bits are the pinned APFloat model result. Equating those bits with gfx942
/// execution remains a separate proof obligation.
pub fn execute_gfx942_scalar_f32_recurrence_step_candidate_v1(
    accumulator_bits: u32,
    left_bits: u32,
    right_bits: u32,
) -> Gfx942ScalarF32RecurrenceStepV1 {
    let product = Single::from_bits(u128::from(left_bits)).mul_r(
        Single::from_bits(u128::from(right_bits)),
        Round::NearestTiesToEven,
    );
    // The admitted machine profile fixes source operand 0 as the product and source operand 1 as
    // the accumulator. Operand order is observable for NaN payload selection in the candidate.
    let sum = product.value.add_r(
        Single::from_bits(u128::from(accumulator_bits)),
        Round::NearestTiesToEven,
    );
    Gfx942ScalarF32RecurrenceStepV1 {
        product_bits: product.value.to_bits() as u32,
        accumulator_bits: sum.value.to_bits() as u32,
        multiply_status: product.status.into(),
        add_status: sum.status.into(),
    }
}

/// Executes a bounded dot product from positive zero under the separate-rounding candidate model.
pub fn execute_gfx942_scalar_f32_dot_product_candidate_v1(
    inputs: &[(u32, u32)],
) -> Result<Gfx942ScalarF32DotProductV1, Gfx942ScalarF32ExecutionErrorV1> {
    if inputs.len() > MAX_GFX942_SCALAR_F32_RECURRENCE_ITERATIONS_V1 {
        return Err(Gfx942ScalarF32ExecutionErrorV1::IterationLimit {
            actual: inputs.len(),
            maximum: MAX_GFX942_SCALAR_F32_RECURRENCE_ITERATIONS_V1,
        });
    }
    let mut accumulator_bits = 0_u32;
    let mut status = Gfx942ScalarF32StatusV1::default();
    for &(left_bits, right_bits) in inputs {
        let step = execute_gfx942_scalar_f32_recurrence_step_candidate_v1(
            accumulator_bits,
            left_bits,
            right_bits,
        );
        accumulator_bits = step.accumulator_bits();
        status = status.union(step.combined_status());
    }
    Ok(Gfx942ScalarF32DotProductV1 {
        accumulator_bits,
        iterations: inputs.len() as u64,
        status,
    })
}

/// Computes a fused reference result for hostile contraction comparisons only.
pub fn execute_binary32_fused_multiply_add_reference_v1(
    left_bits: u32,
    right_bits: u32,
    addend_bits: u32,
) -> (u32, Gfx942ScalarF32StatusV1) {
    let result = Single::from_bits(u128::from(left_bits)).mul_add_r(
        Single::from_bits(u128::from(right_bits)),
        Single::from_bits(u128::from(addend_bits)),
        Round::NearestTiesToEven,
    );
    (result.value.to_bits() as u32, result.status.into())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ScalarF32ExecutionErrorV1 {
    IterationLimit { actual: usize, maximum: usize },
}

impl fmt::Display for Gfx942ScalarF32ExecutionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx942 scalar f32 candidate execution failed: {self:?}"
        )
    }
}

impl Error for Gfx942ScalarF32ExecutionErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942ScalarF32RecurrenceStepArtifactIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942ScalarF32RecurrenceStepArtifactIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Canonical, non-authorizing result of the bounded recurrence-step dataflow analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942ScalarF32RecurrenceStepArtifactV1 {
    authenticated_execution_sha256: [u8; 32],
    authenticated_execution_bytes: u64,
    trace_sha256: [u8; 32],
    trace_bytes: u64,
    function_symbol: String,
    multiply_offset: u64,
    add_offset: u64,
    multiply_encoding_sha256: [u8; 32],
    multiply_encoding_bytes: u16,
    add_encoding_sha256: [u8; 32],
    add_encoding_bytes: u16,
    product_register: u16,
    accumulator_register: u16,
    result_register: u16,
    product_source_operand_index: u8,
    accumulator_source_operand_index: u8,
    canonical_bytes: Box<[u8]>,
}

impl Gfx942ScalarF32RecurrenceStepArtifactV1 {
    /// Decodes an inert artifact and validates its exact canonical representation.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        decode_artifact(bytes)
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn identity(&self) -> Gfx942ScalarF32RecurrenceStepArtifactIdentityV1 {
        Gfx942ScalarF32RecurrenceStepArtifactIdentityV1 {
            sha256: domain_hash(ARTIFACT_IDENTITY_DOMAIN_V1, &self.canonical_bytes),
            byte_len: self.canonical_bytes.len() as u64,
        }
    }

    pub const fn authenticated_execution_identity(&self) -> (&[u8; 32], u64) {
        (
            &self.authenticated_execution_sha256,
            self.authenticated_execution_bytes,
        )
    }

    pub const fn trace_identity(&self) -> (&[u8; 32], u64) {
        (&self.trace_sha256, self.trace_bytes)
    }

    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }

    pub const fn multiply_offset(&self) -> u64 {
        self.multiply_offset
    }

    pub const fn add_offset(&self) -> u64 {
        self.add_offset
    }

    pub const fn product_register(&self) -> u16 {
        self.product_register
    }

    pub const fn accumulator_register(&self) -> u16 {
        self.accumulator_register
    }

    pub const fn result_register(&self) -> u16 {
        self.result_register
    }

    /// Zero-based index in the ADD's two-source operand list.
    pub const fn product_source_operand_index(&self) -> u8 {
        self.product_source_operand_index
    }

    /// Zero-based index in the ADD's two-source operand list.
    pub const fn accumulator_source_operand_index(&self) -> u8 {
        self.accumulator_source_operand_index
    }

    pub fn numeric_model_identity(&self) -> [u8; 32] {
        numeric_model_identity()
    }

    pub const fn binds_authenticated_trace_and_exact_instruction_encodings(&self) -> bool {
        true
    }

    pub const fn validates_separate_recurrence_step_dataflow_shape(&self) -> bool {
        true
    }

    pub const fn excludes_fused_definitions_from_step_inputs(&self) -> bool {
        true
    }

    /// This artifact does not establish an accumulator loop carry or backedge simulation.
    pub const fn establishes_machine_loop_recurrence(&self) -> bool {
        false
    }

    pub const fn provides_executable_candidate_numeric_semantics(&self) -> bool {
        true
    }

    pub const fn establishes_gfx942_instruction_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_worker_v3_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_or_launch_authority(&self) -> bool {
        false
    }
}

/// Move-only custody joining the inert step artifact to its authenticated analyzer run.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1;
///
/// fn consume_twice(analysis: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1) {
///     let _execution = analysis.into_authenticated_execution();
///     let _again = analysis.into_authenticated_execution();
/// }
/// ```
pub struct AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    artifact: Gfx942ScalarF32RecurrenceStepArtifactV1,
}

impl fmt::Debug for AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1")
            .field("artifact_identity", &self.artifact.identity())
            .field("function_symbol", &self.artifact.function_symbol)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
    pub const fn artifact(&self) -> &Gfx942ScalarF32RecurrenceStepArtifactV1 {
        &self.artifact
    }

    pub fn authenticated_execution_identity(
        &self,
    ) -> AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1 {
        self.execution.identity()
    }

    pub const fn authenticates_analyzer_execution(&self) -> bool {
        true
    }

    pub const fn establishes_semantic_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    pub fn into_authenticated_execution(self) -> AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.execution
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ScalarF32RecurrenceStepAnalysisErrorV1 {
    InvalidKernelSymbol,
    KernelNotRequested,
    WrongMultiplyCount { actual: usize },
    WrongAddCount { actual: usize },
    InvalidArithmeticInstruction { offset: u64 },
    MultiplyDoesNotDominateAdd,
    ProductDoesNotReachAdd,
    ProductOperandPosition { actual: u8 },
    AmbiguousAccumulatorOperand,
    ResultDoesNotUpdateAccumulator { result: u16, accumulator: u16 },
    FusedDefinitionReachesAdd { offset: u64 },
    NonUniqueStepInputProvenance { use_offset: u64, definitions: usize },
    UnsupportedStepInputDefinition { offset: u64 },
    StepInputProvenanceCycle { offset: u64, register: u16 },
    StepInputProvenanceLimit,
    Artifact(Gfx942ScalarF32RecurrenceStepArtifactErrorV1),
    ArtifactMismatch,
    RegisterFacts(Gfx942RegisterFactsErrorV1),
    Dataflow(Gfx942MachineDataflowErrorV1),
}

impl fmt::Display for Gfx942ScalarF32RecurrenceStepAnalysisErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid gfx942 scalar f32 recurrence step: {self:?}"
        )
    }
}

impl Error for Gfx942ScalarF32RecurrenceStepAnalysisErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Artifact(error) => Some(error),
            Self::RegisterFacts(error) => Some(error),
            Self::Dataflow(error) => Some(error),
            _ => None,
        }
    }
}

impl From<Gfx942RegisterFactsErrorV1> for Gfx942ScalarF32RecurrenceStepAnalysisErrorV1 {
    fn from(error: Gfx942RegisterFactsErrorV1) -> Self {
        Self::RegisterFacts(error)
    }
}

impl From<Gfx942MachineDataflowErrorV1> for Gfx942ScalarF32RecurrenceStepAnalysisErrorV1 {
    fn from(error: Gfx942MachineDataflowErrorV1) -> Self {
        Self::Dataflow(error)
    }
}

/// Failure that returns authenticated analyzer custody for exact retry or audit.
pub struct Gfx942ScalarF32RecurrenceStepAnalysisFailureV1 {
    execution: Box<AuthenticatedPhysicalMachineAnalysisExecutionV1>,
    error: Gfx942ScalarF32RecurrenceStepAnalysisErrorV1,
}

impl fmt::Debug for Gfx942ScalarF32RecurrenceStepAnalysisFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32RecurrenceStepAnalysisFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32RecurrenceStepAnalysisFailureV1 {
    pub const fn error(&self) -> &Gfx942ScalarF32RecurrenceStepAnalysisErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        AuthenticatedPhysicalMachineAnalysisExecutionV1,
        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1,
    ) {
        (*self.execution, self.error)
    }
}

/// Produces an inert recurrence-step artifact while retaining authenticated analyzer custody.
pub fn check_authenticated_gfx942_scalar_f32_recurrence_step_v1(
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
) -> Result<
    AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    Gfx942ScalarF32RecurrenceStepAnalysisFailureV1,
> {
    match analyze_recurrence_step(&execution, kernel_symbol) {
        Ok(artifact) => Ok(AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
            execution,
            artifact,
        }),
        Err(error) => Err(Gfx942ScalarF32RecurrenceStepAnalysisFailureV1 {
            execution: Box::new(execution),
            error,
        }),
    }
}

/// Replays analysis and consumes authenticated custody only when persisted bytes match exactly.
///
/// Decoding alone establishes only canonical structure. This entry point joins the bytes back to
/// one live authenticated analyzer result and reruns every step obligation before returning the
/// move-only owner.
pub fn verify_authenticated_gfx942_scalar_f32_recurrence_step_artifact_v1(
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
    artifact_bytes: &[u8],
) -> Result<
    AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    Gfx942ScalarF32RecurrenceStepAnalysisFailureV1,
> {
    let result = Gfx942ScalarF32RecurrenceStepArtifactV1::decode_canonical(artifact_bytes)
        .map_err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::Artifact)
        .and_then(|decoded| {
            let derived = analyze_recurrence_step(&execution, kernel_symbol)?;
            if decoded.canonical_bytes() != derived.canonical_bytes() {
                return Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ArtifactMismatch);
            }
            Ok(derived)
        });
    match result {
        Ok(artifact) => Ok(AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
            execution,
            artifact,
        }),
        Err(error) => Err(Gfx942ScalarF32RecurrenceStepAnalysisFailureV1 {
            execution: Box::new(execution),
            error,
        }),
    }
}

fn analyze_recurrence_step(
    execution: &AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
) -> Result<Gfx942ScalarF32RecurrenceStepArtifactV1, Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
    if !valid_symbol(kernel_symbol) {
        return Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidKernelSymbol);
    }
    if execution
        .request()
        .entries()
        .iter()
        .filter(|entry| entry.symbol() == kernel_symbol)
        .count()
        != 1
    {
        return Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::KernelNotRequested);
    }
    let trace = execution.analysis().trace();
    let instructions = trace
        .instructions()
        .iter()
        .filter(|instruction| instruction.function_symbol() == kernel_symbol)
        .collect::<Vec<_>>();
    let multiplies = instructions
        .iter()
        .copied()
        .filter(|instruction| instruction.opcode() == "V_MUL_F32_e32_vi")
        .collect::<Vec<_>>();
    let adds = instructions
        .iter()
        .copied()
        .filter(|instruction| instruction.opcode() == "V_ADD_F32_e32_vi")
        .collect::<Vec<_>>();
    let [multiply] = multiplies.as_slice() else {
        return Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::WrongMultiplyCount {
                actual: multiplies.len(),
            },
        );
    };
    let [add] = adds.as_slice() else {
        return Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::WrongAddCount { actual: adds.len() },
        );
    };
    let multiply = *multiply;
    let add = *add;
    let multiply_shape = arithmetic_shape(multiply)?;
    let add_shape = arithmetic_shape(add)?;
    let dataflow = Gfx942MachineDataflowV1::derive(trace)?;
    if !dataflow.instruction_dominates(
        kernel_symbol,
        multiply.instruction_offset(),
        add.instruction_offset(),
    )? {
        return Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::MultiplyDoesNotDominateAdd);
    }
    let product_positions = add_shape
        .sources
        .iter()
        .enumerate()
        .filter(|(_, source)| **source == multiply_shape.destination)
        .map(|(index, _)| index as u8)
        .collect::<Vec<_>>();
    let [product_position] = product_positions.as_slice() else {
        return Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductDoesNotReachAdd);
    };
    if *product_position != 0 {
        return Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductOperandPosition {
                actual: *product_position,
            },
        );
    }
    require_step_input_provenance_root(
        &dataflow,
        &instructions,
        kernel_symbol,
        add.instruction_offset(),
        multiply_shape.destination,
        Some(multiply.instruction_offset()),
    )?;
    let Some(accumulator) = add_shape.sources.get(1).copied() else {
        return Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::AmbiguousAccumulatorOperand);
    };
    if accumulator == multiply_shape.destination {
        return Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::AmbiguousAccumulatorOperand);
    }
    if add_shape.destination != accumulator {
        return Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ResultDoesNotUpdateAccumulator {
                result: vgpr_index(add_shape.destination),
                accumulator: vgpr_index(accumulator),
            },
        );
    }
    require_step_input_provenance_root(
        &dataflow,
        &instructions,
        kernel_symbol,
        add.instruction_offset(),
        accumulator,
        None,
    )?;

    let execution_identity = execution.identity();
    let trace_identity = trace.identity();
    let mut artifact = Gfx942ScalarF32RecurrenceStepArtifactV1 {
        authenticated_execution_sha256: execution_identity.sha256(),
        authenticated_execution_bytes: execution_identity.byte_len(),
        trace_sha256: trace_identity.sha256(),
        trace_bytes: trace_identity.byte_len(),
        function_symbol: kernel_symbol.to_owned(),
        multiply_offset: multiply.instruction_offset(),
        add_offset: add.instruction_offset(),
        multiply_encoding_sha256: Sha256::digest(multiply.encoding()).into(),
        multiply_encoding_bytes: multiply.encoding().len() as u16,
        add_encoding_sha256: Sha256::digest(add.encoding()).into(),
        add_encoding_bytes: add.encoding().len() as u16,
        product_register: vgpr_index(multiply_shape.destination),
        accumulator_register: vgpr_index(accumulator),
        result_register: vgpr_index(add_shape.destination),
        product_source_operand_index: 0,
        accumulator_source_operand_index: 1,
        canonical_bytes: Box::new([]),
    };
    artifact.canonical_bytes = encode_artifact(&artifact).into_boxed_slice();
    debug_assert!(
        artifact.canonical_bytes.len() <= MAX_GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_BYTES_V1
    );
    Ok(artifact)
}

struct ArithmeticShapeV1 {
    destination: Gfx942RegisterUnitV1,
    sources: Vec<Gfx942RegisterUnitV1>,
}

fn arithmetic_shape(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<ArithmeticShapeV1, Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
    if instruction.branch_target().is_some()
        || instruction.flags().is_terminator()
        || instruction.flags().may_trap()
        || instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
    {
        return Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
                offset: instruction.instruction_offset(),
            },
        );
    }
    let facts = Gfx942InstructionRegisterFactsV1::derive(instruction)?;
    if facts.explicit_definition_count() != 1 || facts.operand_aliases().len() != 3 {
        return Err(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
                offset: instruction.instruction_offset(),
            },
        );
    }
    let destination = single_vgpr(facts.operand_aliases()[0].as_ref()).ok_or(
        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
            offset: instruction.instruction_offset(),
        },
    )?;
    let sources = facts.operand_aliases()[1..]
        .iter()
        .map(|alias| single_vgpr(alias.as_ref()))
        .collect::<Option<Vec<_>>>()
        .ok_or(
            Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::InvalidArithmeticInstruction {
                offset: instruction.instruction_offset(),
            },
        )?;
    Ok(ArithmeticShapeV1 {
        destination,
        sources,
    })
}

fn single_vgpr(alias: Option<&crate::Gfx942RegisterAliasV1>) -> Option<Gfx942RegisterUnitV1> {
    let [unit @ Gfx942RegisterUnitV1::Vgpr(_)] = alias?.units() else {
        return None;
    };
    Some(*unit)
}

fn require_step_input_provenance_root(
    dataflow: &Gfx942MachineDataflowV1,
    instructions: &[&PhysicalMachineInstructionTraceV1],
    function: &str,
    initial_use_offset: u64,
    initial_unit: Gfx942RegisterUnitV1,
    expected_instruction_root: Option<u64>,
) -> Result<(), Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
    let mut use_offset = initial_use_offset;
    let mut unit = initial_unit;
    let mut visited = BTreeSet::new();
    for _ in 0..=instructions.len() {
        if !visited.insert((use_offset, unit)) {
            return Err(
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::StepInputProvenanceCycle {
                    offset: use_offset,
                    register: vgpr_index(unit),
                },
            );
        }
        let definitions = dataflow.reaching_definitions_before(function, use_offset, unit)?;
        let [definition] = definitions.as_slice() else {
            return Err(
                Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::NonUniqueStepInputProvenance {
                    use_offset,
                    definitions: definitions.len(),
                },
            );
        };
        match *definition {
            Gfx942ReachingDefinitionV1::LiveIn => {
                return if expected_instruction_root.is_none() {
                    Ok(())
                } else {
                    Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::ProductDoesNotReachAdd)
                };
            }
            Gfx942ReachingDefinitionV1::Instruction { offset }
                if Some(offset) == expected_instruction_root =>
            {
                return Ok(());
            }
            Gfx942ReachingDefinitionV1::Instruction { offset } => {
                let instruction = instructions
                    .iter()
                    .copied()
                    .find(|instruction| instruction.instruction_offset() == offset)
                    .ok_or(
                        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::UnsupportedStepInputDefinition {
                            offset,
                        },
                    )?;
                if is_fused_f32_opcode(instruction.opcode()) {
                    return Err(
                        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::FusedDefinitionReachesAdd {
                            offset,
                        },
                    );
                }
                let Some(source) = admitted_register_copy_source(instruction, unit)? else {
                    return Err(
                        Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::UnsupportedStepInputDefinition {
                            offset,
                        },
                    );
                };
                use_offset = offset;
                unit = source;
            }
        }
    }
    Err(Gfx942ScalarF32RecurrenceStepAnalysisErrorV1::StepInputProvenanceLimit)
}

fn admitted_register_copy_source(
    instruction: &PhysicalMachineInstructionTraceV1,
    expected_destination: Gfx942RegisterUnitV1,
) -> Result<Option<Gfx942RegisterUnitV1>, Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
    if instruction.opcode() != "V_MOV_B32_e32"
        || instruction.branch_kind() != PhysicalMachineBranchKindV1::None
        || instruction.branch_target().is_some()
        || instruction.flags().bits() != 0
        || instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
    {
        return Ok(None);
    }
    let facts = Gfx942InstructionRegisterFactsV1::derive(instruction)?;
    if facts.explicit_definition_count() != 1
        || facts.operand_aliases().len() != 2
        || !facts.implicit_definitions().is_empty()
        || !facts.implicit_uses().is_empty()
    {
        return Ok(None);
    }
    let Some(destination) = single_vgpr(facts.operand_aliases()[0].as_ref()) else {
        return Ok(None);
    };
    let Some(source) = single_vgpr(facts.operand_aliases()[1].as_ref()) else {
        return Ok(None);
    };
    Ok((destination == expected_destination).then_some(source))
}

fn is_fused_f32_opcode(opcode: &str) -> bool {
    opcode.starts_with("V_FMA") || opcode.starts_with("V_FMAC") || opcode.starts_with("V_MAD_F32")
}

const fn vgpr_index(unit: Gfx942RegisterUnitV1) -> u16 {
    match unit {
        Gfx942RegisterUnitV1::Vgpr(index) => index,
        _ => unreachable!(),
    }
}

fn numeric_model_identity() -> [u8; 32] {
    domain_hash(
        NUMERIC_MODEL_IDENTITY_DOMAIN_V1,
        NUMERIC_MODEL_DESCRIPTION_V1,
    )
}

fn encode_artifact(artifact: &Gfx942ScalarF32RecurrenceStepArtifactV1) -> Vec<u8> {
    let mut output = Vec::with_capacity(256);
    output.extend_from_slice(GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(
        &mut output,
        GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_VERSION_V1,
    );
    output.push(TARGET_TAG_GFX942_XNACK_MINUS_COV6);
    output.push(POLICY_TAG_SEPARATE_MUL_ADD_RNE_PRESERVE_SUBNORMALS);
    push_u16(&mut output, CHECKED_FACTS_V1);
    output.extend_from_slice(&numeric_model_identity());
    output.extend_from_slice(&artifact.authenticated_execution_sha256);
    push_u64(&mut output, artifact.authenticated_execution_bytes);
    output.extend_from_slice(&artifact.trace_sha256);
    push_u64(&mut output, artifact.trace_bytes);
    push_text(&mut output, &artifact.function_symbol);
    push_u64(&mut output, artifact.multiply_offset);
    output.extend_from_slice(&artifact.multiply_encoding_sha256);
    push_u16(&mut output, artifact.multiply_encoding_bytes);
    push_u64(&mut output, artifact.add_offset);
    output.extend_from_slice(&artifact.add_encoding_sha256);
    push_u16(&mut output, artifact.add_encoding_bytes);
    push_u16(&mut output, artifact.product_register);
    push_u16(&mut output, artifact.accumulator_register);
    push_u16(&mut output, artifact.result_register);
    output.push(artifact.product_source_operand_index);
    output.push(artifact.accumulator_source_operand_index);
    let len = output.len() as u32;
    let offset = GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&len.to_le_bytes());
    output
}

fn decode_artifact(
    bytes: &[u8],
) -> Result<Gfx942ScalarF32RecurrenceStepArtifactV1, Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
    if bytes.len() > MAX_GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_BYTES_V1 {
        return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::RecordTooLarge);
    }
    let mut reader = ArtifactReaderV1::new(bytes);
    reader.expect(GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_DOMAIN_V1)?;
    if reader.u32()? as usize != bytes.len() {
        return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::LengthMismatch);
    }
    if reader.u16()? != GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_VERSION_V1 {
        return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::UnsupportedVersion);
    }
    if reader.u8()? != TARGET_TAG_GFX942_XNACK_MINUS_COV6
        || reader.u8()? != POLICY_TAG_SEPARATE_MUL_ADD_RNE_PRESERVE_SUBNORMALS
        || reader.u16()? != CHECKED_FACTS_V1
        || reader.array::<32>()? != numeric_model_identity()
    {
        return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::PolicyMismatch);
    }
    let authenticated_execution_sha256 = reader.array()?;
    let authenticated_execution_bytes = reader.u64()?;
    let trace_sha256 = reader.array()?;
    let trace_bytes = reader.u64()?;
    let function_symbol = reader.text()?;
    let multiply_offset = reader.u64()?;
    let multiply_encoding_sha256 = reader.array()?;
    let multiply_encoding_bytes = reader.u16()?;
    let add_offset = reader.u64()?;
    let add_encoding_sha256 = reader.array()?;
    let add_encoding_bytes = reader.u16()?;
    let product_register = reader.u16()?;
    let accumulator_register = reader.u16()?;
    let result_register = reader.u16()?;
    let product_source_operand_index = reader.u8()?;
    let accumulator_source_operand_index = reader.u8()?;
    reader.finish()?;
    if authenticated_execution_sha256 == [0; 32]
        || authenticated_execution_bytes == 0
        || trace_sha256 == [0; 32]
        || trace_bytes == 0
        || !valid_symbol(&function_symbol)
        || multiply_offset >= add_offset
        || multiply_encoding_sha256 == [0; 32]
        || multiply_encoding_bytes == 0
        || add_encoding_sha256 == [0; 32]
        || add_encoding_bytes == 0
        || product_register > crate::MAX_GFX942_VGPR_INDEX_V1
        || accumulator_register > crate::MAX_GFX942_VGPR_INDEX_V1
        || result_register > crate::MAX_GFX942_VGPR_INDEX_V1
        || product_register == accumulator_register
        || result_register != accumulator_register
        || product_source_operand_index != 0
        || accumulator_source_operand_index != 1
    {
        return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::InvalidField);
    }
    let artifact = Gfx942ScalarF32RecurrenceStepArtifactV1 {
        authenticated_execution_sha256,
        authenticated_execution_bytes,
        trace_sha256,
        trace_bytes,
        function_symbol,
        multiply_offset,
        add_offset,
        multiply_encoding_sha256,
        multiply_encoding_bytes,
        add_encoding_sha256,
        add_encoding_bytes,
        product_register,
        accumulator_register,
        result_register,
        product_source_operand_index,
        accumulator_source_operand_index,
        canonical_bytes: bytes.into(),
    };
    if encode_artifact(&artifact) != bytes {
        return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::NonCanonical);
    }
    Ok(artifact)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ScalarF32RecurrenceStepArtifactErrorV1 {
    RecordTooLarge,
    DomainMismatch,
    LengthMismatch,
    UnsupportedVersion,
    PolicyMismatch,
    Truncated,
    TrailingBytes,
    InvalidText,
    InvalidField,
    NonCanonical,
}

impl fmt::Display for Gfx942ScalarF32RecurrenceStepArtifactErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid gfx942 scalar f32 recurrence-step artifact: {self:?}"
        )
    }
}

impl Error for Gfx942ScalarF32RecurrenceStepArtifactErrorV1 {}

struct ArtifactReaderV1<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> ArtifactReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'a [u8], Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }

    fn expect(
        &mut self,
        expected: &[u8],
    ) -> Result<(), Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        if self.take(expected.len())? != expected {
            return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::DomainMismatch);
        }
        Ok(())
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| Gfx942ScalarF32RecurrenceStepArtifactErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn text(&mut self) -> Result<String, Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        let len = self.u16()? as usize;
        let value = std::str::from_utf8(self.take(len)?)
            .map_err(|_| Gfx942ScalarF32RecurrenceStepArtifactErrorV1::InvalidText)?;
        if !valid_symbol(value) {
            return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::InvalidText);
        }
        Ok(value.to_owned())
    }

    fn finish(self) -> Result<(), Gfx942ScalarF32RecurrenceStepArtifactErrorV1> {
        if self.position != self.bytes.len() {
            return Err(Gfx942ScalarF32RecurrenceStepArtifactErrorV1::TrailingBytes);
        }
        Ok(())
    }
}

fn valid_symbol(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 256
        && (bytes[0].is_ascii_alphabetic() || matches!(bytes[0], b'_' | b'.' | b'$'))
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$'))
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    push_u16(output, value.len() as u16);
    output.extend_from_slice(value.as_bytes());
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separate_rounding_differs_from_fused_for_a_known_witness() {
        // Generated once with the same pinned APFloat implementation; the comparison, not the
        // decimal spelling, is the contraction obligation used by this model.
        let left = 0x3f80_0001;
        let right = 0x3f7f_ffff;
        let addend = 0xbf80_0000;
        let separate = execute_gfx942_scalar_f32_recurrence_step_candidate_v1(addend, left, right);
        let fused = execute_binary32_fused_multiply_add_reference_v1(left, right, addend);
        assert_ne!(separate.accumulator_bits(), fused.0);
    }

    #[test]
    fn candidate_add_order_retains_the_product_nan_payload() {
        let product_nan = 0x7fc0_0042;
        let accumulator_nan = 0xffc0_0099;
        let step = execute_gfx942_scalar_f32_recurrence_step_candidate_v1(
            accumulator_nan,
            product_nan,
            0x3f80_0000,
        );
        assert_eq!(step.product_bits(), product_nan);
        assert_eq!(step.accumulator_bits(), product_nan);
    }

    #[test]
    fn exceptional_values_and_zero_iteration_are_explicit() {
        let zero = execute_gfx942_scalar_f32_dot_product_candidate_v1(&[]).unwrap();
        assert_eq!(zero.accumulator_bits(), 0);
        assert_eq!(zero.iterations(), 0);

        let invalid = execute_gfx942_scalar_f32_recurrence_step_candidate_v1(0, 0x7f80_0000, 0);
        assert!(invalid.multiply_status().invalid_operation());
        assert_eq!(invalid.accumulator_bits() & 0x7f80_0000, 0x7f80_0000);

        let subnormal = execute_gfx942_scalar_f32_recurrence_step_candidate_v1(0, 1, 0x3f80_0000);
        assert_eq!(subnormal.product_bits(), 1);
        assert_eq!(subnormal.accumulator_bits(), 1);

        let negative_zero =
            execute_gfx942_scalar_f32_dot_product_candidate_v1(&[(0x8000_0000, 0x3f80_0000)])
                .unwrap();
        assert_eq!(negative_zero.accumulator_bits(), 0);
    }

    #[test]
    fn iteration_bound_rejects_before_execution() {
        let inputs = vec![(0, 0); MAX_GFX942_SCALAR_F32_RECURRENCE_ITERATIONS_V1 + 1];
        assert!(matches!(
            execute_gfx942_scalar_f32_dot_product_candidate_v1(&inputs),
            Err(Gfx942ScalarF32ExecutionErrorV1::IterationLimit { .. })
        ));
    }
}
