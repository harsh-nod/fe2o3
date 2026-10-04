//! Native session admission, not prepared launch or service activation.
use crate::{
    ProtectedIssuerSupervisorErrorV3 as SupervisorError, ProtectedIssuerSupervisorV3 as Supervisor,
    handoff_checks::{self as checks, Snapshot},
};
use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1, LiveClientPidfdErrorV2 as PidfdError,
    LiveClientPidfdIdentityV2 as LiveClient,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V3 as BYTES,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestError,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
    CompilerExecutionSupervisorHandoffErrorV3 as FrameError,
    CompilerExecutionSupervisorHandoffV3 as Frame,
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
type Result<T> = std::result::Result<T, ProtectedIssuerHandoffErrorV3>;
use ProtectedIssuerHandoffStorageV3 as Storage;

/// Move-only native handoff, exact service peer and one admitted live client pidfd.
///
/// Admission authenticates connection-time peer identities and observes client
/// liveness. It does not prove process ancestry, exclusive endpoint ownership,
/// full child confinement, service readiness, compiler execution or GPU authority.
/// No raw descriptor, signing, V1/V2 conversion, or launch operation is exposed.
/// The shared wire does not identify a protocol family. Admission decodes an
/// actual FrameV3/ManifestV3 and matches the independently admitted V3 policy;
/// a structurally valid V1/V2-policy-bound frame is refused by that exact join.
/// Keep supervisor/control inputs prepaid on the original account and reserve
/// returned growth before retention. Refusal closes consumed descriptors while
/// preserving the caller's reservations, accepted work and denial history.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV3;
/// fn clone<T: Clone>() {}
/// clone::<AcceptedCompilerExecutionHandoffV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV3;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<AcceptedCompilerExecutionHandoffV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AcceptedCompilerExecutionHandoffV1, AcceptedCompilerExecutionHandoffV3};
/// fn upgrade(old: AcceptedCompilerExecutionHandoffV1) -> AcceptedCompilerExecutionHandoffV3 { old.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AcceptedCompilerExecutionHandoffV2 as V2,
///     AcceptedCompilerExecutionHandoffV3 as V3};
/// fn upgrade(old: V2) -> V3 { old.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AcceptedCompilerExecutionHandoffV2 as V2,
///     AcceptedCompilerExecutionHandoffV3 as V3};
/// fn downgrade(value: V3) -> V2 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AcceptedCompilerExecutionHandoffV3 as Handoff,
///     ProtectedIssuerSupervisorV2 as Supervisor};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(h: &Handoff, s: &Supervisor, b: &mut Budget<'_>) { let _ = h.revalidate(s, b); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV3 as Handoff;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2 as Manifest;
/// fn mix(h: &Handoff) -> &Manifest { h.manifest() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV3 as Handoff;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV3 as Manifest;
/// fn escape(h: &Handoff) -> &'static Manifest { h.manifest() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerHandoffErrorV2 as V2,
///     ProtectedIssuerHandoffErrorV3 as V3};
/// fn mix(e: V2) -> V3 { e.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerHandoffStorageV2 as V2,
///     ProtectedIssuerHandoffStorageV3 as V3};
/// fn mix(s: V2) -> V3 { s }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AcceptedCompilerExecutionHandoffV3 as Handoff;
/// fn descriptor(h: Handoff) { let _ = h.into_control(); }
/// ```
pub struct AcceptedCompilerExecutionHandoffV3 {
    control: OwnedFd,
    handoff: Frame,
    service_peer: OwnedFd,
    client: LiveClient,
    control_snapshot: Snapshot,
    service_snapshot: Snapshot,
    pidfd_snapshot: Snapshot,
    retained: usize,
}
type Accepted = AcceptedCompilerExecutionHandoffV3;
use ProtectedIssuerHandoffErrorV3 as LaunchError;
#[path = "handoff_native_launch.rs"]
mod launch;

#[path = "handoff_native_adapter.rs"]
mod adapter;
adapter::handoff!(
    AcceptedCompilerExecutionHandoffV3,
    ProtectedIssuerHandoffStorageV3,
    ProtectedIssuerHandoffErrorV3,
    CompilerExecutionAttestationStorageV3
);

// The raw receiver has no policy decoder. Bind its existing private error import
// to the actual V3 enum, not a converted V2 error, and reuse the unedited body.
mod handoff_v2 {
    pub(super) use super::ProtectedIssuerHandoffErrorV3 as ProtectedIssuerHandoffErrorV2;
}
#[path = "handoff_v2_io.rs"]
mod transport;
const _: () = assert!(
    BYTES == fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V2
);

#[cfg(test)]
#[path = "handoff_native_v3_tests.rs"]
pub(crate) mod tests;
