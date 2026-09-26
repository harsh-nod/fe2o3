//! Native fixed-descriptor startup; cleanup precedes descriptor-owning admission.
use crate::{
    ExternalAnchorServiceReportV1 as Report, NativeExternalAnchorErrorV2,
    NativeExternalAnchorStorageV2 as Storage,
};
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2;
use fe2o3_compiler_execution_lifecycle::{
    CompilerExecutionServiceLifecycleLeaseV2 as Lease, LifecycleLeaseErrorV2,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceCredentialProfileV1 as Credentials,
    ProtectedServiceNamespaceSetV2 as Namespaces, ProtectedServiceProcessProfileV2 as Process,
    ProtectedServiceProfileErrorV2, require_owned_sigchld_v2,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2, ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableV2 as Executable,
};
use std::{
    error::Error as StdError,
    fmt,
    fs::File,
    os::fd::{OwnedFd, RawFd},
};

type Result<T> = std::result::Result<T, NativeExternalAnchorEntrypointErrorV2>;
type Error = NativeExternalAnchorEntrypointErrorV2;

/// Fixed outer startup work. Nested custody and service operations charge additionally.
pub const NATIVE_EXTERNAL_ANCHOR_STARTUP_WORK_V2: usize = 8 + 128 * 1024 + 32 * (MAX_ARGV0 + 1);
/// Logical fixed startup frame, not a generated-stack, RSS or syscall-time bound.
pub const NATIVE_EXTERNAL_ANCHOR_STARTUP_FRAME_STORAGE_V2: usize = 64 * 1024;
/// Finite process-lifetime work quota used by the dedicated native daemon binaries.
pub const NATIVE_EXTERNAL_ANCHOR_PROCESS_WORK_LIMIT_V2: usize = 1 << 40;
/// Finite logical storage quota used by the dedicated native daemon binaries.
pub const NATIVE_EXTERNAL_ANCHOR_PROCESS_STORAGE_LIMIT_V2: usize = 1 << 30;

const MAX_ARGV0: usize = 4096;
const PEER: usize = 0;
const ROOT: usize = 1;
const LIFECYCLE: usize = 2;
const POLICY: usize = 3;
const SUPERVISOR: usize = 4;
const DEPLOYMENT: usize = 5;
const KEY: usize = 6;
const INPUT_FDS: [RawFd; 7] = [3, 4, 5, 202, 220, 221, 222];

const _: () = {
    use fe2o3_compiler_closure_capability::*;
    assert!(INPUT_FDS[POLICY] == COMPILER_EXECUTION_POLICY_CHILD_FD_V1);
    assert!(INPUT_FDS[SUPERVISOR] == COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1);
    assert!(INPUT_FDS[DEPLOYMENT] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1);
    assert!(INPUT_FDS[KEY] == COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1);
};

