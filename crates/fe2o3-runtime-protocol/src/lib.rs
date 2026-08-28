#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

mod application_handoff_v3;
mod static_application;
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
    WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1, WorkerV3ApplicationHandoffAckV1,
    WorkerV3ApplicationHandoffChallengeV1, WorkerV3ApplicationHandoffCodecBudgetV1,
    WorkerV3ApplicationHandoffCommitmentV1, WorkerV3ApplicationHandoffExpectationV1,
    WorkerV3ApplicationHandoffProtocolErrorV1, WorkerV3ApplicationIdentityV1,
    WorkerV3ApplicationInputOccurrenceV1, WorkerV3ApplicationOccurrenceIdentityV1,
    WorkerV3ApplicationOccurrenceV1, WorkerV3LoadEnvelopeIdentityV1,
};
pub use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V1,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1,
    COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V1,
    COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V1,
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
    CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordAttestationIdentityV1,
    CompilerExecutionCurrentRecordAttestationV1, CompilerExecutionCurrentRecordVerificationErrorV1,
    CompilerExecutionCurrentRecordVerificationIdentityV1,
    CompilerExecutionCurrentRecordVerificationV1,
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
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1, VerifiedCompilerExecutionAttestationV1,
    VerifiedCompilerExecutionCurrentRecordV1,
};
pub use static_application::{
    SealedStaticApplicationErrorV1, sealed_static_application_identity_v1,
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
    WorkerV3LoadEnvelopeErrorV2, WorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeWireV2,
    recover_worker_v3_load_envelope_v2,
};
