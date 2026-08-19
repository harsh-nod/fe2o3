//! Versioned numerical correspondence for canonical general-GEMM KIR.
//!
//! The package proves only exact BF16 representation widening. Exact-real
//! schedule theorems do not establish KIR sequencing, and IEEE FP32 result
//! rounding plus gfx942 MFMA behavior remain explicit contracts. Finite
//! differential fixtures are regression evidence only and cannot promote a
//! model or contract to a proof.

use core::fmt;
use std::time::{Duration, Instant};

use sha2::{Digest as _, Sha256};

use crate::general_gemm_runtime_closure_v2::{
    GeneralGemmProofSourceV2, GeneralGemmRuntimeClosureErrorV2, GeneralGemmRuntimeProcessOutputV2,
};
use crate::{
    GENERAL_GEMM_NUMERICAL_PROPERTY_THEOREM_MANIFEST_V1,
    GeneralGemmNumericalPropertyManifestErrorV1, GeneralGemmNumericalPropertyTheoremManifestV1,
    reviewed_general_gemm_numerical_property_theorem_manifest_v1,
};
use crate::{
    GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256, GENERAL_GEMM_VERUS_SHA256_V1,
    GENERAL_GEMM_VERUS_VERSION_V1, GeneralGemmEvidenceIdentityV1, GeneralGemmFloatClassV1,
    GeneralGemmKirModelCorrespondenceV1, GeneralGemmNumericalPolicyErrorV1,
    GeneralGemmNumericalStageV1, GeneralGemmProofScheduleV1, GeneralGemmVerusRuntimeClosureLeaseV2,
    MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1, MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1,
    classify_general_gemm_bf16_v1, classify_general_gemm_f32_v1,
    evaluate_general_gemm_numerical_policy_v1, widen_general_gemm_bf16_v1,
};

/// Stable schema for this KIR-bound numerical correspondence package.
pub const GENERAL_GEMM_NUMERICAL_CORRESPONDENCE_SCHEMA_V1: &str =
    "fe2o3.general-gemm.kir-bf16-fp32-mfma-correspondence.v1";
/// Exact LLVM intrinsic covered by the target instruction contract.
pub const GENERAL_GEMM_GFX942_MFMA_INTRINSIC_V1: &str = "llvm.amdgcn.mfma.f32.16x16x16bf16.1k";
/// Exact gfx942 ISA mnemonic expected from later post-link inspection.
pub const GENERAL_GEMM_GFX942_MFMA_MNEMONIC_V1: &str = "v_mfma_f32_16x16x16_bf16";
/// Number of independently classified numerical properties.
pub const GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1: usize = 11;
/// Boundary-biased finite fixtures used only as mutation-sensitive observations.
pub const GENERAL_GEMM_NUMERICAL_DIFFERENTIAL_FIXTURE_COUNT_V1: usize = 11;

const CLAIM_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-correspondence-claim-v1\0";
const EVIDENCE_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-correspondence-evidence-v1\0";
const OUTPUT_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-output-v1\0";
const BF16_EXHAUSTIVE_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-bf16-exhaustive-v1\0";
const FIXTURE_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-fixtures-v1\0";
const SOURCE_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-source-v1\0";
const SOURCE_CLOSURE_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-source-closure-v1\0";
const TOOL_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-numerical-tool-v1\0";
const MFMA_CONTRACT_DOMAIN_V1: &[u8] = b"fe2o3-general-gemm-gfx942-mfma-contract-v1\0";
const MACHINE_REFINEMENT_JOIN_DOMAIN_V1: &[u8] =
    b"fe2o3-general-gemm-future-machine-refinement-join-v1\0";

const NUMERICAL_SOURCE: &[u8] = include_bytes!("../verus/general_gemm_numerical_contract_v1.rs");
const WIDENING_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_numerical_widening_wrong.rs");
const KIR_REFINEMENT_CLAIM_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_numerical_kir_refinement_claim_wrong.rs");
const MFMA_CLAIM_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_numerical_mfma_claim_wrong.rs");
const MFMA_DESCRIPTOR_CLAIM_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_numerical_mfma_descriptor_claim_wrong.rs");
const ORDER_CLAIM_WRONG_SOURCE: &[u8] =
    include_bytes!("../verus/negative/general_gemm_numerical_order_claim_wrong.rs");

const POSITIVE_STDOUT: &[u8] = b"verification results:: 3 verified, 0 errors\n";
const NEGATIVE_STDOUT: &[u8] = b"verification results:: 1 verified, 1 errors\n";
const ORDER_NEGATIVE_STDOUT: &[u8] = b"verification results:: 1 verified, 2 errors\n";
const WIDENING_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
 --> /proc/self/fd/186/negative/general_gemm_numerical_widening_wrong.rs:7:13
   |
 5 | pub proof fn mutated_bf16_widening_drops_the_sign_bit_v1(bits: nat)
   | ------------------------------------------------------------------- at the end of the function body
 6 |     requires bits < 65536,
 7 |     ensures ((bits % 32768) * 65536) / 65536 == bits,
   |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 1 previous error

"#;
const KIR_REFINEMENT_CLAIM_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
 --> /proc/self/fd/186/negative/general_gemm_numerical_kir_refinement_claim_wrong.rs:8:13
  |
7 | pub proof fn mutated_bf16_bit_placement_claims_rust_kir_refinement_v1()
  | ----------------------------------------------------------------------- at the end of the function body
8 |     ensures bf16_rust_kir_refinement_proved_v1(),
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 1 previous error

"#;
const MFMA_CLAIM_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
  --> /proc/self/fd/186/negative/general_gemm_numerical_mfma_claim_wrong.rs:10:13
   |
 9 | pub proof fn mutated_contract_is_upgraded_to_proved_v1()
   | -------------------------------------------------------- at the end of the function body
