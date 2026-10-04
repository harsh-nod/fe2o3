//! Native service packets, not another service engine or an authority boundary.
//!
//! V2 has explicit framing and identity domains, with no V1 retry. Operations
//! retain the V1 state-machine meanings. Each packet owns only its canonical
//! bytes: construction borrows prepaid leaves; extraction explicitly re-decodes
//! them on the caller's ledger. There are no hidden clones or signing APIs.
//!
//! Current-record V3 is an identity-only wire family and is carried unchanged.
//! Native current-record joins use the explicitly metered native-carriage
//! constructor/authenticator. Packet decoding alone does not establish protected
//! journal custody or independently administered anchor deployment.

use crate::{
    CompilerExecutionAttestationChallengeV2 as Challenge,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionCurrentRecordAttestationV3 as Current,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationErrorV2 as PublicationError,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServiceProtocolErrorV1 as Framing,
    CompilerExecutionServicePublishDispositionV1 as Disposition,
    attestation_challenge_v2 as challenge, attestation_request_v2 as request,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    service::{Reader, decode_versioned_header, derive_identity, encode_versioned_header, put},
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV2 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2 as SUBJECT_BYTES,
    INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2 as SUBJECT_STORAGE,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error as StdError, fmt, mem::size_of};

use crate::attestation_challenge_v2::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V2 as LEAF_ATTESTATION_CHALLENGE_BYTES;
use crate::attestation_challenge_v2::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2 as LEAF_ATTESTATION_CHALLENGE_STORAGE;
use crate::attestation_challenge_v2::COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2 as LEAF_ATTESTATION_CHALLENGE_WORK;
use crate::attestation_receipt_v2::COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2 as LEAF_ATTESTATION_RECEIPT_STORAGE;
use crate::attestation_receipt_v2::COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2 as LEAF_ATTESTATION_RECEIPT_VERIFY_WORK;
use crate::attestation_request_v2::COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2 as LEAF_ATTESTATION_REQUEST_BYTES;
use crate::attestation_request_v2::COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2 as LEAF_ATTESTATION_REQUEST_DECODE_STORAGE;
use crate::attestation_request_v2::COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2 as LEAF_ATTESTATION_REQUEST_DECODE_WORK;
use crate::receipt_carriage_v2::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2 as LEAF_RECEIPT_CARRIAGE_BYTES;
use crate::receipt_carriage_v2::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2 as LEAF_RECEIPT_CARRIAGE_DECODE_STORAGE;
use crate::receipt_carriage_v2::COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2 as LEAF_RECEIPT_CARRIAGE_DECODE_WORK;
use crate::receipt_publication_v2::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2 as LEAF_RECEIPT_PUBLICATION_ACK_BYTES;
use crate::receipt_publication_v2::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2 as LEAF_RECEIPT_PUBLICATION_ACK_STORAGE;
use crate::receipt_publication_v2::COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2 as LEAF_RECEIPT_PUBLICATION_ACK_WORK;
use crate::receipt_publication_v2::COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2 as LEAF_RECEIPT_PUBLICATION_BYTES;
use crate::receipt_publication_v2::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2 as LEAF_RECEIPT_PUBLICATION_DECODE_STORAGE;
use crate::receipt_publication_v2::COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2 as LEAF_RECEIPT_PUBLICATION_DECODE_WORK;

const VERSION: u16 = 2;
const REQUEST_MAGIC: [u8; 8] = *b"F2O3CSQ2";
const RESPONSE_MAGIC: [u8; 8] = *b"F2O3CSP2";
const REQUEST_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-REQUEST/V2\0";
const RESPONSE_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-RESPONSE/V2\0";

crate::service_native_adapter::service_native_adapter!(
    COMPILER_EXECUTION_SERVICE_CONTROL_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PREPARE_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_ISSUE_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_RECOVER_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_VERIFY_CURRENT_REQUEST_BYTES_V2,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_CONTROL_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PREPARED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_ISSUED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISHED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_RECOVERED_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_VERIFIED_CURRENT_RESPONSE_BYTES_V2,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_REQUEST_WORK_V2,
    COMPILER_EXECUTION_SERVICE_RESPONSE_WORK_V2,
    COMPILER_EXECUTION_SERVICE_REQUEST_STORAGE_V2,
    COMPILER_EXECUTION_SERVICE_RESPONSE_STORAGE_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V2,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V2,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V2,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V2,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V2,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V2,
    CompilerExecutionServiceRequestIdentityV2,
    CompilerExecutionServiceResponseIdentityV2,
    CompilerExecutionServiceRequestKindV2,
    CompilerExecutionServiceResponseKindV2,
    CompilerExecutionServiceRequestPayloadV2,
    CompilerExecutionServiceResponsePayloadV2,
    CompilerExecutionServiceRequestV2,
    CompilerExecutionServiceResponseV2,
    CompilerExecutionServiceProtocolErrorV2
);

#[cfg(test)]
#[path = "service_native_tests.rs"]
mod tests;

#[cfg(test)]
use self::{
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V2 as PUBLISH_REQUEST_STORAGE,
    COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V2 as PUBLISH_REQUEST_WORK,
    CompilerExecutionServiceRequestIdentityV2 as RequestIdentity,
    CompilerExecutionServiceRequestV2 as ServiceRequest,
    CompilerExecutionServiceResponseV2 as ServiceResponse,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V2 as MAX_REQUEST_DECODE_STORAGE,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V2 as MAX_REQUEST_DECODE_WORK,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V2 as MAX_RESPONSE_DECODE_STORAGE,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V2 as MAX_RESPONSE_DECODE_WORK,
};
#[cfg(test)]
use crate::CompilerExecutionAttestationReceiptV2 as Receipt;
