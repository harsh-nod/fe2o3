//! Native signed receipts. Signature validity is not protected execution authority.
use crate::{
    CompilerExecutionAttestationChallengeIdentityV2 as ChallengeIdentity,
    CompilerExecutionAttestationErrorV1 as Framing, CompilerExecutionAttestationErrorV2 as Error,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionSubjectBindingV2 as Binding,
    attestation_receipt_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::attestation_receipt_adapter::attestation_receipt_adapter!(
    V2,
    "2",
    "4",
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2,
    CompilerExecutionAttestationReceiptIdentityV2,
    CompilerExecutionAttestationReceiptV2,
    VerifiedCompilerExecutionAttestationV2
);