10 |     ensures gfx942_mfma_numerical_semantics_proved_v1(),
   |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 1 previous error

"#;
const MFMA_DESCRIPTOR_CLAIM_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
 --> /proc/self/fd/186/negative/general_gemm_numerical_mfma_descriptor_claim_wrong.rs:8:13
  |
7 | pub proof fn mutated_constant_shape_claims_mfma_descriptor_projection_v1()
  | -------------------------------------------------------------------------- at the end of the function body
8 |     ensures gfx942_mfma_descriptor_projection_proved_v1(),
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 1 previous error

"#;
const ORDER_CLAIM_WRONG_STDERR: &[u8] = br#"error: postcondition not satisfied
 --> /proc/self/fd/186/negative/general_gemm_numerical_order_claim_wrong.rs:9:13
  |
8 | pub proof fn mutated_model_order_claims_increasing_k_kir_projection_v1()
  | ------------------------------------------------------------------------ at the end of the function body
9 |     ensures increasing_k_kir_projection_proved_v1(),
  |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: postcondition not satisfied
  --> /proc/self/fd/186/negative/general_gemm_numerical_order_claim_wrong.rs:14:13
   |
13 | pub proof fn mutated_model_order_claims_epilogue_kir_projection_v1()
   | -------------------------------------------------------------------- at the end of the function body
14 |     ensures epilogue_kir_projection_proved_v1(),
   |             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ failed this postcondition

error: aborting due to 2 previous errors

"#;

/// Exact numerical property named by the package.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmNumericalPropertyV1 {
    ExactBf16ToF32EncodingWidening = 1,
    Bf16RustKirRefinement = 2,
    Bf16IeeeValueInterpretation = 3,
    Fp32MultiplyRoundToNearestTiesEven = 4,
    Fp32AddRoundToNearestTiesEven = 5,
    IncreasingKSeparateMulAddOrder = 6,
    SeparateAlphaBetaEpilogueOrder = 7,
    Gfx942MfmaShapeAndControls = 8,
    Gfx942MfmaFp32Accumulation = 9,
    ExceptionalAndSubnormalValues = 10,
    EmittedMachineNumericalRefinement = 11,
}

/// Honest maximum authority for one numerical property.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmNumericalCorrespondenceStatusV1 {
    /// A named theorem in the exact retained Verus source discharges the fact.
    Proved = 1,
    /// An exact-real retained model theorem exists, but no KIR/IEEE projection does.
    ModelOnly = 2,
    /// The fact is an explicit typed premise, not a theorem.
    Contracted = 3,
    /// The V1 package rejects or cannot express the behavior.
    Unsupported = 4,
}

/// Why one property has its stated authority level.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmNumericalCorrespondenceBasisV1 {
    VerusBf16EncodingTheorem = 1,
    OpenRustKirRefinement = 2,
    Ieee754Binary32Contract = 3,
    ExactRealScheduleModel = 4,
    Gfx942MfmaInstructionContract = 5,
    FiniteNormalOrZeroPolicy = 6,
    FutureGraphWorkerFinalizerJoin = 7,
}

/// One exact property classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalPropertyFactV1 {
    pub property: GeneralGemmNumericalPropertyV1,
    pub status: GeneralGemmNumericalCorrespondenceStatusV1,
    pub basis: GeneralGemmNumericalCorrespondenceBasisV1,
}

/// Exact target-instruction premise. Numerical semantics remain contracted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmGfx942MfmaContractV1 {
    pub target: &'static str,
    pub llvm_intrinsic: &'static str,
    pub isa_mnemonic: &'static str,
    pub matrix_shape: [u16; 3],
    pub wave_lanes: u16,
    pub accumulators_per_lane: u8,
    pub control_immediates: [u8; 3],
    pub input_element_bits: u8,
    pub accumulator_element_bits: u8,
    pub numerical_status: GeneralGemmNumericalCorrespondenceStatusV1,
}

/// Evidence edges required before emitted-machine numerical refinement exists.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmFutureMachineRefinementInputV1 {
    OwnerBoundPlironGraph = 1,
    DirectLlvmWorkerRequestResponse = 2,
    FinalizerPostLinkIsaResult = 3,
}

/// Explicitly incomplete future graph/worker/finalizer refinement join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmFutureMachineRefinementJoinV1 {
    pub required_inputs: [GeneralGemmFutureMachineRefinementInputV1; 3],
    pub owner_bound_pliron_graph_identity: Option<GeneralGemmEvidenceIdentityV1>,
    pub direct_llvm_worker_request_response_identity: Option<GeneralGemmEvidenceIdentityV1>,
    pub finalizer_post_link_isa_result_identity: Option<GeneralGemmEvidenceIdentityV1>,
    pub status: GeneralGemmNumericalCorrespondenceStatusV1,
}

impl GeneralGemmFutureMachineRefinementJoinV1 {
    pub const fn has_all_required_input_identities(self) -> bool {
        self.owner_bound_pliron_graph_identity.is_some()
            && self.direct_llvm_worker_request_response_identity.is_some()
            && self.finalizer_post_link_isa_result_identity.is_some()
    }
}

