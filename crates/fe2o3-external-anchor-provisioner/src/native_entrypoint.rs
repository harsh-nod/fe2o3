//! Dedicated native helper: actual contexts, retained custody, shared terminal exec.
use crate::{
    ExternalAnchorProvisioningReadyDispositionV1 as ReadyDisposition,
    ExternalAnchorProvisioningReadyV1 as Ready, helper_io,
};
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2;
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV2 as Lease, LifecycleLeaseErrorV2,
};
use fe2o3_external_anchor_service::{
    DurableExternalAnchorOpenDispositionV1 as Disposition, NativeExternalAnchorErrorV2,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Process,
    ProtectedServiceProfileErrorV2, observations, require_owned_sigchld_v2,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2, ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableV2 as Executable,
};
use std::{
    convert::Infallible,
    error::Error as StdError,
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{AsRawFd, OwnedFd, RawFd},
};

type Result<T> = std::result::Result<T, NativeExternalAnchorProvisioningHelperErrorV2>;
type Error = NativeExternalAnchorProvisioningHelperErrorV2;
const INPUT_FDS: [RawFd; 9] = [3, 4, 5, 6, 202, 220, 221, 222, 223];
const BOOTSTRAP: usize = 0;
const ROOT: usize = 1;
const DAEMON: usize = 2;
const LIFECYCLE: usize = 3;
const POLICY: usize = 4;
const SUPERVISOR: usize = 5;
const DEPLOYMENT: usize = 6;
const KEY: usize = 7;
const PROVISIONING: usize = 8;
const FILE_STORAGE: usize = size_of::<(File, usize)>();
const IMAGE_MAX: usize =
    fe2o3_compiler_execution_protocol::MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1
        as usize;
const _: () = {
    use fe2o3_compiler_closure_capability::*;
    assert!(INPUT_FDS[BOOTSTRAP] == crate::EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1);
    assert!(INPUT_FDS[ROOT] == crate::EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1);
    assert!(INPUT_FDS[DAEMON] == crate::EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1);
    assert!(INPUT_FDS[LIFECYCLE] == crate::EXTERNAL_ANCHOR_HELPER_LIFECYCLE_FD_V1);
    assert!(INPUT_FDS[POLICY] == COMPILER_EXECUTION_POLICY_CHILD_FD_V1);
    assert!(INPUT_FDS[SUPERVISOR] == COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1);
    assert!(INPUT_FDS[DEPLOYMENT] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1);
    assert!(INPUT_FDS[KEY] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1);
    assert!(INPUT_FDS[PROVISIONING] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1);
    assert!(Lease::FILE_STORAGE == FILE_STORAGE);
    assert!(
        NATIVE_EXTERNAL_ANCHOR_HELPER_FRAME_STORAGE_V2
            >= observations::DESCRIPTOR_INVOCATION_SCRATCH
    );
};

/// Fixed work for invocation, at most 256 descriptor/transport/terminal attempts and control.
/// Native context, image, lease and persistence operations charge additionally.
pub const NATIVE_EXTERNAL_ANCHOR_HELPER_WORK_V2: usize =
    8 + 256 * 1024 + observations::DESCRIPTOR_INVOCATION_WORK;
/// Logical fixed helper frame, not RSS, generated stack or syscall latency.
pub const NATIVE_EXTERNAL_ANCHOR_HELPER_FRAME_STORAGE_V2: usize = 64 * 1024;
/// Finite logical process work quota for the dedicated native helper binaries.
pub const NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_WORK_V2: usize = 1 << 40;
/// Includes conservative incoming and simultaneously staged maximum-size image charges.
pub const NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2: usize = 1 << 31;

