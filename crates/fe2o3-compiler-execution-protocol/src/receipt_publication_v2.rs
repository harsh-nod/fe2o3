//! Native publication claims. Durability must be independently established.
use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2 as RECEIPT_WORK,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2 as RECEIPT_STORAGE,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationReceiptIdentityV2 as ReceiptIdentity,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionReceiptPublicationErrorV1 as Framing,
    attestation_receipt_v2::RETAINED as RECEIPT_RETAINED,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    receipt_publication_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error as StdError, fmt, mem::size_of};

crate::receipt_publication_adapter::receipt_publication_adapter!(
    V2,
    "2",
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2,
    CompilerExecutionReceiptPublicationIdentityV2,
    CompilerExecutionReceiptPublicationAckIdentityV2,
    CompilerExecutionReceiptPublicationV2,
    CompilerExecutionReceiptPublicationAckV2,
    CompilerExecutionReceiptPublicationErrorV2
);
