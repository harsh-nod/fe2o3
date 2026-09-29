//! A live native observation joined to the exact, still-locked V5 publication.
use super::ExpectedCompilerExecutionPublicationV1 as Expected;
use crate::{
    ProtectedServiceAdmissionV2 as Service,
    compiler_execution_supervision::{NativeObservation, NativeObservationError},
};
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV3 as SubjectError,
    CompilerModuleHandoffConsumptionTokenV5 as Token,
    CompilerModuleHandoffCurrentnessLeaseV5 as Lease, CompilerModuleHandoffErrorV5 as HandoffError,
    CompilerModuleHandoffReceiptV5 as Receipt,
    InertCompilerExecutionSubjectStorageV3 as SubjectStorage,
    InertCompilerExecutionSubjectV3 as Subject,
    acquire_compiler_module_handoff_currentness_lease_v5 as acquire,
    recover_compiler_module_handoff_receipt_v5 as recover,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

const OCCURRENCE_DOMAIN: &[u8] = b"FE2O3/PROTECTED-COMPILER-EXECUTION-OCCURRENCE/V3\0";

fn published_invocation(token: &Token) -> &fe2o3_rustc_invocation::RustcInvocationDescriptorV3 {
    token.handoff().capsule().invocation()
}

include!("compiler_execution_occurrence_native_body.rs");

#[path = "compiler_execution_root_publication.rs"]
mod root_publication;
pub use root_publication::{RootPublicationCustodyErrorV3, RootPublicationCustodyV3};
