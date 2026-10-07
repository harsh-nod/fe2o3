//! Dedicated native startup over the existing descriptor and service contracts.
use crate::{
    IssuerServiceCredentialProfileV1 as Credentials, ProtectedIssuerCleanupServiceV2 as Cleanup,
    native_deployment_io::{self as io, FILE_STORAGE, Sources},
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_protected_service_profile::{
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Process,
    ProtectedServiceProfileErrorV2 as ProfileError, observations, require_owned_sigchld_v2,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as Measurement, ProtectedStaticExecutableV2 as Image,
};
use std::fs::File;

#[path = "native_application_mode.rs"]
mod application_mode;
pub use application_mode::run_inherited_native_supervisor_v3;

/// Local startup work including descriptor custody, root checks and invocation.
/// Native admission, profile, image, readiness and dispatch operations charge extra.
/// The extra 128 fixed 1024-unit allowances cover local checks and owner cleanup,
/// including six root syscalls, one retained-root duplication and PID observations.
pub const NATIVE_ISSUER_STARTUP_WORK_V2: usize =
    io::SOURCE_WORK + observations::DESCRIPTOR_INVOCATION_WORK + 128 * 1024;
/// Fixed logical local frame, excluding full input ownership and nested scratch.
/// Not an allocator, generated-stack, kernel-memory, elapsed-time or RSS bound.
pub const NATIVE_ISSUER_STARTUP_FRAME_STORAGE_V2: usize = 64 * 1024;
/// Finite cumulative process work for the dedicated native supervisor binaries.
pub const NATIVE_ISSUER_PROCESS_WORK_V2: usize = 1 << 40;
/// Finite logical process storage, including full image-copy overlap.
pub const NATIVE_ISSUER_PROCESS_STORAGE_V2: usize = 1 << 31;

const IMAGE_MAX: usize =
    fe2o3_compiler_execution_protocol::MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1 as usize;
const _: () = assert!(NATIVE_ISSUER_STARTUP_FRAME_STORAGE_V2 >= io::SOURCE_SCRATCH);
const _: () =
    assert!(NATIVE_ISSUER_PROCESS_WORK_V2 > NATIVE_ISSUER_STARTUP_WORK_V2 + io::SEND_WORK);

struct Profile {
    process: Process,
    namespaces: Namespaces,
}
impl Profile {
    fn capture(credentials: Credentials, b: &mut Budget<'_>) -> Result<Self, ProfileError> {
        let (process, charge) = Process::capture(credentials, b)?;
        b.reserve_storage(charge.additional_storage())?;
        require_owned_sigchld_v2(b)?;
        let (namespaces, charge) = Namespaces::capture_self(b)?;
        b.reserve_storage(charge.additional_storage())?;
        Ok(Self {
            process,
            namespaces,
        })
    }
    fn revalidate(&self, b: &mut Budget<'_>) -> Result<(), ProfileError> {
        self.process.revalidate_current(b)?;
        self.namespaces.revalidate_self(b)
    }
}

fn measurement(
    m: fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1,
    limit: u64,
) -> Result<Measurement, ImageError> {
    Ok(Measurement::new(m.sha256(), m.byte_len(), limit)?)
}

// Both complete objects coexist during F_DUPFD_CLOEXEC, even for sealed images.
fn intake(
    sources: &mut Sources,
    role: usize,
    charge: usize,
    b: &mut Budget<'_>,
) -> Result<File, io::Error> {
    b.with_prepaid_scope(charge, 0, 0, charge, |_| sources.take(role))
}

macro_rules! family {
    ($module:ident, $Policy:ident, $Deployment:ident, $Key:ident, $Program:ident,
     $ProgramError:ident, $Supervisor:ident, $SupervisorError:ident, $Service:ident,
     $ServiceError:ident, $Session:ident, $Dispatch:ident, $Report:ident, $Ready:ident,
     $ReadyError:ident, $Error:ident, $PolicyRecord:ident, $DeploymentRecord:ident, $PolicyWork:ident) => {
        pub(crate) mod $module {
            use super::*;
            use crate::{
                $Dispatch as DispatchLimits, $Program as Program, $ProgramError as ProgramError,
                $Report as DispatchReport, $Service as Service, $ServiceError as ServiceError,
                $Session as SessionLimits, $Supervisor as Supervisor,
                $SupervisorError as SupervisorError,
            };
            use fe2o3_compiler_closure_capability::{
                CompilerExecutionCapabilityErrorV2 as CapabilityError, $Deployment as Deployment,
                $Key as Key, $Policy as Policy,
            };
            use fe2o3_compiler_execution_protocol::{$Ready as Ready, $ReadyError as ReadyError};
            include!("native_startup_body.rs");
            #[cfg(test)]
            mod tests {
                use super::*;
                use fe2o3_compiler_execution_protocol::{
                    $DeploymentRecord as DeploymentRecord, $PolicyRecord as PolicyRecord,
                    $PolicyWork as POLICY_WORK,
                };
                include!("native_startup_tests.rs");
            }
        }
        pub use $module::Error as $Error;
    };
}
family!(
    v2,
    CompilerExecutionPolicyCapabilityV2,
    CompilerExecutionSupervisorDeploymentCapabilityV2,
    CompilerExecutionSigningKeyCapabilityV2,
    AdmittedIssuerProgramV2,
    IssuerProgramAdmissionErrorV2,
    ProtectedIssuerSupervisorV2,
    ProtectedIssuerSupervisorErrorV2,
    ProtectedIssuerServiceV2,
    ProtectedIssuerServiceErrorV2,
    ProtectedIssuerSessionLimitsV2,
    ProtectedIssuerDispatchLimitsV2,
    ProtectedIssuerDispatchReportV2,
    CompilerExecutionSupervisorReadyV2,
    CompilerExecutionSupervisorReadyErrorV2,
    ProtectedIssuerDeploymentErrorV2,
    CompilerExecutionIssuerPolicyV2,
    CompilerExecutionSupervisorDeploymentV2,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2
);
family!(
    v3,
    CompilerExecutionPolicyCapabilityV3,
    CompilerExecutionSupervisorDeploymentCapabilityV3,
    CompilerExecutionSigningKeyCapabilityV3,
    AdmittedIssuerProgramV3,
    IssuerProgramAdmissionErrorV3,
    ProtectedIssuerSupervisorV3,
    ProtectedIssuerSupervisorErrorV3,
    ProtectedIssuerServiceV3,
    ProtectedIssuerServiceErrorV3,
    ProtectedIssuerSessionLimitsV3,
    ProtectedIssuerDispatchLimitsV3,
    ProtectedIssuerDispatchReportV3,
    CompilerExecutionSupervisorReadyV3,
    CompilerExecutionSupervisorReadyErrorV3,
    ProtectedIssuerDeploymentErrorV3,
    CompilerExecutionIssuerPolicyV3,
    CompilerExecutionSupervisorDeploymentV3,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3
);

/// Full V2 inherited input floor, including both maximum-sized source images.
pub const NATIVE_ISSUER_STARTUP_INPUT_STORAGE_V2: usize = v2::INPUT_STORAGE;
/// Full V3 inherited input floor, including both maximum-sized source images.
pub const NATIVE_ISSUER_STARTUP_INPUT_STORAGE_V3: usize = v3::INPUT_STORAGE;

macro_rules! entrypoint {
    ($run:ident, $module:ident, $Session:ident, $Dispatch:ident, $Report:ident, $Error:ident, $OtherSession:ident) => {
        /// Admits the actual native deployment, publishes readiness, then runs bounded dispatch.
        /// Prepay the matching INPUT_STORAGE on the original account. Every return closes
        /// local startup custody and restores entry storage, preserving work/peak/denial history.
        /// Retire the input floor only after return. A dispatch report is inert: it is not
        /// a receipt proving cleanup empty, provisioning provenance, or GPU qualification.
        /// A lock alias transfers to the separately funded cleanup pool before child
        /// admission. Errors and unwind retain it until successful empty shutdown;
        /// the caller remains responsible for pumping that original cleanup account.
        /// No V1 admission or fallback exists on this path.
        ///
        /// # Safety
        /// Call once in a dedicated, single-threaded supervisor provisioned by its trusted
        /// root parent. Exclusively transfer raw slots 3..=12 and 220 in the fixed ABI;
        /// no Rust owners represent them. No other descriptor owners or I/O actors may
        /// be live: unrelated descriptors, including stdio, are closed. Cleanup must be
        /// freshly admitted with no outstanding reservations or child/descriptor custody.
        /// Retain the original process/account and pump cleanup after a dispatch refusal;
        /// never reinterpret a return or deferred cleanup as successful termination.
        ///
        /// ```
        #[doc = concat!("use fe2o3_compiler_execution_supervisor::{", stringify!($run), " as run, ", stringify!($Session), " as Session, ", stringify!($Dispatch), " as Dispatch, ProtectedIssuerCleanupServiceV2 as Cleanup};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// unsafe fn dedicated(s: Session, d: Dispatch, c: &mut Cleanup, b: &mut Budget<'_>) {
        ///     let _ = unsafe { run(s, d, c, b) };
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_supervisor::{", stringify!($run), " as run, ", stringify!($Session), " as Session, ", stringify!($Dispatch), " as Dispatch, ProtectedIssuerCleanupServiceV2 as Cleanup};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn arbitrary(s: Session, d: Dispatch, c: &mut Cleanup, b: &mut Budget<'_>) { run(s, d, c, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_supervisor::{", stringify!($run), " as run, ", stringify!($OtherSession), " as OtherSession, ", stringify!($Dispatch), " as Dispatch, ProtectedIssuerCleanupServiceV2 as Cleanup};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// unsafe fn mixed(s: OtherSession, d: Dispatch, c: &mut Cleanup, b: &mut Budget<'_>) {
        ///     let _ = unsafe { run(s, d, c, b) };
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_supervisor::{", stringify!($run), " as run, ", stringify!($Session), " as Session, ", stringify!($Dispatch), " as Dispatch, ProtectedIssuerCleanupServiceV2 as Cleanup};")]
        /// unsafe fn unmetered(s: Session, d: Dispatch, c: &mut Cleanup) {
        ///     let _ = unsafe { run(s, d, c) };
        /// }
        /// ```
        pub unsafe fn $run(s: crate::$Session, d: crate::$Dispatch, c: &mut Cleanup,
            b: &mut Budget<'_>) -> Result<crate::$Report, $Error> {
            // SAFETY: the caller transfers the dedicated process and raw-source contract.
            unsafe { $module::run(s, d, c, b) }
        }
    };
}
entrypoint!(
    run_inherited_protected_issuer_service_v2,
    v2,
    ProtectedIssuerSessionLimitsV2,
    ProtectedIssuerDispatchLimitsV2,
    ProtectedIssuerDispatchReportV2,
    ProtectedIssuerDeploymentErrorV2,
    ProtectedIssuerSessionLimitsV3
);
entrypoint!(
    run_inherited_protected_issuer_service_v3,
    v3,
    ProtectedIssuerSessionLimitsV3,
    ProtectedIssuerDispatchLimitsV3,
    ProtectedIssuerDispatchReportV3,
    ProtectedIssuerDeploymentErrorV3,
    ProtectedIssuerSessionLimitsV2
);
