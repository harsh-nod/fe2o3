#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(target_os = "linux"))]
compile_error!(
    "fe2o3-worker-v3-verification-client requires Linux memfd and SOCK_SEQPACKET semantics"
);

mod client;
mod client_v2;
mod client_v5;
mod error;
mod snapshot;

pub use client::{WorkerV3VerificationClientV1, WorkerV3VerificationFramingReceiptV1};
pub use client_v2::{
    PendingWorkerV3VerificationClientV2, RejectedWorkerV3VerificationBeginV2,
    WorkerV3VerificationBeginOutcomeV2, WorkerV3VerificationClientAdmissionFailureV2,
    WorkerV3VerificationClientErrorV2, WorkerV3VerificationClientV2,
    WorkerV3VerificationCurrentRecordChallengeV2, WorkerV3VerificationReservedBeginV2,
};
pub use client_v5::{
    IntakeAuthenticatedWorkerV3CapabilityCustodyV5, PRODUCTION_WORKER_V3_VERIFIER_SOCKET_PATH_V5,
    WorkerV3VerificationCapabilityClientAdmissionFailureV5,
    WorkerV3VerificationCapabilityClientErrorV5, WorkerV3VerificationCapabilityClientV5,
    WorkerV3VerificationCapabilityCompletedReceiptV5,
    WorkerV3VerificationCapabilityExchangeOutcomeV5, WorkerV3VerificationCapabilityPeerPolicyV5,
    WorkerV3VerificationCapabilityRejectedReceiptV5, WorkerV3VerificationCapabilityRequestPlanV5,
    WorkerV3VerificationClientFailureQuarantineV5, WorkerV3VerificationResponseReplayGuardV5,
    production_worker_v3_verifier_measurement_identity_v5,
};
pub use error::WorkerV3VerificationClientErrorV1;
pub use snapshot::WorkerV3VerificationPayloadSnapshotsV1;
