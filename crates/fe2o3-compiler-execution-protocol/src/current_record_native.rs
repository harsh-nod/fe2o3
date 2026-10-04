//! Typed native-carriage authentication over the shared identity-only V3 wire.
//! No admitted V1 policy/carriage is constructed and no protected authority is minted.
use super::*;
use crate::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2 as CARRIAGE_BYTES,
    CompilerExecutionExternalAnchorTransactionV2 as Transaction,
    CompilerExecutionIssuerPolicyV2 as NativePolicy,
    CompilerExecutionNativeJournalErrorV2 as NativeError,
    CompilerExecutionReceiptCarriageV2 as NativeCarriage,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    external_anchor_transaction_v2::Result,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::mem::size_of;

crate::current_record_native_adapter::current_record_native_adapter!(
    new_native,
    external_anchor_currentness_challenge_native,
    verify_expected_native,
    issue_native,
    verify_native
);
