//! Native source-bearing input to the existing external monotonic-anchor protocol.
//!
//! The V3 family consumes actual V3 owners, without a V2 decode or authority upgrade.
//! ```
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV3 as P,
//!     CompilerExecutionAttestationRequestV3 as Q, CompilerExecutionReceiptPublicationV3 as U,
//!     CompilerExecutionExternalAnchorTransactionV3 as T,
//!     CompilerExecutionNativeJournalErrorV3 as E, CompilerExecutionAttestationStorageV3 as S};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn transfer(p: P, q: Q, u: U, b: &mut B<'_>) -> Result<(T, S), E> {
//!     T::new(p, q, u, b)
//! }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV2 as P,
//!     CompilerExecutionAttestationRequestV3 as Q, CompilerExecutionReceiptPublicationV3 as U,
//!     CompilerExecutionExternalAnchorTransactionV3 as T};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(p: P, q: Q, u: U, b: &mut B<'_>) { let _ = T::new(p, q, u, b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorTransactionV3 as T;
//! fn reuse(value: T) { let moved = value; let _ = (moved, value); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorTransactionV3 as T3,
//!     CompilerExecutionExternalAnchorTransactionV2 as T2};
//! fn downgrade(value: T3) -> T2 { value.into() }
//! ```
use crate::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V3 as Q,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3 as P,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V3 as U,
    CompilerExecutionAttestationErrorV3 as AttestationError,
    CompilerExecutionAttestationRequestV3 as Request, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionReceiptPublicationErrorV3 as PublicationError,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServiceProtocolErrorV1 as Framing,
    attestation_resources::CompilerExecutionAttestationStorageV3 as Storage,
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

const VERSION: u16 = 3;
const MAGIC: [u8; 8] = *b"F2O3CAT3";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-TRANSACTION/V3\0";

crate::external_anchor_transaction_adapter::external_anchor_transaction_adapter!(
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V3,
    CompilerExecutionExternalAnchorTransactionIdentityV3,
    CompilerExecutionExternalAnchorTransactionV3,
    CompilerExecutionNativeJournalErrorV3
);