/// Public transport claim; checking it grants no compiler or runtime authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalCorrespondenceClaimV1 {
    pub schema_identity: GeneralGemmEvidenceIdentityV1,
    pub kir_correspondence_identity: GeneralGemmEvidenceIdentityV1,
    pub kir_identity: GeneralGemmEvidenceIdentityV1,
    pub proof_request_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule: GeneralGemmProofScheduleV1,
    pub schedule_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_policy_identity: GeneralGemmEvidenceIdentityV1,
    pub target_identity: GeneralGemmEvidenceIdentityV1,
    pub compiler_toolchain_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule_model_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule_theorem_set_identity: GeneralGemmEvidenceIdentityV1,
    pub schedule_source_closure_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_source_identity: GeneralGemmEvidenceIdentityV1,
    pub property_theorem_manifest_identity: GeneralGemmEvidenceIdentityV1,
    pub property_theorem_binding_identities:
        [GeneralGemmEvidenceIdentityV1; GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1],
    pub numerical_theorem_set_identity: GeneralGemmEvidenceIdentityV1,
    pub numerical_source_closure_identity: GeneralGemmEvidenceIdentityV1,
    pub reviewed_verus_tool_identity: GeneralGemmEvidenceIdentityV1,
    pub exhaustive_bf16_identity: GeneralGemmEvidenceIdentityV1,
    pub differential_fixture_identity: GeneralGemmEvidenceIdentityV1,
    pub mfma_contract_identity: GeneralGemmEvidenceIdentityV1,
    pub mfma_contract: GeneralGemmGfx942MfmaContractV1,
    pub machine_refinement_join_identity: GeneralGemmEvidenceIdentityV1,
    pub machine_refinement_join: GeneralGemmFutureMachineRefinementJoinV1,
    pub properties: [GeneralGemmNumericalPropertyFactV1; GENERAL_GEMM_NUMERICAL_PROPERTY_COUNT_V1],
}

/// Claim field rejected by exact recomputation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GeneralGemmNumericalCorrespondenceFieldV1 {
    SchemaIdentity,
    KirCorrespondenceIdentity,
    KirIdentity,
    ProofRequestIdentity,
    Schedule,
    ScheduleIdentity,
    NumericalPolicyIdentity,
    TargetIdentity,
    CompilerToolchainIdentity,
    ScheduleModelIdentity,
    ScheduleTheoremSetIdentity,
    ScheduleSourceClosureIdentity,
    NumericalSourceIdentity,
    PropertyTheoremManifestIdentity,
    PropertyTheoremBindingIdentities,
    NumericalTheoremSetIdentity,
    NumericalSourceClosureIdentity,
    ReviewedVerusToolIdentity,
    ExhaustiveBf16Identity,
    DifferentialFixtureIdentity,
    MfmaContractIdentity,
    MfmaContract,
    MachineRefinementJoinIdentity,
    MachineRefinementJoin,
    Properties,
}

/// Numerical correspondence construction or execution failed.
#[derive(Debug)]
pub enum GeneralGemmNumericalCorrespondenceErrorV1 {
    FieldMismatch(GeneralGemmNumericalCorrespondenceFieldV1),
    NumericalPolicy(GeneralGemmNumericalPolicyErrorV1),
    DifferentialFixtureMismatch,
    InvalidTimeout,
    PositiveProofMismatch,
    NegativeProofMismatch,
    PropertyManifestAuthenticationMismatch,
    PropertyManifest(GeneralGemmNumericalPropertyManifestErrorV1),
    RuntimeClosure(GeneralGemmRuntimeClosureErrorV2),
}

impl fmt::Display for GeneralGemmNumericalCorrespondenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM numerical correspondence failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmNumericalCorrespondenceErrorV1 {}

impl From<GeneralGemmNumericalPolicyErrorV1> for GeneralGemmNumericalCorrespondenceErrorV1 {
    fn from(value: GeneralGemmNumericalPolicyErrorV1) -> Self {
        Self::NumericalPolicy(value)
    }
}

impl From<GeneralGemmNumericalPropertyManifestErrorV1>
    for GeneralGemmNumericalCorrespondenceErrorV1
{
    fn from(value: GeneralGemmNumericalPropertyManifestErrorV1) -> Self {
        Self::PropertyManifest(value)
    }
}

