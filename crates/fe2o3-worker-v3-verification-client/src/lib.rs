#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(target_os = "linux"))]
compile_error!(
    "fe2o3-worker-v3-verification-client requires Linux memfd and SOCK_SEQPACKET semantics"
);

mod client;
mod error;
mod snapshot;

pub use client::{WorkerV3VerificationClientV1, WorkerV3VerificationFramingReceiptV1};
pub use error::WorkerV3VerificationClientErrorV1;
pub use snapshot::WorkerV3VerificationPayloadSnapshotsV1;
