//! Native issuer custody admission and its bounded durable service consumer.
use super::{
    CurrentStaticIssuerMeasurementsV1 as Measurements, FileSnapshotV1 as Snapshot,
    IssuerAdmissionErrorKindV1 as Kind, ProtectedIssuerProcessV1 as Process,
    RetainedStaticIssuerExecutableV1 as Executable, native_checks::IssuerInspectionError,
    require_close_on_exec, validate_executable_snapshot,
};
use crate::{
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
    ProtectedExternalAnchorServiceErrorV2 as AnchorError,
    ProtectedServiceAdmissionErrorV2 as ServiceError, ProtectedServiceAdmissionV2 as Service,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as KeyError, CompilerExecutionSigningKeyCapabilityV2 as Key,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_runtime_protocol::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionIssuerMeasurementV1,
    SEALED_STATIC_APPLICATION_WORKSPACE_BYTES_V1, SealedStaticApplicationErrorV1,
    sealed_static_application_identity_v1, sealed_static_application_work_bound_v1,
    sealed_static_issuer_runtime_measurement_v1,
};
use sha2::{Digest, Sha256};
use std::{fmt, fs::File, marker::PhantomData, mem::size_of};

#[path = "compiler_execution_issuer_native_service.rs"]
pub(super) mod service;
pub use service::NativeIssuerServiceError as ProtectedCompilerExecutionIssuerServiceErrorV2;

super::native_adapter::issuer_native_adapter!(
/// Protected process, static executable and exact native key/policy/transport custody.
///
/// This consumes native owners without legacy conversion or retry. Process and
/// executable predicates are shared with V1; image reads are bounded positional
/// reads and refuse short reads/EINTR instead of retrying. The key stays inside
/// its existing native capability: no seed is read or duplicated here.
///
/// Admission checks a caller-pinned policy, not its provisioning provenance.
/// The anchor is retained transport custody only: no exchange has authenticated
/// an observation under the policy's anchor key. `serve_native_preparation`
/// consumes this owner into singleton recovery, independently observed issuance,
/// and durable native Worker publication/currentness exchanges. It verifies the
/// separately pinned anchor response and exact journal joins before replying.
/// `serve_native_with_readiness` additionally consumes a private launch pipe,
/// publishing only after recovery and exact manifest/custody checks. Neither
/// entrypoint alters deployment. Raw signing,
/// descriptor extraction and V1 conversions remain unavailable. The V1 serving
/// entrypoint is unchanged.
///
/// Keep input reservations live and use the original cumulative budget for all
/// calls. This owner borrows the work meter's lifetime and rejects a different
/// live meter before revalidation performs I/O. Input storage reservations remain
/// the caller's obligation. Scopes restore entry storage on success, refusal and unwind without
/// refunding work or denial history. Returned storage is only growth; retire
/// the full charge after drop. Logical quotas exclude allocator overhead, kernel
/// objects, generated stack/RSS, latency and diagnostic rendering. Continuity
/// is a point-in-time observation, not exclusive custody or perpetual liveness.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedCompilerExecutionIssuerAdmissionV2<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2;
/// fn fd<T: std::os::fd::AsFd>() {}
/// fd::<ProtectedCompilerExecutionIssuerAdmissionV2<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedCompilerExecutionIssuerAdmissionV1, ProtectedCompilerExecutionIssuerAdmissionV2};
/// fn upgrade(old: ProtectedCompilerExecutionIssuerAdmissionV1) -> ProtectedCompilerExecutionIssuerAdmissionV2<'static> {
///     old.into()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedCompilerExecutionIssuerV1, ProtectedCompilerExecutionIssuerAdmissionV2};
/// fn activate(native: ProtectedCompilerExecutionIssuerAdmissionV2) {
///     let _ = ProtectedCompilerExecutionIssuerV1::admit(native);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedCompilerExecutionIssuerAdmissionV2;
/// fn sign(native: ProtectedCompilerExecutionIssuerAdmissionV2) { let _ = native.signing_key(); }
/// ```
/// The originating work meter cannot be replaced while this owner is live:
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedCompilerExecutionIssuerAdmissionV2 as Admission,
///     ProtectedIssuerProcessV1, ProtectedServiceAdmissionV2, ProtectedExternalAnchorServiceAdmissionV2};
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2;
/// use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn replace_work(process: ProtectedIssuerProcessV1, service: ProtectedServiceAdmissionV2,
///     policy: CompilerExecutionIssuerPolicyV2, key: CompilerExecutionSigningKeyCapabilityV2,
///     anchor: ProtectedExternalAnchorServiceAdmissionV2) {
///     let mut work = Work::new(usize::MAX);
///     let mut budget = Budget::new(&mut work, usize::MAX);
///     let (admission, _) = Admission::admit(process, service, policy, key, anchor, &mut budget).unwrap();
///     drop(budget);
///     work = Work::new(usize::MAX);
///     assert!(!admission.grants_compiler_authority());
/// }
/// ```
ProtectedCompilerExecutionIssuerAdmissionV2,
ProtectedCompilerExecutionIssuerStorageV2,
ProtectedCompilerExecutionIssuerAdmissionErrorV2
);

#[cfg(test)]
#[path = "compiler_execution_issuer_native_tests.rs"]
mod tests;
