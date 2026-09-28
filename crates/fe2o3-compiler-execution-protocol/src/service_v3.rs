//! Native service packets, not another service engine or an authority boundary.
//!
//! V3 has explicit framing and identity domains, with no V1/V2 retry. Operations
//! retain the V1 state-machine meanings. Each packet owns only its canonical
//! bytes: construction borrows prepaid leaves; extraction explicitly re-decodes
//! them on the caller's ledger. There are no hidden clones or signing APIs.
//!
//! Current-record V3 is an identity-only wire family and is carried unchanged.
//! Native current-record joins use the explicitly metered native-carriage
//! constructor/authenticator. Packet decoding alone does not establish protected
//! journal custody or independently administered anchor deployment.
//!
//! The service family accepts actual V3 leaves, not a decoded-family upgrade:
//! ```
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV3 as P,
//!     CompilerExecutionAttestationRequestV3 as Q, CompilerExecutionServiceRequestV3 as S,
//!     CompilerExecutionServiceRequestPayloadV3 as Payload,
//!     CompilerExecutionServiceProtocolErrorV3 as E, CompilerExecutionAttestationStorageV3 as C};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn issue(p: &P, q: &Q, b: &mut B<'_>) -> Result<(S, C), E> {
//!     S::new(p, Payload::Issue(q), b)
//! }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV2 as P,
//!     CompilerExecutionServiceRequestV3 as S, CompilerExecutionServiceRequestPayloadV3 as Payload};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(p: &P, b: &mut B<'_>) { let _ = S::new(p, Payload::Inspect, b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationRequestV2 as Q,
//!     CompilerExecutionServiceRequestPayloadV3 as Payload};
//! fn mix(q: &Q) { let _ = Payload::Issue(q); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceRequestV3 as S3,
//!     CompilerExecutionServiceRequestV2 as S2};
//! fn downgrade(packet: S3) -> S2 { packet.into() }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::CompilerExecutionServiceRequestV3 as S;
//! fn reuse(packet: S) { let moved = packet; let _ = (moved, packet); }
//! ```

use crate::{
    CompilerExecutionAttestationChallengeV3 as Challenge,
    CompilerExecutionAttestationErrorV3 as AttestationError,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionCurrentRecordAttestationV3 as Current,
    CompilerExecutionIssuerPolicyIdentityV3 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationErrorV3 as PublicationError,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServiceProtocolErrorV1 as Framing,
    CompilerExecutionServicePublishDispositionV1 as Disposition,
    attestation_challenge_v3 as challenge, attestation_request_v3 as request,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    service::{Reader, decode_versioned_header, derive_identity, encode_versioned_header, put},
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV3 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V3 as SUBJECT_BYTES,
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V3 as SUBJECT_STORAGE,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V3 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV3 as Subject,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error as StdError, fmt, mem::size_of};

use crate::attestation_challenge_v3::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V3 as LEAF_ATTESTATION_CHALLENGE_BYTES;
use crate::attestation_challenge_v3::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3 as LEAF_ATTESTATION_CHALLENGE_STORAGE;
use crate::attestation_challenge_v3::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3 as LEAF_ATTESTATION_CHALLENGE_WORK;
use crate::attestation_receipt_v3::COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3 as LEAF_ATTESTATION_RECEIPT_STORAGE;
use crate::attestation_receipt_v3::COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V3 as LEAF_ATTESTATION_RECEIPT_VERIFY_WORK;
use crate::attestation_request_v3::COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V3 as LEAF_ATTESTATION_REQUEST_BYTES;
use crate::attestation_request_v3::COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V3 as LEAF_ATTESTATION_REQUEST_DECODE_STORAGE;
use crate::attestation_request_v3::COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3 as LEAF_ATTESTATION_REQUEST_DECODE_WORK;
use crate::receipt_carriage_v3::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3 as LEAF_RECEIPT_CARRIAGE_BYTES;
use crate::receipt_carriage_v3::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V3 as LEAF_RECEIPT_CARRIAGE_DECODE_STORAGE;
use crate::receipt_carriage_v3::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V3 as LEAF_RECEIPT_CARRIAGE_DECODE_WORK;
use crate::receipt_publication_v3::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V3 as LEAF_RECEIPT_PUBLICATION_ACK_BYTES;
use crate::receipt_publication_v3::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V3 as LEAF_RECEIPT_PUBLICATION_ACK_STORAGE;
use crate::receipt_publication_v3::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V3 as LEAF_RECEIPT_PUBLICATION_ACK_WORK;
use crate::receipt_publication_v3::COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V3 as LEAF_RECEIPT_PUBLICATION_BYTES;
use crate::receipt_publication_v3::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V3 as LEAF_RECEIPT_PUBLICATION_DECODE_STORAGE;
use crate::receipt_publication_v3::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V3 as LEAF_RECEIPT_PUBLICATION_DECODE_WORK;

const VERSION: u16 = 3;
const REQUEST_MAGIC: [u8; 8] = *b"F2O3CSQ3";
const RESPONSE_MAGIC: [u8; 8] = *b"F2O3CSP3";
const REQUEST_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-REQUEST/V3\0";
const RESPONSE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-RESPONSE/V3\0";

crate::service_native_adapter::service_native_adapter!(
    COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V3,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V3,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V3,
    COMPILER_EXECUTION_SERVICE_REQUEST_WORK_V3,
    COMPILER_EXECUTION_SERVICE_RESPONSE_WORK_V3,
    COMPILER_EXECUTION_SERVICE_REQUEST_STORAGE_V3,
    COMPILER_EXECUTION_SERVICE_RESPONSE_STORAGE_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V3,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V3,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V3,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V3,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V3,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V3,
    CompilerExecutionServiceRequestIdentityV3,
    CompilerExecutionServiceResponseIdentityV3,
    CompilerExecutionServiceRequestKindV3,
    CompilerExecutionServiceResponseKindV3,
    CompilerExecutionServiceRequestPayloadV3,
    CompilerExecutionServiceResponsePayloadV3,
    CompilerExecutionServiceRequestV3,
    CompilerExecutionServiceResponseV3,
    CompilerExecutionServiceProtocolErrorV3
);

#[cfg(test)]
#[path = "service_native_tests.rs"]
mod tests;

#[cfg(test)]
use self::{
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V3 as PUBLISH_REQUEST_STORAGE,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V3 as PUBLISH_REQUEST_WORK,
    CompilerExecutionServiceRequestIdentityV3 as RequestIdentity,
    CompilerExecutionServiceRequestV3 as ServiceRequest,
    CompilerExecutionServiceResponseV3 as ServiceResponse,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V3 as MAX_REQUEST_DECODE_STORAGE,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V3 as MAX_REQUEST_DECODE_WORK,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V3 as MAX_RESPONSE_DECODE_STORAGE,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V3 as MAX_RESPONSE_DECODE_WORK,
};
#[cfg(test)]
use crate::CompilerExecutionAttestationReceiptV3 as Receipt;
