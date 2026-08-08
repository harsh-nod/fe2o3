#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod codec;
mod error;
mod model;

pub use codec::{WORKER_V2_LOAD_ENVELOPE_MAGIC, WORKER_V2_LOAD_ENVELOPE_VERSION};
pub use error::{EnvelopeDecodeError, EnvelopeValidationError, PublicationClaimFieldV1};
pub use model::{
    BackendPublicationReceiptProjectionV1, DescriptorLineageV1, DurablePublishedClaimV1,
    ExactRawHsacoV1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES, MAX_WORKER_V2_PROOF_EVIDENCE_BYTES,
    MAX_WORKER_V2_RAW_HSACO_BYTES, WorkerV2LoadEnvelopeV1,
};
