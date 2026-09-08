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
use fe2o3_compiler_lineage::{
    CheckedPostLlvmStageContentsV1, StructuredKirBlockLoweringV1,
    StructuredKirControlEdgeLoweringV1, StructuredKirOperationKindV1,
    StructuredKirOperationLoweringV1, StructuredKirToLlvmDerivationErrorV1,
    StructuredKirToLlvmDerivationV1, StructuredKirValueCarrierV1, StructuredKirValueLoweringV1,
    StructuredKirValueTypeV1, StructuredLlvmNumericalPolicyV1, StructuredLlvmOpcodeV1,
    StructuredLlvmTargetV1,
};
use fe2o3_kernel_ir as kir;
use rustc_apfloat::ieee::Single;
use rustc_apfloat::{Float, Round, Status};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

pub const GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-RECURRENCE-STEP-CANDIDATE/V1\0";
pub const GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_VERSION_V1: u16 = 1;
pub const MAX_GFX942_SCALAR_F32_RECURRENCE_STEP_ARTIFACT_BYTES_V1: usize = 1024;
pub const MAX_GFX942_SCALAR_F32_RECURRENCE_ITERATIONS_V1: usize = 1_048_576;

const ARTIFACT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-RECURRENCE-STEP-CANDIDATE-IDENTITY/V1\0";
const NUMERIC_MODEL_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/GFX942-SCALAR-F32-NUMERIC-MODEL/V1\0";
pub const GFX942_SCALAR_F32_MACHINE_REFINEMENT_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-TO-FINAL-MACHINE-REFINEMENT/V1\0";
pub const GFX942_SCALAR_F32_MACHINE_REFINEMENT_EVIDENCE_VERSION_V1: u16 = 1;
const GFX942_SCALAR_F32_MACHINE_REFINEMENT_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-TO-FINAL-MACHINE-REFINEMENT-IDENTITY/V1\0";
pub const GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-RECURRENCE/V1\0";
pub const GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_VERSION_V1: u16 = 1;
const GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-RECURRENCE-IDENTITY/V1\0";
pub const GFX942_SCALAR_F32_KIR_TO_LLVM_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-TO-LLVM-REFINEMENT/V1\0";
pub const GFX942_SCALAR_F32_KIR_TO_LLVM_EVIDENCE_VERSION_V1: u16 = 1;
const GFX942_SCALAR_F32_KIR_TO_LLVM_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-TO-LLVM-REFINEMENT-IDENTITY/V1\0";
pub const GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-MACHINE-RECURRENCE-BINDING/V1\0";
pub const GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_VERSION_V1: u16 = 1;
const GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-SCALAR-F32-KIR-MACHINE-RECURRENCE-BINDING-IDENTITY/V1\0";
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

    /// Returns the authenticated analyzer occurrence and its exact HSACO request payload.
    pub const fn authenticated_execution(
        &self,
    ) -> &AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        &self.execution
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

/// Input matrix whose canonical KIR address recurrence was rejected.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942ScalarF32KirInputV1 {
    A,
    B,
}

/// Named failures for the exact bounded scalar-GEMM KIR V13 profile.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942ScalarF32KirRecurrenceErrorV1 {
    InvalidKernelSymbol,
    InvalidCanonicalKir(Box<kir::VerifiedCanonicalKernelIrErrorV13>),
    CanonicalKirDecode,
    ModuleShape,
    KernelBinding,
    FunctionSignature,
    ContextCapabilityBinding,
    UnsupportedCfgShape,
    OuterGuardMismatch,
    LoopTripMismatch,
    AccumulatorPhiSourceMismatch,
    AddressStrideMismatch { input: Gfx942ScalarF32KirInputV1 },
    ReadGuardMismatch { input: Gfx942ScalarF32KirInputV1 },
    ReadCount { actual: usize },
    RecurrenceSsaMismatch,
    WriteDestinationMismatch,
    WriteGuardMismatch,
    WriteCount { actual: usize },
    InactivePathMismatch,
    WriterInjectivityMismatch,
}

impl fmt::Display for Gfx942ScalarF32KirRecurrenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid bounded scalar-f32 GEMM KIR V13 recurrence: {self:?}"
        )
    }
}

impl Error for Gfx942ScalarF32KirRecurrenceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidCanonicalKir(error) => Some(error),
            _ => None,
        }
    }
}

/// Identity of exact canonical KIR-recurrence evidence bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942ScalarF32KirRecurrenceEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942ScalarF32KirRecurrenceEvidenceIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Move-only exact KIR V13 owner after the bounded scalar-GEMM recurrence check.
///
/// This proves only the accepted KIR graph class. It does not authenticate compiler provenance,
/// LLVM lowering, instruction selection, or final-machine behavior.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedGfx942ScalarF32KirRecurrenceV1;
///
/// fn consume_twice(owner: CheckedGfx942ScalarF32KirRecurrenceV1) {
///     let _first = owner.into_verified_canonical_kir();
///     let _second = owner.into_verified_canonical_kir();
/// }
/// ```
pub struct CheckedGfx942ScalarF32KirRecurrenceV1 {
    canonical_kir: kir::VerifiedCanonicalKernelIrV13,
    function_symbol: String,
    canonical_evidence: Box<[u8]>,
}

impl fmt::Debug for CheckedGfx942ScalarF32KirRecurrenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx942ScalarF32KirRecurrenceV1")
            .field("function_symbol", &self.function_symbol)
            .field("evidence_identity", &self.evidence_identity())
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942ScalarF32KirRecurrenceV1 {
    pub const fn verified_canonical_kir(&self) -> &kir::VerifiedCanonicalKernelIrV13 {
        &self.canonical_kir
    }

    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }

    pub fn canonical_evidence_bytes(&self) -> &[u8] {
        &self.canonical_evidence
    }

    pub fn evidence_identity(&self) -> Gfx942ScalarF32KirRecurrenceEvidenceIdentityV1 {
        gfx942_scalar_f32_kir_recurrence_evidence_identity_v1(&self.canonical_evidence)
    }

    pub const fn establishes_exact_kir_recurrence_ssa_cfg_phi_and_effects(&self) -> bool {
        true
    }

    pub const fn establishes_kir_to_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    pub fn into_verified_canonical_kir(self) -> kir::VerifiedCanonicalKernelIrV13 {
        self.canonical_kir
    }
}

/// Stable failure categories for the exact scalar-f32 KIR-to-LLVM replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942ScalarF32KirToLlvmRefinementErrorV1 {
    InvalidDerivation,
    KirSubjectMismatch,
    LlvmSubjectMismatch,
    TargetMismatch,
    NumericalPolicyMismatch,
    FunctionBindingMismatch,
    WorkgroupMismatch,
    BlockRosterMismatch,
    OperationRosterMismatch,
    ValueRosterMismatch,
    ControlEdgeRosterMismatch,
}

impl fmt::Display for Gfx942ScalarF32KirToLlvmRefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx942 scalar-f32 structured KIR-to-LLVM replay failed: {self:?}"
        )
    }
}

impl Error for Gfx942ScalarF32KirToLlvmRefinementErrorV1 {}

/// Identity of the canonical authority-free scalar-f32 KIR-to-LLVM receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942ScalarF32KirToLlvmRefinementIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942ScalarF32KirToLlvmRefinementIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Move-only exact scalar-f32 KIR V13 to structured gfx942 LLVM refinement receipt.
///
/// The checker independently reconstructs every ABI value, block, operation, result carrier, and
/// CFG edge record from the already checked KIR owner. The receipt binds exact LLVM bytes, but it
/// does not claim optimization, instruction-selection, object, ISA, or machine equivalence.
#[must_use = "dropping structured lowering custody abandons an issue #214 prerequisite"]
pub struct CheckedGfx942ScalarF32KirToLlvmRefinementV1 {
    kir: CheckedGfx942ScalarF32KirRecurrenceV1,
    llvm_ir: Box<[u8]>,
    derivation: StructuredKirToLlvmDerivationV1,
    canonical_evidence: Box<[u8]>,
}

impl fmt::Debug for CheckedGfx942ScalarF32KirToLlvmRefinementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx942ScalarF32KirToLlvmRefinementV1")
            .field("function_symbol", &self.kir.function_symbol())
            .field("identity", &self.evidence_identity())
            .field("derivation", &self.derivation.identity())
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942ScalarF32KirToLlvmRefinementV1 {
    pub const fn kir(&self) -> &CheckedGfx942ScalarF32KirRecurrenceV1 {
        &self.kir
    }

    pub fn llvm_ir(&self) -> &[u8] {
        &self.llvm_ir
    }

    pub const fn derivation(&self) -> &StructuredKirToLlvmDerivationV1 {
        &self.derivation
    }

    pub fn canonical_evidence_bytes(&self) -> &[u8] {
        &self.canonical_evidence
    }

    pub fn evidence_identity(&self) -> Gfx942ScalarF32KirToLlvmRefinementIdentityV1 {
        let mut digest = Sha256::new();
        digest.update(GFX942_SCALAR_F32_KIR_TO_LLVM_EVIDENCE_IDENTITY_DOMAIN_V1);
        digest.update((self.canonical_evidence.len() as u64).to_le_bytes());
        digest.update(&self.canonical_evidence);
        Gfx942ScalarF32KirToLlvmRefinementIdentityV1 {
            sha256: digest.finalize().into(),
            byte_len: self.canonical_evidence.len() as u64,
        }
    }

    pub const fn establishes_exact_structured_kir_to_llvm_refinement(&self) -> bool {
        true
    }

    pub const fn establishes_llvm_to_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_compiler_or_runtime_authority(&self) -> bool {
        false
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirRecurrenceV1,
        Box<[u8]>,
        StructuredKirToLlvmDerivationV1,
        Box<[u8]>,
    ) {
        (
            self.kir,
            self.llvm_ir,
            self.derivation,
            self.canonical_evidence,
        )
    }
}

/// Failed structured replay with every move-only input retained.
pub struct Gfx942ScalarF32KirToLlvmRefinementFailureV1 {
    kir: CheckedGfx942ScalarF32KirRecurrenceV1,
    llvm_ir: Box<[u8]>,
    derivation: StructuredKirToLlvmDerivationV1,
    error: Gfx942ScalarF32KirToLlvmRefinementErrorV1,
}

impl fmt::Debug for Gfx942ScalarF32KirToLlvmRefinementFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32KirToLlvmRefinementFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32KirToLlvmRefinementFailureV1 {
    pub const fn error(&self) -> Gfx942ScalarF32KirToLlvmRefinementErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirRecurrenceV1,
        Box<[u8]>,
        StructuredKirToLlvmDerivationV1,
        Gfx942ScalarF32KirToLlvmRefinementErrorV1,
    ) {
        (self.kir, self.llvm_ir, self.derivation, self.error)
    }
}

/// Independently replays the exact scalar-f32 structured lowering derivation.
pub fn check_gfx942_scalar_f32_structured_kir_to_llvm_refinement_v1(
    kir: CheckedGfx942ScalarF32KirRecurrenceV1,
    llvm_ir: impl Into<Box<[u8]>>,
    derivation: StructuredKirToLlvmDerivationV1,
) -> Result<
    CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    Box<Gfx942ScalarF32KirToLlvmRefinementFailureV1>,
> {
    let llvm_ir = llvm_ir.into();
    let result = replay_gfx942_scalar_f32_structured_lowering_v1(&kir, &llvm_ir, &derivation);
    match result {
        Ok(()) => {
            let canonical_evidence = encode_kir_to_llvm_evidence_v1(&kir, &derivation);
            Ok(CheckedGfx942ScalarF32KirToLlvmRefinementV1 {
                kir,
                llvm_ir,
                derivation,
                canonical_evidence: canonical_evidence.into_boxed_slice(),
            })
        }
        Err(error) => Err(Box::new(Gfx942ScalarF32KirToLlvmRefinementFailureV1 {
            kir,
            llvm_ir,
            derivation,
            error,
        })),
    }
}

fn replay_gfx942_scalar_f32_structured_lowering_v1(
    checked: &CheckedGfx942ScalarF32KirRecurrenceV1,
    llvm_ir: &[u8],
    observed: &StructuredKirToLlvmDerivationV1,
) -> Result<(), Gfx942ScalarF32KirToLlvmRefinementErrorV1> {
    let canonical = checked.verified_canonical_kir();
    let identity = canonical.identity();
    if observed.kir_sha256() != *identity.digest()
        || observed.kir_byte_len() != identity.canonical_length()
    {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::KirSubjectMismatch);
    }
    let llvm_sha256: [u8; 32] = Sha256::digest(llvm_ir).into();
    if observed.llvm_byte_len() != llvm_ir.len() as u64 || observed.llvm_sha256() != llvm_sha256 {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::LlvmSubjectMismatch);
    }
    if observed.target() != StructuredLlvmTargetV1::AmdGfx942XnackMinus {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::TargetMismatch);
    }
    if observed.numerical_policy() != StructuredLlvmNumericalPolicyV1::IeeeBinary32SeparateMulAdd {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::NumericalPolicyMismatch);
    }
    let expected = expected_gfx942_scalar_f32_structured_derivation_v1(canonical, llvm_ir)
        .map_err(|_| Gfx942ScalarF32KirToLlvmRefinementErrorV1::InvalidDerivation)?;
    if observed.function_symbol() != expected.function_symbol() {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::FunctionBindingMismatch);
    }
    if observed.flat_workgroup_size() != expected.flat_workgroup_size() {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::WorkgroupMismatch);
    }
    if observed.blocks() != expected.blocks() {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::BlockRosterMismatch);
    }
    if observed.operations() != expected.operations() {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::OperationRosterMismatch);
    }
    if observed.values() != expected.values() {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::ValueRosterMismatch);
    }
    if observed.edges() != expected.edges() {
        return Err(Gfx942ScalarF32KirToLlvmRefinementErrorV1::ControlEdgeRosterMismatch);
    }
    Ok(())
}

fn encode_kir_to_llvm_evidence_v1(
    kir: &CheckedGfx942ScalarF32KirRecurrenceV1,
    derivation: &StructuredKirToLlvmDerivationV1,
) -> Vec<u8> {
    let kir_identity = kir.evidence_identity();
    let derivation_identity = derivation.identity();
    let mut output = Vec::with_capacity(192);
    output.extend_from_slice(GFX942_SCALAR_F32_KIR_TO_LLVM_EVIDENCE_DOMAIN_V1);
    push_u16(
        &mut output,
        GFX942_SCALAR_F32_KIR_TO_LLVM_EVIDENCE_VERSION_V1,
    );
    output.extend_from_slice(&kir_identity.sha256());
    push_u64(&mut output, kir_identity.byte_len());
    output.extend_from_slice(&derivation_identity.sha256());
    push_u64(&mut output, derivation_identity.byte_len());
    output.extend_from_slice(&derivation.llvm_sha256());
    push_u64(&mut output, derivation.llvm_byte_len());
    output
}

