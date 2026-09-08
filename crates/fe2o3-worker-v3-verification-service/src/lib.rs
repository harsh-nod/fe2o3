#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(target_os = "linux"))]
compile_error!("fe2o3-worker-v3-verification-service requires Linux descriptor semantics");

mod durable_v5;
mod protected_completion_v5;
mod service;
mod service_v2;
mod service_v5;

pub use durable_v5::{DurableWorkerV3VerificationStateErrorV5, DurableWorkerV3VerificationStateV5};
pub use protected_completion_v5::{
    MAX_WORKER_V3_PROTECTED_COMPLETION_REQUEST_BYTES_V5,
    MAX_WORKER_V3_PROTECTED_COMPLETION_RESPONSE_BYTES_V5, ProtectedWorkerV3VerifierSigningKeyV5,
    WorkerV3ProtectedCompletionErrorV5, WorkerV3ProtectedCompletionRequestV5,
    WorkerV3ProtectedCompletionResponseV5, issue_worker_v3_protected_completion_v5,
};
pub use service::{
    FramedWorkerV3VerificationSessionV1, RejectedWorkerV3VerificationSessionV1,
    RetainedWorkerV3VerificationPayloadV1, WorkerV3VerificationCallerV1,
    WorkerV3VerificationChallengeReplayGuardV1, WorkerV3VerificationMeasurementResolverV1,
    WorkerV3VerificationPolicyResolverV1, WorkerV3VerificationRejectionReasonV1,
    WorkerV3VerificationServiceErrorV1, WorkerV3VerificationSessionOutcomeV1,
    prepare_worker_v3_verification_receiver_v1, serve_worker_v3_verification_session_v1,
};
pub use service_v2::{
    CompletedWorkerV3VerificationSessionV2, PendingRejectedWorkerV3VerificationTerminalSessionV2,
    PendingWorkerV3VerificationCurrentRecordSessionV2,
    PendingWorkerV3VerificationTerminalSessionV2, RejectedWorkerV3VerificationBeginV2,
    WorkerV3VerificationAcceptedServiceAdmissionFailureV2,
    WorkerV3VerificationAcceptedServiceEndpointV2, WorkerV3VerificationBeginOutcomeV2,
    WorkerV3VerificationChallengeReservationProviderV2, WorkerV3VerificationCurrentRecordOutcomeV2,
    WorkerV3VerificationRejectedSendFailureV2, WorkerV3VerificationRejectionReasonV2,
    WorkerV3VerificationServiceErrorV2, WorkerV3VerificationTerminalSendFailureV2,
    begin_worker_v3_verification_accepted_session_until_v2,
    begin_worker_v3_verification_session_until_v2, begin_worker_v3_verification_session_v2,
};
pub use service_v5::{
    CompletedWorkerV3VerificationCapabilitySessionV5,
    RejectedWorkerV3VerificationCapabilitySessionV5,
    WorkerV3VerificationCapabilityAttemptQuarantineV5,
    WorkerV3VerificationCapabilityCompletionJournalV5,
    WorkerV3VerificationCapabilityRejectionReasonV5, WorkerV3VerificationCapabilityServiceErrorV5,
    WorkerV3VerificationCapabilitySessionOutcomeV5,
    serve_worker_v3_verification_capability_session_v5,
};
