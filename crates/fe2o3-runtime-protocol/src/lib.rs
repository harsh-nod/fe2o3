#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod application_handoff_v3;
mod conditional_worker_readiness;
mod conditional_worker_readiness_codec;
mod native_conditional_application_identity_v1;
pub use conditional_worker_readiness::{
    ConditionalWorkerReadinessEnvelopeV5, ConditionalWorkerReadinessErrorV5,
    ConditionalWorkerReadinessQuoteV5,
};
pub use conditional_worker_readiness_codec::{
    CONDITIONAL_WORKER_READINESS_MAGIC_V5, ConditionalWorkerReadinessCodecErrorV5,
    InertConditionalWorkerReadinessWireV5, MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5,
};
pub use native_conditional_application_identity_v1::{
    NATIVE_CONDITIONAL_APPLICATION_BINDING_BYTES_V1, NativeConditionalApplicationBindingErrorV1,
    NativeConditionalApplicationBindingQuoteV1, NativeConditionalApplicationBindingV1,
    NativeConditionalApplicationIdentityV1,
};
mod application_proof_v1;
mod application_registration_v1;
mod native_application_currentness_root_v1;
mod native_application_proof_v1;
mod native_application_registration_v1;
mod native_application_root_transfer_v1;
mod native_application_startup_v1;
pub use native_application_currentness_root_v1::{
    NATIVE_APPLICATION_CURRENTNESS_ROOT_BYTES_V1,
    NATIVE_APPLICATION_CURRENTNESS_ROOT_GATE_BYTES_V1, NativeApplicationCurrentnessRootErrorV1,
    NativeApplicationCurrentnessRootKindV1, NativeApplicationCurrentnessRootRecordV1,
    NativeApplicationCurrentnessRootStorageV1,
};
pub use native_application_proof_v1::{
    NATIVE_APPLICATION_PROOF_EVIDENCE_BYTES_V1, NATIVE_APPLICATION_PROOF_INPUT_BYTES_V1,
    NATIVE_APPLICATION_PROOF_MAX_COMPONENT_BYTES_V1, NATIVE_APPLICATION_PROOF_MAX_PACKET_BYTES_V1,
    NATIVE_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1, NativeApplicationProofErrorV1,
    NativeApplicationProofEvidencePartsV1, NativeApplicationProofEvidenceV1,
    NativeApplicationProofInputsV1, NativeApplicationProofKindV1, NativeApplicationProofMessageV1,
    NativeApplicationProofQuoteV1, NativeApplicationProofStorageV1,
};
pub use native_application_registration_v1::{
    NATIVE_APPLICATION_PROOF_SESSION_BYTES_V1, NATIVE_APPLICATION_REGISTRATION_BYTES_V1,
    NATIVE_APPLICATION_REGISTRATION_INPUT_BYTES_V1, NATIVE_APPLICATION_SESSION_MAX_BYTES_V1,
    NativeApplicationProofSessionV1, NativeApplicationRegistrationBindingV1,
    NativeApplicationRegistrationErrorV1, NativeApplicationRegistrationIdentityV1,
    NativeApplicationRegistrationInputsV1, NativeApplicationRegistrationQuoteV1,
    NativeApplicationRegistrationStorageV1, NativeApplicationSessionKindV1,
    NativeApplicationSessionMessageV1, NativeApplicationSessionTranscriptV1,
};
pub use native_application_root_transfer_v1::{
    NATIVE_APPLICATION_ROOT_NONCE_BYTES_V1, NATIVE_APPLICATION_ROOT_TRANSFER_BYTES_V1,
    NativeApplicationRootTransferErrorV1, NativeApplicationRootTransferV1,
};
pub use native_application_startup_v1::{
    NATIVE_APPLICATION_STARTUP_BYTES_V1, NativeApplicationStartupErrorV1,
    NativeApplicationStartupKindV1, NativeApplicationStartupRecordV1,
    NativeApplicationStartupStorageV1,
};
mod worker_v3_load_envelope;
mod worker_v3_load_envelope_v2;

