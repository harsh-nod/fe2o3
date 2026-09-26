//! Native pre-session custody. Process confinement and consuming launch are separate.
use crate::{
    AdmittedIssuerProgramV3 as Program, IssuerProgramAdmissionErrorV3 as ProgramError,
    IssuerServiceCredentialProfileV1 as Credentials,
    root_checks::{self, RootCheckError, RootSnapshot},
};
use fe2o3_broker_authority_service::{
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor,
    ProtectedExternalAnchorServiceErrorV2 as AnchorError,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as KeyError, CompilerExecutionSigningKeyCapabilityV3 as Key,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as AnchorIdentity,
    CompilerExecutionIssuerPolicyV3 as Policy,
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
use ProtectedIssuerSupervisorStorageV3 as Storage;
type Result<T> = std::result::Result<T, ProtectedIssuerSupervisorErrorV3>;

/// Move-only native program, policy-bound key, anchor, root and credential custody.
///
/// Trusted provisioning supplies every input. This checks the current effective
/// UID/GID, not the full child confinement profile. It does not authenticate a
/// compiler handoff, spawn a process, establish readiness, or grant GPU authority.
/// No V1 or V2 admitted owner is upgraded; no descriptor or signing operation escapes.
///
/// All nested operations use the caller's ledger. Entry storage is restored on
/// success, refusal and unwind without refunding work. On consuming refusal,
/// inputs close before the caller retires their old reservations.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV3;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedIssuerSupervisorV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV3;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ProtectedIssuerSupervisorV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV1, ProtectedIssuerSupervisorV3};
/// fn upgrade(old: ProtectedIssuerSupervisorV1) -> ProtectedIssuerSupervisorV3 { old.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as V2,
///     ProtectedIssuerSupervisorV3 as V3};
/// fn upgrade(s: V2) -> V3 { s.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorV2 as V2,
///     ProtectedIssuerSupervisorV3 as V3};
/// fn downgrade(s: V3) -> V2 { s.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AdmittedIssuerProgramV2 as Program,
///     ProtectedIssuerSupervisorV3 as Supervisor, IssuerServiceCredentialProfileV1 as Credentials};
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Key;
/// use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceAdmissionV2 as Anchor;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(p: Program, c: Credentials, r: std::fs::File, k: Key, a: Anchor, b: &mut Budget<'_>) {
///     let _ = Supervisor::bind(p, c, r, k, a, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AdmittedIssuerProgramV3 as Program,
///     ProtectedIssuerSupervisorV3 as Supervisor, IssuerServiceCredentialProfileV1 as Credentials};
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2 as Key;
/// use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceAdmissionV2 as Anchor;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(p: Program, c: Credentials, r: std::fs::File, k: Key, a: Anchor, b: &mut Budget<'_>) {
///     let _ = Supervisor::bind(p, c, r, k, a, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV3 as Supervisor;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
/// fn mix(s: &Supervisor) -> &Policy { s.policy() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerSupervisorV3 as Supervisor;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
/// fn escape(s: &Supervisor) -> &'static Policy { s.policy() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorErrorV2 as V2,
///     ProtectedIssuerSupervisorErrorV3 as V3};
/// fn mix(e: V2) -> V3 { e.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{ProtectedIssuerSupervisorStorageV2 as V2,
///     ProtectedIssuerSupervisorStorageV3 as V3};
/// fn mix(s: V2) -> V3 { s }
/// ```
pub struct ProtectedIssuerSupervisorV3 {
    program: Program,
    credentials: Credentials,
    root: File,
    root_snapshot: RootSnapshot,
    signing_key: Key,
    external_anchor: Anchor,
    retained: usize,
}

crate::shared_adapter::authority!(
    ProtectedIssuerSupervisorV3,
    ProtectedIssuerSupervisorStorageV3,
    ProtectedIssuerSupervisorErrorV3,
    IssuerProgramStorageV3
);

#[cfg(test)]
#[path = "native_authority_v3_tests.rs"]
pub(crate) mod tests;
