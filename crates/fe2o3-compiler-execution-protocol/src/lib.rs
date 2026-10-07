#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

/// Fixed child descriptor reserved for the compiler-execution service peer.
pub const COMPILER_EXECUTION_SERVICE_CHILD_FD_V1: i32 = 195;

mod attestation;
mod attestation_challenge_adapter;
mod attestation_challenge_v2;
mod attestation_challenge_v3;
mod attestation_receipt_adapter;
mod attestation_receipt_codec;
mod attestation_receipt_v2;
mod attestation_receipt_v3;
mod attestation_request_adapter;
mod attestation_request_codec;
mod attestation_request_v2;
mod attestation_request_v3;
mod attestation_resources;
mod client_profile;
mod client_profile_adapter;
mod client_profile_codec;
mod client_profile_v2;
mod client_profile_v3;
mod current_record_native_adapter;
mod current_record_verification;
mod external_anchor_deployment;
mod external_anchor_deployment_adapter;
mod external_anchor_deployment_codec;
mod external_anchor_deployment_v2;
mod external_anchor_deployment_v3;
mod external_anchor_provisioning;
mod external_anchor_provisioning_adapter;
mod external_anchor_provisioning_codec;
mod external_anchor_provisioning_v2;
mod external_anchor_provisioning_v3;
mod external_anchor_service;
mod external_anchor_transaction;
mod external_anchor_transaction_adapter;
mod external_anchor_transaction_v2;
mod external_anchor_transaction_v3;
pub use external_anchor_transaction_v2::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2,
    CompilerExecutionExternalAnchorTransactionIdentityV2,
    CompilerExecutionExternalAnchorTransactionV2, CompilerExecutionNativeJournalErrorV2,
};
pub use external_anchor_transaction_v3::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V3,
    CompilerExecutionExternalAnchorTransactionIdentityV3,
    CompilerExecutionExternalAnchorTransactionV3, CompilerExecutionNativeJournalErrorV3,
};
mod issuer_policy_adapter;
mod issuer_policy_codec;
mod issuer_policy_family_v1;
pub use issuer_policy_family_v1::CompilerExecutionPolicyFamilyV1;
mod issuer_policy_v2;
mod issuer_policy_v3;
mod launch_manifest;
mod launch_manifest_adapter;
mod launch_manifest_codec;
mod launch_manifest_v2;
mod launch_manifest_v3;
mod native_proof_custodian_configuration_v1;
pub use native_proof_custodian_configuration_v1::{
    NATIVE_PROOF_CUSTODIAN_CONFIGURATION_BYTES_V1, NATIVE_PROOF_CUSTODIAN_MAX_ANALYZER_BYTES_V1,
    NATIVE_PROOF_CUSTODIAN_MAX_CONTROLLER_BYTES_V1, NativeApplicationProofCustodianConfigurationV1,
    NativeProofCustodianConfigurationErrorV1, NativeProofCustodianConfigurationPartsV1,
    NativeProofCustodianConfigurationStorageV1,
};
mod proof_executor_bootstrap_v1;
pub use proof_executor_bootstrap_v1::{
    PROOF_EXECUTOR_BOOTSTRAP_BYTES_V1, PROOF_EXECUTOR_BOOTSTRAP_STORAGE_V1,
    PROOF_EXECUTOR_BOOTSTRAP_WORK_V1, ProofExecutorBootstrapErrorV1,
    ProofExecutorBootstrapFramingErrorV1, ProofExecutorBootstrapKindV1,
    ProofExecutorBootstrapRecordV1, ProofExecutorBootstrapStorageV1,
};
mod receipt_carriage_adapter;
mod receipt_carriage_v2;
mod receipt_carriage_v3;
mod receipt_publication;
mod receipt_publication_adapter;
mod receipt_publication_codec;
mod receipt_publication_v2;
mod receipt_publication_v3;
mod root_control_v3;
pub use root_control_v3::{
    COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3, COMPILER_EXECUTION_ROOT_CONTROL_PAYLOAD_BYTES_V3,
    COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3, COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3,
    CompilerExecutionRootControlBindingV3, CompilerExecutionRootControlErrorV3,
    CompilerExecutionRootControlKindV3, CompilerExecutionRootControlRecordV3,
};
mod root_gate_v3;
pub use root_gate_v3::{
    COMPILER_EXECUTION_ROOT_GATE_STORAGE_V3, COMPILER_EXECUTION_ROOT_GATE_WORK_V3,
    compiler_execution_root_gate_reply_v3, compiler_execution_root_gate_request_v3,
    validate_compiler_execution_root_gate_reply_v3,
    validate_compiler_execution_root_gate_request_v3,
};
mod root_intake_v3;
pub use root_intake_v3::{
    COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V3, COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V3,
    COMPILER_EXECUTION_ROOT_INTAKE_WORK_V3, CompilerExecutionRootIntakeErrorV3,
    CompilerExecutionRootIntakeKindV3, CompilerExecutionRootIntakeRecordV3,
    CompilerExecutionRootIntakeRoleV3,
};
mod root_completion_v1;
mod root_publication_completion_v1;
pub use root_completion_v1::{
    COMPILER_EXECUTION_ROOT_COMPLETION_BYTES_V1, COMPILER_EXECUTION_ROOT_COMPLETION_STORAGE_V1,
    COMPILER_EXECUTION_ROOT_COMPLETION_WORK_V1, CompilerExecutionRootCompletionErrorV1,
    CompilerExecutionRootCompletionRecordV1, CompilerExecutionRootTerminationV1,
};
pub use root_publication_completion_v1::{
    COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_BYTES_V1,
    COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_DECODE_WORK_V1,
    COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_MATCH_WORK_V1,
    COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_NEW_WORK_V1,
    COMPILER_EXECUTION_ROOT_PUBLICATION_COMPLETION_STORAGE_V1,
    CompilerExecutionRootPublicationCompletionErrorV1,
    CompilerExecutionRootPublicationCompletionV1,
};
mod root_intake_v4;
pub use root_intake_v4::{
    COMPILER_EXECUTION_ROOT_INTAKE_BYTES_V4, COMPILER_EXECUTION_ROOT_INTAKE_OUTPUT_FD_V4,
    COMPILER_EXECUTION_ROOT_INTAKE_STORAGE_V4, COMPILER_EXECUTION_ROOT_INTAKE_WORK_V4,
    CompilerExecutionRootIntakeErrorV4, CompilerExecutionRootIntakeKindV4,
    CompilerExecutionRootIntakeRecordV4, CompilerExecutionRootIntakeRoleV4,
};
mod service;
mod service_native_adapter;
mod service_ready;
mod service_ready_adapter;
mod service_ready_codec;
mod service_ready_v2;
mod service_ready_v3;
mod service_v2;
mod service_v3;
mod supervisor_deployment;
mod supervisor_deployment_adapter;
mod supervisor_deployment_codec;
mod supervisor_deployment_v2;
mod supervisor_deployment_v3;
mod supervisor_handoff;
mod supervisor_handoff_adapter;
mod supervisor_handoff_codec;
mod supervisor_handoff_v2;
mod supervisor_handoff_v3;
mod supervisor_ready;
mod supervisor_ready_native_adapter;
mod supervisor_ready_native_codec;
mod supervisor_ready_v2;
mod supervisor_ready_v3;
mod worker_anchor_journal;
mod worker_anchor_journal_codec;
mod worker_anchor_journal_v2;
mod worker_anchor_journal_v3;
pub use worker_anchor_journal_v2::{
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2, CompilerExecutionWorkerAnchorJournalErrorV2,
    CompilerExecutionWorkerAnchorJournalV2,
};
pub use worker_anchor_journal_v3::{
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V3, CompilerExecutionWorkerAnchorJournalErrorV3,
    CompilerExecutionWorkerAnchorJournalV3,
};

