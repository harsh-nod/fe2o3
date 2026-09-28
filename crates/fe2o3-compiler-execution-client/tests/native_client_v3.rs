//! Public native client transcripts. Fixture keys are diagnostic only: these
//! tests do not establish protected service custody, journal durability or GPU authority.
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
use fe2o3_compiler_execution_client::{
    CompilerExecutionClientErrorV3 as Error, CompilerExecutionClientV3 as Client,
    CompilerExecutionReceiptRecoveryV3 as Recovery,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionAttestationStorageV3 as Storage,
    CompilerExecutionCurrentRecordAttestationV3 as Current,
    CompilerExecutionCurrentRecordVerificationV3 as Verification,
    CompilerExecutionExternalAnchorTransactionV3 as Transaction,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServicePublishDispositionV1 as Disposition,
    CompilerExecutionServiceRequestKindV3 as QueryKind,
    CompilerExecutionServiceRequestPayloadV3 as QueryPayload,
    CompilerExecutionServiceRequestV3 as Query, CompilerExecutionServiceResponsePayloadV3 as Reply,
    CompilerExecutionServiceResponseV3 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V3 as MAX_REQUEST,
};
use fe2o3_external_anchor_protocol::{
    AnchorChallengeV1, AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
    HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use rustix::net::{self, AddressFamily, RecvFlags, SendFlags, SocketFlags, SocketType};
use std::{mem::size_of, os::fd::OwnedFd, thread, time::Duration};

#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;

#[path = "support/native_client_transcripts.rs"]
mod transcripts;
transcripts::native_client_transcripts!(
    3,
    external_anchor_currentness_challenge_native_v3,
    new_native_v3,
    issue_native_v3
);
