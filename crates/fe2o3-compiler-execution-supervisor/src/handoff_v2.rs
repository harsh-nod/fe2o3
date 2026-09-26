//! Native session admission, not prepared launch or service activation.
use crate::{
    ProtectedIssuerSupervisorErrorV2 as SupervisorError, ProtectedIssuerSupervisorV2 as Supervisor,
    handoff_checks::{self as checks, Snapshot},
    handoff_v2_io as transport,
};
use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1, LiveClientPidfdErrorV2 as PidfdError,
    LiveClientPidfdIdentityV2 as LiveClient,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2 as BYTES,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
    CompilerExecutionSupervisorHandoffErrorV2 as FrameError,
    CompilerExecutionSupervisorHandoffV2 as Frame,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    mem::{align_of, size_of},
    os::fd::OwnedFd,
    time::{Duration, Instant},
};

const ENTRY: usize = 8;
type Result<T> = std::result::Result<T, ProtectedIssuerHandoffErrorV2>;
use ProtectedIssuerHandoffStorageV2 as Storage;

/// Move-only native handoff, exact service peer and one admitted live client pidfd.
///
/// Admission authenticates connection-time peer identities and observes client
/// liveness. It does not prove process ancestry, exclusive endpoint ownership,
/// full child confinement, service readiness, compiler execution or GPU authority.
/// No raw descriptor, signing, V1 conversion, or launch operation is exposed.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV2;
/// fn clone<T: Clone>() {}
/// clone::<AcceptedCompilerExecutionHandoffV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<AcceptedCompilerExecutionHandoffV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AcceptedCompilerExecutionHandoffV1, AcceptedCompilerExecutionHandoffV2};
/// fn upgrade(old: AcceptedCompilerExecutionHandoffV1) -> AcceptedCompilerExecutionHandoffV2 { old.into() }
/// ```
pub struct AcceptedCompilerExecutionHandoffV2 {
    control: OwnedFd,
    handoff: Frame,
    service_peer: OwnedFd,
    client: LiveClient,
    control_snapshot: Snapshot,
    service_snapshot: Snapshot,
    pidfd_snapshot: Snapshot,
    retained: usize,
}
type Accepted = AcceptedCompilerExecutionHandoffV2;
#[path = "handoff_native_adapter.rs"]
mod adapter;
adapter::handoff!(
    AcceptedCompilerExecutionHandoffV2,
    ProtectedIssuerHandoffStorageV2,
    ProtectedIssuerHandoffErrorV2,
    CompilerExecutionAttestationStorageV2
);

use ProtectedIssuerHandoffErrorV2 as LaunchError;
#[path = "handoff_native_launch.rs"]
mod launch;

#[cfg(test)]
#[path = "handoff_v2_tests.rs"]
pub(crate) mod tests;
