//! Native prepared custody over the shared static-launch descriptor contract.
use crate::{
    AcceptedCompilerExecutionHandoffV2 as Accepted, ProtectedIssuerSupervisorV2 as Supervisor,
    launch_checks as checks,
};
use checks::*;
use fe2o3_broker_authority_service::current_process_start_time_ticks_v2;
use fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV2 as Capability;
use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2 as Manifest;
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

#[path = "launch_v2_error.rs"]
mod error;
#[path = "launch_v2_io.rs"]
mod image;
use ProtectedIssuerLaunchPreparationErrorV2 as Error;
pub use error::ProtectedIssuerLaunchPreparationErrorV2;
type Result<T> = std::result::Result<T, Error>;
const ENTRY: usize = 8;
use ProtectedIssuerLaunchStorageV2 as Storage;

/// Move-only native handoff, sealed program inputs and fixed static-launch table.
///
/// This is preparation only, not process creation, confinement, readiness,
/// service execution, proof or GPU authority. All nested operations share one
/// caller ledger. No admitted V1 owner or fallback is used. The V1-named static
/// manifest is an inert bounded wire record, not executable authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV2;
/// fn clone<T: Clone>() {}
/// clone::<PreparedProtectedIssuerLaunchV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::PreparedProtectedIssuerLaunchV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<PreparedProtectedIssuerLaunchV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{PreparedProtectedIssuerLaunchV1, PreparedProtectedIssuerLaunchV2};
/// fn upgrade(old: PreparedProtectedIssuerLaunchV1) -> PreparedProtectedIssuerLaunchV2 { old.into() }
/// ```
pub struct PreparedProtectedIssuerLaunchV2 {
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
type Prepared = PreparedProtectedIssuerLaunchV2;

#[path = "launch_native_adapter.rs"]
mod adapter;
adapter::launch!(
    PreparedProtectedIssuerLaunchV2,
    ProtectedIssuerLaunchStorageV2,
    LaunchedInputsV2
);

#[cfg(test)]
#[path = "launch_native_test_support.rs"]
pub(crate) mod test_support;

#[cfg(test)]
#[path = "launch_v2_tests.rs"]
pub(crate) mod tests;