fn expected_gfx942_scalar_f32_structured_derivation_v1(
    canonical: &kir::VerifiedCanonicalKernelIrV13,
    llvm_ir: &[u8],
) -> Result<StructuredKirToLlvmDerivationV1, StructuredKirToLlvmDerivationErrorV1> {
    let module = kir::decode_module_v13(canonical.canonical_bytes())
        .map_err(|_| StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField)?;
    let function = &module.functions[0];
    let body = function
        .body
        .as_ref()
        .ok_or(StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField)?;
    let mut blocks = Vec::with_capacity(body.blocks.len());
    let mut operations = Vec::new();
    let mut values = Vec::new();
    let mut llvm_argument = 0_u16;
    for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        let ty = expected_structured_scalar_type_v1(ty)
            .ok_or(StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField)?;
        let count = if matches!(
            ty,
            StructuredKirValueTypeV1::GlobalReadSliceF32
                | StructuredKirValueTypeV1::GlobalWriteSliceF32
        ) {
            2
        } else {
            1
        };
        let first = llvm_argument;
        llvm_argument = llvm_argument
            .checked_add(count)
            .ok_or(StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?;
        values.push(StructuredKirValueLoweringV1::new(
            0,
            value.0,
            ty,
            StructuredKirValueCarrierV1::Argument {
                first,
                count: count as u8,
            },
        ));
    }
    for (block_ordinal, block) in body.blocks.iter().enumerate() {
        blocks.push(StructuredKirBlockLoweringV1::new(
            0,
            block.id.0,
            u32::try_from(block_ordinal)
                .map_err(|_| StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?,
        ));
        for (parameter_ordinal, parameter) in block.parameters.iter().enumerate() {
            let parameter_ordinal = u16::try_from(parameter_ordinal)
                .map_err(|_| StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?;
            values.push(StructuredKirValueLoweringV1::new(
                0,
                parameter.id.0,
                expected_structured_scalar_type_v1(&parameter.ty)
                    .ok_or(StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField)?,
                StructuredKirValueCarrierV1::Phi {
                    block: block.id.0,
                    ordinal: parameter_ordinal,
                },
            ));
        }
        for (operation_index, operation) in block.operations.iter().enumerate() {
            let operation_index = u32::try_from(operation_index)
                .map_err(|_| StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?;
            let (kind, llvm_opcodes) = expected_structured_scalar_operation_v1(operation)
                .ok_or(StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField)?;
            let llvm_instruction_count = u16::try_from(llvm_opcodes.len())
                .map_err(|_| StructuredKirToLlvmDerivationErrorV1::LengthOverflow)?;
            let mut result_types = Vec::with_capacity(operation.results.len());
            for result in &operation.results {
                let ty = expected_structured_scalar_type_v1(&result.ty)
                    .ok_or(StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField)?;
                result_types.push(ty);
                let carrier = match &operation.kind {
                    kir::OperationKind::KernelContextIssue(_) => {
                        StructuredKirValueCarrierV1::Erased
                    }
                    kir::OperationKind::GlobalCapabilityBind(binding) => {
                        StructuredKirValueCarrierV1::Alias {
                            value: binding.physical.0,
                        }
                    }
                    kir::OperationKind::GlobalCapabilityIndex(index) => {
                        StructuredKirValueCarrierV1::Alias {
                            value: index.index.0,
                        }
                    }
                    kir::OperationKind::Constant(_) => StructuredKirValueCarrierV1::Immediate,
                    _ => StructuredKirValueCarrierV1::Instruction {
                        block: block.id.0,
                        operation: operation_index,
                        ordinal: llvm_instruction_count.saturating_sub(1),
                    },
                };
                values.push(StructuredKirValueLoweringV1::new(
                    0,
                    result.id.0,
                    ty,
                    carrier,
                ));
            }
            operations.push(StructuredKirOperationLoweringV1::new(
                0,
                block.id.0,
                operation_index,
                kind,
                operation
                    .kind
                    .operands()
                    .into_iter()
                    .map(|value| value.0)
                    .collect::<Vec<_>>(),
                operation
                    .results
                    .iter()
                    .map(|result| result.id.0)
                    .collect::<Vec<_>>(),
                result_types,
                llvm_opcodes,
            )?);
        }
    }
    let edges = expected_structured_scalar_edges_v1(body)?;
    let identity = canonical.identity();
    StructuredKirToLlvmDerivationV1::new(
        *identity.digest(),
        identity.canonical_length(),
        llvm_ir,
        StructuredLlvmTargetV1::AmdGfx942XnackMinus,
        StructuredLlvmNumericalPolicyV1::IeeeBinary32SeparateMulAdd,
        function.id.as_str(),
        256,
        blocks,
        operations,
        values,
        edges,
    )
}

fn expected_structured_scalar_type_v1(ty: &kir::Type) -> Option<StructuredKirValueTypeV1> {
    match ty {
        kir::Type::KernelContext(_) => Some(StructuredKirValueTypeV1::KernelContext),
        kir::Type::Scalar(kir::ScalarType::Bool) => Some(StructuredKirValueTypeV1::Bool),
        kir::Type::Scalar(kir::ScalarType::Index) => Some(StructuredKirValueTypeV1::Index),
        kir::Type::Scalar(kir::ScalarType::F32) => Some(StructuredKirValueTypeV1::F32),
        kir::Type::Slice(slice)
            if slice.element.as_ref() == &kir::Type::F32
                && slice.address_space == kir::AddressSpace::Global
                && slice.access == kir::AccessMode::ReadOnly =>
        {
            Some(StructuredKirValueTypeV1::GlobalReadSliceF32)
        }
        kir::Type::Slice(slice)
            if slice.element.as_ref() == &kir::Type::F32
                && slice.address_space == kir::AddressSpace::Global
                && slice.access == kir::AccessMode::WriteOnly =>
        {
            Some(StructuredKirValueTypeV1::GlobalWriteSliceF32)
        }
        kir::Type::GlobalCapability(capability)
            if capability.element() == &kir::Type::F32
                && capability.role() == kir::GlobalCapabilityRoleV1::ReadOnly =>
        {
            Some(StructuredKirValueTypeV1::GlobalReadCapabilityF32)
        }
        kir::Type::GlobalCapability(capability)
            if capability.element() == &kir::Type::F32
                && matches!(
                    capability.role(),
                    kir::GlobalCapabilityRoleV1::DisjointWrite(_)
                ) =>
        {
            Some(StructuredKirValueTypeV1::GlobalWriteCapabilityF32)
        }
        kir::Type::Pointer(pointer)
            if pointer.pointee.as_ref() == &kir::Type::F32
                && pointer.address_space == kir::AddressSpace::Global
                && pointer.access == kir::AccessMode::ReadOnly =>
        {
            Some(StructuredKirValueTypeV1::GlobalReadPointerF32)
        }
        kir::Type::Pointer(pointer)
            if pointer.pointee.as_ref() == &kir::Type::F32
                && pointer.address_space == kir::AddressSpace::Global
                && pointer.access == kir::AccessMode::WriteOnly =>
        {
            Some(StructuredKirValueTypeV1::GlobalWritePointerF32)
        }
        _ => None,
    }
}

fn expected_structured_scalar_operation_v1(
    operation: &kir::Operation,
) -> Option<(StructuredKirOperationKindV1, Vec<StructuredLlvmOpcodeV1>)> {
    Some(match &operation.kind {
        kir::OperationKind::KernelContextIssue(_) => {
            (StructuredKirOperationKindV1::KernelContextIssue, vec![])
        }
        kir::OperationKind::GlobalCapabilityBind(_) => {
            (StructuredKirOperationKindV1::GlobalCapabilityBind, vec![])
        }
        kir::OperationKind::GlobalCapabilityIndex(_) => {
            (StructuredKirOperationKindV1::GlobalCapabilityIndex, vec![])
        }
        kir::OperationKind::Intrinsic(intrinsic)
            if matches!(
                intrinsic.kind,
                kir::IntrinsicKind::InvocationIndex {
                    kind: kir::IndexKind::Global,
                    axis: kir::Axis::X
                }
            ) =>
        {
            (
                StructuredKirOperationKindV1::GlobalId1d,
                vec![
                    StructuredLlvmOpcodeV1::CallWorkitemIdX,
                    StructuredLlvmOpcodeV1::CallWorkgroupIdX,
                    StructuredLlvmOpcodeV1::ZeroExtendI32ToI64,
                    StructuredLlvmOpcodeV1::ZeroExtendI32ToI64,
                    StructuredLlvmOpcodeV1::MultiplyI64,
                    StructuredLlvmOpcodeV1::AddI64,
                ],
            )
        }
        kir::OperationKind::Constant(kir::Constant::Index(_)) => {
            (StructuredKirOperationKindV1::ConstantIndex, vec![])
        }
        kir::OperationKind::Constant(kir::Constant::F32Bits(_)) => {
            (StructuredKirOperationKindV1::ConstantF32, vec![])
        }
        kir::OperationKind::Compare {
            predicate: kir::ComparePredicate::NotEqual,
            ..
        } => (
            StructuredKirOperationKindV1::IntegerNotEqual,
            vec![StructuredLlvmOpcodeV1::CompareNotEqualI64],
        ),
        kir::OperationKind::Compare {
            predicate: kir::ComparePredicate::LessThan,
            ..
        } => (
            StructuredKirOperationKindV1::IntegerLessThan,
            vec![StructuredLlvmOpcodeV1::CompareUnsignedLessThanI64],
        ),
        kir::OperationKind::Select { .. } => (
            StructuredKirOperationKindV1::Select,
            vec![StructuredLlvmOpcodeV1::Select],
        ),
        kir::OperationKind::Binary {
            op: kir::BinaryOp::Multiply,
            ..
        } if operation
            .results
            .first()
            .is_some_and(|result| result.ty == kir::Type::F32) =>
        {
            (
                StructuredKirOperationKindV1::F32Multiply,
                vec![StructuredLlvmOpcodeV1::MultiplyF32],
            )
        }
        kir::OperationKind::Binary {
            op: kir::BinaryOp::Add,
            ..
        } if operation
            .results
            .first()
            .is_some_and(|result| result.ty == kir::Type::F32) =>
        {
            (
                StructuredKirOperationKindV1::F32Add,
                vec![StructuredLlvmOpcodeV1::AddF32],
            )
        }
        kir::OperationKind::Binary {
            op: kir::BinaryOp::Multiply,
            ..
        } => (
            StructuredKirOperationKindV1::IntegerMultiply,
            vec![StructuredLlvmOpcodeV1::MultiplyI64],
        ),
        kir::OperationKind::Binary {
            op: kir::BinaryOp::Add,
            ..
        } => (
            StructuredKirOperationKindV1::IntegerAdd,
            vec![StructuredLlvmOpcodeV1::AddI64],
        ),
        kir::OperationKind::Binary {
            op: kir::BinaryOp::BitAnd,
            ..
        } => (
            StructuredKirOperationKindV1::BooleanAnd,
            vec![StructuredLlvmOpcodeV1::AndI1],
        ),
        kir::OperationKind::Binary {
            op: kir::BinaryOp::Divide,
            ..
        } => (
            StructuredKirOperationKindV1::IntegerDivide,
            vec![StructuredLlvmOpcodeV1::DivideUnsignedI64],
        ),
        kir::OperationKind::Binary {
            op: kir::BinaryOp::Remainder,
            ..
        } => (
            StructuredKirOperationKindV1::IntegerRemainder,
            vec![StructuredLlvmOpcodeV1::RemainderUnsignedI64],
        ),
        kir::OperationKind::SliceLength { .. } => (
            StructuredKirOperationKindV1::SliceLength,
            vec![StructuredLlvmOpcodeV1::CopySliceLengthI64],
        ),
        kir::OperationKind::SliceData { .. } => (
            StructuredKirOperationKindV1::SliceData,
            vec![StructuredLlvmOpcodeV1::ProjectSliceDataGlobal],
        ),
        kir::OperationKind::GetElementPointer { .. } => (
            StructuredKirOperationKindV1::GlobalGetElementPointer,
            vec![StructuredLlvmOpcodeV1::GetElementPointerGlobalF32],
        ),
        kir::OperationKind::GuardedLoad { access, .. }
            if access.address_space == kir::AddressSpace::Global && access.alignment == 4 =>
        {
            (
                StructuredKirOperationKindV1::GuardedLoadF32,
                vec![
                    StructuredLlvmOpcodeV1::ConditionalBranch,
                    StructuredLlvmOpcodeV1::LoadGlobalF32,
                    StructuredLlvmOpcodeV1::Branch,
                    StructuredLlvmOpcodeV1::Branch,
                    StructuredLlvmOpcodeV1::PhiF32,
                ],
            )
        }
        kir::OperationKind::GuardedStore { access, .. }
            if access.address_space == kir::AddressSpace::Global && access.alignment == 4 =>
        {
            (
                StructuredKirOperationKindV1::GuardedStoreF32,
                vec![
                    StructuredLlvmOpcodeV1::ConditionalBranch,
                    StructuredLlvmOpcodeV1::StoreGlobalF32,
                    StructuredLlvmOpcodeV1::Branch,
                ],
            )
        }
        _ => return None,
    })
}

fn expected_structured_scalar_edges_v1(
    body: &kir::FunctionBody,
) -> Result<Vec<StructuredKirControlEdgeLoweringV1>, StructuredKirToLlvmDerivationErrorV1> {
    let mut raw = Vec::<(u32, u16, u32, Vec<u32>)>::new();
    for block in &body.blocks {
        match block.terminator.as_ref() {
            Some(kir::Terminator::Branch { target, arguments }) => raw.push((
                block.id.0,
                0,
                target.0,
                arguments.iter().map(|value| value.0).collect(),
            )),
            Some(kir::Terminator::ConditionalBranch {
                then_target,
                then_arguments,
                else_target,
                else_arguments,
                ..
            }) => {
                raw.push((
                    block.id.0,
                    0,
                    then_target.0,
                    then_arguments.iter().map(|value| value.0).collect(),
                ));
                raw.push((
                    block.id.0,
                    1,
                    else_target.0,
                    else_arguments.iter().map(|value| value.0).collect(),
                ));
            }
            Some(kir::Terminator::Return { values }) if values.is_empty() => {}
            _ => return Err(StructuredKirToLlvmDerivationErrorV1::EmptyRequiredField),
        }
    }
    raw.iter()
        .map(|(predecessor, ordinal, successor, arguments)| {
            let outgoing = raw.iter().filter(|edge| edge.0 == *predecessor).count();
            let incoming = raw.iter().filter(|edge| edge.2 == *successor).count();
            let duplicate = raw
                .iter()
                .filter(|edge| edge.0 == *predecessor && edge.2 == *successor)
                .count()
                > 1;
            let target_has_phi = body
                .blocks
                .iter()
                .find(|block| block.id.0 == *successor)
                .is_some_and(|block| !block.parameters.is_empty());
            StructuredKirControlEdgeLoweringV1::new(
                0,
                *predecessor,
                *ordinal,
                *successor,
                arguments.clone(),
                target_has_phi && (duplicate || (outgoing > 1 && incoming > 1)),
            )
        })
        .collect()
}

/// Failed KIR analysis with exact canonical custody retained.
pub struct Gfx942ScalarF32KirRecurrenceFailureV1 {
    canonical_kir: kir::VerifiedCanonicalKernelIrV13,
    error: Gfx942ScalarF32KirRecurrenceErrorV1,
}

impl fmt::Debug for Gfx942ScalarF32KirRecurrenceFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32KirRecurrenceFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32KirRecurrenceFailureV1 {
    pub const fn error(&self) -> &Gfx942ScalarF32KirRecurrenceErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        kir::VerifiedCanonicalKernelIrV13,
        Gfx942ScalarF32KirRecurrenceErrorV1,
    ) {
        (self.canonical_kir, self.error)
    }
}

/// Checks the exact five-block canonical KIR V13 scalar-GEMM profile.
///
/// The profile has a global-id outer guard, a two-parameter `(t, accumulator)` loop header,
/// guarded row-major A/B reads, separate f32 multiply and add operations, and one guarded
/// `C[global_id]` write through an `Index1d` disjoint-write capability. Any additional graph,
/// operation, memory effect, or control-flow shape is outside this checker.
pub fn check_verified_canonical_gfx942_scalar_f32_kir_recurrence_v1(
    canonical_kir: kir::VerifiedCanonicalKernelIrV13,
    kernel_symbol: &str,
) -> Result<CheckedGfx942ScalarF32KirRecurrenceV1, Gfx942ScalarF32KirRecurrenceFailureV1> {
    match analyze_kir_recurrence(&canonical_kir, kernel_symbol) {
        Ok(()) => {
            let canonical_evidence =
                encode_kir_recurrence_evidence(&canonical_kir, kernel_symbol).into_boxed_slice();
            Ok(CheckedGfx942ScalarF32KirRecurrenceV1 {
                canonical_kir,
                function_symbol: kernel_symbol.to_owned(),
                canonical_evidence,
            })
        }
        Err(error) => Err(Gfx942ScalarF32KirRecurrenceFailureV1 {
            canonical_kir,
            error,
        }),
    }
}

fn analyze_kir_recurrence(
    canonical_kir: &kir::VerifiedCanonicalKernelIrV13,
    kernel_symbol: &str,
) -> Result<(), Gfx942ScalarF32KirRecurrenceErrorV1> {
    if !valid_symbol(kernel_symbol) {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::InvalidKernelSymbol);
    }
    canonical_kir.revalidate().map_err(|error| {
        Gfx942ScalarF32KirRecurrenceErrorV1::InvalidCanonicalKir(Box::new(error))
    })?;
    let module = kir::decode_module_v13(canonical_kir.canonical_bytes())
        .map_err(|_| Gfx942ScalarF32KirRecurrenceErrorV1::CanonicalKirDecode)?;
    let [kernel] = module.kernels.as_slice() else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ModuleShape);
    };
    let [function] = module.functions.as_slice() else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ModuleShape);
    };
    if !module.required_capabilities.is_empty()
        || kernel.id.as_str() != kernel_symbol
        || kernel.entry.as_str() != kernel_symbol
        || kernel.domain
            != (kir::LaunchDomain::D1 {
                x: kir::LaunchExtent::Dynamic,
            })
        || kernel.workgroup_size != Some(kir::WorkgroupSize::new(256, 1, 1))
        || !kernel.required_capabilities.is_empty()
        || function.id.as_str() != kernel_symbol
        || function.role != kir::FunctionRole::KernelEntry
        || !function.required_capabilities.is_empty()
    {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::KernelBinding);
    }

    let f32_type = kir::Type::F32;
    let input_slice = kir::Type::slice(
        f32_type.clone(),
        kir::AddressSpace::Global,
        kir::AccessMode::ReadOnly,
    );
    let output_slice = kir::Type::slice(
        f32_type.clone(),
        kir::AddressSpace::Global,
        kir::AccessMode::WriteOnly,
    );
    if function.signature.parameters
        != [
            input_slice.clone(),
            input_slice,
            output_slice,
            kir::Type::INDEX,
            kir::Type::INDEX,
            kir::Type::INDEX,
        ]
        || !function.signature.results.is_empty()
    {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::FunctionSignature);
    }
    let Some(function_body) = &function.body else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::FunctionSignature);
    };
    let [a_physical, b_physical, c_physical, m, n, k] = function_body.parameters.as_slice() else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::FunctionSignature);
    };
    let [entry, header, body, store, exit] = function_body.blocks.as_slice() else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::UnsupportedCfgShape);
    };
    let mut memory_operations = function_body
        .blocks
        .iter()
        .flat_map(|block| &block.operations);
    let read_count = memory_operations
        .clone()
        .filter(|operation| matches!(operation.kind, kir::OperationKind::GuardedLoad { .. }))
        .count();
    if read_count != 2 {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ReadCount { actual: read_count });
    }
    let write_count = memory_operations
        .clone()
        .filter(|operation| {
            matches!(
                operation.kind,
                kir::OperationKind::GuardedStore { .. } | kir::OperationKind::Store { .. }
            )
        })
        .count();
    if write_count != 1 {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriteCount {
            actual: write_count,
        });
    }
    if memory_operations.any(|operation| {
        matches!(
            operation.kind,
            kir::OperationKind::Load { .. }
                | kir::OperationKind::Store { .. }
                | kir::OperationKind::Atomic(_)
        )
    }) {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::UnsupportedCfgShape);
    }
    if !entry.parameters.is_empty()
        || !body.parameters.is_empty()
        || !store.parameters.is_empty()
        || !exit.parameters.is_empty()
        || entry.operations.len() != 15
        || header.operations.len() != 1
        || body.operations.len() != 22
        || store.operations.len() != 7
        || !exit.operations.is_empty()
    {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::UnsupportedCfgShape);
    }

    let entry_operations = &entry.operations;
    let [context_result] = entry_operations[0].results.as_slice() else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ContextCapabilityBinding);
    };
    let kir::Type::KernelContext(context_type) = &context_result.ty else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ContextCapabilityBinding);
    };
    if !matches!(
        entry_operations[0].kind,
        kir::OperationKind::KernelContextIssue(_)
    ) || context_type.root().as_str() != kernel_symbol
    {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ContextCapabilityBinding);
    }
    let (a_capability, a_capability_type) = kir_capability_bind(
        &entry_operations[1],
        context_result.id,
        *a_physical,
        context_type,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ContextCapabilityBinding)?;
    let (b_capability, b_capability_type) = kir_capability_bind(
        &entry_operations[2],
        context_result.id,
        *b_physical,
        context_type,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ContextCapabilityBinding)?;
    let (c_capability, c_capability_type) = kir_capability_bind(
        &entry_operations[3],
        context_result.id,
        *c_physical,
        context_type,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ContextCapabilityBinding)?;
    if a_capability_type.element() != &f32_type
        || b_capability_type.element() != &f32_type
        || c_capability_type.element() != &f32_type
        || a_capability_type.role() != kir::GlobalCapabilityRoleV1::ReadOnly
        || b_capability_type.role() != kir::GlobalCapabilityRoleV1::ReadOnly
    {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ContextCapabilityBinding);
    }
    let kir::GlobalCapabilityRoleV1::DisjointWrite(c_index_contract) = c_capability_type.role()
    else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriterInjectivityMismatch);
    };
    if c_index_contract.mapping() != kir::GlobalDisjointIndexSpaceV1::Index1d {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriterInjectivityMismatch);
    }

    let global_id = kir_intrinsic_result(
        &entry_operations[4],
        &kir::IntrinsicOperation::global_id_1d(),
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let zero_index = kir_constant_result(
        &entry_operations[5],
        &kir::Constant::Index(0),
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let zero_f32 = kir_constant_result(
        &entry_operations[6],
        &kir::Constant::F32Bits(0),
        &kir::Type::F32,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::AccumulatorPhiSourceMismatch)?;
    let one_index = kir_constant_result(
        &entry_operations[7],
        &kir::Constant::Index(1),
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let n_nonzero = kir_compare_result(
        &entry_operations[8],
        kir::ComparePredicate::NotEqual,
        *n,
        zero_index,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let safe_n = kir_select_result(
        &entry_operations[9],
        n_nonzero,
        *n,
        one_index,
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let output_extent = kir_binary_result(
        &entry_operations[10],
        kir::BinaryOp::Multiply,
        *m,
        *n,
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let output_in_range = kir_compare_result(
        &entry_operations[11],
        kir::ComparePredicate::LessThan,
        global_id,
        output_extent,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let active = kir_binary_result(
        &entry_operations[12],
        kir::BinaryOp::BitAnd,
        n_nonzero,
        output_in_range,
        &kir::Type::BOOL,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let row = kir_binary_result(
        &entry_operations[13],
        kir::BinaryOp::Divide,
        global_id,
        safe_n,
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;
    let column = kir_binary_result(
        &entry_operations[14],
        kir::BinaryOp::Remainder,
        global_id,
        safe_n,
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch)?;

    let [iteration, accumulator] = header.parameters.as_slice() else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::AccumulatorPhiSourceMismatch);
    };
    if iteration.ty != kir::Type::INDEX || accumulator.ty != kir::Type::F32 {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::AccumulatorPhiSourceMismatch);
    }
    match entry.terminator.as_ref() {
        Some(kir::Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        }) if *condition == active
            && *then_target == header.id
            && then_arguments == &[zero_index, zero_f32]
            && *else_target == exit.id
            && else_arguments.is_empty() => {}
        Some(kir::Terminator::ConditionalBranch { condition, .. }) if *condition != active => {
            return Err(Gfx942ScalarF32KirRecurrenceErrorV1::OuterGuardMismatch);
        }
        _ => return Err(Gfx942ScalarF32KirRecurrenceErrorV1::InactivePathMismatch),
    }

    let loop_condition = kir_compare_result(
        &header.operations[0],
        kir::ComparePredicate::LessThan,
        iteration.id,
        *k,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::LoopTripMismatch)?;
    match header.terminator.as_ref() {
        Some(kir::Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        }) if *condition == loop_condition
            && *then_target == body.id
            && then_arguments.is_empty()
            && *else_target == store.id
            && else_arguments.is_empty() => {}
        _ => return Err(Gfx942ScalarF32KirRecurrenceErrorV1::LoopTripMismatch),
    }

    let a_value = check_kir_guarded_read(
        &body.operations[0..9],
        Gfx942ScalarF32KirInputV1::A,
        a_capability,
        row,
        *k,
        iteration.id,
        zero_index,
        zero_f32,
    )?;
    let b_value = check_kir_guarded_read(
        &body.operations[9..18],
        Gfx942ScalarF32KirInputV1::B,
        b_capability,
        iteration.id,
        *n,
        column,
        zero_index,
        zero_f32,
    )?;
    let product = kir_binary_result(
        &body.operations[18],
        kir::BinaryOp::Multiply,
        a_value,
        b_value,
        &kir::Type::F32,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::RecurrenceSsaMismatch)?;
    let next_accumulator = kir_binary_result(
        &body.operations[19],
        kir::BinaryOp::Add,
        product,
        accumulator.id,
        &kir::Type::F32,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::RecurrenceSsaMismatch)?;
    let one = kir_constant_result(
        &body.operations[20],
        &kir::Constant::Index(1),
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::LoopTripMismatch)?;
    let next_iteration = kir_binary_result(
        &body.operations[21],
        kir::BinaryOp::Add,
        iteration.id,
        one,
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::LoopTripMismatch)?;
    match body.terminator.as_ref() {
        Some(kir::Terminator::Branch { target, arguments })
            if *target == header.id && arguments == &[next_iteration, next_accumulator] => {}
        Some(kir::Terminator::Branch { target, .. }) if *target == header.id => {
            return Err(Gfx942ScalarF32KirRecurrenceErrorV1::AccumulatorPhiSourceMismatch);
        }
        _ => return Err(Gfx942ScalarF32KirRecurrenceErrorV1::UnsupportedCfgShape),
    }

    check_kir_guarded_write(
        &store.operations,
        c_capability,
        c_index_contract,
        global_id,
        zero_index,
        accumulator.id,
    )?;
    match store.terminator.as_ref() {
        Some(kir::Terminator::Branch { target, arguments })
            if *target == exit.id && arguments.is_empty() => {}
        _ => return Err(Gfx942ScalarF32KirRecurrenceErrorV1::InactivePathMismatch),
    }
    match exit.terminator.as_ref() {
        Some(kir::Terminator::Return { values }) if values.is_empty() => {}
        _ => return Err(Gfx942ScalarF32KirRecurrenceErrorV1::InactivePathMismatch),
    }
    Ok(())
}

fn kir_capability_bind<'a>(
    operation: &'a kir::Operation,
    context: kir::ValueId,
    physical: kir::ValueId,
    context_type: &kir::KernelContextTypeV1,
) -> Option<(kir::ValueId, &'a kir::GlobalCapabilityTypeV1)> {
    let [result] = operation.results.as_slice() else {
        return None;
    };
    let kir::Type::GlobalCapability(capability) = &result.ty else {
        return None;
    };
    let kir::OperationKind::GlobalCapabilityBind(binding) = operation.kind else {
        return None;
    };
    (binding.context == context
        && binding.physical == physical
        && capability.context() == context_type)
        .then_some((result.id, capability))
}

#[allow(clippy::too_many_arguments)]
fn check_kir_guarded_read(
    operations: &[kir::Operation],
    input: Gfx942ScalarF32KirInputV1,
    capability: kir::ValueId,
    stride_lhs: kir::ValueId,
    stride_rhs: kir::ValueId,
    addend: kir::ValueId,
    zero_index: kir::ValueId,
    zero_f32: kir::ValueId,
) -> Result<kir::ValueId, Gfx942ScalarF32KirRecurrenceErrorV1> {
    let [
        stride,
        index,
        projection,
        length,
        guard,
        safe_index,
        data,
        pointer,
        load,
    ] = operations
    else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ReadCount {
            actual: operations.len(),
        });
    };
    let stride = kir_binary_result(
        stride,
        kir::BinaryOp::Multiply,
        stride_lhs,
        stride_rhs,
        &kir::Type::INDEX,
    )
    .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::AddressStrideMismatch { input })?;
    let index = kir_binary_result(index, kir::BinaryOp::Add, stride, addend, &kir::Type::INDEX)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::AddressStrideMismatch { input })?;
    let projected = kir_global_index_result(projection, capability, index, None)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::AddressStrideMismatch { input })?;
    let length = kir_slice_length_result(length, capability)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ReadGuardMismatch { input })?;
    let guard = kir_compare_result(guard, kir::ComparePredicate::LessThan, projected, length)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ReadGuardMismatch { input })?;
    let safe_index = kir_select_result(safe_index, guard, projected, zero_index, &kir::Type::INDEX)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ReadGuardMismatch { input })?;
    let pointer_type = kir::Type::pointer(
        kir::Type::F32,
        kir::AddressSpace::Global,
        kir::AccessMode::ReadOnly,
    );
    let data = kir_slice_data_result(data, capability, &pointer_type)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ReadGuardMismatch { input })?;
    let pointer = kir_gep_result(pointer, data, safe_index, &pointer_type)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::ReadGuardMismatch { input })?;
    let [result] = load.results.as_slice() else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::ReadGuardMismatch { input });
    };
    match &load.kind {
        kir::OperationKind::GuardedLoad {
            pointer: load_pointer,
            predicate,
            fallback,
            access,
        } if *load_pointer == pointer
            && *predicate == guard
            && *fallback == zero_f32
            && result.ty == kir::Type::F32
            && exact_global_f32_access(*access) =>
        {
            Ok(result.id)
        }
        _ => Err(Gfx942ScalarF32KirRecurrenceErrorV1::ReadGuardMismatch { input }),
    }
}

