//! Complete native carriage with same-ledger nested decoding and exact joins.
use crate::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2 as VS,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2 as VW,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2 as QS,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2 as QW,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2 as PS,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as PW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2 as AS,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2 as AW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2 as US,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2 as UW,
    CompilerExecutionAttestationRequestV2 as Request, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationV2 as Publication,
    attestation_request_v2::RETAINED as REQUEST_RETAINED,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    issuer_policy_v2::RETAINED as POLICY_RETAINED,
    receipt_publication_codec as codec,
    receipt_publication_v2::{
        ACK_RETAINED, CompilerExecutionReceiptPublicationErrorV2 as Error, PUBLICATION_RETAINED,
        Result,
    },
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fmt, mem::size_of};

crate::receipt_carriage_adapter::receipt_carriage_adapter!(
    V2,
    "2",
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V2,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2,
    CompilerExecutionReceiptCarriageIdentityV2,
    CompilerExecutionReceiptCarriageV2
);