/// Native startup failures preserve the original ledger's accepted work and denial history.
#[derive(Debug)]
pub enum NativeExternalAnchorEntrypointErrorV2 {
    /// Work, storage, arithmetic or prepaid input accounting was refused.
    Resource(Resource),
    /// The process has arguments/environment, or an overlong/noncanonical argv0.
    RuntimeConfiguration,
    /// The current nonroot credentials differ from the actual deployment.
    Credentials,
    /// Native process or namespace admission failed.
    Profile(ProtectedServiceProfileErrorV2),
    /// Native sealed public configuration or key admission failed.
    Capability(CompilerExecutionCapabilityErrorV2),
    /// Running sealed executable admission failed.
    Executable(ProtectedStaticExecutableErrorV2),
    /// Root-owned lifecycle admission failed.
    Lifecycle(LifecycleLeaseErrorV2),
    /// Native durable state or serving failed.
    Anchor(NativeExternalAnchorErrorV2),
    /// Fixed descriptor transfer failed in the shared V1 mechanical helper.
    Descriptor(crate::ExternalAnchorEntrypointErrorV1),
    /// One bounded syscall failed; interruptions reject rather than retry.
    Io {
        operation: &'static str,
        source: rustix::io::Errno,
    },
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
            Self::Descriptor(e) => e.fmt(f),
            Self::RuntimeConfiguration => {
                f.write_str("native anchor requires one bounded argv0 and no environment")
            }
            Self::Credentials => {
                f.write_str("native anchor credentials do not match its deployment")
            }
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
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
            Self::Descriptor(e) => Some(e),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
macro_rules! from_error {
    ($T:ty, $variant:ident) => {
        impl From<$T> for Error {
            fn from(e: $T) -> Self {
                Self::$variant(e)
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
from_error!(crate::ExternalAnchorEntrypointErrorV1, Descriptor);

// This guard is constructed only at the unsafe isolated-process boundary. It
// closes unclaimed raw slots without interpreting secret descriptors on refusal.
struct Sources {
    live: [bool; INPUT_FDS.len()],
}
impl Sources {
    fn validate(&self) -> Result<()> {
        // Validate the whole occupied table before a duplicate can reuse a missing
        // input slot. This observes descriptor flags, never secret bytes.
        for fd in INPUT_FDS {
            crate::entrypoint::require_inherited(fd, "native source")?;
        }
        Ok(())
    }
    fn close(&mut self, index: usize) -> Result<()> {
        self.live[index] = false;
        crate::entrypoint::close_inherited(INPUT_FDS[index]).map_err(Into::into)
    }
    fn take(&mut self, index: usize, target: RawFd, label: &'static str) -> Result<OwnedFd> {
        let owned = crate::entrypoint::take_inherited_at(INPUT_FDS[index], target, label)?;
        self.live[index] = false;
        Ok(owned)
    }
}
impl Drop for Sources {
    fn drop(&mut self) {
        for (fd, live) in INPUT_FDS.iter().zip(self.live) {
            if live {
                // SAFETY: the unsafe caller transfers these unrepresented raw slots.
                // A missing slot reports EBADF; no close is retried.
                unsafe {
                    libc::close(*fd);
                }
            }
        }
    }
}

fn close_unrelated() -> Result<()> {
    for (low, high) in [(0_u32, 2_u32), (6, 201), (203, 219), (223, u32::MAX)] {
        // SAFETY: called only by the isolated-process entrypoint, before any
        // descriptor-owning native capability/executable/lease has been created.
        if unsafe { libc::syscall(libc::SYS_close_range, low, high, 0) } != 0 {
            return Err(Error::Io {
                operation: "close unrelated native anchor descriptors",
                source: rustix::io::Errno::from_raw_os_error(
                    std::io::Error::last_os_error()
                        .raw_os_error()
                        .unwrap_or(libc::EIO),
                ),
            });
        }
    }
    Ok(())
}

fn require_invocation() -> Result<()> {
    use rustix::{
        fs::{Mode, OFlags, open},
        io::read,
    };
    let io = |source| Error::Io {
        operation: "inspect native anchor invocation",
        source,
    };
    let command = open(
        c"/proc/self/cmdline",
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io)?;
    let mut bytes = [0_u8; MAX_ARGV0 + 1];
    let n = read(&command, &mut bytes).map_err(io)?;
    let mut extra = [0_u8; 1];
    if n < 2
        || n > MAX_ARGV0
        || bytes[n - 1] != 0
        || bytes[..n - 1].contains(&0)
        || read(&command, &mut extra).map_err(io)? != 0
    {
        return Err(Error::RuntimeConfiguration);
    }
    let environment = open(
        c"/proc/self/environ",
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io)?;
    if read(&environment, &mut extra).map_err(io)? != 0 {
        return Err(Error::RuntimeConfiguration);
    }
    Ok(())
}

struct Profile {
    process: Option<Process>,
    namespaces: Option<Namespaces>,
}
impl Profile {
    fn capture(b: &mut Budget<'_>) -> Result<Self> {
        let credentials = Credentials::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .map_err(|_| Error::Credentials)?;
        let (process, c) = Process::capture(credentials, b)?;
        b.reserve_storage(c.additional_storage())?;
        require_owned_sigchld_v2(b)?;
        let (namespaces, c) = Namespaces::capture_self(b)?;
        b.reserve_storage(c.additional_storage())?;
        let result = Self {
            process: Some(process),
            namespaces: Some(namespaces),
        };
        result.revalidate(b)?;
        Ok(result)
    }
    fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> {
        if let Some(p) = &self.process {
            p.revalidate_current(b)?;
        }
        if let Some(n) = &self.namespaces {
            n.revalidate_self(b)?;
        }
        if self.process.is_some() != self.namespaces.is_some() {
            return Err(Error::Credentials);
        }
        Ok(())
    }
}

trait StartupHooks {
    fn invocation(&mut self) -> Result<()> {
        require_invocation()
    }
    fn profile(&mut self, b: &mut Budget<'_>) -> Result<Profile> {
        Profile::capture(b)
    }
    fn executable(
        &mut self,
        m: Measurement,
        o: Owner,
        b: &mut Budget<'_>,
    ) -> Result<Option<Executable>> {
        let (e, c) = Executable::admit_running(m, o, "native external-anchor daemon", b)?;
        b.reserve_storage(c.additional_storage())?;
        Ok(Some(e))
    }
    fn lifecycle(&mut self, file: File, root: &OwnedFd, b: &mut Budget<'_>) -> Result<Lease> {
        let (lease, c) = Lease::admit(file, root, b)?;
        b.reserve_storage(c.additional_storage())?;
        Ok(lease)
    }
    fn ready(&mut self, _b: &mut Budget<'_>) -> Result<()> {
        Ok(())
    }
}
struct SystemStartup;
impl StartupHooks for SystemStartup {}

mod v2 {
    use super::*;
    use crate::{DurableExternalAnchorV2 as Anchor, serve_connected_peer_v2 as serve};
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionExternalAnchorDeploymentCapabilityV2 as Deployment,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as Key,
        CompilerExecutionPolicyCapabilityV2 as Policy,
        CompilerExecutionSupervisorDeploymentCapabilityV2 as Supervisor,
    };
    include!("native_startup_body.rs");
}
mod v3 {
    use super::*;
    use crate::{DurableExternalAnchorV3 as Anchor, serve_connected_peer_v3 as serve};
    use fe2o3_compiler_closure_capability::{
        CompilerExecutionExternalAnchorDeploymentCapabilityV3 as Deployment,
        CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as Key,
        CompilerExecutionPolicyCapabilityV3 as Policy,
        CompilerExecutionSupervisorDeploymentCapabilityV3 as Supervisor,
    };
    include!("native_startup_body.rs");
}

/// Complete prepaid source-slot charge for native V2 startup.
pub const NATIVE_EXTERNAL_ANCHOR_STARTUP_INPUT_STORAGE_V2: usize = v2::INPUT_STORAGE;
/// Complete prepaid source-slot charge for native V3 startup.
pub const NATIVE_EXTERNAL_ANCHOR_STARTUP_INPUT_STORAGE_V3: usize = v3::INPUT_STORAGE;

macro_rules! entrypoint {
    ($name:ident, $family:ident) => {
        /// Runs a native daemon from fixed descriptors, with no V1 custody fallback.
        /// Prepay the family's STARTUP_INPUT_STORAGE on the original ledger. Every
        /// input slot is consumed/closed on success, refusal or unwind; retire those
        /// reservations after return. Success returns the FULL unreserved report charge.
        /// Work, peak and first-denial history survive cleanup. Opens existing durable
        /// state only, never genesis. Requires independent trusted parent provenance.
        ///
        /// # Safety
        /// Call only once at entry to a dedicated, single-threaded daemon process.
        /// Slots 3/4/5/202/220/221/222 must be exclusively transferred raw descriptors
        /// (peer/root/lifecycle/policy/supervisor/deployment/key), not represented by
        /// Rust owners. There must be no other live Rust descriptor owners or I/O
        /// threads: unrelated descriptors, including standard streams, are closed.
        /// On failure the process must exit, not retry or enter another service.
        ///
        /// ```
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        #[doc = concat!("use fe2o3_external_anchor_service::", stringify!($name), " as run;")]
        /// unsafe fn dedicated_process(b: &mut Budget<'_>) {
        ///     let _ = unsafe { run(b) };
        /// }
        /// ```
        /// ```compile_fail
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        #[doc = concat!("use fe2o3_external_anchor_service::", stringify!($name), " as run;")]
        /// fn arbitrary_application(b: &mut Budget<'_>) { let _ = run(b); }
        /// ```
        pub unsafe fn $name(b: &mut Budget<'_>) -> Result<(Report, Storage)> {
            $family::run(b, &mut SystemStartup)
        }
    };
}
entrypoint!(run_inherited_external_anchor_service_v2, v2);
entrypoint!(run_inherited_external_anchor_service_v3, v3);

#[cfg(test)]
mod tests;
