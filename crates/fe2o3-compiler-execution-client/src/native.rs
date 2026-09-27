//! Consuming native exchanges on the existing inherited service channel.
//! The endpoint response is cryptographic evidence, not protected-key or GPU authority.
use super::{
    CompilerExecutionClientErrorV1 as TransportError, receive_packet_with_io, send_packet_with_io,
    set_close_on_exec, validate_seqpacket_peer,
};
use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectStorageV2 as SubjectStorage,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV2 as Challenge,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionAttestationStorageV2 as Storage, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionNativeJournalErrorV2 as JournalError,
    CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionReceiptPublicationErrorV2 as PublicationError,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServiceProtocolErrorV2 as ProtocolError,
    CompilerExecutionServiceRequestPayloadV2 as Payload,
    CompilerExecutionServiceRequestV2 as ServiceRequest,
    CompilerExecutionServiceResponseKindV2 as Kind, CompilerExecutionServiceResponseV2 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2 as MAX_REQUEST,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2 as MAX_RESPONSE,
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
    CompilerExecutionClientErrorV2,
    CompilerExecutionClientStorageV2,
    CompilerExecutionClientV2,
    CompilerExecutionReceiptRecoveryV2,
    verify_native
);

crate::inherited_admission_adapter::inherited_admission_adapter!(
    CompilerExecutionClientV2,
    CompilerExecutionClientErrorV2
);

#[cfg(test)]
use CompilerExecutionClientV2 as TestClient;
#[cfg(test)]
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceResponsePayloadV2 as Reply;
#[cfg(test)]
#[path = "native_admission_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "native_preparation_tests.rs"]
mod preparation_tests;