/// Checked KIR-bound package. Private fields and no `Clone` preserve linearity.
///
/// ```compile_fail
/// fn duplicate(value: &fe2o3_verifier::GeneralGemmNumericalCorrespondenceV1) {
///     let _copy = (*value).clone();
/// }
/// ```
///
/// ```compile_fail
/// fn forge() -> fe2o3_verifier::GeneralGemmNumericalCorrespondenceV1 {
///     fe2o3_verifier::GeneralGemmNumericalCorrespondenceV1 {
///         kir_correspondence: todo!(),
///         claim: todo!(),
///         identity: todo!(),
///     }
/// }
/// ```
#[derive(Debug)]
#[must_use = "numerical correspondence remains non-authoritative until machine refinement"]
pub struct GeneralGemmNumericalCorrespondenceV1 {
    kir_correspondence: GeneralGemmKirModelCorrespondenceV1,
    property_manifest: GeneralGemmNumericalPropertyTheoremManifestV1,
    claim: GeneralGemmNumericalCorrespondenceClaimV1,
    identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmNumericalCorrespondenceV1 {
    pub const fn kir_correspondence(&self) -> &GeneralGemmKirModelCorrespondenceV1 {
        &self.kir_correspondence
    }

    pub const fn claim(&self) -> GeneralGemmNumericalCorrespondenceClaimV1 {
        self.claim
    }

    pub const fn property_manifest(&self) -> &GeneralGemmNumericalPropertyTheoremManifestV1 {
        &self.property_manifest
    }

    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Exact output identity from one retained proof process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmNumericalProofOutputV1 {
    identity: GeneralGemmEvidenceIdentityV1,
    stdout_bytes: u64,
    stderr_bytes: u64,
}

impl GeneralGemmNumericalProofOutputV1 {
    pub const fn identity(self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    pub const fn stdout_bytes(self) -> u64 {
        self.stdout_bytes
    }

    pub const fn stderr_bytes(self) -> u64 {
        self.stderr_bytes
    }
}

/// Retained-root execution evidence. It remains non-`Clone` and non-authoritative.
#[derive(Debug)]
#[must_use = "numerical proof execution still requires emitted-machine refinement"]
pub struct ExecutedGeneralGemmNumericalCorrespondenceV1 {
    correspondence: GeneralGemmNumericalCorrespondenceV1,
    runtime_closure_identity: GeneralGemmEvidenceIdentityV1,
    positive_output: GeneralGemmNumericalProofOutputV1,
    negative_outputs: [GeneralGemmNumericalProofOutputV1; 5],
    identity: GeneralGemmEvidenceIdentityV1,
}

impl ExecutedGeneralGemmNumericalCorrespondenceV1 {
    pub const fn correspondence(&self) -> &GeneralGemmNumericalCorrespondenceV1 {
        &self.correspondence
    }

    pub const fn runtime_closure_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.runtime_closure_identity
    }

    pub const fn positive_output(&self) -> GeneralGemmNumericalProofOutputV1 {
        self.positive_output
    }

    pub const fn negative_outputs(&self) -> &[GeneralGemmNumericalProofOutputV1; 5] {
        &self.negative_outputs
    }

    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }
}

/// Derives every numerical identity and authority classification from checked KIR correspondence.
pub fn derive_general_gemm_numerical_correspondence_claim_v1(
    correspondence: &GeneralGemmKirModelCorrespondenceV1,
) -> Result<GeneralGemmNumericalCorrespondenceClaimV1, GeneralGemmNumericalCorrespondenceErrorV1> {
    let kir_claim = correspondence.claim();
    let request = correspondence.proof_request();
    let property_manifest = reviewed_general_gemm_numerical_property_theorem_manifest_v1()?;
    let bindings = property_manifest.bindings();
    let properties = bindings.map(|binding| binding.fact());
    let mfma_contract = canonical_mfma_contract();
    let machine_refinement_join = canonical_machine_refinement_join();
    Ok(GeneralGemmNumericalCorrespondenceClaimV1 {
        schema_identity: hash_parts(
            SOURCE_DOMAIN_V1,
            &[GENERAL_GEMM_NUMERICAL_CORRESPONDENCE_SCHEMA_V1.as_bytes()],
        ),
        kir_correspondence_identity: correspondence.identity(),
        kir_identity: kir_claim.kir_identity,
        proof_request_identity: kir_claim.proof_request_identity,
        schedule: request.schedule(),
        schedule_identity: request.schedule_identity(),
        numerical_policy_identity: request.numerical_policy_identity(),
        target_identity: request.target_identity(),
        compiler_toolchain_identity: request.toolchain_identity(),
        schedule_model_identity: kir_claim.model_identity,
        schedule_theorem_set_identity: kir_claim.theorem_set_identity,
        schedule_source_closure_identity: kir_claim.source_closure_identity,
        numerical_source_identity: numerical_source_identity(),
        property_theorem_manifest_identity: property_manifest.identity(),
        property_theorem_binding_identities: bindings
            .map(|binding| binding.statement_source_identity()),
        numerical_theorem_set_identity: property_manifest.theorem_set_identity(),
        numerical_source_closure_identity: numerical_source_closure_identity(),
        reviewed_verus_tool_identity: reviewed_tool_identity(),
        exhaustive_bf16_identity: exhaustive_bf16_identity()?,
        differential_fixture_identity: differential_fixture_identity()?,
        mfma_contract_identity: mfma_contract_identity(mfma_contract),
        mfma_contract,
        machine_refinement_join_identity: machine_refinement_join_identity(machine_refinement_join),
        machine_refinement_join,
        properties,
    })
}

/// Checks every transported field and takes ownership of the KIR correspondence.
pub fn check_general_gemm_numerical_correspondence_v1(
    correspondence: GeneralGemmKirModelCorrespondenceV1,
    claimed: GeneralGemmNumericalCorrespondenceClaimV1,
) -> Result<GeneralGemmNumericalCorrespondenceV1, GeneralGemmNumericalCorrespondenceErrorV1> {
    let expected = derive_general_gemm_numerical_correspondence_claim_v1(&correspondence)?;
    compare_claims(expected, claimed)?;
    let property_manifest = reviewed_general_gemm_numerical_property_theorem_manifest_v1()?;
    Ok(GeneralGemmNumericalCorrespondenceV1 {
        kir_correspondence: correspondence,
        property_manifest,
        claim: expected,
        identity: claim_identity(expected),
    })
}

/// Executes only the exact retained positive and expected-negative numerical sources.
pub fn execute_general_gemm_numerical_correspondence_with_runtime_closure_v1(
    correspondence: GeneralGemmNumericalCorrespondenceV1,
    runtime: &GeneralGemmVerusRuntimeClosureLeaseV2,
    timeout_seconds: u32,
) -> Result<ExecutedGeneralGemmNumericalCorrespondenceV1, GeneralGemmNumericalCorrespondenceErrorV1>
{
    if timeout_seconds == 0 || timeout_seconds > MAX_GENERAL_GEMM_PROOF_TIMEOUT_SECONDS_V1 {
        return Err(GeneralGemmNumericalCorrespondenceErrorV1::InvalidTimeout);
    }
    runtime
        .revalidate()
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    authenticate_property_manifest(&correspondence)?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(u64::from(timeout_seconds)))
        .ok_or(GeneralGemmNumericalCorrespondenceErrorV1::InvalidTimeout)?;
    let positive = runtime
        .execute_rust_verify(
            GeneralGemmProofSourceV2::NumericalContract,
            deadline,
            MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1,
        )
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    require_output(&positive, Some(0), POSITIVE_STDOUT, &[], true)?;
    let widening = runtime
        .execute_rust_verify(
            GeneralGemmProofSourceV2::NumericalWideningWrong,
            deadline,
            MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1,
        )
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    require_output(
        &widening,
        Some(1),
        NEGATIVE_STDOUT,
        WIDENING_WRONG_STDERR,
        false,
    )?;
    let kir_refinement = runtime
        .execute_rust_verify(
            GeneralGemmProofSourceV2::NumericalKirRefinementClaimWrong,
            deadline,
            MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1,
        )
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    require_output(
        &kir_refinement,
        Some(1),
        NEGATIVE_STDOUT,
        KIR_REFINEMENT_CLAIM_WRONG_STDERR,
        false,
    )?;
    let order = runtime
        .execute_rust_verify(
            GeneralGemmProofSourceV2::NumericalOrderClaimWrong,
            deadline,
            MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1,
        )
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    require_output(
        &order,
        Some(1),
        ORDER_NEGATIVE_STDOUT,
        ORDER_CLAIM_WRONG_STDERR,
        false,
    )?;
    let mfma_descriptor = runtime
        .execute_rust_verify(
            GeneralGemmProofSourceV2::NumericalMfmaDescriptorClaimWrong,
            deadline,
            MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1,
        )
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    require_output(
        &mfma_descriptor,
        Some(1),
        NEGATIVE_STDOUT,
        MFMA_DESCRIPTOR_CLAIM_WRONG_STDERR,
        false,
    )?;
    let mfma = runtime
        .execute_rust_verify(
            GeneralGemmProofSourceV2::NumericalMfmaClaimWrong,
            deadline,
            MAX_GENERAL_GEMM_PROOF_OUTPUT_BYTES_V1,
        )
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    require_output(
        &mfma,
        Some(1),
        NEGATIVE_STDOUT,
        MFMA_CLAIM_WRONG_STDERR,
        false,
    )?;
    runtime
        .revalidate()
        .map_err(GeneralGemmNumericalCorrespondenceErrorV1::RuntimeClosure)?;
    authenticate_property_manifest(&correspondence)?;

