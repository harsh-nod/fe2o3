//! Bounded planning and result records for an external GPU-kernel verifier.
//!
//! V1 constructs canonical proof requests and executes an evidence recorder
//! through a bounded, shell-free process boundary. It measures and seals
//! recorder, claimed-verifier, and claimed-solver images, but launches only the
//! recorder. On Linux x86_64, V2 separately launches pinned solver and Verus
//! snapshots under a pidfd-owned, two-nonce, process-creation-denied controller
//! protocol with ptrace-unresumable checkpoints. It records normalized executable
//! baselines, anonymous mappings, live executable-page bytes, and runtime/security
//! state. Those checkpoint identities do not imply exclusive measured-image
//! execution between observations. Stock Verus/Z3 integration remains future
//! work, and neither path grants proof or GPU authority. The legacy planning path
//! retains caller-supplied identities for compatibility.

mod alpha_zeta_manifest;
mod alpha_zeta_proof;
mod artifact_record;
mod authenticated_execution;
mod authenticated_proof_binding;
mod authenticated_verus_execution_v2;
mod control_flow_binding;
mod executor;
mod flash_attention_memory_v1;
mod general_gemm_final_admission_v1;
mod general_gemm_numerical_v1;
mod general_gemm_proof_v1;
mod model;
mod moe_expert_compact_plan_v1;
mod moe_routing_memory_v1;
mod monomorphization_dead_binding;
mod multi_kernel_proof;
mod persistent_freshness;
mod plan;
mod proof_capsule;
mod result;
mod row_softmax_certificate;
mod scalar_gemm_hardware_evidence;
mod scalar_gemm_proof;
mod scalar_gemm_v1;
mod static_view_proof;

