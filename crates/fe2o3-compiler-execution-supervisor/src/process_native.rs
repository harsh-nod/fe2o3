//! Native consuming custody over the one clone/pidfd/cleanup engine.
use super::{
    ChildProcessError, ChildProfileV1, IssuerChild, ProfileReportRead, ReportError, StagedLaunchV1,
    child_work, spawn_child,
};
use crate::{
    AcceptedCompilerExecutionHandoffV2 as Accepted, PreparedProtectedIssuerLaunchV2 as Prepared,
    ProtectedIssuerCleanupServiceV2 as Cleanup, ProtectedIssuerHandoffErrorV2 as HandoffError,
    ProtectedIssuerLaunchPreparationErrorV2 as PreparationError,
    ProtectedIssuerSupervisorErrorV2 as SupervisorError, ProtectedIssuerSupervisorV2 as Supervisor,
    launch_v2::LaunchedInputsV2 as LaunchedInputs, process_cleanup::CleanupPollV1,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_READY_BYTES_V2 as READY_BYTES,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
    CompilerExecutionServiceReadyErrorV2 as ReadyError,
    CompilerExecutionServiceReadyV2 as Readiness,
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

#[path = "process_native_error.rs"]
mod error;
#[path = "process_native_wait.rs"]
mod wait;
use ProtectedIssuerBoundaryV2 as Boundary;
use ProtectedIssuerLaunchErrorV2 as Error;
use ProtectedIssuerWaitV2 as Wait;
pub use error::ProtectedIssuerLaunchErrorV2;
pub use wait::{ProtectedIssuerBoundaryV2, ProtectedIssuerWaitV2};
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
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<LaunchedProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::LaunchedProtectedIssuerV2;
/// fn fd<T: std::os::fd::AsFd>() {}
/// fd::<LaunchedProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as Supervisor,
///     PreparedProtectedIssuerLaunchV2 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV2 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_live(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let mut child = s.launch(p, c, w, b).unwrap();
///     b.release_storage(1).unwrap();
///     child.is_live().unwrap();
/// }
/// ```
pub struct LaunchedProtectedIssuerV2<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Native exact private-pipe readiness plus retained live pidfd custody.
/// Readiness remains inert; compiler and publication authority need their own checks.
pub struct ReadyProtectedIssuerV2<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Native issuer custody after atomic readiness publication to Cargo.
/// No descriptor, signing operation, compiler receipt, or GPU authority is exposed.
pub struct ServingProtectedIssuerV2<'a, 'work> {
    session: Session<'a, 'work>,
}

/// Inert result of one exact consuming terminal wait, never a deferred-cleanup claim.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ReadyProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<ReadyProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ServingProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<ServingProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ExitedProtectedIssuerV2;
/// fn clone<T: Clone>() {}
/// clone::<ExitedProtectedIssuerV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as Supervisor,
///     PreparedProtectedIssuerLaunchV2 as Prepared, ProtectedIssuerCleanupServiceV2 as Cleanup,
///     ProtectedIssuerWaitV2 as Wait};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn release_exited(s: &Supervisor, p: Prepared, c: &mut Cleanup, w: Wait, b: &mut Budget<'_>) {
///     let exited = s.launch(p, c, w, b).unwrap().await_readiness(w).unwrap()
///         .publish_readiness(w).unwrap().wait_for_exit(w).unwrap();
///     b.release_storage(1).unwrap();
///     drop(exited);
/// }
/// ```
pub struct ExitedProtectedIssuerV2<'a, 'work> {
    pid: u32,
    readiness: Readiness,
    termination: super::ProtectedIssuerTerminationV1,
    funding: RequestFunding<'a, 'work>,
}

type Launched<'a, 'work> = LaunchedProtectedIssuerV2<'a, 'work>;
type Ready<'a, 'work> = ReadyProtectedIssuerV2<'a, 'work>;
type Serving<'a, 'work> = ServingProtectedIssuerV2<'a, 'work>;
type Exited<'a, 'work> = ExitedProtectedIssuerV2<'a, 'work>;

/// Fixed logical launch work; native observations and finite waits charge separately.
pub const PROTECTED_ISSUER_LAUNCH_WORK_V2: usize = LAUNCH_WORK;
/// Logical staging/control scratch, not RSS, stack or kernel-memory bounds.
pub const PROTECTED_ISSUER_LAUNCH_SCRATCH_V2: usize = LAUNCH_SCRATCH;

include!("process_native_body.rs");

#[path = "process_native_session_adapter.rs"]
mod session_adapter;
session_adapter::native_session!(
    ProtectedIssuerSessionLimitsV2,
    ProtectedIssuerSessionErrorV2
);
use ProtectedIssuerSessionErrorV2 as SessionError;
use ProtectedIssuerSessionLimitsV2 as SessionLimits;
include!("process_native_session_body.rs");

#[cfg(test)]
#[path = "process_native_session_tests.rs"]
mod session_tests;

#[cfg(test)]
#[path = "process_native_tests.rs"]
mod tests;