    let runtime_closure_identity =
        GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(runtime.identity().as_bytes());
    let positive_output = output_identity(&positive);
    let negative_outputs = [
        output_identity(&widening),
        output_identity(&kir_refinement),
        output_identity(&order),
        output_identity(&mfma_descriptor),
        output_identity(&mfma),
    ];
    let mut hasher = Sha256::new();
    hasher.update(EVIDENCE_DOMAIN_V1);
    hasher.update(correspondence.identity().as_bytes());
    hasher.update(runtime_closure_identity.as_bytes());
    hasher.update(positive_output.identity.as_bytes());
    for output in negative_outputs {
        hasher.update(output.identity.as_bytes());
    }
    let identity = GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into());
    Ok(ExecutedGeneralGemmNumericalCorrespondenceV1 {
        correspondence,
        runtime_closure_identity,
        positive_output,
        negative_outputs,
        identity,
    })
}

const fn canonical_mfma_contract() -> GeneralGemmGfx942MfmaContractV1 {
    GeneralGemmGfx942MfmaContractV1 {
        target: "gfx942:xnack-:wave64",
        llvm_intrinsic: GENERAL_GEMM_GFX942_MFMA_INTRINSIC_V1,
        isa_mnemonic: GENERAL_GEMM_GFX942_MFMA_MNEMONIC_V1,
        matrix_shape: [16, 16, 16],
        wave_lanes: 64,
        accumulators_per_lane: 4,
        control_immediates: [0, 0, 0],
        input_element_bits: 16,
        accumulator_element_bits: 32,
        numerical_status: GeneralGemmNumericalCorrespondenceStatusV1::Contracted,
    }
}

const fn canonical_machine_refinement_join() -> GeneralGemmFutureMachineRefinementJoinV1 {
    GeneralGemmFutureMachineRefinementJoinV1 {
        required_inputs: [
            GeneralGemmFutureMachineRefinementInputV1::OwnerBoundPlironGraph,
            GeneralGemmFutureMachineRefinementInputV1::DirectLlvmWorkerRequestResponse,
            GeneralGemmFutureMachineRefinementInputV1::FinalizerPostLinkIsaResult,
        ],
        owner_bound_pliron_graph_identity: None,
        direct_llvm_worker_request_response_identity: None,
        finalizer_post_link_isa_result_identity: None,
        status: GeneralGemmNumericalCorrespondenceStatusV1::Unsupported,
    }
}

fn authenticate_property_manifest(
    correspondence: &GeneralGemmNumericalCorrespondenceV1,
) -> Result<(), GeneralGemmNumericalCorrespondenceErrorV1> {
    let manifest = reviewed_general_gemm_numerical_property_theorem_manifest_v1()?;
    let bindings = manifest.bindings();
    let claim = correspondence.claim;
    if &manifest != correspondence.property_manifest()
        || manifest.identity() != claim.property_theorem_manifest_identity
        || manifest.theorem_set_identity() != claim.numerical_theorem_set_identity
        || bindings.map(|binding| binding.fact()) != claim.properties
        || bindings.map(|binding| binding.statement_source_identity())
            != claim.property_theorem_binding_identities
    {
        return Err(
            GeneralGemmNumericalCorrespondenceErrorV1::PropertyManifestAuthenticationMismatch,
        );
    }
    Ok(())
}

fn exhaustive_bf16_identity()
-> Result<GeneralGemmEvidenceIdentityV1, GeneralGemmNumericalCorrespondenceErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(BF16_EXHAUSTIVE_DOMAIN_V1);
    for bits in 0..=u16::MAX {
        let widened = widen_general_gemm_bf16_v1(bits).to_bits();
        if widened != u32::from(bits) << 16
            || classify_general_gemm_bf16_v1(bits) != classify_general_gemm_f32_v1(widened)
        {
            return Err(GeneralGemmNumericalCorrespondenceErrorV1::DifferentialFixtureMismatch);
        }
        hasher.update(bits.to_le_bytes());
        hasher.update(widened.to_le_bytes());
    }
    Ok(GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(
        hasher.finalize().into(),
    ))
}

struct FixtureV1 {
    a: &'static [u16],
    b: &'static [u16],
    c_bits: u32,
    alpha_bits: u32,
    beta_bits: u32,
    expected: FixtureExpectationV1,
}

