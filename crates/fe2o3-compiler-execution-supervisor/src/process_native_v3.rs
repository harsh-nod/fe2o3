//! SubjectV3 consuming custody over the same policy-neutral process/cleanup engine.
use super::{
    ChildProcessError, ChildProfileV1, IssuerChild, ProfileReportRead, ReportError, StagedLaunchV1,
    child_work, spawn_child,
};
use crate::{
    AcceptedCompilerExecutionHandoffV3 as Accepted, PreparedProtectedIssuerLaunchV3 as Prepared,
    ProtectedIssuerCleanupServiceV2 as Cleanup, ProtectedIssuerHandoffErrorV3 as HandoffError,
    ProtectedIssuerLaunchPreparationErrorV3 as PreparationError,
    ProtectedIssuerSupervisorErrorV3 as SupervisorError, ProtectedIssuerSupervisorV3 as Supervisor,
    launch_v3::LaunchedInputsV3 as LaunchedInputs, process_cleanup::CleanupPollV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V3 as READY_BYTES,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    CompilerExecutionServiceReadyErrorV3 as ReadyError,
    CompilerExecutionServiceReadyV3 as Readiness,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::observations::{
    CHILD_NAMESPACE_REPORT_BYTES, CHILD_NAMESPACE_REPORT_CAPTURE_SCRATCH,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Profile,
    require_owned_sigchld_v2,
};
use rustix::{
    io::Errno,
    net::SendFlags,
    pipe::{PipeFlags, pipe_with},
};
use std::{
    fmt,
    mem::size_of,
    os::fd::{AsRawFd, OwnedFd},
};

#[path = "process_native_error_v3.rs"]
mod error;
#[path = "process_native_wait_v3.rs"]
mod wait;
use ProtectedIssuerBoundaryV3 as Boundary;
use ProtectedIssuerLaunchErrorV3 as Error;
use ProtectedIssuerWaitV3 as Wait;
pub use error::ProtectedIssuerLaunchErrorV3;
pub use wait::{ProtectedIssuerBoundaryV3, ProtectedIssuerWaitV3};
use wait::{attempts, before_deadline, live};
type Result<T> = std::result::Result<T, Error>;
const ENTRY: usize = 8;

/// Move-only native launched child, not ready service or compiler authority.
///
/// This borrows the original native supervisor and retains its exact launch
/// capability. Exclusively borrows the original request ledger for its complete
/// lifetime; the caller cannot retire or replace that ledger during custody.
/// Consuming failure drops endpoints and uses the request-prepaid single
/// cleanup transition; pending custody moves into the persistently funded pool.
/// No admitted legacy process, policy, readiness, or handoff owner is used.
/// The V2-named cleanup service and process-profile observations are shared
/// policy-neutral facilities, not conversions from admitted V2 custody.
/// This API does not select a deployment or activate a production compiler.
///
/// The complete consuming API retains the original account through terminal output:
///
/// ```
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as Supervisor,
///     PreparedProtectedIssuerLaunchV3 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV3 as Wait, ProtectedIssuerLaunchErrorV3 as Error,
///     ExitedProtectedIssuerV3 as Exited};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn run<'a, 'w>(s: &'a Supervisor, p: Prepared, c: &mut Cleanup, w: Wait,
///                b: &'a mut Budget<'w>) -> Result<Exited<'a, 'w>, Error> {
///     s.launch(p, c, w, b)?.await_readiness(w)?
///         .publish_readiness(w)?.wait_for_exit(w)
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV3;
/// fn clone<T: Clone>() {}
/// clone::<LaunchedProtectedIssuerV3<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV3;
/// fn fd<T: std::os::fd::AsFd>() {}
/// fd::<LaunchedProtectedIssuerV3<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as Supervisor,
///     PreparedProtectedIssuerLaunchV3 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV3 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_live(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let mut child = s.launch(p, c, w, b).unwrap();
///     b.release_storage(1).unwrap();
///     child.is_live().unwrap();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{LaunchedProtectedIssuerV2 as V2,
///     LaunchedProtectedIssuerV3 as V3};
/// fn mix<'a, 'w>(child: V2<'a, 'w>) -> V3<'a, 'w> { child.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerLaunchErrorV2 as V2,
///     ProtectedIssuerLaunchErrorV3 as V3};
/// fn mix(error: V2) -> V3 { error.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as Supervisor,
///     PreparedProtectedIssuerLaunchV2 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV3 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let _ = s.launch(p, c, w, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as Supervisor,
///     PreparedProtectedIssuerLaunchV3 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV2 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let _ = s.launch(p, c, w, b);
/// }
/// ```
pub struct LaunchedProtectedIssuerV3<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Native exact private-pipe readiness plus retained live pidfd custody.
/// Readiness remains inert; compiler and publication authority need their own checks.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ReadyProtectedIssuerV3 as Ready;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV2 as Record;
/// fn mix<'a>(ready: &'a Ready<'_, '_>) -> &'a Record { ready.readiness() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ReadyProtectedIssuerV3 as Ready;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV3 as Record;
/// fn escape(ready: &Ready<'_, '_>) -> &'static Record { ready.readiness() }
/// ```
pub struct ReadyProtectedIssuerV3<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Native issuer custody after atomic readiness publication to Cargo.
/// No descriptor, signing operation, compiler receipt, or GPU authority is exposed.
pub struct ServingProtectedIssuerV3<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Inert result of one exact consuming terminal wait, never a deferred-cleanup claim.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ReadyProtectedIssuerV3;
/// fn clone<T: Clone>() {}
/// clone::<ReadyProtectedIssuerV3<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ServingProtectedIssuerV3;
/// fn clone<T: Clone>() {}
/// clone::<ServingProtectedIssuerV3<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ExitedProtectedIssuerV3;
/// fn clone<T: Clone>() {}
/// clone::<ExitedProtectedIssuerV3<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as Supervisor,
///     PreparedProtectedIssuerLaunchV3 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV3 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_exited(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let exited = s.launch(p, c, w, b).unwrap().await_readiness(w).unwrap()
///         .publish_readiness(w).unwrap().wait_for_exit(w).unwrap();
///     b.release_storage(1).unwrap();
///     drop(exited);
/// }
/// ```
pub struct ExitedProtectedIssuerV3<'a, 'work> {
    pid: u32,
    readiness: Readiness,
    termination: super::ProtectedIssuerTerminationV1,
    funding: RequestFunding<'a, 'work>,
}

type Launched<'a, 'work> = LaunchedProtectedIssuerV3<'a, 'work>;
type Ready<'a, 'work> = ReadyProtectedIssuerV3<'a, 'work>;
type Serving<'a, 'work> = ServingProtectedIssuerV3<'a, 'work>;
type Exited<'a, 'work> = ExitedProtectedIssuerV3<'a, 'work>;

/// Fixed logical launch work; native observations and finite waits charge separately.
pub const PROTECTED_ISSUER_LAUNCH_WORK_V3: usize = LAUNCH_WORK;
/// Logical staging/control scratch, not RSS, stack or kernel-memory bounds.
pub const PROTECTED_ISSUER_LAUNCH_SCRATCH_V3: usize = LAUNCH_SCRATCH;

include!("process_native_body.rs");

#[path = "process_native_session_adapter.rs"]
mod session_adapter;
session_adapter::native_session!(
    ProtectedIssuerSessionLimitsV3,
    ProtectedIssuerSessionErrorV3
);
use ProtectedIssuerSessionErrorV3 as SessionError;
use ProtectedIssuerSessionLimitsV3 as SessionLimits;
include!("process_native_session_body.rs");

#[cfg(test)]
#[path = "process_native_session_tests.rs"]
mod session_tests;

#[cfg(test)]
#[path = "process_native_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "process_native_v3_tests.rs"]
mod binding_tests;
