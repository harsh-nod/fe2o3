//! Native policy-bound program custody. Supervisor/process authority is separate.
use crate::{
    IssuerProgramAdmissionErrorV1 as LegacyError,
    ProvisionedStaticExecutableMeasurementV1 as Provisioned, protected_measurement,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionPolicyCapabilityV3 as PolicyCapability,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV3 as Policy, SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1,
    sealed_static_issuer_runtime_measurement_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableStorageV2 as ImageStorage,
    ProtectedStaticExecutableV2 as Image,
};
use fe2o3_static_preexec_manifest::StaticPreexecObjectIdentityV1 as Object;
use std::{error::Error, fmt, fs::File, mem::size_of};

const ENTRY: usize = 8;
type Result<T> = std::result::Result<T, IssuerProgramAdmissionErrorV3>;

/// Fresh native policy plus two independently sealed static executable owners.
/// Admission does not bind service credentials, signing key, protected root,
/// anchor transport, running process, readiness or GPU authority. The native
/// supervisor and serving entrypoint must still be integrated before activation.
/// No admitted V1 or V2 program is converted or projected into this owner.
///
/// All inputs and outputs use one caller ledger. Scopes restore entry storage
/// without refunding work; retire consumed reservations after an error returns.
/// Caller-provided launcher measurement and policy must come from trusted
/// provisioning, not the requesting compiler or inherited pair agreement alone.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV3;
/// fn duplicate(p:AdmittedIssuerProgramV3) { let _=p.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV3;
/// fn descriptor<T:std::os::fd::AsFd>() {}
/// descriptor::<AdmittedIssuerProgramV3>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AdmittedIssuerProgramV3 as Program,
///     ProvisionedStaticExecutableMeasurementV1 as Measurement};
/// use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV2 as Policy;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn mix(l: std::fs::File, m: Measurement, i: std::fs::File, p: Policy, b: &mut Budget<'_>) {
///     let _ = Program::provision(l, m, i, p, b);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AdmittedIssuerProgramV2 as V2, AdmittedIssuerProgramV3 as V3};
/// fn upgrade(p: V2) -> V3 { p.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{AdmittedIssuerProgramV2 as V2, AdmittedIssuerProgramV3 as V3};
/// fn downgrade(p: V3) -> V2 { p.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV3 as Program;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
/// fn mix(p: &Program) -> &Policy { p.policy() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::AdmittedIssuerProgramV3 as Program;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
/// fn escape(p: &Program) -> &'static Policy { p.policy() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{IssuerProgramAdmissionErrorV2 as V2,
///     IssuerProgramAdmissionErrorV3 as V3};
/// fn mix(e: V2) -> V3 { e.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::{IssuerProgramStorageV2 as V2, IssuerProgramStorageV3 as V3};
/// fn mix(s: V2) -> V3 { s }
/// ```
pub struct AdmittedIssuerProgramV3 {
    launcher: Image,
    issuer: Image,
    policy: PolicyCapability,
}
crate::shared_adapter::program!(
    AdmittedIssuerProgramV3,
    IssuerProgramStorageV3,
    IssuerProgramAdmissionErrorV3
);

#[cfg(test)]
#[path = "native_program_v3_tests.rs"]
pub(crate) mod tests;
