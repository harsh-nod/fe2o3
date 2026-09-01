#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(target_os = "linux"))]
compile_error!("fe2o3-worker-v3-verification-service requires Linux descriptor semantics");

mod service;

pub use service::{
    FramedWorkerV3VerificationSessionV1, RejectedWorkerV3VerificationSessionV1,
    RetainedWorkerV3VerificationPayloadV1, WorkerV3VerificationCallerV1,
    WorkerV3VerificationChallengeReplayGuardV1, WorkerV3VerificationMeasurementResolverV1,
    WorkerV3VerificationPolicyResolverV1, WorkerV3VerificationRejectionReasonV1,
    WorkerV3VerificationServiceErrorV1, WorkerV3VerificationSessionOutcomeV1,
    prepare_worker_v3_verification_receiver_v1, serve_worker_v3_verification_session_v1,
};