fn check_kir_guarded_write(
    operations: &[kir::Operation],
    capability: kir::ValueId,
    index_contract: kir::GlobalDisjointIndexContractV1,
    output_index: kir::ValueId,
    zero_index: kir::ValueId,
    accumulator: kir::ValueId,
) -> Result<(), Gfx942ScalarF32KirRecurrenceErrorV1> {
    let [projection, length, guard, safe_index, data, pointer, store] = operations else {
        return Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriteCount {
            actual: operations.len(),
        });
    };
    let projected = match &projection.kind {
        kir::OperationKind::GlobalCapabilityIndex(projected)
            if projected.capability == capability
                && projected.index_space == Some(index_contract) =>
        {
            let Some(projected_result) = kir_result(projection, &kir::Type::INDEX) else {
                return Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriterInjectivityMismatch);
            };
            if projected.index != output_index {
                return Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriteDestinationMismatch);
            }
            projected_result
        }
        _ => return Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriterInjectivityMismatch),
    };
    let length = kir_slice_length_result(length, capability)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::WriteGuardMismatch)?;
    let guard = kir_compare_result(guard, kir::ComparePredicate::LessThan, projected, length)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::WriteGuardMismatch)?;
    let safe_index = kir_select_result(safe_index, guard, projected, zero_index, &kir::Type::INDEX)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::WriteGuardMismatch)?;
    let pointer_type = kir::Type::pointer(
        kir::Type::F32,
        kir::AddressSpace::Global,
        kir::AccessMode::WriteOnly,
    );
    let data = kir_slice_data_result(data, capability, &pointer_type)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::WriteGuardMismatch)?;
    let pointer = kir_gep_result(pointer, data, safe_index, &pointer_type)
        .ok_or(Gfx942ScalarF32KirRecurrenceErrorV1::WriteGuardMismatch)?;
    match &store.kind {
        kir::OperationKind::GuardedStore {
            pointer: store_pointer,
            predicate,
            value,
            access,
        } if store.results.is_empty()
            && *store_pointer == pointer
            && *predicate == guard
            && *value == accumulator
            && exact_global_f32_access(*access) =>
        {
            Ok(())
        }
        _ => Err(Gfx942ScalarF32KirRecurrenceErrorV1::WriteDestinationMismatch),
    }
}

fn kir_result(operation: &kir::Operation, expected_type: &kir::Type) -> Option<kir::ValueId> {
    let [result] = operation.results.as_slice() else {
        return None;
    };
    (&result.ty == expected_type).then_some(result.id)
}

fn kir_intrinsic_result(
    operation: &kir::Operation,
    expected: &kir::IntrinsicOperation,
) -> Option<kir::ValueId> {
    let kir::OperationKind::Intrinsic(intrinsic) = &operation.kind else {
        return None;
    };
    (intrinsic == expected)
        .then(|| kir_result(operation, &expected.result_type))
        .flatten()
}

fn kir_constant_result(
    operation: &kir::Operation,
    expected: &kir::Constant,
    expected_type: &kir::Type,
) -> Option<kir::ValueId> {
    match &operation.kind {
        kir::OperationKind::Constant(constant) if constant == expected => {
            kir_result(operation, expected_type)
        }
        _ => None,
    }
}