/// Sole production runtime directory for the protected compiler-execution supervisor.
pub const COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_V1: &str = "/run/fe2o3";

/// Exact root-owned production runtime-directory mode.
pub const COMPILER_EXECUTION_SUPERVISOR_RUNTIME_DIRECTORY_MODE_V1: u32 = 0o755;

/// Exact root-owned, service-group-accessible production socket mode.
pub const COMPILER_EXECUTION_SUPERVISOR_SOCKET_MODE_V1: u32 = 0o660;

/// Sole production Unix socket pathname for the protected compiler-execution supervisor.
pub const COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1: &str =
    "/run/fe2o3/compiler-execution-supervisor.sock";

/// Distinct native application-registration ingress; never a compiler request socket.
pub const NATIVE_APPLICATION_SUPERVISOR_SOCKET_PATH_V3: &str =
    "/run/fe2o3/native-application-supervisor.sock";

/// Sole production durable-root pathname for the protected compiler-execution supervisor.
pub const COMPILER_EXECUTION_SUPERVISOR_STATE_ROOT_PATH_V1: &str =
    "/var/lib/fe2o3/compiler-execution";

/// Exact dedicated-service-owned production durable-root mode.
pub const COMPILER_EXECUTION_SUPERVISOR_STATE_ROOT_MODE_V1: u32 = 0o700;