/// Native startup/resource failure; no successful protected readiness is implied.
#[derive(Debug)]
pub enum NativeExternalAnchorProvisioningHelperErrorV2 {
    /// Original-ledger resource refusal.
    Resource(Resource),
    /// Process/namespace/invocation observation refused.
    Profile(ProtectedServiceProfileErrorV2),
    /// Actual native context or key custody refused.
    Capability(CompilerExecutionCapabilityErrorV2),
    /// Measured sealed executable refused.
    Executable(ProtectedStaticExecutableErrorV2),
    /// Root-owned lifecycle custody refused.
    Lifecycle(LifecycleLeaseErrorV2),
    /// Native durable-state operation refused.
    Anchor(NativeExternalAnchorErrorV2),
    /// Shared mechanical descriptor or bootstrap operation failed; no V1 owner is admitted.
    Io(crate::ExternalAnchorProvisioningHelperErrorV1),
    /// Credentials, descriptor layout or a private admission boundary is invalid.
    Invalid(&'static str),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::Capability(e) => e.fmt(f),
            Self::Executable(e) => e.fmt(f),
            Self::Lifecycle(e) => e.fmt(f),
            Self::Anchor(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
            Self::Invalid(s) => f.write_str(s),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Capability(e) => Some(e),
            Self::Executable(e) => Some(e),
            Self::Lifecycle(e) => Some(e),
            Self::Anchor(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Invalid(_) => None,
        }
    }
}
macro_rules! from_error {
    ($T:ty, $v:ident) => {
        impl From<$T> for Error {
            fn from(e: $T) -> Self {
                Self::$v(e)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(ProtectedServiceProfileErrorV2, Profile);
from_error!(CompilerExecutionCapabilityErrorV2, Capability);
from_error!(ProtectedStaticExecutableErrorV2, Executable);
from_error!(LifecycleLeaseErrorV2, Lifecycle);
from_error!(NativeExternalAnchorErrorV2, Anchor);
from_error!(crate::ExternalAnchorProvisioningHelperErrorV1, Io);

struct Sources([bool; 9]);
impl Sources {
    fn validate(&self) -> Result<()> {
        for fd in INPUT_FDS {
            // SAFETY: observes the exclusively transferred raw source slots only.
            if unsafe { libc::fcntl(fd, libc::F_GETFD) } != 0 {
                return Err(Error::Invalid("missing or CLOEXEC native helper source"));
            }
        }
        Ok(())
    }
    fn close(&mut self, index: usize) -> Result<()> {
        self.0[index] = false;
        helper_io::close_fixed(INPUT_FDS[index]).map_err(Into::into)
    }
    fn take(&mut self, index: usize) -> Result<File> {
        let f = helper_io::take_fixed(INPUT_FDS[index], "native helper source")?;
        self.0[index] = false;
        Ok(f.into())
    }
}
impl Drop for Sources {
    fn drop(&mut self) {
        for (fd, live) in INPUT_FDS.into_iter().zip(self.0) {
            if live {
                // SAFETY: unsafe entry transfers these raw slots; close is never retried.
                unsafe {
                    libc::close(fd);
                }
            }
        }
    }
}
fn close_unrelated() -> Result<()> {
    for (low, high) in [(0_u32, 2_u32), (7, 201), (203, 219), (224, u32::MAX)] {
        // SAFETY: isolated process, before any descriptor-owning native admission.
        if unsafe { libc::syscall(libc::SYS_close_range, low, high, 0) } != 0 {
            return Err(helper_io::io_error(
                "close unrelated native helper descriptors",
                std::io::Error::last_os_error(),
            )
            .into());
        }
    }
    Ok(())
}

struct Profile {
    process: Option<Process>,
    namespaces: Option<Namespaces>,
}
impl Profile {
    fn capture(b: &mut Budget<'_>) -> Result<Self> {
        let c = Credentials::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .map_err(|_| Error::Invalid("helper requires nonroot credentials"))?;
        let (p, s) = Process::capture(c, b)?;
        b.reserve_storage(s.additional_storage())?;
        require_owned_sigchld_v2(b)?;
        let (n, s) = Namespaces::capture_self(b)?;
        b.reserve_storage(s.additional_storage())?;
        let result = Self {
            process: Some(p),
            namespaces: Some(n),
        };
        result.revalidate(b)?;
        Ok(result)
    }
    fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        if self.process.is_some() != self.namespaces.is_some() {
            return Err(Error::Invalid("partial helper profile"));
        }
        if let Some(p) = &self.process {
            p.revalidate_current(b)?;
        }
        if let Some(n) = &self.namespaces {
            n.revalidate_self(b)?;
        }
        Ok(())
    }
}
trait StartupHooks {
    fn invocation(&mut self) -> Result<()> {
        observations::require_descriptor_only_invocation().map_err(|e| Error::Profile(e.into()))
    }
    fn profile(&mut self, b: &mut Budget<'_>) -> Result<Profile> {
        Profile::capture(b)
    }
    fn running(
        &mut self,
        m: Measurement,
        o: Owner,
        b: &mut Budget<'_>,
    ) -> Result<Option<Executable>> {
        let (e, c) = Executable::admit_running(m, o, "native external-anchor helper", b)?;
        b.reserve_storage(c.additional_storage())?;
        Ok(Some(e))
    }
    fn bootstrap(&mut self, f: &OwnedFd) -> Result<()> {
        helper_io::validate_bootstrap::<true>(f).map_err(Into::into)
    }
    fn lifecycle(&mut self, f: File, root: &File, b: &mut Budget<'_>) -> Result<Lease> {
        let (l, c) = Lease::admit(f, root, b)?;
        b.reserve_storage(c.additional_storage())?;
        Ok(l)
    }
    fn before_state(&mut self, _b: &mut Budget<'_>) -> Result<()> {
        Ok(())
    }
    fn after_state(&mut self, _d: Disposition, _b: &mut Budget<'_>) -> Result<()> {
        Ok(())
    }
    fn ready(
        &mut self,
        boot: &OwnedFd,
        peer: &OwnedFd,
        r: &Ready,
        _b: &mut Budget<'_>,
    ) -> Result<()> {
        helper_io::send_ready(boot, peer, r).map_err(Into::into)
    }
    fn before_exec(&mut self, _b: &mut Budget<'_>) -> Result<()> {
        Ok(())
    }
}
struct SystemStartup;
impl StartupHooks for SystemStartup {}

fn measurement(
    m: fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1,
) -> Result<Measurement> {
    Measurement::new(m.sha256(), m.byte_len(), IMAGE_MAX as u64)
        .map_err(ProtectedStaticExecutableErrorV2::from)
        .map_err(Into::into)
}

fn stage_file(file: File, charge: usize, next: &mut RawFd, b: &mut Budget<'_>) -> Result<File> {
    b.with_prepaid_scope(charge, 0, 0, charge, move |_| {
        let staged = helper_io::stage_above(&file, next, "native staged source")
            .map(File::from)
            .map_err(Error::from);
        drop(file);
        staged
    })
}
fn duplicate(
    source: &impl std::os::fd::AsFd,
    next: &mut RawFd,
    b: &mut Budget<'_>,
) -> Result<File> {
    b.reserve_storage(FILE_STORAGE)?;
    helper_io::stage_above(source, next, "native helper duplicate")
        .map(File::from)
        .map_err(Into::into)
}

mod v2 {
    use super::*;
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionExternalAnchorDeploymentCapabilityV2 as Deployment,
        CompilerExecutionExternalAnchorProvisioningCapabilityV2 as Provisioning,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as Key,
        CompilerExecutionPolicyCapabilityV2 as Policy,
        CompilerExecutionSupervisorDeploymentCapabilityV2 as Supervisor,
    };
    use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV2 as DeploymentRecord;
    use fe2o3_external_anchor_service::DurableExternalAnchorV2 as Anchor;
    const NAME: &std::ffi::CStr = c"fe2o3-external-anchor-service-v2";
    include!("native_startup_body.rs");
}
mod v3 {
    use super::*;
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionExternalAnchorDeploymentCapabilityV3 as Deployment,
        CompilerExecutionExternalAnchorProvisioningCapabilityV3 as Provisioning,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as Key,
        CompilerExecutionPolicyCapabilityV3 as Policy,
        CompilerExecutionSupervisorDeploymentCapabilityV3 as Supervisor,
    };
    use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV3 as DeploymentRecord;
    use fe2o3_external_anchor_service::DurableExternalAnchorV3 as Anchor;
    const NAME: &std::ffi::CStr = c"fe2o3-external-anchor-service-v3";
    include!("native_startup_body.rs");
}
impl v2::Hooks for SystemStartup {}
impl v3::Hooks for SystemStartup {}

/// Complete V2 source reservation, including the maximum bounded daemon image.
pub const NATIVE_EXTERNAL_ANCHOR_HELPER_INPUT_STORAGE_V2: usize = v2::INPUT_STORAGE;
/// Complete V3 source reservation, including the maximum bounded daemon image.
pub const NATIVE_EXTERNAL_ANCHOR_HELPER_INPUT_STORAGE_V3: usize = v3::INPUT_STORAGE;

macro_rules! entrypoint {
    ($name:ident,$family:ident) => {
        /// Admits native helper custody and replaces this process with the measured daemon.
        /// Prepay INPUT_STORAGE on the original ledger. Errors consume/close all inputs and
        /// restore entry storage, preserving work/peak/denials; retire that input reservation.
        /// Success does not return. Late failure may follow durable genesis or readiness send.
        /// Trusted root-parent provisioning is required; no V1 owner fallback is available.
        ///
        /// # Safety
        /// Call once in a dedicated single-threaded helper. Slots 3/4/5/6/202/220/221/222/223
        /// are exclusively transferred raw descriptors (bootstrap/root/daemon/lifecycle/
        /// policy/supervisor/deployment/template/provisioning), not represented by Rust owners.
        /// No other Rust descriptor owners or I/O threads may be live: unrelated descriptors
        /// are closed, and successful terminal exec replaces fixed slots. Exit on failure.
        ///
        /// ```
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        #[doc=concat!("use fe2o3_external_anchor_provisioner::",stringify!($name)," as run;")]
        /// unsafe fn dedicated_helper(b:&mut Budget<'_>) { let _=unsafe { run(b) }; }
        /// ```
        /// ```compile_fail
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        #[doc=concat!("use fe2o3_external_anchor_provisioner::",stringify!($name)," as run;")]
        /// fn arbitrary_application(b:&mut Budget<'_>) { let _=run(b); }
        /// ```
        pub unsafe fn $name(b: &mut Budget<'_>) -> Result<Infallible> {
            $family::run(b, &mut SystemStartup)
        }
    };
}
entrypoint!(run_inherited_external_anchor_provisioning_helper_v2, v2);
entrypoint!(run_inherited_external_anchor_provisioning_helper_v3, v3);

#[cfg(test)]
mod tests;
