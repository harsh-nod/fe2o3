//! Native publication claims. Durability must be independently established.
//!
//! A conditional signed receipt transfers only into its own publication family:
//! ```
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationReceiptV3 as R,
//!     CompilerExecutionReceiptPublicationV3 as P, CompilerExecutionReceiptPublicationAckV3 as A,
//!     CompilerExecutionReceiptPublicationErrorV3 as E, CompilerExecutionAttestationStorageV3 as S};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn publication(r: R, b: &mut B<'_>) -> Result<(P, S), E> {
//!     P::new([1; 32], [2; 32], r, b)
//! }
//! fn acknowledgment(p: &P, b: &mut B<'_>) -> Result<(A, S), E> {
//!     A::new(p, [3; 32], b)
//! }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationReceiptV2 as R,
//!     CompilerExecutionReceiptPublicationV3 as P};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(r: R, b: &mut B<'_>) { let _ = P::new([1; 32], [2; 32], r, b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationReceiptV3 as R,
//!     CompilerExecutionReceiptPublicationV2 as P};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(r: R, b: &mut B<'_>) { let _ = P::new([1; 32], [2; 32], r, b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionReceiptPublicationV2 as P,
//!     CompilerExecutionReceiptPublicationAckV3 as A};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(p: &P, b: &mut B<'_>) { let _ = A::new(p, [3; 32], b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionReceiptPublicationV3 as P,
//!     CompilerExecutionReceiptPublicationAckV2 as A};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(p: &P, b: &mut B<'_>) { let _ = A::new(p, [3; 32], b); }
//! ```
use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V3 as RECEIPT_WORK,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3 as RECEIPT_STORAGE,
    CompilerExecutionAttestationErrorV3 as AttestationError,
    CompilerExecutionAttestationReceiptIdentityV3 as ReceiptIdentity,
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionIssuerPolicyIdentityV3 as PolicyIdentity,
    CompilerExecutionReceiptPublicationErrorV1 as Framing,
    attestation_receipt_v3::RETAINED as RECEIPT_RETAINED,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    receipt_publication_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error as StdError, fmt, mem::size_of};

crate::receipt_publication_adapter::receipt_publication_adapter!(
    V3,
    "3",
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V3,
    CompilerExecutionReceiptPublicationIdentityV3,
    CompilerExecutionReceiptPublicationAckIdentityV3,
    CompilerExecutionReceiptPublicationV3,
    CompilerExecutionReceiptPublicationAckV3,
    CompilerExecutionReceiptPublicationErrorV3
);