/// Sole root-owned deployment lifecycle-lock pathname.
pub const COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1: &str =
    "/var/lib/fe2o3/compiler-execution-lifecycle-v1";

/// Exact root-only deployment lifecycle-lock mode.
pub const COMPILER_EXECUTION_LIFECYCLE_LOCK_MODE_V1: u32 = 0o400;

/// Sole production public client-profile pathname.
pub const COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1: &str =
    "/etc/fe2o3/compiler-execution/client-profile-v1";

/// Sole native public client-profile pathname. No legacy-profile fallback.
pub const COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V2: &str =
    "/etc/fe2o3/compiler-execution/client-profile-v2";

pub use attestation::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1, COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
    CompilerExecutionAttestationChallengeIdentityV1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationErrorV1, CompilerExecutionAttestationReceiptIdentityV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestIdentityV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyIdentityV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionSubjectBindingV1, SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1,
    VerifiedCompilerExecutionAttestationV1, sealed_static_issuer_runtime_measurement_v1,
};
pub use attestation_challenge_v2::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V2,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2,
    CompilerExecutionAttestationChallengeIdentityV2, CompilerExecutionAttestationChallengeV2,
    CompilerExecutionSubjectBindingV2,
};
pub use attestation_challenge_v3::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V3,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3,
    CompilerExecutionAttestationChallengeIdentityV3, CompilerExecutionAttestationChallengeV3,
    CompilerExecutionSubjectBindingV3,
};
pub use attestation_receipt_v2::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2,
    CompilerExecutionAttestationReceiptIdentityV2, CompilerExecutionAttestationReceiptV2,
    VerifiedCompilerExecutionAttestationV2,
};
pub use attestation_receipt_v3::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V3,
    CompilerExecutionAttestationReceiptIdentityV3, CompilerExecutionAttestationReceiptV3,
    VerifiedCompilerExecutionAttestationV3,
};
pub use attestation_request_v2::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2, CompilerExecutionAttestationRequestIdentityV2,
    CompilerExecutionAttestationRequestV2,
};
pub use attestation_request_v3::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V3,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V3, CompilerExecutionAttestationRequestIdentityV3,
    CompilerExecutionAttestationRequestV3,
};
pub use attestation_resources::CompilerExecutionAttestationStorageV2;
pub use attestation_resources::CompilerExecutionAttestationStorageV3;
pub use client_profile::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1, CompilerExecutionClientProfileErrorV1,
    CompilerExecutionClientProfileIdentityV1, CompilerExecutionClientProfileV1,
};
pub use client_profile_v2::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2, COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2, CompilerExecutionClientProfileErrorV2,
    CompilerExecutionClientProfileIdentityV2, CompilerExecutionClientProfileV2,
};
pub use client_profile_v3::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3, COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V3,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3, COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3,
    CompilerExecutionClientProfileErrorV3, CompilerExecutionClientProfileIdentityV3,
    CompilerExecutionClientProfileV3,
};
pub use current_record_verification::{
    COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3,
    COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3,
    CompilerExecutionCurrentRecordAttestationIdentityV3,
    CompilerExecutionCurrentRecordAttestationV3, CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationIdentityV3,
    CompilerExecutionCurrentRecordVerificationV3, VerifiedCompilerExecutionCurrentRecordV3,
};
pub use external_anchor_deployment::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1,
    CompilerExecutionExternalAnchorDeploymentErrorV1,
    CompilerExecutionExternalAnchorDeploymentIdentityV1,
    CompilerExecutionExternalAnchorDeploymentV1,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
};
pub use external_anchor_deployment_v2::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V2,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V2,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V2,
    CompilerExecutionExternalAnchorDeploymentErrorV2,
    CompilerExecutionExternalAnchorDeploymentIdentityV2,
    CompilerExecutionExternalAnchorDeploymentV2,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V2,
};
pub use external_anchor_deployment_v3::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3,
    CompilerExecutionExternalAnchorDeploymentErrorV3,
    CompilerExecutionExternalAnchorDeploymentIdentityV3,
    CompilerExecutionExternalAnchorDeploymentV3,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V3,
};
pub use external_anchor_provisioning::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
    CompilerExecutionExternalAnchorProvisioningErrorV1,
    CompilerExecutionExternalAnchorProvisioningIdentityV1,
    CompilerExecutionExternalAnchorProvisioningV1,
};
pub use external_anchor_provisioning_v2::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V2,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V2,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V2,
    CompilerExecutionExternalAnchorProvisioningErrorV2,
    CompilerExecutionExternalAnchorProvisioningIdentityV2,
    CompilerExecutionExternalAnchorProvisioningV2,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V2,
};
pub use external_anchor_provisioning_v3::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V3,
    CompilerExecutionExternalAnchorProvisioningErrorV3,
    CompilerExecutionExternalAnchorProvisioningIdentityV3,
    CompilerExecutionExternalAnchorProvisioningV3,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V3,
};
pub use external_anchor_service::{
    CompilerExecutionExternalAnchorServiceIdentityErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityV1,
};
pub use external_anchor_transaction::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1,
    CompilerExecutionExternalAnchorTransactionErrorV1,
    CompilerExecutionExternalAnchorTransactionIdentityV1,
    CompilerExecutionExternalAnchorTransactionV1,
};
pub use issuer_policy_v2::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2, COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2, CompilerExecutionAttestationErrorV2,
    CompilerExecutionIssuerPolicyIdentityV2, CompilerExecutionIssuerPolicyV2,
};
pub use issuer_policy_v3::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3, COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3, CompilerExecutionAttestationErrorV3,
    CompilerExecutionIssuerPolicyIdentityV3, CompilerExecutionIssuerPolicyV3,
};
pub use launch_manifest::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1, CompilerExecutionClientProcessIdentityV1,
    CompilerExecutionServiceLaunchManifestErrorV1,
    CompilerExecutionServiceLaunchManifestIdentityV1, CompilerExecutionServiceLaunchManifestV1,
};
pub use launch_manifest_v2::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2,
    CompilerExecutionServiceLaunchManifestErrorV2, CompilerExecutionServiceLaunchManifestV2,
};
pub use launch_manifest_v3::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3,
    CompilerExecutionServiceLaunchManifestErrorV3, CompilerExecutionServiceLaunchManifestV3,
};
pub use receipt_carriage_v2::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2, COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2,
    CompilerExecutionReceiptCarriageIdentityV2, CompilerExecutionReceiptCarriageV2,
};
pub use receipt_carriage_v3::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V3, COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V3,
    CompilerExecutionReceiptCarriageIdentityV3, CompilerExecutionReceiptCarriageV3,
};
pub use receipt_publication::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, CompilerExecutionReceiptCarriageIdentityV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckIdentityV1,
    CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationErrorV1,
    CompilerExecutionReceiptPublicationIdentityV1, CompilerExecutionReceiptPublicationV1,
};
pub use receipt_publication_v2::{
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2,
    CompilerExecutionReceiptPublicationAckIdentityV2, CompilerExecutionReceiptPublicationAckV2,
    CompilerExecutionReceiptPublicationErrorV2, CompilerExecutionReceiptPublicationIdentityV2,
    CompilerExecutionReceiptPublicationV2,
};
pub use receipt_publication_v3::{
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V3,
    CompilerExecutionReceiptPublicationAckIdentityV3, CompilerExecutionReceiptPublicationAckV3,
    CompilerExecutionReceiptPublicationErrorV3, CompilerExecutionReceiptPublicationIdentityV3,
    CompilerExecutionReceiptPublicationV3,
};
pub use service::{
    COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V1,
    COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V1,
    CompilerExecutionServiceProtocolErrorV1, CompilerExecutionServicePublishDispositionV1,
    CompilerExecutionServiceRequestIdentityV1, CompilerExecutionServiceRequestKindV1,
    CompilerExecutionServiceRequestV1, CompilerExecutionServiceResponseIdentityV1,
    CompilerExecutionServiceResponseKindV1, CompilerExecutionServiceResponseV1,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1,
};
pub use service_ready::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V1, CompilerExecutionServiceReadyErrorV1,
    CompilerExecutionServiceReadyIdentityV1, CompilerExecutionServiceReadyV1,
};
pub use service_ready_v2::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2, COMPILER_EXECUTION_SERVICE_READY_STORAGE_V2,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V2, CompilerExecutionServiceReadyErrorV2,
    CompilerExecutionServiceReadyV2,
};
pub use service_ready_v3::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3, COMPILER_EXECUTION_SERVICE_READY_STORAGE_V3,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V3, CompilerExecutionServiceReadyErrorV3,
    CompilerExecutionServiceReadyV3,
};
pub use service_v2::{
    COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_REQUEST_STORAGE_V2, COMPILER_EXECUTION_SERVICE_REQUEST_WORK_V2,
    COMPILER_EXECUTION_SERVICE_RESPONSE_STORAGE_V2, COMPILER_EXECUTION_SERVICE_RESPONSE_WORK_V2,
    COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V2,
    CompilerExecutionServiceProtocolErrorV2, CompilerExecutionServiceRequestIdentityV2,
    CompilerExecutionServiceRequestKindV2, CompilerExecutionServiceRequestPayloadV2,
    CompilerExecutionServiceRequestV2, CompilerExecutionServiceResponseIdentityV2,
    CompilerExecutionServiceResponseKindV2, CompilerExecutionServiceResponsePayloadV2,
    CompilerExecutionServiceResponseV2, MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V2,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V2,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V2,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V2,
};
pub use service_v3::{
    COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_REQUEST_STORAGE_V3, COMPILER_EXECUTION_SERVICE_REQUEST_WORK_V3,
    COMPILER_EXECUTION_SERVICE_RESPONSE_STORAGE_V3, COMPILER_EXECUTION_SERVICE_RESPONSE_WORK_V3,
    COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V3,
    CompilerExecutionServiceProtocolErrorV3, CompilerExecutionServiceRequestIdentityV3,
    CompilerExecutionServiceRequestKindV3, CompilerExecutionServiceRequestPayloadV3,
    CompilerExecutionServiceRequestV3, CompilerExecutionServiceResponseIdentityV3,
    CompilerExecutionServiceResponseKindV3, CompilerExecutionServiceResponsePayloadV3,
    CompilerExecutionServiceResponseV3, MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V3,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V3,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V3,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V3,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V3,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V3,
};
pub use supervisor_deployment::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
    CompilerExecutionSupervisorDeploymentErrorV1, CompilerExecutionSupervisorDeploymentIdentityV1,
    CompilerExecutionSupervisorDeploymentV1, MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
};
pub use supervisor_deployment_v2::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V2,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2, CompilerExecutionSupervisorDeploymentErrorV2,
    CompilerExecutionSupervisorDeploymentIdentityV2, CompilerExecutionSupervisorDeploymentV2,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V2,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V2,
};
pub use supervisor_deployment_v3::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3, CompilerExecutionSupervisorDeploymentErrorV3,
    CompilerExecutionSupervisorDeploymentIdentityV3, CompilerExecutionSupervisorDeploymentV3,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V3,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3,
};
pub use supervisor_handoff::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1, CompilerExecutionSupervisorHandoffErrorV1,
    CompilerExecutionSupervisorHandoffIdentityV1, CompilerExecutionSupervisorHandoffV1,
};
pub use supervisor_handoff_v2::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V2,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V2, CompilerExecutionSupervisorHandoffErrorV2,
    CompilerExecutionSupervisorHandoffV2,
};
pub use supervisor_handoff_v3::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V3,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_STORAGE_V3,
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_WORK_V3, CompilerExecutionSupervisorHandoffErrorV3,
    CompilerExecutionSupervisorHandoffV3,
};
pub use supervisor_ready::{
    COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1, CompilerExecutionSupervisorReadyErrorV1,
    CompilerExecutionSupervisorReadyIdentityV1, CompilerExecutionSupervisorReadyV1,
};
pub use supervisor_ready_v2::{
    COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V2, COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V2,
    COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V2, CompilerExecutionSupervisorReadyErrorV2,
    CompilerExecutionSupervisorReadyIdentityV2, CompilerExecutionSupervisorReadyV2,
};
pub use supervisor_ready_v3::{
    COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V3, COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V3,
    COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V3, CompilerExecutionSupervisorReadyErrorV3,
    CompilerExecutionSupervisorReadyIdentityV3, CompilerExecutionSupervisorReadyV3,
};
pub use worker_anchor_journal::{
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1, CompilerExecutionWorkerAnchorJournalErrorV1,
    CompilerExecutionWorkerAnchorJournalIdentityV1, CompilerExecutionWorkerAnchorJournalStageV1,
    CompilerExecutionWorkerAnchorJournalV1,
};
