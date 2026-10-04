//! A live native observation joined to the exact, still-locked V4 publication.
use super::ExpectedCompilerExecutionPublicationV1 as Expected;
use crate::{
    ProtectedServiceAdmissionV2 as Service,
    compiler_execution_supervision::{NativeObservation, NativeObservationError},
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV2 as SubjectError,
    CompilerModuleHandoffConsumptionTokenV4 as Token,
    CompilerModuleHandoffCurrentnessLeaseV4 as Lease, CompilerModuleHandoffErrorV4 as HandoffError,
    CompilerModuleHandoffReceiptV4 as Receipt,
    InertCompilerExecutionSubjectStorageV2 as SubjectStorage,
    InertCompilerExecutionSubjectV2 as Subject,
    acquire_compiler_module_handoff_currentness_lease_v4 as acquire,
    recover_compiler_module_handoff_receipt_v4 as recover,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

const OCCURRENCE_DOMAIN: &[u8] = b"FE2O3/PROTECTED-COMPILER-EXECUTION-OCCURRENCE/V2\0";

fn published_invocation(token: &Token) -> &fe2o3_rustc_invocation::RustcInvocationDescriptorV3 {
    token.handoff().capsule().base().invocation()
}

include!("compiler_execution_occurrence_native_body.rs");
