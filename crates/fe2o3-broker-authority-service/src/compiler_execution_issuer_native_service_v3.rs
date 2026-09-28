//! Consuming native issuer session with durable Worker/external-anchor joins.
use super::{Admission, Budget, Key, KeyError, Policy, Resource};
use crate::compiler_execution_external_anchor::NativeAnchorV3 as NativeAnchor;
use crate::compiler_execution_occurrence::NativeOccurrenceV3 as NativeOccurrence;
use crate::compiler_execution_service::{
    COMPILER_EXECUTION_SERVICE_SESSION_TIMEOUT_V1, MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1,
    receive_packet_metered, send_packet_metered,
};
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV3 as Manifest;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV3 as Challenge,
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionAttestationStorageV3 as ProtocolStorage,
    CompilerExecutionCurrentRecordVerificationV3 as Current,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServicePublishDispositionV1 as Disposition,
    CompilerExecutionServiceRequestKindV3 as Kind, CompilerExecutionServiceRequestV3 as Packet,
    CompilerExecutionServiceResponsePayloadV3 as Payload,
    CompilerExecutionServiceResponseV3 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V3 as REQUEST_BYTES,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V3 as RESPONSE_BYTES,
};
use std::{os::fd::OwnedFd, time::Instant};

#[path = "compiler_execution_issuer_native_readiness.rs"]
mod readiness;

#[path = "compiler_execution_issuer_native_error.rs"]
mod error;
#[path = "compiler_execution_issuer_native_ledger.rs"]
mod ledger;
#[path = "compiler_execution_issuer_native_record.rs"]
mod record;
#[path = "compiler_execution_worker_ledger_native_v3.rs"]
mod worker;
pub use error::NativeIssuerServiceError;
use error::NativeIssuerServiceError as Error;
type Result<T> = std::result::Result<T, Error>;
use ledger::Ledger;
use record::{Body, Record};

use crate::compiler_execution_occurrence::NativeOccurrenceErrorV3 as OccurrenceError;
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV3 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V3 as SUBJECT_BYTES,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V3 as CHALLENGE_BYTES,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V3 as RECEIPT_BYTES,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V3 as ATTESTATION_REQUEST_BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V3 as TRANSACTION_BYTES,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V3 as ACK_BYTES,
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V3 as ANCHOR_JOURNAL_BYTES,
    CompilerExecutionAttestationErrorV3 as AttestationError,
    CompilerExecutionExternalAnchorTransactionV3 as Transaction,
    CompilerExecutionNativeJournalErrorV3 as JournalError,
    CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionReceiptPublicationErrorV3 as PublicationError,
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestError,
    CompilerExecutionServiceProtocolErrorV3 as ProtocolError,
    CompilerExecutionServiceReadyErrorV3 as ReadyError, CompilerExecutionServiceReadyV3 as Ready,
    CompilerExecutionWorkerAnchorJournalErrorV3 as WorkerJournalError,
    CompilerExecutionWorkerAnchorJournalV3 as AnchorJournal,
};

const JOURNAL_VERSION: u16 = 4;
const ISSUER_MAGIC: &[u8; 8] = b"F2O3CEJ4";
const ISSUER_SIGN_DOMAIN: &[u8] = b"FE2O3/NATIVE-ISSUER-JOURNAL-SIGNATURE/V4\0";
const ISSUER_ID_DOMAIN: &[u8] = b"FE2O3/NATIVE-ISSUER-JOURNAL-IDENTITY/V4\0";
const WORKER_MAGIC: &[u8; 8] = b"F2O3CEW4";
const WORKER_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-RECORD/V4\0";
const POLICY_JOIN_DOMAIN: &[u8] = b"FE2O3/NATIVE-PROTECTED-POLICY-VERIFICATION/V3\0";
const WORKER_JOIN_DOMAIN: &[u8] = b"FE2O3/NATIVE-PROTECTED-WORKER-VERIFICATION/V3\0";
#[cfg(test)]
const SUBJECT_VERSION: u16 = 3;
#[cfg(test)]
const ACK_MAGIC: &[u8; 8] = b"F2O3CEA3";
#[cfg(test)]
const ACK_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V3\0";

include!("compiler_execution_issuer_native_service_body.rs");

#[cfg(test)]
fn verify_test_current(
    attestation: fe2o3_compiler_execution_protocol::CompilerExecutionCurrentRecordAttestationV3,
    policy: &Policy,
    carriage: &Carriage,
    challenge: [u8; 32],
    b: &mut Budget<'_>,
) -> Result<()> {
    attestation.verify_native_v3(policy, carriage, challenge, b)?;
    Ok(())
}
