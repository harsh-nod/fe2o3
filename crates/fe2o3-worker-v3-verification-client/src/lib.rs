#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(target_os = "linux"))]
compile_error!(
    "fe2o3-worker-v3-verification-client requires Linux memfd and SOCK_SEQPACKET semantics"
);

mod client;
mod client_v2;
mod error;
mod snapshot;

pub use client::{WorkerV3VerificationClientV1, WorkerV3VerificationFramingReceiptV1};
pub use client_v2::{
    PendingWorkerV3VerificationClientV2, RejectedWorkerV3VerificationBeginV2,
    WorkerV3VerificationBeginOutcomeV2, WorkerV3VerificationClientErrorV2,
    WorkerV3VerificationClientV2, WorkerV3VerificationCurrentRecordChallengeV2,
    WorkerV3VerificationReservedBeginV2,
};
pub use error::WorkerV3VerificationClientErrorV1;
pub use snapshot::WorkerV3VerificationPayloadSnapshotsV1;