fn kir_binary_result(
    operation: &kir::Operation,
    expected_operator: kir::BinaryOp,
    expected_lhs: kir::ValueId,
    expected_rhs: kir::ValueId,
    expected_type: &kir::Type,
) -> Option<kir::ValueId> {
    match operation.kind {
        kir::OperationKind::Binary { op, lhs, rhs }
            if op == expected_operator && lhs == expected_lhs && rhs == expected_rhs =>
        {
            kir_result(operation, expected_type)
        }
        _ => None,
    }
}

fn kir_compare_result(
    operation: &kir::Operation,
    expected_predicate: kir::ComparePredicate,
    expected_lhs: kir::ValueId,
    expected_rhs: kir::ValueId,
) -> Option<kir::ValueId> {
    match operation.kind {
        kir::OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } if predicate == expected_predicate && lhs == expected_lhs && rhs == expected_rhs => {
            kir_result(operation, &kir::Type::BOOL)
        }
        _ => None,
    }
}

fn kir_select_result(
    operation: &kir::Operation,
    expected_condition: kir::ValueId,
    expected_true: kir::ValueId,
    expected_false: kir::ValueId,
    expected_type: &kir::Type,
) -> Option<kir::ValueId> {
    match operation.kind {
        kir::OperationKind::Select {
            condition,
            true_value,
            false_value,
        } if condition == expected_condition
            && true_value == expected_true
            && false_value == expected_false =>
        {
            kir_result(operation, expected_type)
        }
        _ => None,
    }
}

fn kir_global_index_result(
    operation: &kir::Operation,
    expected_capability: kir::ValueId,
    expected_index: kir::ValueId,
    expected_contract: Option<kir::GlobalDisjointIndexContractV1>,
) -> Option<kir::ValueId> {
    match operation.kind {
        kir::OperationKind::GlobalCapabilityIndex(index)
            if index.capability == expected_capability
                && index.index == expected_index
                && index.index_space == expected_contract =>
        {
            kir_result(operation, &kir::Type::INDEX)
        }
        _ => None,
    }
}

fn kir_slice_length_result(
    operation: &kir::Operation,
    expected_slice: kir::ValueId,
) -> Option<kir::ValueId> {
    match operation.kind {
        kir::OperationKind::SliceLength { slice } if slice == expected_slice => {
            kir_result(operation, &kir::Type::INDEX)
        }
        _ => None,
    }
}

fn kir_slice_data_result(
    operation: &kir::Operation,
    expected_slice: kir::ValueId,
    expected_type: &kir::Type,
) -> Option<kir::ValueId> {
    match operation.kind {
        kir::OperationKind::SliceData { slice } if slice == expected_slice => {
            kir_result(operation, expected_type)
        }
        _ => None,
    }
}

fn kir_gep_result(
    operation: &kir::Operation,
    expected_base: kir::ValueId,
    expected_offset: kir::ValueId,
    expected_type: &kir::Type,
) -> Option<kir::ValueId> {
    match operation.kind {
        kir::OperationKind::GetElementPointer { base, offset }
            if base == expected_base && offset == expected_offset =>
        {
            kir_result(operation, expected_type)
        }
        _ => None,
    }
}

fn exact_global_f32_access(access: kir::MemoryAccess) -> bool {
    access == kir::MemoryAccess::new(kir::AddressSpace::Global, 4)
}

fn encode_kir_recurrence_evidence(
    canonical_kir: &kir::VerifiedCanonicalKernelIrV13,
    kernel_symbol: &str,
) -> Vec<u8> {
    let identity = canonical_kir.identity();
    let mut output = Vec::with_capacity(
        GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_DOMAIN_V1.len() + kernel_symbol.len() + 64,
    );
    output.extend_from_slice(GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(
        &mut output,
        GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_VERSION_V1,
    );
    output.extend_from_slice(identity.digest());
    push_u64(&mut output, identity.canonical_length());
    push_text(&mut output, kernel_symbol);
    let length = output.len() as u32;
    let offset = GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

fn gfx942_scalar_f32_kir_recurrence_evidence_identity_v1(
    canonical_evidence: &[u8],
) -> Gfx942ScalarF32KirRecurrenceEvidenceIdentityV1 {
    let byte_len = canonical_evidence.len() as u64;
    let mut digest = Sha256::new();
    digest.update(
        (GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes(),
    );
    digest.update(GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_IDENTITY_DOMAIN_V1);
    digest.update(GFX942_SCALAR_F32_KIR_RECURRENCE_EVIDENCE_VERSION_V1.to_le_bytes());
    digest.update(byte_len.to_le_bytes());
    digest.update(canonical_evidence);
    Gfx942ScalarF32KirRecurrenceEvidenceIdentityV1 {
        sha256: digest.finalize().into(),
        byte_len,
    }
}

/// Proof-bearing inputs that are not present in the authenticated machine-analysis V1 record.
///
/// This list is deliberately typed and exhaustive for the bounded issue #214 scalar-GEMM
/// profile. A final-machine checker must not replace any item with a boolean, an untrusted digest,
/// a source-name match, or a successful hardware run.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum Gfx942ScalarF32MachineRefinementRequiredInputV1 {
    /// Move-only exact canonical KIR V13 owner for the optimized scalar-GEMM graph.
    VerifiedCanonicalKirV13Owner,
    /// Independently replayable KIR-to-LLVM relation covering SSA, CFG, phis, ABI, and addresses.
    KirToLlvmSsaCfgPhiAbiReplay,
    /// Exact pre/post optimization bitcode and the complete ordered pass configuration.
    PrePostOptimizationBitcodeAndPassConfiguration,
    /// Checked assembly relation from the exact lowered LLVM module to pre-optimization bitcode.
    LlvmTextToPreOptimizationBitcodeAssembly,
    /// Checked semantic preservation for every configured LLVM optimization pass.
    LlvmOptimizationSemanticPreservation,
    /// Checked LLVM-to-gfx942 instruction-selection relation, including definitions and uses.
    LlvmToGfx942InstructionSelection,
    /// Exact generated relocatable object bytes retained by the production compiler transaction.
    GeneratedRelocatableObject,
    /// Checked object-to-final-HSACO relocation, symbol, and section preservation relation.
    ObjectToHsacoRelocationSymbolSectionPreservation,
    /// Executable gfx942 branch, EXEC-mask, integer, and effective-address semantics.
    Gfx942ControlAndAddressOperationalSemantics,
    /// Exact gfx942 MODE plus IEEE binary32 MUL/ADD semantics for every exceptional value.
    Gfx942IeeeBinary32OperationalSemantics,
    /// Inductive machine-loop carry, inactive-path, memory-effect, and writer-injectivity proof.
    MachineLoopEffectsAndWriterInjectivity,
}

impl Gfx942ScalarF32MachineRefinementRequiredInputV1 {
    pub const fn diagnostic_name(self) -> &'static str {
        match self {
            Self::VerifiedCanonicalKirV13Owner => "verified-canonical-kir-v13-owner",
            Self::KirToLlvmSsaCfgPhiAbiReplay => "kir-to-llvm-ssa-cfg-phi-abi-replay",
            Self::PrePostOptimizationBitcodeAndPassConfiguration => {
                "pre-post-optimization-bitcode-and-pass-configuration"
            }
            Self::LlvmTextToPreOptimizationBitcodeAssembly => {
                "llvm-text-to-pre-optimization-bitcode-assembly"
            }
            Self::LlvmOptimizationSemanticPreservation => "llvm-optimization-semantic-preservation",
            Self::LlvmToGfx942InstructionSelection => "llvm-to-gfx942-instruction-selection",
            Self::GeneratedRelocatableObject => "generated-relocatable-object",
            Self::ObjectToHsacoRelocationSymbolSectionPreservation => {
                "object-to-hsaco-relocation-symbol-section-preservation"
            }
            Self::Gfx942ControlAndAddressOperationalSemantics => {
                "gfx942-control-address-operational-semantics"
            }
            Self::Gfx942IeeeBinary32OperationalSemantics => {
                "gfx942-ieee-binary32-operational-semantics"
            }
            Self::MachineLoopEffectsAndWriterInjectivity => {
                "machine-loop-effects-writer-injectivity"
            }
        }
    }
}

const GFX942_SCALAR_F32_MACHINE_REFINEMENT_REQUIRED_INPUTS_V1:
    [Gfx942ScalarF32MachineRefinementRequiredInputV1; 11] = [
    Gfx942ScalarF32MachineRefinementRequiredInputV1::VerifiedCanonicalKirV13Owner,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::KirToLlvmSsaCfgPhiAbiReplay,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::PrePostOptimizationBitcodeAndPassConfiguration,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmTextToPreOptimizationBitcodeAssembly,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmOptimizationSemanticPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmToGfx942InstructionSelection,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::GeneratedRelocatableObject,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::ObjectToHsacoRelocationSymbolSectionPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942ControlAndAddressOperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942IeeeBinary32OperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::MachineLoopEffectsAndWriterInjectivity,
];

const GFX942_SCALAR_F32_MACHINE_REFINEMENT_INPUTS_AFTER_KIR_V1:
    [Gfx942ScalarF32MachineRefinementRequiredInputV1; 10] = [
    Gfx942ScalarF32MachineRefinementRequiredInputV1::KirToLlvmSsaCfgPhiAbiReplay,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::PrePostOptimizationBitcodeAndPassConfiguration,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmTextToPreOptimizationBitcodeAssembly,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmOptimizationSemanticPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmToGfx942InstructionSelection,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::GeneratedRelocatableObject,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::ObjectToHsacoRelocationSymbolSectionPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942ControlAndAddressOperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942IeeeBinary32OperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::MachineLoopEffectsAndWriterInjectivity,
];

const GFX942_SCALAR_F32_MACHINE_REFINEMENT_INPUTS_AFTER_STRUCTURED_LLVM_V1:
    [Gfx942ScalarF32MachineRefinementRequiredInputV1; 9] = [
    Gfx942ScalarF32MachineRefinementRequiredInputV1::PrePostOptimizationBitcodeAndPassConfiguration,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmTextToPreOptimizationBitcodeAssembly,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmOptimizationSemanticPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmToGfx942InstructionSelection,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::GeneratedRelocatableObject,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::ObjectToHsacoRelocationSymbolSectionPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942ControlAndAddressOperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942IeeeBinary32OperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::MachineLoopEffectsAndWriterInjectivity,
];

const GFX942_SCALAR_F32_MACHINE_REFINEMENT_INPUTS_AFTER_POST_LLVM_CONTENT_V1:
    [Gfx942ScalarF32MachineRefinementRequiredInputV1; 8] = [
    Gfx942ScalarF32MachineRefinementRequiredInputV1::PrePostOptimizationBitcodeAndPassConfiguration,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmTextToPreOptimizationBitcodeAssembly,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmOptimizationSemanticPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::LlvmToGfx942InstructionSelection,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::ObjectToHsacoRelocationSymbolSectionPreservation,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942ControlAndAddressOperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::Gfx942IeeeBinary32OperationalSemantics,
    Gfx942ScalarF32MachineRefinementRequiredInputV1::MachineLoopEffectsAndWriterInjectivity,
];

/// Identity of exact KIR/recurrence-step conjunction evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942ScalarF32KirMachineBindingEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942ScalarF32KirMachineBindingEvidenceIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Move-only conjunction of exact KIR recurrence facts and one authenticated machine step.
///
/// The evidence binds both owners and is stronger than either alone, but it deliberately does not
/// assert that LLVM selected the observed machine instructions from this KIR graph.
#[must_use = "dropping KIR/machine recurrence custody abandons an issue #214 prerequisite"]
pub struct Gfx942ScalarF32KirMachineRecurrenceReadinessV1 {
    kir: CheckedGfx942ScalarF32KirRecurrenceV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    canonical_evidence: Box<[u8]>,
}

impl fmt::Debug for Gfx942ScalarF32KirMachineRecurrenceReadinessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32KirMachineRecurrenceReadinessV1")
            .field("function_symbol", &self.kir.function_symbol())
            .field("evidence_identity", &self.evidence_identity())
            .field(
                "authenticated_execution_identity",
                &self.machine.authenticated_execution_identity(),
            )
            .field("missing", &self.missing_inputs())
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32KirMachineRecurrenceReadinessV1 {
    pub const fn kir(&self) -> &CheckedGfx942ScalarF32KirRecurrenceV1 {
        &self.kir
    }

    pub const fn machine(&self) -> &AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
        &self.machine
    }

    pub fn canonical_evidence_bytes(&self) -> &[u8] {
        &self.canonical_evidence
    }

    pub fn evidence_identity(&self) -> Gfx942ScalarF32KirMachineBindingEvidenceIdentityV1 {
        gfx942_scalar_f32_kir_machine_binding_evidence_identity_v1(&self.canonical_evidence)
    }

    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        &GFX942_SCALAR_F32_MACHINE_REFINEMENT_INPUTS_AFTER_KIR_V1
    }

    pub const fn binds_exact_kir_and_authenticated_machine_step(&self) -> bool {
        true
    }

    pub const fn establishes_kir_to_final_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_worker_v3_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_or_launch_authority(&self) -> bool {
        false
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirRecurrenceV1,
        AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
        Box<[u8]>,
        Gfx942ScalarF32KirMachineBindingEvidenceIdentityV1,
    ) {
        let identity =
            gfx942_scalar_f32_kir_machine_binding_evidence_identity_v1(&self.canonical_evidence);
        (self.kir, self.machine, self.canonical_evidence, identity)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942ScalarF32KirMachineBindingErrorV1 {
    FunctionSymbolMismatch,
}

impl fmt::Display for Gfx942ScalarF32KirMachineBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KIR and authenticated recurrence-step function symbols differ")
    }
}

impl Error for Gfx942ScalarF32KirMachineBindingErrorV1 {}

/// Failed conjunction retaining both move-only owners.
pub struct Gfx942ScalarF32KirMachineBindingFailureV1 {
    kir: CheckedGfx942ScalarF32KirRecurrenceV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    error: Gfx942ScalarF32KirMachineBindingErrorV1,
}

impl fmt::Debug for Gfx942ScalarF32KirMachineBindingFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32KirMachineBindingFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32KirMachineBindingFailureV1 {
    pub const fn error(&self) -> Gfx942ScalarF32KirMachineBindingErrorV1 {
        self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirRecurrenceV1,
        AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
        Gfx942ScalarF32KirMachineBindingErrorV1,
    ) {
        (self.kir, self.machine, self.error)
    }
}

/// Binds exact KIR recurrence evidence to the existing authenticated machine-step certificate.
pub fn bind_gfx942_scalar_f32_kir_and_authenticated_machine_recurrence_v1(
    kir: CheckedGfx942ScalarF32KirRecurrenceV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
) -> Result<
    Gfx942ScalarF32KirMachineRecurrenceReadinessV1,
    Box<Gfx942ScalarF32KirMachineBindingFailureV1>,
> {
    if kir.function_symbol() != machine.artifact().function_symbol() {
        return Err(Box::new(Gfx942ScalarF32KirMachineBindingFailureV1 {
            kir,
            machine,
            error: Gfx942ScalarF32KirMachineBindingErrorV1::FunctionSymbolMismatch,
        }));
    }
    let canonical_evidence = encode_kir_machine_binding_evidence(&kir, &machine).into_boxed_slice();
    Ok(Gfx942ScalarF32KirMachineRecurrenceReadinessV1 {
        kir,
        machine,
        canonical_evidence,
    })
}

