//! Native signed receipts. Signature validity is not protected execution authority.
use crate::{
    CompilerExecutionAttestationChallengeIdentityV3 as ChallengeIdentity,
    CompilerExecutionAttestationErrorV1 as Framing, CompilerExecutionAttestationErrorV3 as Error,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionIssuerPolicyIdentityV3 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionSubjectBindingV3 as Binding,
    attestation_receipt_codec as codec,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
};
use ed25519_dalek::{Signature, SigningKey, VerifyingKey};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::attestation_receipt_adapter::attestation_receipt_adapter!(
    V3,
    "3",
    "5",
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V3,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3,
    CompilerExecutionAttestationReceiptIdentityV3,
    CompilerExecutionAttestationReceiptV3,
    VerifiedCompilerExecutionAttestationV3
);