pub use alpha_zeta_manifest::{
    ALPHA_ZETA_LOCKFILE_PATH_V1, ALPHA_ZETA_PACKAGE_MANIFEST_PATH_V1,
    ALPHA_ZETA_PERMISSION_MODEL_PATH_V1, ALPHA_ZETA_PROOF_HARNESS_PATH_V1,
    ALPHA_ZETA_RUST_MODEL_PATH_V1, ALPHA_ZETA_SHARED_BODY_PATH_V1, ALPHA_ZETA_TOOLCHAIN_PATH_V1,
    ALPHA_ZETA_WORKSPACE_MANIFEST_PATH_V1, AlphaZetaDependencyEdgeV1, AlphaZetaDependencyKindV1,
    AlphaZetaProofSourcesV1, AlphaZetaSourceFileIdentityV1, AlphaZetaSourceRoleV1,
    AlphaZetaTrustedConstructKindV1, AlphaZetaTrustedConstructV1, AlphaZetaTrustedInventoryV1,
    MAX_GFX942_ALPHA_ZETA_DEPENDENCY_EDGES_V1, MAX_GFX942_ALPHA_ZETA_SOURCE_BYTES_V1,
    MAX_GFX942_ALPHA_ZETA_SOURCE_FILES_V1, MAX_GFX942_ALPHA_ZETA_SOURCE_TREE_BYTES_V1,
    MAX_GFX942_ALPHA_ZETA_TRUSTED_CONSTRUCTS_V1,
};
pub use alpha_zeta_proof::{
    AlphaZetaExecutableEvidenceReviewV1, AlphaZetaExecutionReviewV1, AlphaZetaProofErrorV1,
    AlphaZetaReviewLedgerV1, ExecutableEvidenceAlphaZetaExecutionV1,
    GFX942_ALPHA_ZETA_AUTHENTICATED_PROPERTIES_V1, GFX942_ALPHA_ZETA_MODEL_VERSION_V1,
    GFX942_ALPHA_ZETA_PROOF_DOMAIN_V1, GFX942_ALPHA_ZETA_PROOF_VERSION_V1,
    GFX942_ALPHA_ZETA_REQUIRED_PROPERTIES_V1, GFX942_ALPHA_ZETA_REVIEW_DOMAIN_V1,
    GFX942_ALPHA_ZETA_SET_DOMAIN_V1, GFX942_XNACK_MINUS_TARGET_V1, Gfx942AlphaZetaKernelV1,
    Gfx942AlphaZetaProofInputV1, Gfx942XnackMinusTargetIdentityV1,
    InertAlphaZetaExecutableEvidenceSetV1, MAX_GFX942_ALPHA_ZETA_REVIEW_RECORDS_V1,
    ReviewedAlphaZetaExecutionV1, ReviewedAlphaZetaProofSetV1, alpha_zeta_abi_identity_v1,
    alpha_zeta_inert_configuration_v1, alpha_zeta_launch_identity_v1,
    record_descriptive_alpha_zeta_execution_v1, record_inert_alpha_zeta_executable_evidence_v1,
};
pub use artifact_record::{
    ArtifactProofEvidenceV1, ArtifactRecordConversionError, ReviewedInvocationIdentityV1,
    canonical_invocation_digest, convert_to_artifact_proof_record,
};
pub use authenticated_execution::{
    AuthenticatedBindingField, AuthenticatedExecutionError, AuthenticatedRecorderOutputV1,
    AuthenticatedResultError, BoundExecutionPayloadV1, DataOperation, ExecutableMeasurementV1,
    ExecutableOperation, ExecutableRole, MAX_EXECUTABLE_BYTES, MeasuredRecorderInputsV1,
    execute_authenticated_recorder,
};
pub use authenticated_verus_execution_v2::{
    AuthenticatedVerusExecutionDependencyV2, AuthenticatedVerusExecutionErrorKindV2,
    AuthenticatedVerusExecutionErrorV2, AuthenticatedVerusExecutionInputsV2,
    AuthenticatedVerusExecutionPolicyV2, AuthenticatedVerusExecutionReceiptV2,
    AuthenticatedVerusProcessOccurrenceV2, AuthenticatedVerusToolExecutionV2,
    BoundExecutionPayloadV2, ProcessFailureV2, RuntimeClosureMeasurementV2,
    RuntimeExecutableBaselineV2, VerusExecutionRoleV2, execute_authenticated_verus_v2,
};
// Deprecated compatibility exports. Despite their Verus-oriented names, these
// authenticate and execute only the recorder; they do not show that Verus or a
// solver ran.
#[allow(deprecated)]
pub use authenticated_execution::{
    AuthenticatedExecutionProgramsV1, AuthenticatedVerusExecutionEvidenceV1,
    execute_authenticated_verus,
};
pub use authenticated_proof_binding::{
    AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
    AUTHENTICATED_PROOF_EXECUTABLE_BINDING_VERSION_V1, AuthenticatedExecutionFreshnessV1,
    AuthenticatedPayloadIdentityV1, AuthenticatedProofExecutableBindingError,
    AuthenticatedProofExecutableBindingV1, AuthenticatedProofExecutablePolicyV1,
    AuthenticatedProofExecutionIdentityV1,
    PERSISTENT_AUTHENTICATED_PROOF_EXECUTABLE_BINDING_DOMAIN_V1,
    PersistentlyFreshProofExecutableBindingV1, PersistentlyFreshProofExecutableIdentityV1,
    bind_authenticated_proof_executable_persistent_v1, bind_authenticated_proof_executable_v1,
};
pub use control_flow_binding::{
    AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
    AuthenticatedControlFlowExecutableBindingV1, CONTROL_FLOW_BINDING_VERSION_V1,
    CONTROL_FLOW_FUNCTIONAL_SPECIFICATION_DOMAIN_V1, CONTROL_FLOW_REQUEST_BINDING_DOMAIN_V1,
    CONTROL_FLOW_SOURCE_BINDING_DOMAIN_V1, ControlFlowBindingErrorV1, ControlFlowClaimsV1,
    ControlFlowIntegerSwitchCaseClaimV1, ControlFlowIntegerSwitchClaimV1, ControlFlowLoopClaimV1,
    ControlFlowPayloadIdentityV1, ControlFlowProofRequestBindingV1, ControlFlowSourceBindingV1,
    MAX_BOUND_CONTROL_FLOW_LOOPS_V1, MAX_BOUND_CONTROL_FLOW_SWITCHES_V1,
    PERSISTENT_AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
    PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1,
    bind_authenticated_control_flow_executable_v1, bind_control_flow_proof_request_v1,
    bind_persistently_fresh_authenticated_control_flow_executable_v1,
    derive_control_flow_functional_specification_digest_v1, reconcile_control_flow_source_v1,
};
pub use executor::{
    ExecutionError, ExecutionErrorKind, ExecutionLimits, ExecutionPath, ExecutionStage,
    ExecutionSuccess, MAX_CAPTURE_BYTES, OutputStream, ProcessOutput, execute_recorder,
};
pub use flash_attention_memory_v1::*;
pub use general_gemm_final_admission_v1::*;
pub use general_gemm_numerical_v1::*;
pub use general_gemm_proof_v1::*;
pub use model::{
    AxiomPolicy, Configuration, ConfigurationEntry, CorrelationId, Digest, ExecutionTools,
    MAX_CONFIGURATION_ENTRIES, MAX_PROPERTIES, MAX_TEXT_BYTES, MAX_TRUSTED_ITEMS,
    MeasuredToolIdentity, ModelError, ProofOutcome, ProofProperty, ProofRequestV1,
    ProofTargetIdentity, Text, TrustedItem, VerificationModelIdentity,
};
pub use moe_expert_compact_plan_v1::*;
pub use moe_routing_memory_v1::*;
pub use monomorphization_dead_binding::{
    MONOMORPHIZATION_DEAD_BINDING_DOMAIN_V1, MONOMORPHIZATION_DEAD_BINDING_VERSION_V1,
    MonomorphizationDeadBindingErrorV1, MonomorphizationDeadClaimV1,
    MonomorphizationDeadIdentityBindingV1, reconcile_monomorphization_dead_evidence_v1,
};
pub use multi_kernel_proof::{
    KernelProofAdmissionIdentityV1, KernelProofAdmissionRequestV1,
    MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1, MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1,
    MultiKernelProofAdmissionErrorV1, MultiKernelProofAdmissionV1,
    PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_DOMAIN_V1,
    PERSISTENT_MULTI_KERNEL_PROOF_ADMISSION_VERSION_V1,
    PersistentlyFreshKernelProofAdmissionIdentityV1,
    PersistentlyFreshKernelProofAdmissionRequestV1,
    PersistentlyFreshMultiKernelProofAdmissionErrorV1,
    PersistentlyFreshMultiKernelProofAdmissionV1,
};
pub use persistent_freshness::{
    MAX_PERSISTENT_FRESHNESS_ENTRIES_V1, MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1,
    PERSISTENT_FRESHNESS_INTENT_MAGIC_V1, PERSISTENT_FRESHNESS_STATE_MAGIC_V1,
    PERSISTENT_FRESHNESS_VERSION_V1, PersistentFreshnessIdentityFieldV1,
    PersistentFreshnessIdentityV1, PersistentFreshnessIntentInspectionV1,
    PersistentFreshnessLedgerErrorV1, PersistentFreshnessLedgerFileV1,
    PersistentFreshnessLedgerOperationV1, PersistentFreshnessReceiptV1,
    PersistentFreshnessRecordErrorV1, PersistentFreshnessRecoveryV1,
    PersistentFreshnessStateInspectionV1, PersistentProofFreshnessLedgerV1,
    PersistentProofFreshnessTransactionV1, inspect_persistent_freshness_intent_v1,
    inspect_persistent_freshness_state_v1,
};
pub use plan::{
    CommandSpec, InvocationPaths, InvocationPlan, MAX_PATH_BYTES, MAX_TIMEOUT_SECONDS, PlanError,
    VerifierPolicy, build_invocation_plan,
};
pub use proof_capsule::{
    MAX_PROCESS_LOCAL_PROOF_CAPSULE_RECORDS_V1, MAX_PROOF_CAPSULE_BYTES_V1,
    MAX_PROOF_CAPSULE_DEPENDENCIES_V1, MAX_PROOF_CAPSULE_FEATURES_V1,
    MAX_PROOF_CAPSULE_SEALED_RESULT_BYTES_V1, PROOF_CAPSULE_MAGIC_V1, PROOF_CAPSULE_VERSION_V1,
    ProcessLocalProofCapsuleDuplicateDetectorV1, ProofCapsuleBuildErrorV1,
    ProofCapsuleContextErrorV1, ProofCapsuleDecodeErrorV1, ProofCapsuleDependencyV1,
    ProofCapsuleExecutionV1, ProofCapsuleExpectationV1, ProofCapsuleFreshnessExpectationV1,
    ProofCapsuleFreshnessIdentityV1, ProofCapsuleIdentityFieldV1, ProofCapsulePayloadIdentityV1,
    ProofCapsulePolicyV1, ProofCapsuleResultV1, ProofCapsuleTargetV1, ProofCapsuleV1,
};
pub use result::{
    MAX_RESULT_BYTES, ProofResultV1, RecorderTermination, ResultError, parse_recorder_result,
};
pub use row_softmax_certificate::{
    AuthenticatedRowSoftmaxVerificationCertificateIdentityV1,
    AuthenticatedRowSoftmaxVerificationCertificateV1,
    RowSoftmaxVerificationCertificateAuthenticationErrorV1,
    RowSoftmaxVerificationCertificateObservationV1, RowSoftmaxVerificationFileObservationV1,
    authenticate_row_softmax_verification_certificate_v1,
};
pub use scalar_gemm_hardware_evidence::{
    SCALAR_GEMM_COV6_IMPLICIT_KERNARG_BYTES_V1, SCALAR_GEMM_EXPLICIT_KERNARG_BYTES_V1,
    SCALAR_GEMM_HARDWARE_EVIDENCE_DOMAIN_V1, SCALAR_GEMM_HARDWARE_EVIDENCE_VERSION_V1,
    SCALAR_GEMM_HARDWARE_EXPECTATION_DOMAIN_V1, SCALAR_GEMM_HARDWARE_FORMAL_CLAIMS_V1,
    SCALAR_GEMM_HARDWARE_MAX_CASE_NAME_BYTES_V1, SCALAR_GEMM_HARDWARE_MAX_CASES_V1,
    SCALAR_GEMM_KERNARG_ALIGNMENT_V1, SCALAR_GEMM_TOTAL_KERNARG_BYTES_V1,
    SCALAR_GEMM_WAVEFRONT_SIZE_V1, ScalarGemmAdjacentCanaryObservationV1,
    ScalarGemmArtifactObservationV1, ScalarGemmDispatchObservationV1,
    ScalarGemmFrontendObservationV1, ScalarGemmHardwareCaseExpectationV1,
    ScalarGemmHardwareCaseObservationV1, ScalarGemmHardwareEvidenceErrorV1,
    ScalarGemmHardwareEvidenceExpectationV1, ScalarGemmHardwareEvidenceRecorderV1,
    ScalarGemmHardwareFormalClaimV1, ScalarGemmHardwareObservedFactsV1,
    ScalarGemmHsaLoadObservationV1, ScalarGemmInputImmutabilityObservationV1,
    ScalarGemmKernelAdmissionObservationV1, ScalarGemmOutputObservationV1,
    ScalarGemmProtectedHardwareEvidenceV1, ScalarGemmUnloadObservationV1,
    ScalarGemmWorkerExchangeObservationV1,
};
pub use scalar_gemm_proof::{
    MAX_SCALAR_GEMM_PROOF_REVIEWS_V1, MAX_SCALAR_GEMM_PROOF_SOURCE_BYTES_V1,
    ReviewedScalarGemmProofV1, SCALAR_GEMM_PROOF_DOMAIN_V1, SCALAR_GEMM_PROOF_MODEL_VERSION_V1,
    SCALAR_GEMM_PROOF_REQUIRED_PROPERTIES_V1, SCALAR_GEMM_PROOF_REVIEW_DOMAIN_V1,
    SCALAR_GEMM_PROOF_SOURCE_PATH_V1, SCALAR_GEMM_PROOF_TARGET_V1, SCALAR_GEMM_PROOF_VERSION_V1,
    ScalarGemmProofErrorV1, ScalarGemmProofProfileV1, ScalarGemmProofReviewLedgerV1,
    ScalarGemmProofReviewV1, ScalarGemmProofSourceV1, review_scalar_gemm_proof_v1,
};
pub use scalar_gemm_v1::{
    SCALAR_GEMM_COVERAGE_PROFILE_V1, SCALAR_GEMM_F32_NUMERICAL_CONTRACT_V1,
    SCALAR_GEMM_GLOBAL_ADDRESS_SPACE_V1, SCALAR_GEMM_MAX_GRID_THREADS_V1,
    SCALAR_GEMM_MAX_MODEL_WORK_ITEMS_V1, SCALAR_GEMM_ROOT_SYMBOL_V1, SCALAR_GEMM_TARGET_V1,
    SCALAR_GEMM_WORKGROUP_THREADS_V1, ScalarGemmBufferRegionV1, ScalarGemmDotRecurrenceV1,
    ScalarGemmDotStepV1, ScalarGemmF32NumericalContractV1, ScalarGemmHostAdmissionErrorV1,
    ScalarGemmHostAdmissionV1, ScalarGemmHostRequestV1, ScalarGemmInvocationV1,
    ScalarGemmModelErrorV1, ScalarGemmShapeV1, ScalarGemmToolchainV1, admit_scalar_gemm_host_v1,
    evaluate_scalar_gemm_abstract_invocation_v1, scalar_gemm_accesses_are_in_bounds_v1,
    scalar_gemm_canonical_domain_initializes_output_v1, scalar_gemm_f32_oracle_v1,
    scalar_gemm_flattened_index_is_correct_v1, scalar_gemm_output_initialized_by_invocation_v1,
    scalar_gemm_writers_are_injective_v1,
};
pub use static_view_proof::{
    STATIC_VIEW_PROOF_EVIDENCE_DOMAIN_V1, STATIC_VIEW_PROOF_OBLIGATION_DOMAIN_V1,
    STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1, STATIC_VIEW_PROOF_VERSION_V1,
    StaticViewLifetimeEpochClaimV1, StaticViewProofErrorV1, StaticViewProofEvidenceV1,
    StaticViewProofObligationV1, bind_static_view_proof_evidence_v1,
    derive_static_view_functional_specification_digest_v1,
};
