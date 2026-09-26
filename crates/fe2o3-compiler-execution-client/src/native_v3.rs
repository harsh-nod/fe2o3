//! Consuming native exchanges on the existing inherited service channel.
//! The endpoint response is cryptographic evidence, not protected-key or GPU authority.
//!
//! Conditional V3 exchanges use actual V3 owners and never retry V1/V2. A
//! caller must independently admit the peer and pinned policy's provenance.
//! This terminal exclusive budget borrow does not activate early production
//! compiler admission or manufacture a protected issuer/readiness owner.
//!
//! ```compile_fail
//! use fe2o3_compiler_execution_client::CompilerExecutionClientV3 as Client;
//! use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
//! use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
//! fn mix(client: Client<'_, '_>, policy: &Policy, subject: Subject) {
//!     let _ = client.acquire(policy, subject);
//! }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_client::{CompilerExecutionClientV3 as C3,
//!     CompilerExecutionClientV2 as C2};
//! fn downgrade<'a, 'b>(client: C3<'a, 'b>) -> C2<'a, 'b> { client.into() }
//! ```
use super::{
    CompilerExecutionClientErrorV1 as TransportError, receive_packet_with_io, send_packet_with_io,
    set_close_on_exec, validate_seqpacket_peer,
};
use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectStorageV3 as SubjectStorage,
    InertCompilerExecutionSubjectV3 as Subject,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV3 as Challenge,
    CompilerExecutionAttestationErrorV3 as AttestationError,
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionAttestationStorageV3 as Storage, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionNativeJournalErrorV3 as JournalError,
    CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionReceiptPublicationErrorV3 as PublicationError,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServiceProtocolErrorV3 as ProtocolError,
    CompilerExecutionServiceRequestPayloadV3 as Payload,
    CompilerExecutionServiceRequestV3 as ServiceRequest,
    CompilerExecutionServiceResponseKindV3 as Kind, CompilerExecutionServiceResponseV3 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V3 as MAX_REQUEST,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V3 as MAX_RESPONSE,
    VerifiedCompilerExecutionCurrentRecordV3 as VerifiedCurrent,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    os::fd::OwnedFd,
    time::{Duration, Instant},
};

crate::native_adapter::native_client_adapter!(
    CompilerExecutionClientErrorV3,
    CompilerExecutionClientStorageV3,
    CompilerExecutionClientV3,
    verify_native_v3
);

#[cfg(test)]
use CompilerExecutionClientV3 as TestClient;
#[cfg(test)]
#[path = "native_admission_tests.rs"]
mod tests;