fn encode_kir_machine_binding_evidence(
    kir: &CheckedGfx942ScalarF32KirRecurrenceV1,
    machine: &AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
) -> Vec<u8> {
    let kir_identity = kir.evidence_identity();
    let machine_identity = machine.artifact().identity();
    let execution_identity = machine.authenticated_execution_identity();
    let mut output = Vec::with_capacity(
        GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_DOMAIN_V1.len()
            + kir.function_symbol().len()
            + 160,
    );
    output.extend_from_slice(GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(
        &mut output,
        GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_VERSION_V1,
    );
    output.extend_from_slice(&kir_identity.sha256());
    push_u64(&mut output, kir_identity.byte_len());
    output.extend_from_slice(&machine_identity.sha256());
    push_u64(&mut output, machine_identity.byte_len());
    output.extend_from_slice(&execution_identity.sha256());
    push_u64(&mut output, execution_identity.byte_len());
    push_text(&mut output, kir.function_symbol());
    let length = output.len() as u32;
    let offset = GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

fn gfx942_scalar_f32_kir_machine_binding_evidence_identity_v1(
    canonical_evidence: &[u8],
) -> Gfx942ScalarF32KirMachineBindingEvidenceIdentityV1 {
    let byte_len = canonical_evidence.len() as u64;
    let mut digest = Sha256::new();
    digest.update(
        (GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_IDENTITY_DOMAIN_V1.len() as u32)
            .to_le_bytes(),
    );
    digest.update(GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_IDENTITY_DOMAIN_V1);
    digest.update(GFX942_SCALAR_F32_KIR_MACHINE_BINDING_EVIDENCE_VERSION_V1.to_le_bytes());
    digest.update(byte_len.to_le_bytes());
    digest.update(canonical_evidence);
    Gfx942ScalarF32KirMachineBindingEvidenceIdentityV1 {
        sha256: digest.finalize().into(),
        byte_len,
    }
}

/// Terminal unavailability after the exact KIR and authenticated machine-step layers succeed.
#[derive(Debug)]
pub struct Gfx942ScalarF32MachineRefinementAfterKirIncompleteV1 {
    readiness: Gfx942ScalarF32KirMachineRecurrenceReadinessV1,
}

impl Gfx942ScalarF32MachineRefinementAfterKirIncompleteV1 {
    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        self.readiness.missing_inputs()
    }

    pub fn into_readiness(self) -> Gfx942ScalarF32KirMachineRecurrenceReadinessV1 {
        self.readiness
    }
}

impl fmt::Display for Gfx942ScalarF32MachineRefinementAfterKirIncompleteV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "gfx942 scalar-f32 refinement remains unavailable after exact KIR recurrence binding; missing typed inputs: ",
        )?;
        for (index, input) in self.missing_inputs().iter().enumerate() {
            if index != 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(input.diagnostic_name())?;
        }
        Ok(())
    }
}

impl Error for Gfx942ScalarF32MachineRefinementAfterKirIncompleteV1 {}

/// Keeps the successful terminal unreachable until every post-KIR typed input exists.
pub fn require_authenticated_gfx942_scalar_f32_machine_refinement_after_kir_v1(
    readiness: Gfx942ScalarF32KirMachineRecurrenceReadinessV1,
) -> Result<
    CheckedGfx942ScalarF32MachineRefinementV1,
    Box<Gfx942ScalarF32MachineRefinementAfterKirIncompleteV1>,
> {
    Err(Box::new(
        Gfx942ScalarF32MachineRefinementAfterKirIncompleteV1 { readiness },
    ))
}

/// Move-only conjunction after exact KIR-to-LLVM replay and authenticated machine-step checking.
pub struct Gfx942ScalarF32StructuredLlvmMachineReadinessV1 {
    lowering: CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
}

impl fmt::Debug for Gfx942ScalarF32StructuredLlvmMachineReadinessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32StructuredLlvmMachineReadinessV1")
            .field("lowering", &self.lowering.evidence_identity())
            .field("machine", &self.machine.authenticated_execution_identity())
            .field("missing", &self.missing_inputs())
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32StructuredLlvmMachineReadinessV1 {
    pub const fn lowering(&self) -> &CheckedGfx942ScalarF32KirToLlvmRefinementV1 {
        &self.lowering
    }

    pub const fn machine(&self) -> &AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
        &self.machine
    }

    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        &GFX942_SCALAR_F32_MACHINE_REFINEMENT_INPUTS_AFTER_STRUCTURED_LLVM_V1
    }

    pub const fn establishes_structured_kir_to_llvm_refinement(&self) -> bool {
        true
    }

    pub const fn establishes_llvm_to_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirToLlvmRefinementV1,
        AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    ) {
        (self.lowering, self.machine)
    }
}

/// Failure to bind a structured lowering receipt to an authenticated machine-step owner.
pub struct Gfx942ScalarF32StructuredLlvmMachineBindingFailureV1 {
    lowering: CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
}

impl fmt::Debug for Gfx942ScalarF32StructuredLlvmMachineBindingFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32StructuredLlvmMachineBindingFailureV1")
            .field("error", &"function-symbol-mismatch")
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32StructuredLlvmMachineBindingFailureV1 {
    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirToLlvmRefinementV1,
        AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    ) {
        (self.lowering, self.machine)
    }
}

/// Binds the exact structured lowering receipt to the authenticated scalar machine step.
pub fn bind_gfx942_scalar_f32_structured_llvm_and_authenticated_machine_recurrence_v1(
    lowering: CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
) -> Result<
    Gfx942ScalarF32StructuredLlvmMachineReadinessV1,
    Box<Gfx942ScalarF32StructuredLlvmMachineBindingFailureV1>,
> {
    if lowering.kir().function_symbol() != machine.artifact().function_symbol() {
        return Err(Box::new(
            Gfx942ScalarF32StructuredLlvmMachineBindingFailureV1 { lowering, machine },
        ));
    }
    Ok(Gfx942ScalarF32StructuredLlvmMachineReadinessV1 { lowering, machine })
}

/// Fail-closed terminal after KIR-to-LLVM replay, retaining the remaining #214 inputs.
#[derive(Debug)]
pub struct Gfx942ScalarF32MachineRefinementAfterStructuredLlvmIncompleteV1 {
    readiness: Gfx942ScalarF32StructuredLlvmMachineReadinessV1,
}

impl Gfx942ScalarF32MachineRefinementAfterStructuredLlvmIncompleteV1 {
    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        self.readiness.missing_inputs()
    }

    pub fn into_readiness(self) -> Gfx942ScalarF32StructuredLlvmMachineReadinessV1 {
        self.readiness
    }
}

impl fmt::Display for Gfx942ScalarF32MachineRefinementAfterStructuredLlvmIncompleteV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "gfx942 scalar-f32 refinement remains unavailable after structured KIR-to-LLVM replay; missing typed inputs: ",
        )?;
        for (index, input) in self.missing_inputs().iter().enumerate() {
            if index != 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(input.diagnostic_name())?;
        }
        Ok(())
    }
}

impl Error for Gfx942ScalarF32MachineRefinementAfterStructuredLlvmIncompleteV1 {}

/// Keeps final-machine success unreachable while acknowledging the checked KIR-to-LLVM stage.
pub fn require_authenticated_gfx942_scalar_f32_machine_refinement_after_structured_llvm_v1(
    readiness: Gfx942ScalarF32StructuredLlvmMachineReadinessV1,
) -> Result<
    CheckedGfx942ScalarF32MachineRefinementV1,
    Box<Gfx942ScalarF32MachineRefinementAfterStructuredLlvmIncompleteV1>,
> {
    Err(Box::new(
        Gfx942ScalarF32MachineRefinementAfterStructuredLlvmIncompleteV1 { readiness },
    ))
}

/// Identity of exact structured-LLVM/post-LLVM content conjunction evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942ScalarF32PostLlvmContentEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942ScalarF32PostLlvmContentEvidenceIdentityV1 {
    /// Returns the conjunction digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the canonical conjunction byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Move-only custody of exact post-LLVM contents bound to the checked structured LLVM module.
///
/// This narrows issue #214's missing inputs by retaining exact pre/post bitcode, the complete
/// bounded phase/pass occurrence transcript, exact text-assembly replay, and exact
/// object/code-object bytes. It does not infer that the unattested transcript came from the
/// production occurrence, nor optimization, instruction-selection, relocation, or machine
/// semantics from those bytes or their identities.
#[must_use = "dropping post-LLVM content custody abandons issue #214 inputs"]
pub struct Gfx942ScalarF32PostLlvmContentReadinessV1 {
    lowering: CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    stages: CheckedPostLlvmStageContentsV1,
    canonical_evidence: Box<[u8]>,
}

impl fmt::Debug for Gfx942ScalarF32PostLlvmContentReadinessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32PostLlvmContentReadinessV1")
            .field("lowering", &self.lowering.evidence_identity())
            .field("stages", &self.stages.record().identity())
            .field("missing", &self.missing_inputs())
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32PostLlvmContentReadinessV1 {
    /// Returns the checked structured KIR-to-LLVM owner.
    pub const fn lowering(&self) -> &CheckedGfx942ScalarF32KirToLlvmRefinementV1 {
        &self.lowering
    }

    /// Returns the independently matched exact post-LLVM stage bodies.
    pub const fn stages(&self) -> &CheckedPostLlvmStageContentsV1 {
        &self.stages
    }

    /// Returns the canonical conjunction evidence.
    pub fn canonical_evidence_bytes(&self) -> &[u8] {
        &self.canonical_evidence
    }

    /// Returns the conjunction evidence identity.
    pub fn evidence_identity(&self) -> Gfx942ScalarF32PostLlvmContentEvidenceIdentityV1 {
        let mut digest = Sha256::new();
        digest.update(b"FE2O3/GFX942-SCALAR-F32-POST-LLVM-CONTENT-IDENTITY/V1\0");
        digest.update((self.canonical_evidence.len() as u64).to_le_bytes());
        digest.update(&self.canonical_evidence);
        Gfx942ScalarF32PostLlvmContentEvidenceIdentityV1 {
            sha256: digest.finalize().into(),
            byte_len: self.canonical_evidence.len() as u64,
        }
    }

    /// Returns the remaining semantic and machine-refinement obligations.
    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        &GFX942_SCALAR_F32_MACHINE_REFINEMENT_INPUTS_AFTER_POST_LLVM_CONTENT_V1
    }

    /// Reports exact bitcode bodies and recorded ordered pass-declaration custody.
    pub const fn retains_pre_post_optimization_bitcode_and_recorded_passes(&self) -> bool {
        true
    }

    /// Reports exact generated-object and final-code-object byte custody.
    pub const fn retains_generated_object_and_final_code_object(&self) -> bool {
        true
    }

    /// Reports complete bounded occurrence and exact assembly-replay custody.
    pub const fn retains_complete_pipeline_occurrence_and_assembly_replay(&self) -> bool {
        true
    }

    /// Exact stage custody does not prove the lowered LLVM was assembled to the bitcode.
    pub const fn establishes_llvm_text_to_bitcode_assembly(&self) -> bool {
        false
    }

    /// Exact stage custody does not prove configured optimization semantics.
    pub const fn establishes_llvm_optimization_semantic_preservation(&self) -> bool {
        false
    }

    /// Exact stage custody does not prove final-machine refinement.
    pub const fn establishes_llvm_to_final_machine_refinement(&self) -> bool {
        false
    }

    /// This readiness owner grants no compiler, publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Decomposes this owner while retaining canonical evidence bytes.
    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirToLlvmRefinementV1,
        CheckedPostLlvmStageContentsV1,
        Box<[u8]>,
    ) {
        (self.lowering, self.stages, self.canonical_evidence)
    }
}

/// Failed exact lowered-LLVM identity binding with both move-only owners retained.
pub struct Gfx942ScalarF32PostLlvmContentBindingFailureV1 {
    lowering: CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    stages: CheckedPostLlvmStageContentsV1,
    reason: Gfx942ScalarF32PostLlvmContentBindingErrorV1,
}

/// Why checked post-LLVM contents cannot enter recurrence readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942ScalarF32PostLlvmContentBindingErrorV1 {
    /// The checked structured LLVM bytes differ from post-LLVM custody.
    LoweredLlvmContentMismatch,
    /// Complete phase/pass occurrence and exact assembly replay custody were not supplied.
    MissingPipelineOccurrenceAndAssemblyReplay,
}

impl fmt::Debug for Gfx942ScalarF32PostLlvmContentBindingFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32PostLlvmContentBindingFailureV1")
            .field("error", &self.reason)
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32PostLlvmContentBindingFailureV1 {
    /// Returns the fail-closed binding reason.
    pub const fn reason(&self) -> Gfx942ScalarF32PostLlvmContentBindingErrorV1 {
        self.reason
    }

    /// Recovers both move-only inputs after a mismatch.
    pub fn into_parts(
        self,
    ) -> (
        CheckedGfx942ScalarF32KirToLlvmRefinementV1,
        CheckedPostLlvmStageContentsV1,
    ) {
        (self.lowering, self.stages)
    }
}

/// Binds exact post-LLVM content custody to the exact structured LLVM module bytes.
pub fn bind_gfx942_scalar_f32_post_llvm_stage_contents_v1(
    lowering: CheckedGfx942ScalarF32KirToLlvmRefinementV1,
    stages: CheckedPostLlvmStageContentsV1,
) -> Result<
    Gfx942ScalarF32PostLlvmContentReadinessV1,
    Box<Gfx942ScalarF32PostLlvmContentBindingFailureV1>,
> {
    if lowering.llvm_ir() != stages.lowered_llvm_module() {
        return Err(Box::new(Gfx942ScalarF32PostLlvmContentBindingFailureV1 {
            lowering,
            stages,
            reason: Gfx942ScalarF32PostLlvmContentBindingErrorV1::LoweredLlvmContentMismatch,
        }));
    }
    let occurrence_identity = match (
        stages.retains_complete_occurrence_and_assembly_replay_custody(),
        stages.occurrence_transcript(),
    ) {
        (true, Some(transcript)) => transcript.identity(),
        _ => {
            return Err(Box::new(Gfx942ScalarF32PostLlvmContentBindingFailureV1 {
                lowering,
                stages,
                reason:
                    Gfx942ScalarF32PostLlvmContentBindingErrorV1::MissingPipelineOccurrenceAndAssemblyReplay,
            }));
        }
    };
    let lowering_identity = lowering.evidence_identity();
    let stage_identity = stages.record().identity();
    let mut canonical_evidence = Vec::with_capacity(112);
    canonical_evidence.extend_from_slice(b"FE2O3/GFX942-SCALAR-F32-POST-LLVM-CONTENT/V1\0");
    canonical_evidence.extend_from_slice(&lowering_identity.sha256());
    canonical_evidence.extend_from_slice(&lowering_identity.byte_len().to_le_bytes());
    canonical_evidence.extend_from_slice(&stage_identity.sha256());
    canonical_evidence.extend_from_slice(&stage_identity.byte_len().to_le_bytes());
    canonical_evidence.extend_from_slice(&occurrence_identity.sha256());
    canonical_evidence.extend_from_slice(&occurrence_identity.byte_len().to_le_bytes());
    Ok(Gfx942ScalarF32PostLlvmContentReadinessV1 {
        lowering,
        stages,
        canonical_evidence: canonical_evidence.into_boxed_slice(),
    })
}

/// One exact operational translation fact that current evidence cannot validate semantically.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Gfx942OperationalTranslationUnsupportedV1 {
    /// A machine opcode has no admitted structural or operational model.
    MachineOpcode {
        /// Exact instruction offset.
        offset: u64,
        /// Exact authenticated opcode identifier.
        opcode: String,
    },
    /// A structurally checked direct branch still lacks executable EXEC/control semantics.
    MachineControlSemantics {
        /// Exact instruction offset.
        offset: u64,
        /// Exact authenticated opcode identifier.
        opcode: String,
    },
    /// A structurally checked memory operation still lacks effective-address semantics.
    MachineEffectiveAddressSemantics {
        /// Exact instruction offset.
        offset: u64,
        /// Exact authenticated opcode identifier.
        opcode: String,
        /// Authenticated access width.
        byte_width: u16,
    },
    /// A separate f32 operation still lacks authenticated gfx942 MODE/opcode semantics.
    MachineIeeeBinary32Semantics {
        /// Exact instruction offset.
        offset: u64,
        /// Exact authenticated opcode identifier.
        opcode: String,
    },
    /// No compiler-emitted relation maps source loop backedges to machine natural loops.
    LoopCorrespondence {
        /// Number of dominance-qualified structured KIR backedges.
        structured_backedges: u32,
        /// Number of authenticated machine natural loops.
        machine_natural_loops: u32,
    },
}

/// Independently checked operational translation structure with explicit unsupported semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942OperationalTranslationValidationV1 {
    structured_operations: u32,
    structured_backedges: u32,
    machine_instructions: u32,
    machine_natural_loops: u32,
    unsupported: Box<[Gfx942OperationalTranslationUnsupportedV1]>,
}

impl Gfx942OperationalTranslationValidationV1 {
    /// Returns the number of independently checked structured operations.
    pub const fn structured_operations(&self) -> u32 {
        self.structured_operations
    }

    /// Returns the number of dominance-qualified structured backedges.
    pub const fn structured_backedges(&self) -> u32 {
        self.structured_backedges
    }

    /// Returns the number of authenticated machine instructions structurally classified.
    pub const fn machine_instructions(&self) -> u32 {
        self.machine_instructions
    }

