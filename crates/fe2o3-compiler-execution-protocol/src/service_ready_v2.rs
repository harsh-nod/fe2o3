//! Native ownership and metering over the existing inert readiness wire.
use crate::{
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestIdentityV1 as ManifestIdentity,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
    CompilerExecutionServiceReadyErrorV1 as Framing,
    CompilerExecutionServiceReadyIdentityV1 as Identity,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    service_ready_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Move-only inert readiness framing for native consumers. Construction,
/// decoding and matching grant no process, signing, compiler, launch,
/// publication, loading or execution authority, and prove no admission or
/// recovery. Private-channel provenance and child liveness remain supervisor
/// obligations.
///
/// The unchanged 120-byte wire contains opaque identities, so structural
/// decoding also accepts legacy-bound frames. Consumers must match the exact
/// native manifest and independently pinned PolicyV2 before use. There is no
/// conversion from an admitted V1 owner and no fallback policy admission.
///
/// Keep complete borrowed owners prepaid on the original caller ledger.
/// Operations restore entry storage on success, error and unwind. Reserve the
/// returned additional storage before retaining the owner, then release its
/// `retained_storage()` when it is retired.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV2;
/// fn duplicate(value: CompilerExecutionServiceReadyV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV2;
/// fn requires_copy<T: Copy>() {}
/// requires_copy::<CompilerExecutionServiceReadyV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceReadyV1,
///     CompilerExecutionServiceReadyV2};
/// fn legacy(_: &CompilerExecutionServiceReadyV1) {}
/// fn mix(value: &CompilerExecutionServiceReadyV2) { legacy(value); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceReadyV1,
///     CompilerExecutionServiceReadyV2};
/// fn upgrade(value: CompilerExecutionServiceReadyV1) -> CompilerExecutionServiceReadyV2 {
///     value.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionServiceReadyV1,
///     CompilerExecutionServiceReadyV2};
/// fn downgrade(value: CompilerExecutionServiceReadyV2) -> CompilerExecutionServiceReadyV1 {
///     value.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV1,
///     CompilerExecutionServiceLaunchManifestV2, CompilerExecutionServiceReadyV2};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(m: &CompilerExecutionServiceLaunchManifestV2,
///        p: &CompilerExecutionIssuerPolicyV1, b: &mut Budget<'_>) {
///     let _ = CompilerExecutionServiceReadyV2::new(1, m, p, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV2,
///     CompilerExecutionServiceLaunchManifestV1, CompilerExecutionServiceReadyV2};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(m: &CompilerExecutionServiceLaunchManifestV1,
///        p: &CompilerExecutionIssuerPolicyV2, b: &mut Budget<'_>) {
///     let _ = CompilerExecutionServiceReadyV2::new(1, m, p, b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct CompilerExecutionServiceReadyV2 {
    record: codec::Record,
}

crate::service_ready_adapter::service_ready_adapter!(
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2,
    COMPILER_EXECUTION_SERVICE_READY_WORK_V2,
    COMPILER_EXECUTION_SERVICE_READY_STORAGE_V2,
    CompilerExecutionServiceReadyV2,
    CompilerExecutionServiceReadyErrorV2
);

#[cfg(test)]
#[path = "service_ready_v2_tests.rs"]
mod tests;
