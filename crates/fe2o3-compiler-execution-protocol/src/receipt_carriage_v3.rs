//! Complete native carriage with same-ledger nested decoding and exact joins.
//!
//! The complete constructor consumes the four actual V3 owners:
//! ```
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV3 as P,
//!     CompilerExecutionAttestationRequestV3 as Q, CompilerExecutionReceiptPublicationV3 as U,
//!     CompilerExecutionReceiptPublicationAckV3 as A, CompilerExecutionReceiptCarriageV3 as C,
//!     CompilerExecutionReceiptPublicationErrorV3 as E, CompilerExecutionAttestationStorageV3 as S};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn transfer(p: P, q: Q, u: U, a: A, b: &mut B<'_>) -> Result<(C, S), E> {
//!     C::new(p, q, u, a, b)
//! }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV2 as P,
//!     CompilerExecutionAttestationRequestV3 as Q, CompilerExecutionReceiptPublicationV3 as U,
//!     CompilerExecutionReceiptPublicationAckV3 as A, CompilerExecutionReceiptCarriageV3 as C};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(p: P, q: Q, u: U, a: A, b: &mut B<'_>) { let _ = C::new(p, q, u, a, b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV3 as P,
//!     CompilerExecutionAttestationRequestV3 as Q, CompilerExecutionReceiptPublicationV3 as U,
//!     CompilerExecutionReceiptPublicationAckV2 as A, CompilerExecutionReceiptCarriageV3 as C};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(p: P, q: Q, u: U, a: A, b: &mut B<'_>) { let _ = C::new(p, q, u, a, b); }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionReceiptCarriageV3 as C3,
//!     CompilerExecutionReceiptCarriageV2 as C2};
//! fn downgrade(value: C3) -> C2 { value.into() }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV3 as C;
//! fn reuse(value: C) { let moved = value; let _ = (moved, value); }
//! ```
use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3 as VS,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V3 as VW,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V3 as QS,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3 as QW,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3 as PS,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as PW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V3 as AS,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V3 as AW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V3 as US,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V3 as UW,
    CompilerExecutionAttestationRequestV3 as Request, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
    attestation_request_v3::RETAINED as REQUEST_RETAINED,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    issuer_policy_v3::RETAINED as POLICY_RETAINED,
    receipt_publication_codec as codec,
    receipt_publication_v3::{
        ACK_RETAINED, CompilerExecutionReceiptPublicationErrorV3 as Error, PUBLICATION_RETAINED,
        Result,
    },
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::receipt_carriage_adapter::receipt_carriage_adapter!(
    V3,
    "3",
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V3,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V3,
    CompilerExecutionReceiptCarriageIdentityV3,
    CompilerExecutionReceiptCarriageV3
);