    /// Returns the number of authenticated machine natural loops.
    pub const fn machine_natural_loops(&self) -> u32 {
        self.machine_natural_loops
    }

    /// Returns every exact operation-level semantic gap.
    pub fn unsupported(&self) -> &[Gfx942OperationalTranslationUnsupportedV1] {
        &self.unsupported
    }

    /// Reports independent checks of closed KIR/LLVM operation, CFG, address, and IEEE shape.
    pub const fn validates_admitted_structured_operational_shapes(&self) -> bool {
        true
    }

    /// Returns false while any operation-level semantic gap remains.
    pub fn establishes_complete_operational_translation(&self) -> bool {
        self.unsupported.is_empty()
    }

    /// Structural translation checks are not source-to-machine semantic equivalence.
    pub const fn proves_semantic_equivalence(&self) -> bool {
        false
    }
}

/// Structural failure while independently validating admitted operational translation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942OperationalTranslationValidationErrorV1 {
    /// A structured operation violated its closed independent contract.
    StructuredOperationContractMismatch,
    /// A structured value definition or use was absent or had the wrong type/carrier.
    StructuredValueContractMismatch,
    /// One operation result or operand violated its independently checked value contract.
    StructuredOperationValueContractMismatch {
        /// Source block identifier.
        block: u32,
        /// Source operation ordinal.
        operation: u32,
        /// Closed source operation kind.
        kind: StructuredKirOperationKindV1,
    },
    /// Structured CFG blocks, edges, or ordinals were malformed.
    StructuredControlFlowMismatch,
    /// The bounded independent CFG computation exhausted its work budget.
    StructuredControlFlowBudgetExceeded,
    /// Authenticated machine branch facts were internally inconsistent.
    MachineControlShapeMismatch,
    /// Authenticated machine memory facts were internally inconsistent.
    MachineMemoryShapeMismatch,
    /// Authenticated machine dataflow could not derive loop structure.
    MachineDataflowUnavailable,
    /// A checked count could not be represented in the transcript schema.
    CountOverflow,
}

impl fmt::Display for Gfx942OperationalTranslationValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx942 operational translation rejected: {self:?}"
        )
    }
}

impl Error for Gfx942OperationalTranslationValidationErrorV1 {}

/// Why exact post-LLVM custody could not be joined to authenticated machine recurrence facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942ScalarF32PostLlvmMachineBindingErrorV1 {
    /// The analyzer did not consume the final code-object bytes retained by compiler custody.
    FinalCodeObjectMismatch,
    /// The structured LLVM function and analyzed machine function differ.
    FunctionSymbolMismatch,
    /// The admitted structured LLVM subset does not contain one separate scalar multiply/add edge.
    UnsupportedStructuredScalarDataflow,
    /// The structured scalar result carriers do not name their exact operation results.
    StructuredResultCarrierMismatch,
    /// The authenticated machine trace no longer agrees with its checked recurrence artifact.
    AuthenticatedMachineDataflowMismatch,
    /// Independent structured/machine operational shape validation failed.
    OperationalTranslationStructureMismatch(Gfx942OperationalTranslationValidationErrorV1),
}

impl fmt::Display for Gfx942ScalarF32PostLlvmMachineBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx942 post-LLVM/machine scalar correspondence rejected: {self:?}"
        )
    }
}

impl Error for Gfx942ScalarF32PostLlvmMachineBindingErrorV1 {}

/// Move-only exact-HSACO and admitted scalar def-use correspondence.
///
/// The checker establishes that the authenticated analyzer consumed the exact final code object in
/// custody and that the unique structured non-contracted `fmul` -> `fadd` SSA edge corresponds to
/// the unique authenticated `V_MUL_F32_e32_vi` -> `V_ADD_F32_e32_vi` register edge. It does not
/// prove that LLVM selected those instructions from the post-optimization bitcode, does not assign
/// gfx942 operational meaning to their encodings, and does not cover loop control or addresses.
#[must_use = "dropping scalar correspondence abandons exact final-HSACO machine custody"]
pub struct Gfx942ScalarF32PostLlvmMachineReadinessV1 {
    post_llvm: Gfx942ScalarF32PostLlvmContentReadinessV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    operational_translation: Gfx942OperationalTranslationValidationV1,
    canonical_evidence: Box<[u8]>,
}

impl fmt::Debug for Gfx942ScalarF32PostLlvmMachineReadinessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32PostLlvmMachineReadinessV1")
            .field("post_llvm", &self.post_llvm.evidence_identity())
            .field("machine", &self.machine.authenticated_execution_identity())
            .field("missing", &self.missing_inputs())
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32PostLlvmMachineReadinessV1 {
    /// Returns exact post-LLVM compiler custody.
    pub const fn post_llvm(&self) -> &Gfx942ScalarF32PostLlvmContentReadinessV1 {
        &self.post_llvm
    }

    /// Returns the authenticated recurrence-step analysis.
    pub const fn machine(&self) -> &AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
        &self.machine
    }

    /// Returns independent operation/CFG/machine-shape checks and exact unsupported semantics.
    pub const fn operational_translation(&self) -> &Gfx942OperationalTranslationValidationV1 {
        &self.operational_translation
    }

    /// Returns the canonical conjunction evidence.
    pub fn canonical_evidence_bytes(&self) -> &[u8] {
        &self.canonical_evidence
    }

    /// The exact final code object is the authenticated analyzer request payload.
    pub const fn binds_exact_final_code_object_to_authenticated_machine_analysis(&self) -> bool {
        true
    }

    /// The closed scalar operation and machine register def-use edges were checked independently.
    pub const fn validates_admitted_scalar_operation_def_use_correspondence(&self) -> bool {
        true
    }

    /// No compiler-emitted mapping binds post-optimization LLVM instructions to machine offsets.
    pub const fn establishes_llvm_instruction_selection(&self) -> bool {
        false
    }

    /// No operational ISA model was used for control, addresses, or floating-point instructions.
    pub const fn establishes_gfx942_operational_semantics(&self) -> bool {
        false
    }

    /// Returns the still-unavailable proof inputs.
    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        &GFX942_SCALAR_F32_MACHINE_REFINEMENT_INPUTS_AFTER_POST_LLVM_CONTENT_V1
    }

    /// This correspondence grants no compiler, publication, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Decomposes this move-only conjunction.
    pub fn into_parts(
        self,
    ) -> (
        Gfx942ScalarF32PostLlvmContentReadinessV1,
        AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
        Gfx942OperationalTranslationValidationV1,
        Box<[u8]>,
    ) {
        (
            self.post_llvm,
            self.machine,
            self.operational_translation,
            self.canonical_evidence,
        )
    }
}

/// Failed exact-HSACO/scalar correspondence retaining both move-only owners.
pub struct Gfx942ScalarF32PostLlvmMachineBindingFailureV1 {
    post_llvm: Gfx942ScalarF32PostLlvmContentReadinessV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    error: Gfx942ScalarF32PostLlvmMachineBindingErrorV1,
}

impl fmt::Debug for Gfx942ScalarF32PostLlvmMachineBindingFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32PostLlvmMachineBindingFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32PostLlvmMachineBindingFailureV1 {
    /// Returns the rejected axis.
    pub const fn error(&self) -> Gfx942ScalarF32PostLlvmMachineBindingErrorV1 {
        self.error
    }

    /// Recovers both move-only owners.
    pub fn into_parts(
        self,
    ) -> (
        Gfx942ScalarF32PostLlvmContentReadinessV1,
        AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
        Gfx942ScalarF32PostLlvmMachineBindingErrorV1,
    ) {
        (self.post_llvm, self.machine, self.error)
    }
}

/// Joins exact compiler custody to the authenticated final-HSACO scalar recurrence analysis.
pub fn bind_gfx942_scalar_f32_post_llvm_and_authenticated_machine_v1(
    post_llvm: Gfx942ScalarF32PostLlvmContentReadinessV1,
    machine: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
) -> Result<
    Gfx942ScalarF32PostLlvmMachineReadinessV1,
    Box<Gfx942ScalarF32PostLlvmMachineBindingFailureV1>,
> {
    let result = check_post_llvm_machine_scalar_correspondence_v1(&post_llvm, &machine);
    if let Err(error) = result {
        return Err(Box::new(Gfx942ScalarF32PostLlvmMachineBindingFailureV1 {
            post_llvm,
            machine,
            error,
        }));
    }
    let operational_translation = match validate_gfx942_operational_translation_v1(
        &post_llvm, &machine,
    ) {
        Ok(validation) => validation,
        Err(validation_error) => {
            return Err(Box::new(Gfx942ScalarF32PostLlvmMachineBindingFailureV1 {
                    post_llvm,
                    machine,
                    error: Gfx942ScalarF32PostLlvmMachineBindingErrorV1::OperationalTranslationStructureMismatch(
                        validation_error,
                    ),
                }));
        }
    };

    let post_identity = post_llvm.evidence_identity();
    let execution_identity = machine.authenticated_execution_identity();
    let trace_identity = machine
        .authenticated_execution()
        .analysis()
        .trace()
        .identity();
    let artifact = machine.artifact();
    let mut canonical_evidence = Vec::with_capacity(192);
    canonical_evidence
        .extend_from_slice(b"FE2O3/GFX942-SCALAR-F32-POST-LLVM-MACHINE-CORRESPONDENCE/V1\0");
    canonical_evidence.extend_from_slice(&post_identity.sha256());
    canonical_evidence.extend_from_slice(&post_identity.byte_len().to_le_bytes());
    canonical_evidence.extend_from_slice(&execution_identity.sha256());
    canonical_evidence.extend_from_slice(&execution_identity.byte_len().to_le_bytes());
    canonical_evidence.extend_from_slice(&trace_identity.sha256());
    canonical_evidence.extend_from_slice(&trace_identity.byte_len().to_le_bytes());
    canonical_evidence.extend_from_slice(&artifact.multiply_offset().to_le_bytes());
    canonical_evidence.extend_from_slice(&artifact.add_offset().to_le_bytes());
    canonical_evidence.extend_from_slice(&artifact.product_register().to_le_bytes());
    canonical_evidence.extend_from_slice(&artifact.accumulator_register().to_le_bytes());
    canonical_evidence.extend_from_slice(&artifact.result_register().to_le_bytes());
    canonical_evidence.extend_from_slice(
        &operational_translation
            .structured_operations()
            .to_le_bytes(),
    );
    canonical_evidence
        .extend_from_slice(&operational_translation.structured_backedges().to_le_bytes());
    canonical_evidence
        .extend_from_slice(&operational_translation.machine_instructions().to_le_bytes());
    canonical_evidence.extend_from_slice(
        &operational_translation
            .machine_natural_loops()
            .to_le_bytes(),
    );
    Ok(Gfx942ScalarF32PostLlvmMachineReadinessV1 {
        post_llvm,
        machine,
        operational_translation,
        canonical_evidence: canonical_evidence.into_boxed_slice(),
    })
}

/// Independently validates the admitted structured operation contracts and machine trace shape.
///
/// Unsupported entries are successful fail-closed output, not validation failures. They identify
/// exact machine operations for which executable ISA semantics or a compiler-emitted loop mapping
/// is still absent.
pub fn validate_gfx942_operational_translation_v1(
    post_llvm: &Gfx942ScalarF32PostLlvmContentReadinessV1,
    machine: &AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
) -> Result<Gfx942OperationalTranslationValidationV1, Gfx942OperationalTranslationValidationErrorV1>
{
    const MAX_CFG_BLOCKS: usize = 256;
    const MAX_CFG_WORK: usize = 1_000_000;

    let derivation = post_llvm.lowering().derivation();
    let mut values = BTreeMap::new();
    for value in derivation.values() {
        if value.function() != 0 || values.insert(value.value(), *value).is_some() {
            return Err(
                Gfx942OperationalTranslationValidationErrorV1::StructuredValueContractMismatch,
            );
        }
    }
    let mut blocks = BTreeMap::new();
    for block in derivation.blocks() {
        if block.function() != 0 || blocks.insert(block.block(), block.llvm_ordinal()).is_some() {
            return Err(
                Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch,
            );
        }
    }
    if blocks.is_empty()
        || blocks.len() > MAX_CFG_BLOCKS
        || blocks.values().copied().collect::<BTreeSet<_>>() != (0..blocks.len() as u32).collect()
    {
        return Err(Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch);
    }
    let mut next_operation = BTreeMap::<u32, u32>::new();
    for operation in derivation.operations() {
        if operation.function() != 0 || !blocks.contains_key(&operation.block()) {
            return Err(
                Gfx942OperationalTranslationValidationErrorV1::StructuredOperationContractMismatch,
            );
        }
        let next = next_operation.entry(operation.block()).or_default();
        if operation.operation() != *next {
            return Err(
                Gfx942OperationalTranslationValidationErrorV1::StructuredOperationContractMismatch,
            );
        }
        *next = next
            .checked_add(1)
            .ok_or(Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?;
        validate_structured_operation_contract_v1(operation, &values)?;
    }
    let structured_backedges = validate_structured_cfg_and_count_backedges_v1(
        derivation.blocks(),
        derivation.edges(),
        &values,
        MAX_CFG_WORK,
    )?;

    let execution = machine.authenticated_execution();
    let trace = execution.analysis().trace();
    let dataflow = Gfx942MachineDataflowV1::derive(trace)
        .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::MachineDataflowUnavailable)?;
    let machine_natural_loops = dataflow
        .natural_loops(derivation.function_symbol())
        .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::MachineDataflowUnavailable)?
        .len();
    let mut unsupported = Vec::new();
    let mut machine_instructions = 0_usize;
    for instruction in trace
        .instructions()
        .iter()
        .filter(|instruction| instruction.function_symbol() == derivation.function_symbol())
    {
        machine_instructions = machine_instructions
            .checked_add(1)
            .ok_or(Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?;
        validate_machine_instruction_shape_v1(instruction)?;
        let offset = instruction.instruction_offset();
        let opcode = instruction.opcode().to_owned();
        if matches!(
            instruction.opcode(),
            "V_MUL_F32_e32_vi" | "V_ADD_F32_e32_vi"
        ) {
            arithmetic_shape(instruction).map_err(|_| {
                Gfx942OperationalTranslationValidationErrorV1::MachineMemoryShapeMismatch
            })?;
            unsupported.push(
                Gfx942OperationalTranslationUnsupportedV1::MachineIeeeBinary32Semantics {
                    offset,
                    opcode,
                },
            );
        } else if instruction.branch_kind() != PhysicalMachineBranchKindV1::None
            || instruction.opcode() == "S_ENDPGM"
        {
            unsupported.push(
                Gfx942OperationalTranslationUnsupportedV1::MachineControlSemantics {
                    offset,
                    opcode,
                },
            );
        } else if instruction.memory_access() != PhysicalMachineMemoryAccessV1::None {
            unsupported.push(
                Gfx942OperationalTranslationUnsupportedV1::MachineEffectiveAddressSemantics {
                    offset,
                    opcode,
                    byte_width: instruction.memory_access().byte_width(),
                },
            );
        } else {
            unsupported
                .push(Gfx942OperationalTranslationUnsupportedV1::MachineOpcode { offset, opcode });
        }
    }
    if structured_backedges != 0 || machine_natural_loops != 0 {
        unsupported.push(
            Gfx942OperationalTranslationUnsupportedV1::LoopCorrespondence {
                structured_backedges: u32::try_from(structured_backedges)
                    .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?,
                machine_natural_loops: u32::try_from(machine_natural_loops)
                    .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?,
            },
        );
    }
    Ok(Gfx942OperationalTranslationValidationV1 {
        structured_operations: u32::try_from(derivation.operations().len())
            .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?,
        structured_backedges: u32::try_from(structured_backedges)
            .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?,
        machine_instructions: u32::try_from(machine_instructions)
            .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?,
        machine_natural_loops: u32::try_from(machine_natural_loops)
            .map_err(|_| Gfx942OperationalTranslationValidationErrorV1::CountOverflow)?,
        unsupported: unsupported.into_boxed_slice(),
    })
}

