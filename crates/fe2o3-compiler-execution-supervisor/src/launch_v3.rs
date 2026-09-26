//! Native prepared custody over the shared static-launch descriptor contract.
use crate::{
    AcceptedCompilerExecutionHandoffV3 as Accepted, ProtectedIssuerSupervisorV3 as Supervisor,
    launch_checks as checks,
};
use checks::*;
use fe2o3_broker_authority_service::current_process_start_time_ticks_v2;
use fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV3 as Capability;
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV3 as Manifest;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_static_preexec_manifest::{
    PREEXEC_MANIFEST_BYTES_V1 as BYTES, StaticPreexecDescriptorV1 as Descriptor,
    StaticPreexecManifestV1 as StaticManifest, StaticPreexecObjectIdentityV1 as Object,
};
use std::{
    fmt,
    fs::File,
    mem::{align_of, size_of},
    os::fd::OwnedFd,
};

#[path = "launch_v3_error.rs"]
mod error;
#[path = "launch_v2_io.rs"]
mod image;
use ProtectedIssuerLaunchPreparationErrorV3 as Error;
pub use error::ProtectedIssuerLaunchPreparationErrorV3;
type Result<T> = std::result::Result<T, Error>;
const ENTRY: usize = 8;
use ProtectedIssuerLaunchStorageV3 as Storage;

/// Move-only native handoff, sealed program inputs and fixed static-launch table.
///
/// This is preparation only, not process creation, confinement, readiness,
/// service execution, proof or GPU authority. All nested operations share one
/// caller ledger. No admitted V1/V2 owner or fallback is used. The V1-named static
/// manifest is an inert bounded wire record, not executable authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV3;
/// fn clone<T: Clone>() {}
/// clone::<PreparedProtectedIssuerLaunchV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV3;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<PreparedProtectedIssuerLaunchV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{PreparedProtectedIssuerLaunchV1, PreparedProtectedIssuerLaunchV3};
/// fn upgrade(old: PreparedProtectedIssuerLaunchV1) -> PreparedProtectedIssuerLaunchV3 { old.into() }
/// ```
///
/// The launch record and sealed capability are actual V3 owners bound to the
/// accepted V3 handoff and supervisor policy. The static manifest, sealed
/// launcher and staged descriptor ABI are policy-neutral runtime contracts.
/// Revalidation borrows the original caller budget throughout. Preserve the
/// consumed handoff reservation and reserve returned growth before retention.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{PreparedProtectedIssuerLaunchV2 as V2,
///     PreparedProtectedIssuerLaunchV3 as V3};
/// fn upgrade(p: V2) -> V3 { p.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{PreparedProtectedIssuerLaunchV2 as V2,
///     PreparedProtectedIssuerLaunchV3 as V3};
/// fn downgrade(p: V3) -> V2 { p.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV3 as S,
///     AcceptedCompilerExecutionHandoffV2 as H};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
/// fn mix(s: &S, h: H, b: &mut B<'_>) { let _ = s.prepare_launch(h, b); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as S,
///     AcceptedCompilerExecutionHandoffV3 as H};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
/// fn mix(s: &S, h: H, b: &mut B<'_>) { let _ = s.prepare_launch(h, b); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{PreparedProtectedIssuerLaunchV3 as P,
///     ProtectedIssuerSupervisorV2 as S};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
/// fn mix(p: &P, s: &S, b: &mut B<'_>) { let _ = p.revalidate(s, b); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV3 as P;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2 as M;
/// fn mix(p: &P) -> &M { p.service_manifest() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV3 as P;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV3 as M;
/// fn escape(p: &P) -> &'static M { p.service_manifest() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerLaunchStorageV2 as V2,
///     ProtectedIssuerLaunchStorageV3 as V3};
/// fn mix(s: V2) -> V3 { s }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerLaunchPreparationErrorV2 as V2,
///     ProtectedIssuerLaunchPreparationErrorV3 as V3};
/// fn mix(e: V2) -> V3 { e.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV3 as P;
/// fn escape(p: &P) { let _ = p.staged_input(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV3 as P;
/// fn escape(p: P) { let _ = p.into_launched(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV3 as P;
/// fn escape(p: P) { let _ = p.launch_capability; }
/// ```
pub struct PreparedProtectedIssuerLaunchV3 {
    accepted: Accepted,
    launch_capability: Capability,
    launcher: File,
    issuer: File,
    static_manifest_file: File,
    sources: [File; SOURCE_COUNT_V1],
    stdout_reader: OwnedFd,
    stderr_reader: OwnedFd,
    readiness_reader: OwnedFd,
    static_manifest: StaticManifest,
    manifest_object: Object,
    retained: usize,
}
type Prepared = PreparedProtectedIssuerLaunchV3;

#[path = "launch_native_adapter.rs"]
mod adapter;
adapter::launch!(
    PreparedProtectedIssuerLaunchV3,
    ProtectedIssuerLaunchStorageV3,
    LaunchedInputsV3
);

#[cfg(test)]
#[path = "launch_v3_tests.rs"]
pub(crate) mod tests;
