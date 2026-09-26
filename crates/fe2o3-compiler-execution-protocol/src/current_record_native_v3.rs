//! Typed native-carriage authentication over the shared identity-only V3 wire.
//! No admitted V1 policy/carriage is constructed and no protected authority is minted.
//!
//! Current-record V3 is an unchanged identity-only wire family. These methods
//! bind it to conditional policy/carriage V3, not to protected service custody.
//! ```
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionCurrentRecordAttestationV3 as A,
//!     CompilerExecutionIssuerPolicyV3 as P, CompilerExecutionReceiptCarriageV3 as C,
//!     CompilerExecutionNativeJournalErrorV3 as E, CompilerExecutionAttestationStorageV3 as S,
//!     VerifiedCompilerExecutionCurrentRecordV3 as V};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn verify(a: A, p: &P, c: &C, b: &mut B<'_>) -> Result<(V, S), E> {
//!     a.verify_native_v3(p, c, [7; 32], b)
//! }
//! ```
//! ```compile_fail
//! use fe2o3_compiler_execution_protocol::{CompilerExecutionCurrentRecordAttestationV3 as A,
//!     CompilerExecutionIssuerPolicyV3 as P, CompilerExecutionReceiptCarriageV2 as C};
//! use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
//! fn mix(a: A, p: &P, c: &C, b: &mut B<'_>) { let _ = a.verify_native_v3(p, c, [7; 32], b); }
//! ```
use super::*;
use crate::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3 as CARRIAGE_BYTES,
    CompilerExecutionExternalAnchorTransactionV3 as Transaction,
    CompilerExecutionIssuerPolicyV3 as NativePolicy,
    CompilerExecutionNativeJournalErrorV3 as NativeError,
    CompilerExecutionReceiptCarriageV3 as NativeCarriage,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    external_anchor_transaction_v3::Result,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::mem::size_of;

crate::current_record_native_adapter::current_record_native_adapter!(
    new_native_v3,
    external_anchor_currentness_challenge_native_v3,
    verify_expected_native_v3,
    issue_native_v3,
    verify_native_v3
);
