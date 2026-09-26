//! Native pre-session custody. Process confinement and consuming launch are separate.
use crate::{
    AdmittedIssuerProgramV2 as Program, IssuerProgramAdmissionErrorV2 as ProgramError,
    IssuerServiceCredentialProfileV1 as Credentials,
    root_checks::{self, RootCheckError, RootSnapshot},
};
use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
    ProtectedExternalAnchorServiceErrorV2 as AnchorError,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as KeyError, CompilerExecutionSigningKeyCapabilityV2 as Key,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as AnchorIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    fs::File,
    mem::{align_of, size_of},
};

const ENTRY: usize = 8;
#[path = "authority_v2_launch.rs"]
pub(super) mod launch;
use ProtectedIssuerSupervisorStorageV2 as Storage;
type Result<T> = std::result::Result<T, ProtectedIssuerSupervisorErrorV2>;

/// Move-only native program, policy-bound key, anchor, root and credential custody.
///
/// Trusted provisioning supplies every input. This checks the current effective
/// UID/GID, not the full child confinement profile. It does not authenticate a
/// compiler handoff, spawn a process, establish readiness, or grant GPU authority.
/// No V1 admitted owner is upgraded, and no descriptor or signing operation escapes.
///
/// All nested operations use the caller's ledger. Entry storage is restored on
/// success, refusal and unwind without refunding work. On consuming refusal,
/// inputs close before the caller retires their old reservations.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedIssuerSupervisorV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ProtectedIssuerSupervisorV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV1, ProtectedIssuerSupervisorV2};
/// fn upgrade(old: ProtectedIssuerSupervisorV1) -> ProtectedIssuerSupervisorV2 { old.into() }
/// ```
pub struct ProtectedIssuerSupervisorV2 {
    program: Program,
    credentials: Credentials,
    root: File,
    root_snapshot: RootSnapshot,
    signing_key: Key,
    external_anchor: Anchor,
    retained: usize,
}

crate::shared_adapter::authority!(
    ProtectedIssuerSupervisorV2,
    ProtectedIssuerSupervisorStorageV2,
    ProtectedIssuerSupervisorErrorV2,
    IssuerProgramStorageV2
);

#[cfg(test)]
#[path = "authority_v2_tests.rs"]
pub(crate) mod tests;