enum FixtureExpectationV1 {
    Output {
        accumulator_bits: u32,
        output_bits: u32,
    },
    Rejected {
        stage: GeneralGemmNumericalStageV1,
        class: GeneralGemmFloatClassV1,
    },
}

const FIXTURES: &[FixtureV1] = &[
    FixtureV1 {
        a: &[0x3f80],
        b: &[0x3f80],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Output {
            accumulator_bits: 0x3f80_0000,
            output_bits: 0x3f80_0000,
        },
    },
    FixtureV1 {
        a: &[0x3f80, 0x4040],
        b: &[0x4000, 0x4080],
        c_bits: 0x4100_0000,
        alpha_bits: 0x3f00_0000,
        beta_bits: 0x3e80_0000,
        expected: FixtureExpectationV1::Output {
            accumulator_bits: 0x4160_0000,
            output_bits: 0x4110_0000,
        },
    },
    FixtureV1 {
        a: &[0x4f80, 0xcf80, 0x3f80],
        b: &[0x3f80; 3],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Output {
            accumulator_bits: 0x3f80_0000,
            output_bits: 0x3f80_0000,
        },
    },
    // Two half-ULP products expose reassociation: increasing-K FP32 rounds each
    // addition back to 1.0; grouping them first produces 1.0 + 2^-23.
    FixtureV1 {
        a: &[0x3f80; 3],
        b: &[0x3f80, 0x3380, 0x3380],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Output {
            accumulator_bits: 0x3f80_0000,
            output_bits: 0x3f80_0000,
        },
    },
    FixtureV1 {
        a: &[0x0080],
        b: &[0x3f80],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Output {
            accumulator_bits: 0x0080_0000,
            output_bits: 0x0080_0000,
        },
    },
    FixtureV1 {
        a: &[0x7f7f],
        b: &[0x3f00],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Output {
            accumulator_bits: 0x7eff_0000,
            output_bits: 0x7eff_0000,
        },
    },
    FixtureV1 {
        a: &[0x8000],
        b: &[0x3f80],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Output {
            accumulator_bits: 0,
            output_bits: 0,
        },
    },
    FixtureV1 {
        a: &[0x0001],
        b: &[0x3f80],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Rejected {
            stage: GeneralGemmNumericalStageV1::AInput,
            class: GeneralGemmFloatClassV1::Subnormal,
        },
    },
    FixtureV1 {
        a: &[0x7f80],
        b: &[0x3f80],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Rejected {
            stage: GeneralGemmNumericalStageV1::AInput,
            class: GeneralGemmFloatClassV1::Infinity,
        },
    },
    FixtureV1 {
        a: &[0x7fc1],
        b: &[0x3f80],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Rejected {
            stage: GeneralGemmNumericalStageV1::AInput,
            class: GeneralGemmFloatClassV1::NaN,
        },
    },
    FixtureV1 {
        a: &[0x7f7f],
        b: &[0x4000],
        c_bits: 0,
        alpha_bits: 0x3f80_0000,
        beta_bits: 0,
        expected: FixtureExpectationV1::Rejected {
            stage: GeneralGemmNumericalStageV1::Product,
            class: GeneralGemmFloatClassV1::Infinity,
        },
    },
];

fn differential_fixture_identity()
-> Result<GeneralGemmEvidenceIdentityV1, GeneralGemmNumericalCorrespondenceErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(FIXTURE_DOMAIN_V1);
    if FIXTURES.len() != GENERAL_GEMM_NUMERICAL_DIFFERENTIAL_FIXTURE_COUNT_V1 {
        return Err(GeneralGemmNumericalCorrespondenceErrorV1::DifferentialFixtureMismatch);
    }
    for fixture in FIXTURES {
        hasher.update((fixture.a.len() as u32).to_le_bytes());
        for bits in fixture.a {
            hasher.update(bits.to_le_bytes());
        }
        for bits in fixture.b {
            hasher.update(bits.to_le_bytes());
        }
        hasher.update(fixture.c_bits.to_le_bytes());
        hasher.update(fixture.alpha_bits.to_le_bytes());
        hasher.update(fixture.beta_bits.to_le_bytes());
        let observed = evaluate_general_gemm_numerical_policy_v1(
            fixture.a,
            fixture.b,
            f32::from_bits(fixture.c_bits),
            f32::from_bits(fixture.alpha_bits),
            f32::from_bits(fixture.beta_bits),
        );
        match (&fixture.expected, observed) {
            (
                FixtureExpectationV1::Output {
                    accumulator_bits,
                    output_bits,
                },
                Ok(value),
            ) if value.accumulator_bits() == *accumulator_bits
                && value.output_bits() == *output_bits =>
            {
                hasher.update([1]);
                hasher.update(accumulator_bits.to_le_bytes());
                hasher.update(output_bits.to_le_bytes());
            }
            (
                FixtureExpectationV1::Rejected { stage, class },
                Err(GeneralGemmNumericalPolicyErrorV1::UnsupportedValue {
                    stage: actual_stage,
                    class: actual_class,
                    ..
                }),
            ) if *stage == actual_stage && *class == actual_class => {
                hasher.update([2, *stage as u8, *class as u8]);
            }
            _ => {
                return Err(GeneralGemmNumericalCorrespondenceErrorV1::DifferentialFixtureMismatch);
            }
        }
    }
    Ok(GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(
        hasher.finalize().into(),
    ))
}

fn numerical_source_identity() -> GeneralGemmEvidenceIdentityV1 {
    hash_parts(
        SOURCE_DOMAIN_V1,
        &[b"general_gemm_numerical_contract_v1.rs", NUMERICAL_SOURCE],
    )
}

