#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod attestation;
mod attestation_challenge_v2;
mod attestation_receipt_codec;
mod attestation_receipt_v2;
mod attestation_request_codec;
mod attestation_request_v2;
mod attestation_resources;
mod client_profile;
mod client_profile_codec;
mod client_profile_v2;
mod current_record_verification;
mod external_anchor_deployment;
mod external_anchor_provisioning;
mod external_anchor_service;
mod external_anchor_transaction;
mod external_anchor_transaction_v2;
pub use external_anchor_transaction_v2::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2,
    CompilerExecutionExternalAnchorTransactionIdentityV2,
    CompilerExecutionExternalAnchorTransactionV2, CompilerExecutionNativeJournalErrorV2,
};
mod issuer_policy_codec;
mod issuer_policy_v2;
mod launch_manifest;
mod launch_manifest_codec;
mod launch_manifest_v2;
mod receipt_carriage_v2;
mod receipt_publication;
mod receipt_publication_codec;
mod receipt_publication_v2;
mod service;
mod service_ready;
mod service_ready_codec;
mod service_ready_v2;
mod service_v2;
mod supervisor_deployment;
mod supervisor_handoff;
mod supervisor_handoff_codec;
mod supervisor_handoff_v2;
mod supervisor_ready;
mod worker_anchor_journal;
mod worker_anchor_journal_codec;
mod worker_anchor_journal_v2;
pub use worker_anchor_journal_v2::{
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2, CompilerExecutionWorkerAnchorJournalErrorV2,
    CompilerExecutionWorkerAnchorJournalV2,
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
pub use attestation_request_v2::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2, CompilerExecutionAttestationRequestIdentityV2,
    CompilerExecutionAttestationRequestV2,
};
pub use attestation_resources::CompilerExecutionAttestationStorageV2;
pub use client_profile::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1, CompilerExecutionClientProfileErrorV1,
    CompilerExecutionClientProfileIdentityV1, CompilerExecutionClientProfileV1,
};
pub use client_profile_v2::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2, COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2, CompilerExecutionClientProfileErrorV2,
    CompilerExecutionClientProfileIdentityV2, CompilerExecutionClientProfileV2,
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
pub use external_anchor_provisioning::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1,
    CompilerExecutionExternalAnchorProvisioningErrorV1,
    CompilerExecutionExternalAnchorProvisioningIdentityV1,
    CompilerExecutionExternalAnchorProvisioningV1,
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
pub use receipt_carriage_v2::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2, COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2,
    CompilerExecutionReceiptCarriageIdentityV2, CompilerExecutionReceiptCarriageV2,
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
pub use supervisor_deployment::{
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1,
    CompilerExecutionSupervisorDeploymentErrorV1, CompilerExecutionSupervisorDeploymentIdentityV1,
    CompilerExecutionSupervisorDeploymentV1, MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V1,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1,
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
pub use supervisor_ready::{
    COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1, CompilerExecutionSupervisorReadyErrorV1,
    CompilerExecutionSupervisorReadyIdentityV1, CompilerExecutionSupervisorReadyV1,
};
pub use worker_anchor_journal::{
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1, CompilerExecutionWorkerAnchorJournalErrorV1,
    CompilerExecutionWorkerAnchorJournalIdentityV1, CompilerExecutionWorkerAnchorJournalStageV1,
    CompilerExecutionWorkerAnchorJournalV1,
};