pub use application_handoff_v3::{
    MAX_WORKER_V3_APPLICATION_HANDOFF_ALLOCATION_BYTES_V1, MAX_WORKER_V3_APPLICATION_INPUTS_V1,
    MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1, WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
    WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_ACK_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_BYTES_V1,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
    WORKER_V3_APPLICATION_HANDOFF_EXPECTATION_BYTES_V1, WORKER_V3_APPLICATION_HANDOFF_VERSION_V1,
    WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1, WORKER_V3_APPLICATION_PROOF_FD_ENV_V1,
    WorkerV3ApplicationHandoffAckV1, WorkerV3ApplicationHandoffChallengeV1,
    WorkerV3ApplicationHandoffCodecBudgetV1, WorkerV3ApplicationHandoffCommitmentV1,
    WorkerV3ApplicationHandoffExpectationV1, WorkerV3ApplicationHandoffProtocolErrorV1,
    WorkerV3ApplicationIdentityV1, WorkerV3ApplicationInputOccurrenceV1,
    WorkerV3ApplicationOccurrenceIdentityV1, WorkerV3ApplicationOccurrenceV1,
    WorkerV3LoadEnvelopeIdentityV1,
};
pub use application_proof_v1::{
    WORKER_V3_APPLICATION_PROOF_MAX_PACKET_BYTES_V1,
    WORKER_V3_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1, WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1,
    WorkerV3ApplicationProofInputsV1, WorkerV3ApplicationProofKindV1,
    WorkerV3ApplicationProofMessageV1, WorkerV3ApplicationProofProtocolErrorV1,
    WorkerV3ApplicationProofSessionV1,
};
pub use application_registration_v1::{
    WORKER_V3_APPLICATION_CUSTODIAN_HANDOFF_BYTES_V1,
    WORKER_V3_APPLICATION_CUSTODIAN_READY_BYTES_V1, WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1,
    WORKER_V3_APPLICATION_SESSION_MAX_BYTES_V1, WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1,
    WorkerV3ApplicationCustodianHandoffV1, WorkerV3ApplicationCustodianRouteErrorV1,
    WorkerV3ApplicationCustodianSupervisorReadyV1, WorkerV3ApplicationRegistrationBindingV1,
    WorkerV3ApplicationRegistrationDescriptorsV1, WorkerV3ApplicationRegistrationErrorV1,
    WorkerV3ApplicationRegistrationIdentityV1, WorkerV3ApplicationRegistrationInputsV1,
    WorkerV3ApplicationSessionKindV1, WorkerV3ApplicationSessionMessageV1,
    WorkerV3ApplicationSessionTranscriptV1, WorkerV3ApplicationSupervisorReadyErrorV1,
    WorkerV3ApplicationSupervisorReadyV1,
};
pub use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3,
    COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1,
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
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V1,
    CompilerExecutionAttestationChallengeIdentityV1, CompilerExecutionAttestationChallengeV1,
    CompilerExecutionAttestationErrorV1, CompilerExecutionAttestationReceiptIdentityV1,
    CompilerExecutionAttestationReceiptV1, CompilerExecutionAttestationRequestIdentityV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordAttestationIdentityV3,
    CompilerExecutionCurrentRecordAttestationV3, CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationIdentityV3,
    CompilerExecutionCurrentRecordVerificationV3,
    CompilerExecutionExternalAnchorServiceIdentityErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityV1,
    CompilerExecutionExternalAnchorTransactionErrorV1,
    CompilerExecutionExternalAnchorTransactionIdentityV1,
    CompilerExecutionExternalAnchorTransactionV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyIdentityV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageIdentityV1, CompilerExecutionReceiptCarriageV1,
    CompilerExecutionReceiptPublicationAckIdentityV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionReceiptPublicationIdentityV1,
    CompilerExecutionReceiptPublicationV1, CompilerExecutionServiceProtocolErrorV1,
    CompilerExecutionServicePublishDispositionV1, CompilerExecutionServiceRequestIdentityV1,
    CompilerExecutionServiceRequestKindV1, CompilerExecutionServiceRequestV1,
    CompilerExecutionServiceResponseIdentityV1, CompilerExecutionServiceResponseKindV1,
    CompilerExecutionServiceResponseV1, CompilerExecutionSubjectBindingV1,
    CompilerExecutionWorkerAnchorJournalErrorV1, CompilerExecutionWorkerAnchorJournalIdentityV1,
    CompilerExecutionWorkerAnchorJournalStageV1, CompilerExecutionWorkerAnchorJournalV1,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1, SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1,
    VerifiedCompilerExecutionAttestationV1, VerifiedCompilerExecutionCurrentRecordV3,
    sealed_static_issuer_runtime_measurement_v1,
};
pub use fe2o3_static_executable_format::{
    SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1, SealedStaticApplicationErrorV1,
    sealed_static_application_identity_v1, sealed_static_application_work_bound_v1,
};
pub use worker_v3_load_envelope::{
    MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V1, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V1,
    RecoveredWorkerV3LoadEnvelopeV1, WORKER_V3_LOAD_ENVELOPE_MAGIC_V1,
    WORKER_V3_LOAD_ENVELOPE_VERSION_V1, WorkerV3LoadEnvelopeBindingFieldV1,
    WorkerV3LoadEnvelopeCodecBudgetV1, WorkerV3LoadEnvelopeErrorV1, WorkerV3LoadEnvelopeV1,
    WorkerV3LoadEnvelopeWireV1, recover_worker_v3_load_envelope_v1,
};
pub use worker_v3_load_envelope_v2::{
    MAX_WORKER_V3_LOAD_ENVELOPE_ALLOCATION_BYTES_V2, MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2,
    MAX_WORKER_V3_LOAD_ENVELOPE_REPLAY_BYTES_V2, RecoveredWorkerV3LoadEnvelopeV2,
    WORKER_V3_LOAD_ENVELOPE_MAGIC_V2, WORKER_V3_LOAD_ENVELOPE_VERSION_V2,
    WorkerV3LoadEnvelopeBindingFieldV2, WorkerV3LoadEnvelopeCodecBudgetV2,
    WorkerV3LoadEnvelopeErrorV2, WorkerV3LoadEnvelopeEvidenceViewV2, WorkerV3LoadEnvelopeV2,
    WorkerV3LoadEnvelopeWireV2, recover_worker_v3_load_envelope_from_retained_directory_v2,
    recover_worker_v3_load_envelope_v2,
};