fn validate_structured_operation_contract_v1(
    operation: &StructuredKirOperationLoweringV1,
    values: &BTreeMap<u32, StructuredKirValueLoweringV1>,
) -> Result<(), Gfx942OperationalTranslationValidationErrorV1> {
    let expected_opcodes = expected_structured_llvm_opcodes_v1(operation.kind());
    let (operand_count, result_count) = structured_operation_arity_v1(operation.kind());
    if operation.llvm_opcodes() != expected_opcodes
        || operation.operands().len() != operand_count
        || operation.results().len() != result_count
        || operation.result_types().len() != result_count
        || operation
            .operands()
            .iter()
            .any(|operand| !values.contains_key(operand))
    {
        return Err(
            Gfx942OperationalTranslationValidationErrorV1::StructuredOperationContractMismatch,
        );
    }
    for (result_index, result) in operation.results().iter().enumerate() {
        let value = values.get(result).ok_or(
            Gfx942OperationalTranslationValidationErrorV1::StructuredValueContractMismatch,
        )?;
        if value.ty() != operation.result_types()[result_index]
            || value.carrier() != expected_result_carrier_v1(operation)
        {
            return Err(Gfx942OperationalTranslationValidationErrorV1::StructuredOperationValueContractMismatch {
                block: operation.block(),
                operation: operation.operation(),
                kind: operation.kind(),
            });
        }
    }
    validate_structured_operation_types_v1(operation, values).map_err(|_| {
        Gfx942OperationalTranslationValidationErrorV1::StructuredOperationValueContractMismatch {
            block: operation.block(),
            operation: operation.operation(),
            kind: operation.kind(),
        }
    })
}

fn expected_result_carrier_v1(
    operation: &StructuredKirOperationLoweringV1,
) -> StructuredKirValueCarrierV1 {
    match operation.kind() {
        StructuredKirOperationKindV1::KernelContextIssue => StructuredKirValueCarrierV1::Erased,
        StructuredKirOperationKindV1::GlobalCapabilityBind => StructuredKirValueCarrierV1::Alias {
            value: operation.operands()[1],
        },
        StructuredKirOperationKindV1::GlobalCapabilityIndex => StructuredKirValueCarrierV1::Alias {
            value: operation.operands()[1],
        },
        StructuredKirOperationKindV1::ConstantIndex | StructuredKirOperationKindV1::ConstantF32 => {
            StructuredKirValueCarrierV1::Immediate
        }
        _ => StructuredKirValueCarrierV1::Instruction {
            block: operation.block(),
            operation: operation.operation(),
            ordinal: operation.llvm_instruction_count().saturating_sub(1),
        },
    }
}

fn structured_operation_arity_v1(kind: StructuredKirOperationKindV1) -> (usize, usize) {
    use StructuredKirOperationKindV1 as Kind;
    match kind {
        Kind::KernelContextIssue | Kind::GlobalId1d | Kind::ConstantIndex | Kind::ConstantF32 => {
            (0, 1)
        }
        Kind::GlobalCapabilityBind
        | Kind::GlobalCapabilityIndex
        | Kind::IntegerNotEqual
        | Kind::IntegerLessThan
        | Kind::IntegerMultiply
        | Kind::IntegerAdd
        | Kind::BooleanAnd
        | Kind::IntegerDivide
        | Kind::IntegerRemainder
        | Kind::GlobalGetElementPointer
        | Kind::F32Multiply
        | Kind::F32Add => (2, 1),
        Kind::Select | Kind::GuardedLoadF32 => (3, 1),
        Kind::SliceLength | Kind::SliceData => (1, 1),
        Kind::GuardedStoreF32 => (3, 0),
    }
}

fn expected_structured_llvm_opcodes_v1(
    kind: StructuredKirOperationKindV1,
) -> &'static [StructuredLlvmOpcodeV1] {
    use StructuredLlvmOpcodeV1 as Opcode;
    match kind {
        StructuredKirOperationKindV1::KernelContextIssue
        | StructuredKirOperationKindV1::GlobalCapabilityBind
        | StructuredKirOperationKindV1::GlobalCapabilityIndex
        | StructuredKirOperationKindV1::ConstantIndex
        | StructuredKirOperationKindV1::ConstantF32 => &[],
        StructuredKirOperationKindV1::GlobalId1d => &[
            Opcode::CallWorkitemIdX,
            Opcode::CallWorkgroupIdX,
            Opcode::ZeroExtendI32ToI64,
            Opcode::ZeroExtendI32ToI64,
            Opcode::MultiplyI64,
            Opcode::AddI64,
        ],
        StructuredKirOperationKindV1::IntegerNotEqual => &[Opcode::CompareNotEqualI64],
        StructuredKirOperationKindV1::IntegerLessThan => &[Opcode::CompareUnsignedLessThanI64],
        StructuredKirOperationKindV1::Select => &[Opcode::Select],
        StructuredKirOperationKindV1::IntegerMultiply => &[Opcode::MultiplyI64],
        StructuredKirOperationKindV1::IntegerAdd => &[Opcode::AddI64],
        StructuredKirOperationKindV1::BooleanAnd => &[Opcode::AndI1],
        StructuredKirOperationKindV1::IntegerDivide => &[Opcode::DivideUnsignedI64],
        StructuredKirOperationKindV1::IntegerRemainder => &[Opcode::RemainderUnsignedI64],
        StructuredKirOperationKindV1::SliceLength => &[Opcode::CopySliceLengthI64],
        StructuredKirOperationKindV1::SliceData => &[Opcode::ProjectSliceDataGlobal],
        StructuredKirOperationKindV1::GlobalGetElementPointer => {
            &[Opcode::GetElementPointerGlobalF32]
        }
        StructuredKirOperationKindV1::GuardedLoadF32 => &[
            Opcode::ConditionalBranch,
            Opcode::LoadGlobalF32,
            Opcode::Branch,
            Opcode::Branch,
            Opcode::PhiF32,
        ],
        StructuredKirOperationKindV1::GuardedStoreF32 => &[
            Opcode::ConditionalBranch,
            Opcode::StoreGlobalF32,
            Opcode::Branch,
        ],
        StructuredKirOperationKindV1::F32Multiply => &[Opcode::MultiplyF32],
        StructuredKirOperationKindV1::F32Add => &[Opcode::AddF32],
    }
}

fn validate_structured_operation_types_v1(
    operation: &StructuredKirOperationLoweringV1,
    values: &BTreeMap<u32, StructuredKirValueLoweringV1>,
) -> Result<(), Gfx942OperationalTranslationValidationErrorV1> {
    use StructuredKirOperationKindV1 as Kind;
    use StructuredKirValueTypeV1 as Type;
    let operand_type = |index: usize| {
        values
            .get(&operation.operands()[index])
            .map(|value| value.ty())
    };
    let result_type = operation.result_types().first().copied();
    let valid = match operation.kind() {
        Kind::KernelContextIssue => result_type == Some(Type::KernelContext),
        Kind::GlobalCapabilityBind => matches!(
            (operand_type(0), operand_type(1), result_type),
            (
                Some(Type::KernelContext),
                Some(Type::GlobalReadSliceF32),
                Some(Type::GlobalReadCapabilityF32)
            ) | (
                Some(Type::KernelContext),
                Some(Type::GlobalWriteSliceF32),
                Some(Type::GlobalWriteCapabilityF32)
            )
        ),
        Kind::GlobalCapabilityIndex => {
            matches!(
                operand_type(0),
                Some(Type::GlobalReadCapabilityF32 | Type::GlobalWriteCapabilityF32)
            ) && operand_type(1) == Some(Type::Index)
                && result_type == Some(Type::Index)
        }
        Kind::GlobalId1d | Kind::ConstantIndex => result_type == Some(Type::Index),
        Kind::ConstantF32 => result_type == Some(Type::F32),
        Kind::IntegerNotEqual | Kind::IntegerLessThan => {
            operand_type(0) == Some(Type::Index)
                && operand_type(1) == Some(Type::Index)
                && result_type == Some(Type::Bool)
        }
        Kind::Select => {
            operand_type(0) == Some(Type::Bool)
                && operand_type(1) == operand_type(2)
                && result_type == operand_type(1)
        }
        Kind::IntegerMultiply | Kind::IntegerAdd | Kind::IntegerDivide | Kind::IntegerRemainder => {
            operand_type(0) == Some(Type::Index)
                && operand_type(1) == Some(Type::Index)
                && result_type == Some(Type::Index)
        }
        Kind::BooleanAnd => {
            operand_type(0) == Some(Type::Bool)
                && operand_type(1) == Some(Type::Bool)
                && result_type == Some(Type::Bool)
        }
        Kind::SliceLength => {
            matches!(
                operand_type(0),
                Some(
                    Type::GlobalReadSliceF32
                        | Type::GlobalWriteSliceF32
                        | Type::GlobalReadCapabilityF32
                        | Type::GlobalWriteCapabilityF32
                )
            ) && result_type == Some(Type::Index)
        }
        Kind::SliceData => matches!(
            (operand_type(0), result_type),
            (
                Some(Type::GlobalReadSliceF32 | Type::GlobalReadCapabilityF32),
                Some(Type::GlobalReadPointerF32)
            ) | (
                Some(Type::GlobalWriteSliceF32 | Type::GlobalWriteCapabilityF32),
                Some(Type::GlobalWritePointerF32)
            )
        ),
        Kind::GlobalGetElementPointer => {
            matches!(
                operand_type(0),
                Some(Type::GlobalReadPointerF32 | Type::GlobalWritePointerF32)
            ) && operand_type(1) == Some(Type::Index)
                && result_type == operand_type(0)
        }
        Kind::GuardedLoadF32 => {
            operand_type(0) == Some(Type::GlobalReadPointerF32)
                && operand_type(1) == Some(Type::Bool)
                && operand_type(2) == Some(Type::F32)
                && result_type == Some(Type::F32)
        }
        Kind::GuardedStoreF32 => {
            operand_type(0) == Some(Type::GlobalWritePointerF32)
                && operand_type(1) == Some(Type::Bool)
                && operand_type(2) == Some(Type::F32)
        }
        Kind::F32Multiply | Kind::F32Add => {
            operand_type(0) == Some(Type::F32)
                && operand_type(1) == Some(Type::F32)
                && result_type == Some(Type::F32)
        }
    };
    valid
        .then_some(())
        .ok_or(Gfx942OperationalTranslationValidationErrorV1::StructuredValueContractMismatch)
}

