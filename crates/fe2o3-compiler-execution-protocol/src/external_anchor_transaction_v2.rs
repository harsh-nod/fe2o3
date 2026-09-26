//! Native source-bearing input to the existing external monotonic-anchor protocol.
use crate::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2 as Q,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2 as P,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2 as U,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationRequestV2 as Request, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionReceiptPublicationErrorV2 as PublicationError,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServiceProtocolErrorV1 as Framing,
    attestation_resources::CompilerExecutionAttestationStorageV2 as Storage,
    external_anchor_transaction::encode_transaction,
    service::{Reader, decode_versioned_header},
};
use fe2o3_external_anchor_protocol::{
    AnchorProtocolErrorV1 as AnchorError, TransactionDigestV1, derive_transaction_digest_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

const VERSION: u16 = 2;
const MAGIC: [u8; 8] = *b"F2O3CAT2";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-TRANSACTION/V2\0";

crate::external_anchor_transaction_adapter::external_anchor_transaction_adapter!(
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2,
    CompilerExecutionExternalAnchorTransactionIdentityV2,
    CompilerExecutionExternalAnchorTransactionV2,
    CompilerExecutionNativeJournalErrorV2
);