fn numerical_source_closure_identity() -> GeneralGemmEvidenceIdentityV1 {
    hash_parts(
        SOURCE_CLOSURE_DOMAIN_V1,
        &[
            b"general_gemm_numerical_contract_v1.rs",
            NUMERICAL_SOURCE,
            b"negative/general_gemm_numerical_kir_refinement_claim_wrong.rs",
            KIR_REFINEMENT_CLAIM_WRONG_SOURCE,
            b"negative/general_gemm_numerical_mfma_claim_wrong.rs",
            MFMA_CLAIM_WRONG_SOURCE,
            b"negative/general_gemm_numerical_mfma_descriptor_claim_wrong.rs",
            MFMA_DESCRIPTOR_CLAIM_WRONG_SOURCE,
            b"negative/general_gemm_numerical_order_claim_wrong.rs",
            ORDER_CLAIM_WRONG_SOURCE,
            b"negative/general_gemm_numerical_widening_wrong.rs",
            WIDENING_WRONG_SOURCE,
            b"pins/GENERAL_GEMM_NUMERICAL_PROPERTIES_V1.manifest",
            GENERAL_GEMM_NUMERICAL_PROPERTY_THEOREM_MANIFEST_V1,
        ],
    )
}

fn reviewed_tool_identity() -> GeneralGemmEvidenceIdentityV1 {
    hash_parts(
        TOOL_DOMAIN_V1,
        &[
            GENERAL_GEMM_VERUS_VERSION_V1.as_bytes(),
            &GENERAL_GEMM_VERUS_SHA256_V1,
            &GENERAL_GEMM_RUNTIME_CLOSURE_V2_MANIFEST_SHA256,
        ],
    )
}

fn mfma_contract_identity(
    contract: GeneralGemmGfx942MfmaContractV1,
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(MFMA_CONTRACT_DOMAIN_V1);
    put_blob(&mut hasher, contract.target.as_bytes());
    put_blob(&mut hasher, contract.llvm_intrinsic.as_bytes());
    put_blob(&mut hasher, contract.isa_mnemonic.as_bytes());
    for value in contract.matrix_shape {
        hasher.update(value.to_le_bytes());
    }
    hasher.update(contract.wave_lanes.to_le_bytes());
    hasher.update([contract.accumulators_per_lane]);
    hasher.update(contract.control_immediates);
    hasher.update([
        contract.input_element_bits,
        contract.accumulator_element_bits,
        contract.numerical_status as u8,
    ]);
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

fn machine_refinement_join_identity(
    join: GeneralGemmFutureMachineRefinementJoinV1,
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(MACHINE_REFINEMENT_JOIN_DOMAIN_V1);
    for required in join.required_inputs {
        hasher.update([required as u8]);
    }
    for identity in [
        join.owner_bound_pliron_graph_identity,
        join.direct_llvm_worker_request_response_identity,
        join.finalizer_post_link_isa_result_identity,
    ] {
        match identity {
            Some(identity) => {
                hasher.update([1]);
                hasher.update(identity.as_bytes());
            }
            None => hasher.update([0]),
        }
    }
    hasher.update([join.status as u8]);
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

fn claim_identity(
    claim: GeneralGemmNumericalCorrespondenceClaimV1,
) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(CLAIM_DOMAIN_V1);
    for identity in [
        claim.schema_identity,
        claim.kir_correspondence_identity,
        claim.kir_identity,
        claim.proof_request_identity,
        claim.schedule_identity,
        claim.numerical_policy_identity,
        claim.target_identity,
        claim.compiler_toolchain_identity,
        claim.schedule_model_identity,
        claim.schedule_theorem_set_identity,
        claim.schedule_source_closure_identity,
        claim.numerical_source_identity,
        claim.property_theorem_manifest_identity,
        claim.numerical_theorem_set_identity,
        claim.numerical_source_closure_identity,
        claim.reviewed_verus_tool_identity,
        claim.exhaustive_bf16_identity,
        claim.differential_fixture_identity,
        claim.mfma_contract_identity,
        claim.machine_refinement_join_identity,
    ] {
        hasher.update(identity.as_bytes());
    }
    for identity in claim.property_theorem_binding_identities {
        hasher.update(identity.as_bytes());
    }
    hasher.update([claim.schedule as u8]);
    for fact in claim.properties {
        hasher.update([fact.property as u8, fact.status as u8, fact.basis as u8]);
    }
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

macro_rules! require_field {
    ($expected:ident, $claimed:ident, $field:ident, $name:ident) => {
        if $expected.$field != $claimed.$field {
            return Err(GeneralGemmNumericalCorrespondenceErrorV1::FieldMismatch(
                GeneralGemmNumericalCorrespondenceFieldV1::$name,
            ));
        }
    };
}

fn compare_claims(
    expected: GeneralGemmNumericalCorrespondenceClaimV1,
    claimed: GeneralGemmNumericalCorrespondenceClaimV1,
) -> Result<(), GeneralGemmNumericalCorrespondenceErrorV1> {
    require_field!(expected, claimed, schema_identity, SchemaIdentity);
    require_field!(
        expected,
        claimed,
        kir_correspondence_identity,
        KirCorrespondenceIdentity
    );
    require_field!(expected, claimed, kir_identity, KirIdentity);
    require_field!(
        expected,
        claimed,
        proof_request_identity,
        ProofRequestIdentity
    );
    require_field!(expected, claimed, schedule, Schedule);
    require_field!(expected, claimed, schedule_identity, ScheduleIdentity);
    require_field!(
        expected,
        claimed,
        numerical_policy_identity,
        NumericalPolicyIdentity
    );
    require_field!(expected, claimed, target_identity, TargetIdentity);
    require_field!(
        expected,
        claimed,
        compiler_toolchain_identity,
        CompilerToolchainIdentity
    );
    require_field!(
        expected,
        claimed,
        schedule_model_identity,
        ScheduleModelIdentity
    );
    require_field!(
        expected,
        claimed,
        schedule_theorem_set_identity,
        ScheduleTheoremSetIdentity
    );
    require_field!(
        expected,
        claimed,
        schedule_source_closure_identity,
        ScheduleSourceClosureIdentity
    );
    require_field!(
        expected,
        claimed,
        numerical_source_identity,
        NumericalSourceIdentity
    );
    require_field!(
        expected,
        claimed,
        property_theorem_manifest_identity,
        PropertyTheoremManifestIdentity
    );
    require_field!(
        expected,
        claimed,
        property_theorem_binding_identities,
        PropertyTheoremBindingIdentities
    );
    require_field!(
        expected,
        claimed,
        numerical_theorem_set_identity,
        NumericalTheoremSetIdentity
    );
    require_field!(
        expected,
        claimed,
        numerical_source_closure_identity,
        NumericalSourceClosureIdentity
    );
    require_field!(
        expected,
        claimed,
        reviewed_verus_tool_identity,
        ReviewedVerusToolIdentity
    );
    require_field!(
        expected,
        claimed,
        exhaustive_bf16_identity,
        ExhaustiveBf16Identity
    );
    require_field!(
        expected,
        claimed,
        differential_fixture_identity,
        DifferentialFixtureIdentity
    );
    require_field!(
        expected,
        claimed,
        mfma_contract_identity,
        MfmaContractIdentity
    );
    require_field!(expected, claimed, mfma_contract, MfmaContract);
    require_field!(
        expected,
        claimed,
        machine_refinement_join_identity,
        MachineRefinementJoinIdentity
    );
    require_field!(
        expected,
        claimed,
        machine_refinement_join,
        MachineRefinementJoin
    );
    require_field!(expected, claimed, properties, Properties);
    Ok(())
}

fn require_output(
    observed: &GeneralGemmRuntimeProcessOutputV2,
    exit_code: Option<i32>,
    stdout: &[u8],
    stderr: &[u8],
    positive: bool,
) -> Result<(), GeneralGemmNumericalCorrespondenceErrorV1> {
    if observed.exit_code != exit_code
        || observed.signal.is_some()
        || observed.stdout != stdout
        || observed.stderr != stderr
    {
        return Err(if positive {
            GeneralGemmNumericalCorrespondenceErrorV1::PositiveProofMismatch
        } else {
            GeneralGemmNumericalCorrespondenceErrorV1::NegativeProofMismatch
        });
    }
    Ok(())
}

fn output_identity(
    observed: &GeneralGemmRuntimeProcessOutputV2,
) -> GeneralGemmNumericalProofOutputV1 {
    let mut hasher = Sha256::new();
    hasher.update(OUTPUT_DOMAIN_V1);
    hasher.update(observed.exit_code.unwrap_or(-1).to_le_bytes());
    hasher.update(observed.signal.unwrap_or(0).to_le_bytes());
    put_blob(&mut hasher, &observed.stdout);
    put_blob(&mut hasher, &observed.stderr);
    GeneralGemmNumericalProofOutputV1 {
        identity: GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into()),
        stdout_bytes: observed.stdout.len() as u64,
        stderr_bytes: observed.stderr.len() as u64,
    }
}

fn hash_parts(domain: &[u8], parts: &[&[u8]]) -> GeneralGemmEvidenceIdentityV1 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        put_blob(&mut hasher, part);
    }
    GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into())
}