fn validate_structured_cfg_and_count_backedges_v1(
    blocks: &[StructuredKirBlockLoweringV1],
    edges: &[StructuredKirControlEdgeLoweringV1],
    values: &BTreeMap<u32, StructuredKirValueLoweringV1>,
    max_work: usize,
) -> Result<usize, Gfx942OperationalTranslationValidationErrorV1> {
    let by_id = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.block(), index))
        .collect::<BTreeMap<_, _>>();
    let entry = blocks
        .iter()
        .position(|block| block.llvm_ordinal() == 0)
        .ok_or(Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch)?;
    let mut predecessors = vec![Vec::new(); blocks.len()];
    let mut edge_ordinals = BTreeMap::<u32, Vec<u16>>::new();
    let mut seen = BTreeSet::new();
    let mut phis = BTreeMap::<u32, BTreeMap<u16, StructuredKirValueTypeV1>>::new();
    for value in values.values() {
        if let StructuredKirValueCarrierV1::Phi { block, ordinal } = value.carrier() {
            if !by_id.contains_key(&block)
                || phis
                    .entry(block)
                    .or_default()
                    .insert(ordinal, value.ty())
                    .is_some()
            {
                return Err(
                    Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch,
                );
            }
        }
    }
    if phis
        .values()
        .any(|block_phis| block_phis.keys().copied().ne(0..block_phis.len() as u16))
    {
        return Err(Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch);
    }
    for edge in edges {
        let predecessor = *by_id
            .get(&edge.predecessor())
            .ok_or(Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch)?;
        let successor = *by_id
            .get(&edge.successor())
            .ok_or(Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch)?;
        if edge.function() != 0
            || !seen.insert((edge.predecessor(), edge.ordinal(), edge.successor()))
        {
            return Err(
                Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch,
            );
        }
        let successor_phis = phis.get(&edge.successor());
        if edge.arguments().len() != successor_phis.map_or(0, BTreeMap::len)
            || edge
                .arguments()
                .iter()
                .enumerate()
                .any(|(ordinal, argument)| {
                    let Some(argument) = values.get(argument) else {
                        return true;
                    };
                    successor_phis
                        .and_then(|phis| phis.get(&(ordinal as u16)))
                        .is_none_or(|phi_type| *phi_type != argument.ty())
                })
        {
            return Err(
                Gfx942OperationalTranslationValidationErrorV1::StructuredValueContractMismatch,
            );
        }
        predecessors[successor].push(predecessor);
        edge_ordinals
            .entry(edge.predecessor())
            .or_default()
            .push(edge.ordinal());
    }
    if edge_ordinals.values_mut().any(|ordinals| {
        ordinals.sort_unstable();
        ordinals.iter().copied().ne(0..ordinals.len() as u16)
    }) {
        return Err(Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowMismatch);
    }

    let all = (0..blocks.len()).collect::<BTreeSet<_>>();
    let mut dominators = vec![all.clone(); blocks.len()];
    dominators[entry] = [entry].into_iter().collect();
    let mut work = 0_usize;
    loop {
        let mut changed = false;
        for block in 0..blocks.len() {
            if block == entry {
                continue;
            }
            work = work
                .checked_add(predecessors[block].len().max(1) * blocks.len())
                .ok_or(
                    Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowBudgetExceeded,
                )?;
            if work > max_work {
                return Err(
                    Gfx942OperationalTranslationValidationErrorV1::StructuredControlFlowBudgetExceeded,
                );
            }
            let mut next = if let Some(first) = predecessors[block].first() {
                dominators[*first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in predecessors[block].iter().skip(1) {
                next.retain(|candidate| dominators[*predecessor].contains(candidate));
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(edges
        .iter()
        .filter(|edge| {
            let predecessor = by_id[&edge.predecessor()];
            let successor = by_id[&edge.successor()];
            dominators[predecessor].contains(&successor)
        })
        .count())
}

fn validate_machine_instruction_shape_v1(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<(), Gfx942OperationalTranslationValidationErrorV1> {
    let branch_valid = match instruction.branch_kind() {
        PhysicalMachineBranchKindV1::None => instruction.branch_target().is_none(),
        PhysicalMachineBranchKindV1::ConditionalDirect
        | PhysicalMachineBranchKindV1::UnconditionalDirect => {
            instruction.branch_target().is_some() && instruction.flags().is_terminator()
        }
        PhysicalMachineBranchKindV1::DirectCall => instruction.branch_target().is_some(),
        PhysicalMachineBranchKindV1::Return => {
            instruction.branch_target().is_none() && instruction.flags().is_terminator()
        }
    };
    if !branch_valid {
        return Err(Gfx942OperationalTranslationValidationErrorV1::MachineControlShapeMismatch);
    }
    let flags = instruction.flags();
    let memory_valid = match instruction.memory_access() {
        PhysicalMachineMemoryAccessV1::None => !flags.may_load() && !flags.may_store(),
        PhysicalMachineMemoryAccessV1::Read { byte_width }
        | PhysicalMachineMemoryAccessV1::WorkgroupRead { byte_width } => {
            byte_width != 0 && flags.may_load() && !flags.may_store()
        }
        PhysicalMachineMemoryAccessV1::Write { byte_width }
        | PhysicalMachineMemoryAccessV1::WorkgroupWrite { byte_width } => {
            byte_width != 0 && !flags.may_load() && flags.may_store()
        }
        PhysicalMachineMemoryAccessV1::ReadWrite { byte_width }
        | PhysicalMachineMemoryAccessV1::WorkgroupReadWrite { byte_width } => {
            byte_width != 0 && flags.may_load() && flags.may_store()
        }
    };
    memory_valid
        .then_some(())
        .ok_or(Gfx942OperationalTranslationValidationErrorV1::MachineMemoryShapeMismatch)
}

fn check_post_llvm_machine_scalar_correspondence_v1(
    post_llvm: &Gfx942ScalarF32PostLlvmContentReadinessV1,
    machine: &AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
) -> Result<(), Gfx942ScalarF32PostLlvmMachineBindingErrorV1> {
    let execution = machine.authenticated_execution();
    if post_llvm.stages().final_code_object() != execution.request().exact_payload_bytes() {
        return Err(Gfx942ScalarF32PostLlvmMachineBindingErrorV1::FinalCodeObjectMismatch);
    }
    let derivation = post_llvm.lowering().derivation();
    if derivation.function_symbol() != machine.artifact().function_symbol() {
        return Err(Gfx942ScalarF32PostLlvmMachineBindingErrorV1::FunctionSymbolMismatch);
    }

    let multiplies = derivation
        .operations()
        .iter()
        .filter(|operation| operation.kind() == StructuredKirOperationKindV1::F32Multiply)
        .collect::<Vec<_>>();
    let adds = derivation
        .operations()
        .iter()
        .filter(|operation| operation.kind() == StructuredKirOperationKindV1::F32Add)
        .collect::<Vec<_>>();
    let ([multiply], [add]) = (multiplies.as_slice(), adds.as_slice()) else {
        return Err(
            Gfx942ScalarF32PostLlvmMachineBindingErrorV1::UnsupportedStructuredScalarDataflow,
        );
    };
    let ([product], [next_accumulator]) = (multiply.results(), add.results()) else {
        return Err(
            Gfx942ScalarF32PostLlvmMachineBindingErrorV1::UnsupportedStructuredScalarDataflow,
        );
    };
    let [product_operand, accumulator_operand] = add.operands() else {
        return Err(
            Gfx942ScalarF32PostLlvmMachineBindingErrorV1::UnsupportedStructuredScalarDataflow,
        );
    };
    if multiply.llvm_opcodes() != [StructuredLlvmOpcodeV1::MultiplyF32]
        || add.llvm_opcodes() != [StructuredLlvmOpcodeV1::AddF32]
        || product_operand != product
        || accumulator_operand == product
    {
        return Err(
            Gfx942ScalarF32PostLlvmMachineBindingErrorV1::UnsupportedStructuredScalarDataflow,
        );
    }
    for (value, operation) in [(product, multiply), (next_accumulator, add)] {
        let mappings = derivation
            .values()
            .iter()
            .filter(|mapping| mapping.value() == *value)
            .collect::<Vec<_>>();
        let [mapping] = mappings.as_slice() else {
            return Err(
                Gfx942ScalarF32PostLlvmMachineBindingErrorV1::StructuredResultCarrierMismatch,
            );
        };
        if mapping.ty() != StructuredKirValueTypeV1::F32
            || mapping.carrier()
                != (StructuredKirValueCarrierV1::Instruction {
                    block: operation.block(),
                    operation: operation.operation(),
                    ordinal: 0,
                })
        {
            return Err(
                Gfx942ScalarF32PostLlvmMachineBindingErrorV1::StructuredResultCarrierMismatch,
            );
        }
    }

    let trace = execution.analysis().trace();
    let artifact = machine.artifact();
    let multiply_instruction = trace.instructions().iter().find(|instruction| {
        instruction.function_symbol() == derivation.function_symbol()
            && instruction.instruction_offset() == artifact.multiply_offset()
    });
    let add_instruction = trace.instructions().iter().find(|instruction| {
        instruction.function_symbol() == derivation.function_symbol()
            && instruction.instruction_offset() == artifact.add_offset()
    });
    let (Some(multiply_instruction), Some(add_instruction)) =
        (multiply_instruction, add_instruction)
    else {
        return Err(
            Gfx942ScalarF32PostLlvmMachineBindingErrorV1::AuthenticatedMachineDataflowMismatch,
        );
    };
    let multiply_shape = arithmetic_shape(multiply_instruction).map_err(|_| {
        Gfx942ScalarF32PostLlvmMachineBindingErrorV1::AuthenticatedMachineDataflowMismatch
    })?;
    let add_shape = arithmetic_shape(add_instruction).map_err(|_| {
        Gfx942ScalarF32PostLlvmMachineBindingErrorV1::AuthenticatedMachineDataflowMismatch
    })?;
    if multiply_instruction.opcode() != "V_MUL_F32_e32_vi"
        || add_instruction.opcode() != "V_ADD_F32_e32_vi"
        || vgpr_index(multiply_shape.destination) != artifact.product_register()
        || add_shape.sources.first() != Some(&multiply_shape.destination)
        || add_shape
            .sources
            .get(1)
            .map(|register| vgpr_index(*register))
            != Some(artifact.accumulator_register())
        || vgpr_index(add_shape.destination) != artifact.result_register()
    {
        return Err(
            Gfx942ScalarF32PostLlvmMachineBindingErrorV1::AuthenticatedMachineDataflowMismatch,
        );
    }
    Ok(())
}

/// Terminal unavailability after exact HSACO and scalar def-use correspondence succeeds.
#[derive(Debug)]
pub struct Gfx942ScalarF32MachineRefinementAfterScalarCorrespondenceIncompleteV1 {
    readiness: Gfx942ScalarF32PostLlvmMachineReadinessV1,
}

impl Gfx942ScalarF32MachineRefinementAfterScalarCorrespondenceIncompleteV1 {
    /// Returns the remaining typed obligations.
    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        self.readiness.missing_inputs()
    }

    /// Recovers exact compiler and authenticated machine custody.
    pub fn into_readiness(self) -> Gfx942ScalarF32PostLlvmMachineReadinessV1 {
        self.readiness
    }
}

impl fmt::Display for Gfx942ScalarF32MachineRefinementAfterScalarCorrespondenceIncompleteV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "gfx942 scalar-f32 refinement remains unavailable after exact final-HSACO and scalar def-use correspondence; missing typed inputs: ",
        )?;
        for (index, input) in self.missing_inputs().iter().enumerate() {
            if index != 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(input.diagnostic_name())?;
        }
        Ok(())
    }
}

impl Error for Gfx942ScalarF32MachineRefinementAfterScalarCorrespondenceIncompleteV1 {}

/// Keeps terminal equivalence unavailable after the strongest currently checked conjunction.
pub fn require_gfx942_scalar_f32_machine_refinement_after_scalar_correspondence_v1(
    readiness: Gfx942ScalarF32PostLlvmMachineReadinessV1,
) -> Result<
    CheckedGfx942ScalarF32MachineRefinementV1,
    Box<Gfx942ScalarF32MachineRefinementAfterScalarCorrespondenceIncompleteV1>,
> {
    Err(Box::new(
        Gfx942ScalarF32MachineRefinementAfterScalarCorrespondenceIncompleteV1 { readiness },
    ))
}

/// Fail-closed terminal after exact post-LLVM content custody succeeds.
#[derive(Debug)]
pub struct Gfx942ScalarF32MachineRefinementAfterPostLlvmContentIncompleteV1 {
    readiness: Gfx942ScalarF32PostLlvmContentReadinessV1,
}

impl Gfx942ScalarF32MachineRefinementAfterPostLlvmContentIncompleteV1 {
    /// Returns the remaining typed obligations.
    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        self.readiness.missing_inputs()
    }

    /// Recovers the post-LLVM content owner.
    pub fn into_readiness(self) -> Gfx942ScalarF32PostLlvmContentReadinessV1 {
        self.readiness
    }
}

impl fmt::Display for Gfx942ScalarF32MachineRefinementAfterPostLlvmContentIncompleteV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "gfx942 scalar-f32 refinement remains unavailable after exact post-LLVM content custody; missing typed inputs: ",
        )?;
        for (index, input) in self.missing_inputs().iter().enumerate() {
            if index != 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(input.diagnostic_name())?;
        }
        Ok(())
    }
}

impl Error for Gfx942ScalarF32MachineRefinementAfterPostLlvmContentIncompleteV1 {}

/// Keeps final-machine success unreachable after exact stage content is retained.
pub fn require_gfx942_scalar_f32_machine_refinement_after_post_llvm_content_v1(
    readiness: Gfx942ScalarF32PostLlvmContentReadinessV1,
) -> Result<
    CheckedGfx942ScalarF32MachineRefinementV1,
    Box<Gfx942ScalarF32MachineRefinementAfterPostLlvmContentIncompleteV1>,
> {
    Err(Box::new(
        Gfx942ScalarF32MachineRefinementAfterPostLlvmContentIncompleteV1 { readiness },
    ))
}

/// Move-only custody of the strongest result derivable from the current authenticated inputs.
///
/// The owner binds an authenticated final-HSACO analysis to the checked recurrence-step artifact
/// and records the exact proof inputs still required by issue #214. It is intentionally not a
/// machine-refinement receipt and cannot be converted into one.
pub struct Gfx942ScalarF32MachineRefinementReadinessV1 {
    recurrence: AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1,
    missing: Box<[Gfx942ScalarF32MachineRefinementRequiredInputV1]>,
}

impl fmt::Debug for Gfx942ScalarF32MachineRefinementReadinessV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Gfx942ScalarF32MachineRefinementReadinessV1")
            .field(
                "authenticated_execution_identity",
                &self.recurrence.authenticated_execution_identity(),
            )
            .field(
                "recurrence_artifact",
                &self.recurrence.artifact().identity(),
            )
            .field("missing", &self.missing)
            .finish_non_exhaustive()
    }
}

impl Gfx942ScalarF32MachineRefinementReadinessV1 {
    pub const fn recurrence(&self) -> &AuthenticatedGfx942ScalarF32RecurrenceStepAnalysisV1 {
        &self.recurrence
    }

    pub fn authenticated_execution_identity(
        &self,
    ) -> AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1 {
        self.recurrence.authenticated_execution_identity()
    }

    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        &self.missing
    }

    pub const fn establishes_kir_to_final_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_worker_v3_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_or_launch_authority(&self) -> bool {
        false
    }

    pub fn into_authenticated_execution(self) -> AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.recurrence.into_authenticated_execution()
    }
}

/// Audits the current authenticated inputs without upgrading their authority.
///
/// The V1 analyzer record contains exact final-HSACO bytes, a measured decoded trace, CFG facts,
/// static effect sites, and instruction encodings. It does not carry the other proof-bearing
/// artifacts required to establish KIR-to-final-machine refinement, so this function returns all
/// of them as explicit missing typed inputs after first checking the recurrence-step sub-proof.
pub fn audit_authenticated_gfx942_scalar_f32_machine_refinement_inputs_v1(
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
) -> Result<
    Gfx942ScalarF32MachineRefinementReadinessV1,
    Gfx942ScalarF32RecurrenceStepAnalysisFailureV1,
> {
    check_authenticated_gfx942_scalar_f32_recurrence_step_v1(execution, kernel_symbol).map(
        |recurrence| Gfx942ScalarF32MachineRefinementReadinessV1 {
            recurrence,
            missing: GFX942_SCALAR_F32_MACHINE_REFINEMENT_REQUIRED_INPUTS_V1.into(),
        },
    )
}

/// Named fail-closed result of asking the current V1 inputs for a full issue #214 receipt.
#[derive(Debug)]
pub struct Gfx942ScalarF32MachineRefinementIncompleteV1 {
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    recurrence_error: Option<Gfx942ScalarF32RecurrenceStepAnalysisErrorV1>,
    missing: Box<[Gfx942ScalarF32MachineRefinementRequiredInputV1]>,
}

impl Gfx942ScalarF32MachineRefinementIncompleteV1 {
    pub fn missing_inputs(&self) -> &[Gfx942ScalarF32MachineRefinementRequiredInputV1] {
        &self.missing
    }

    pub const fn recurrence_error(&self) -> Option<&Gfx942ScalarF32RecurrenceStepAnalysisErrorV1> {
        self.recurrence_error.as_ref()
    }

    pub fn into_authenticated_execution(self) -> AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.execution
    }
}

impl fmt::Display for Gfx942ScalarF32MachineRefinementIncompleteV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("gfx942 scalar-f32 KIR-to-final-machine refinement is incomplete")?;
        if let Some(error) = &self.recurrence_error {
            write!(formatter, "; recurrence obligation failed: {error}")?;
        }
        formatter.write_str("; missing typed inputs: ")?;
        for (index, input) in self.missing_inputs().iter().enumerate() {
            if index != 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(input.diagnostic_name())?;
        }
        Ok(())
    }
}

impl Error for Gfx942ScalarF32MachineRefinementIncompleteV1 {}

/// Identity of exact canonical bounded-refinement evidence bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942ScalarF32MachineRefinementEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Gfx942ScalarF32MachineRefinementEvidenceIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Move-only successful output of the bounded issue #214 checker.
///
/// There is intentionally no public constructor and no arbitrary-byte decoder. Only the checker
/// may create this owner after validating every typed input and emitting canonical evidence bytes.
/// The protected service can borrow those bytes and their identity, then consume the owner to
/// recover the exact authenticated analyzer execution.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedGfx942ScalarF32MachineRefinementV1;
///
/// fn consume_twice(owner: CheckedGfx942ScalarF32MachineRefinementV1) {
///     let _first = owner.into_parts();
///     let _second = owner.into_parts();
/// }
/// ```
#[must_use = "dropping checked machine-refinement custody abandons the protected-service input"]
pub struct CheckedGfx942ScalarF32MachineRefinementV1 {
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    canonical_evidence: Box<[u8]>,
}

impl fmt::Debug for CheckedGfx942ScalarF32MachineRefinementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedGfx942ScalarF32MachineRefinementV1")
            .field("identity", &self.evidence_identity())
            .field(
                "authenticated_execution_identity",
                &self.execution.identity(),
            )
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942ScalarF32MachineRefinementV1 {
    pub fn canonical_evidence_bytes(&self) -> &[u8] {
        &self.canonical_evidence
    }

    pub fn evidence_identity(&self) -> Gfx942ScalarF32MachineRefinementEvidenceIdentityV1 {
        gfx942_scalar_f32_machine_refinement_evidence_identity_v1(&self.canonical_evidence)
    }

    pub fn authenticated_execution_identity(
        &self,
    ) -> AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1 {
        self.execution.identity()
    }

    pub const fn establishes_bounded_kir_to_final_machine_refinement(&self) -> bool {
        true
    }

    pub const fn grants_worker_v3_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_load_or_launch_authority(&self) -> bool {
        false
    }

    pub fn into_parts(
        self,
    ) -> (
        AuthenticatedPhysicalMachineAnalysisExecutionV1,
        Box<[u8]>,
        Gfx942ScalarF32MachineRefinementEvidenceIdentityV1,
    ) {
        let identity =
            gfx942_scalar_f32_machine_refinement_evidence_identity_v1(&self.canonical_evidence);
        (self.execution, self.canonical_evidence, identity)
    }
}

fn gfx942_scalar_f32_machine_refinement_evidence_identity_v1(
    canonical_evidence: &[u8],
) -> Gfx942ScalarF32MachineRefinementEvidenceIdentityV1 {
    let byte_len = canonical_evidence.len() as u64;
    let mut digest = Sha256::new();
    digest.update(
        (GFX942_SCALAR_F32_MACHINE_REFINEMENT_EVIDENCE_IDENTITY_DOMAIN_V1.len() as u32)
            .to_le_bytes(),
    );
    digest.update(GFX942_SCALAR_F32_MACHINE_REFINEMENT_EVIDENCE_IDENTITY_DOMAIN_V1);
    digest.update(GFX942_SCALAR_F32_MACHINE_REFINEMENT_EVIDENCE_VERSION_V1.to_le_bytes());
    digest.update(byte_len.to_le_bytes());
    digest.update(canonical_evidence);
    Gfx942ScalarF32MachineRefinementEvidenceIdentityV1 {
        sha256: digest.finalize().into(),
        byte_len,
    }
}

/// Fails closed instead of manufacturing a full issue #214 receipt from trace-only evidence.
///
/// This is the production-safe behavior until the typed producers named by
/// [`Gfx942ScalarF32MachineRefinementRequiredInputV1`] are carried by one protected transaction.
pub fn require_authenticated_gfx942_scalar_f32_machine_refinement_v1(
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
) -> Result<
    CheckedGfx942ScalarF32MachineRefinementV1,
    Box<Gfx942ScalarF32MachineRefinementIncompleteV1>,
> {
    match audit_authenticated_gfx942_scalar_f32_machine_refinement_inputs_v1(
        execution,
        kernel_symbol,
    ) {
        Ok(readiness) => Err(Box::new(Gfx942ScalarF32MachineRefinementIncompleteV1 {
            execution: readiness.into_authenticated_execution(),
            recurrence_error: None,
            missing: GFX942_SCALAR_F32_MACHINE_REFINEMENT_REQUIRED_INPUTS_V1.into(),
        })),
        Err(failure) => {
            let (execution, error) = failure.into_parts();
            Err(Box::new(Gfx942ScalarF32MachineRefinementIncompleteV1 {
                execution,
                recurrence_error: Some(error),
                missing: GFX942_SCALAR_F32_MACHINE_REFINEMENT_REQUIRED_INPUTS_V1.into(),
            }))
        }
    }
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
