//! Consuming native issuer session with durable Worker/external-anchor joins.
use super::{Admission, Budget, Key, KeyError, Policy, Resource};
use crate::compiler_execution_external_anchor::NativeAnchor;
use crate::compiler_execution_occurrence::NativeOccurrence;
use crate::compiler_execution_service::{
    COMPILER_EXECUTION_SERVICE_SESSION_TIMEOUT_V1, MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1,
    receive_packet_metered, send_packet_metered,
};
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2 as Subject;
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2 as Manifest;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV2 as Challenge,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionAttestationStorageV2 as ProtocolStorage,
    CompilerExecutionCurrentRecordVerificationV3 as Current,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServicePublishDispositionV1 as Disposition,
    CompilerExecutionServiceRequestKindV2 as Kind, CompilerExecutionServiceRequestV2 as Packet,
    CompilerExecutionServiceResponsePayloadV2 as Payload,
    CompilerExecutionServiceResponseV2 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2 as REQUEST_BYTES,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2 as RESPONSE_BYTES,
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
#[path = "compiler_execution_worker_ledger_native.rs"]
mod worker;
pub use error::NativeIssuerServiceError;
use error::NativeIssuerServiceError as Error;
type Result<T> = std::result::Result<T, Error>;
use ledger::Ledger;
use record::{Body, Record};

use crate::compiler_execution_occurrence::NativeOccurrenceError as OccurrenceError;
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV2 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2 as SUBJECT_BYTES,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V2 as CHALLENGE_BYTES,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V2 as RECEIPT_BYTES,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2 as ATTESTATION_REQUEST_BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2 as TRANSACTION_BYTES,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2 as ACK_BYTES,
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2 as ANCHOR_JOURNAL_BYTES,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionExternalAnchorTransactionV2 as Transaction,
    CompilerExecutionNativeJournalErrorV2 as JournalError,
    CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionReceiptPublicationErrorV2 as PublicationError,
    CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError,
    CompilerExecutionServiceProtocolErrorV2 as ProtocolError,
    CompilerExecutionServiceReadyErrorV2 as ReadyError, CompilerExecutionServiceReadyV2 as Ready,
    CompilerExecutionWorkerAnchorJournalErrorV2 as WorkerJournalError,
    CompilerExecutionWorkerAnchorJournalV2 as AnchorJournal,
};

const JOURNAL_VERSION: u16 = 3;
const ISSUER_MAGIC: &[u8; 8] = b"F2O3CEJ3";
const ISSUER_SIGN_DOMAIN: &[u8] = b"FE2O3/NATIVE-ISSUER-JOURNAL-SIGNATURE/V3\0";
const ISSUER_ID_DOMAIN: &[u8] = b"FE2O3/NATIVE-ISSUER-JOURNAL-IDENTITY/V3\0";
const WORKER_MAGIC: &[u8; 8] = b"F2O3CEW3";
const WORKER_DOMAIN: &[u8] = b"FE2O3/NATIVE-WORKER-RECORD/V3\0";
const POLICY_JOIN_DOMAIN: &[u8] = b"FE2O3/NATIVE-PROTECTED-POLICY-VERIFICATION/V2\0";
const WORKER_JOIN_DOMAIN: &[u8] = b"FE2O3/NATIVE-PROTECTED-WORKER-VERIFICATION/V2\0";
#[cfg(test)]
const SUBJECT_VERSION: u16 = 2;
#[cfg(test)]
const ACK_MAGIC: &[u8; 8] = b"F2O3CEA2";
#[cfg(test)]
const ACK_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V2\0";

include!("compiler_execution_issuer_native_service_body.rs");

#[cfg(test)]
fn verify_test_current(
    attestation: fe2o3_compiler_execution_protocol::CompilerExecutionCurrentRecordAttestationV3,
    policy: &Policy,
    carriage: &Carriage,
    challenge: [u8; 32],
    b: &mut Budget<'_>,
) -> Result<()> {
    attestation.verify_native(policy, carriage, challenge, b)?;
    Ok(())
}