fn put_blob(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_output_parsers_reject_substitution() {
        let positive = GeneralGemmRuntimeProcessOutputV2 {
            exit_code: Some(0),
            signal: None,
            stdout: POSITIVE_STDOUT.to_vec(),
            stderr: vec![],
        };
        require_output(&positive, Some(0), POSITIVE_STDOUT, &[], true).unwrap();
        let mut changed = GeneralGemmRuntimeProcessOutputV2 {
            stdout: positive.stdout.clone(),
            ..positive
        };
        changed.stdout.push(b' ');
        assert!(matches!(
            require_output(&changed, Some(0), POSITIVE_STDOUT, &[], true),
            Err(GeneralGemmNumericalCorrespondenceErrorV1::PositiveProofMismatch)
        ));

        for stderr in [
            WIDENING_WRONG_STDERR,
            KIR_REFINEMENT_CLAIM_WRONG_STDERR,
            MFMA_DESCRIPTOR_CLAIM_WRONG_STDERR,
            MFMA_CLAIM_WRONG_STDERR,
        ] {
            let negative = GeneralGemmRuntimeProcessOutputV2 {
                exit_code: Some(1),
                signal: None,
                stdout: NEGATIVE_STDOUT.to_vec(),
                stderr: stderr.to_vec(),
            };
            require_output(&negative, Some(1), NEGATIVE_STDOUT, stderr, false).unwrap();
        }
        let order = GeneralGemmRuntimeProcessOutputV2 {
            exit_code: Some(1),
            signal: None,
            stdout: ORDER_NEGATIVE_STDOUT.to_vec(),
            stderr: ORDER_CLAIM_WRONG_STDERR.to_vec(),
        };
        require_output(
            &order,
            Some(1),
            ORDER_NEGATIVE_STDOUT,
            ORDER_CLAIM_WRONG_STDERR,
            false,
        )
        .unwrap();
    }

    #[test]
    fn finite_fixtures_never_change_authority() {
        assert_ne!(
            differential_fixture_identity().unwrap().as_bytes(),
            &[0; 32]
        );
        let properties = reviewed_general_gemm_numerical_property_theorem_manifest_v1()
            .unwrap()
            .bindings()
            .map(|binding| binding.fact());
        assert_eq!(
            properties[5].status,
            GeneralGemmNumericalCorrespondenceStatusV1::ModelOnly
        );
        assert_eq!(
            properties[8].status,
            GeneralGemmNumericalCorrespondenceStatusV1::Contracted
        );
        assert_eq!(
            properties[10].status,
            GeneralGemmNumericalCorrespondenceStatusV1::Unsupported
        );
    }
}
