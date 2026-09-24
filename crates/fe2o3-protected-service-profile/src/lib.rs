//! Shared exact Linux process-profile admission for protected fe2o3 services.
//!
//! This crate validates identity and confinement facts only. Its values grant no signing,
//! compiler, publication, linking, loading, launch, execution, or GPU authority.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::error::Error;
use std::fmt;
use std::io;

pub mod observations;

mod native;
pub use native::{
    ProtectedServiceNamespaceSetV2, ProtectedServiceProcessProfileV2,
    ProtectedServiceProfileErrorV2, ProtectedServiceProfileStorageV2,
    REQUIRE_OWNED_SIGCHLD_SCRATCH_V2, REQUIRE_OWNED_SIGCHLD_WORK_V2, require_owned_sigchld_v2,
};

#[cfg(test)]
mod native_tests;

const INVALID_ID: u32 = u32::MAX;

const SECBIT_NOROOT: u32 = 1 << 0;
const SECBIT_NOROOT_LOCKED: u32 = 1 << 1;
const SECBIT_NO_SETUID_FIXUP: u32 = 1 << 2;
const SECBIT_NO_SETUID_FIXUP_LOCKED: u32 = 1 << 3;
const SECBIT_KEEP_CAPS_LOCKED: u32 = 1 << 5;
const SECBIT_NO_CAP_AMBIENT_RAISE: u32 = 1 << 6;
const SECBIT_NO_CAP_AMBIENT_RAISE_LOCKED: u32 = 1 << 7;

/// Exact securebits value required by every protected service process.
///
/// Root privilege, set-ID capability fixups, retained capabilities, and future
/// ambient-capability raises are all disabled and locked. `KEEP_CAPS` itself is clear while its
/// lock bit is set.
pub const PROTECTED_SERVICE_SECUREBITS_V1: u32 = SECBIT_NOROOT
    | SECBIT_NOROOT_LOCKED
    | SECBIT_NO_SETUID_FIXUP
    | SECBIT_NO_SETUID_FIXUP_LOCKED
    | SECBIT_KEEP_CAPS_LOCKED
    | SECBIT_NO_CAP_AMBIENT_RAISE
    | SECBIT_NO_CAP_AMBIENT_RAISE_LOCKED;

#[allow(unsafe_code)]
mod secure_start {
    core::arch::global_asm!(include_str!("secure_start_x86_64.S"), options(att_syntax));

    unsafe extern "C" {
        fn fe2o3_secure_start_v1();
    }

    /// Retains and returns the shared syscall-only protected-service entrypoint.
    ///
    /// Static protected binaries reference this function and select the returned symbol as their
    /// ELF entry address. The assembly repeats nondumpability, `no_new_privs`, and the zero core
    /// limit before libc or Rust startup can inspect inherited descriptors.
    #[inline(never)]
    pub fn protected_service_secure_start_address_v1() -> usize {
        fe2o3_secure_start_v1 as *const () as usize
    }
}

pub use secure_start::protected_service_secure_start_address_v1;

/// Stable failure constructing one protected-service credential profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedServiceCredentialProfileErrorV1 {
    InvalidUid,
    InvalidGid,
}

impl fmt::Display for ProtectedServiceCredentialProfileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidUid => "invalid protected service UID",
            Self::InvalidGid => "invalid protected service GID",
        })
    }
}

impl Error for ProtectedServiceCredentialProfileErrorV1 {}

/// Trusted configuration for one dedicated protected-service identity.
///
/// The process must have all real, effective, saved, and filesystem IDs equal to this non-root
/// identity; no supplementary groups; empty effective, permitted, inheritable, ambient, and
/// bounding capability sets; [`PROTECTED_SERVICE_SECUREBITS_V1`]; `no_new_privs=1`; `dumpable=0`;
/// a zero core limit; umask `077`; an owned default `SIGCHLD` disposition; and stable user, mount,
/// PID, network, IPC, UTS, cgroup, and time namespaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedServiceCredentialProfileV1 {
    uid: u32,
    gid: u32,
}

impl ProtectedServiceCredentialProfileV1 {
    pub const fn new(uid: u32, gid: u32) -> Result<Self, ProtectedServiceCredentialProfileErrorV1> {
        if uid == 0 || uid == INVALID_ID {
            return Err(ProtectedServiceCredentialProfileErrorV1::InvalidUid);
        }
        if gid == 0 || gid == INVALID_ID {
            return Err(ProtectedServiceCredentialProfileErrorV1::InvalidGid);
        }
        Ok(Self { uid, gid })
    }

    pub const fn uid(self) -> u32 {
        self.uid
    }

    pub const fn gid(self) -> u32 {
        self.gid
    }

    pub const fn securebits(self) -> u32 {
        PROTECTED_SERVICE_SECUREBITS_V1
    }
}

/// Stable failure observing an exact protected-service process profile.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedServiceProfileErrorV1 {
    ProcessProfile(&'static str),
    Namespace(&'static str),
    InvalidState(&'static str),
    Io {
        operation: &'static str,
        source: io::Error,
    },
}

impl fmt::Display for ProtectedServiceProfileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessProfile(reason) => {
                write!(
                    formatter,
                    "protected service process profile mismatch: {reason}"
                )
            }
            Self::Namespace(namespace) => {
                write!(
                    formatter,
                    "protected service namespace changed: {namespace}"
                )
            }
            Self::InvalidState(reason) => {
                write!(
                    formatter,
                    "invalid protected service profile state: {reason}"
                )
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ProtectedServiceProfileErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Retained exact process-profile facts for current-process and child revalidation.
pub struct ProtectedServiceProcessProfileV1 {
    observation: observations::ProcessProfile,
}

impl ProtectedServiceProcessProfileV1 {
    pub fn capture(
        credentials: ProtectedServiceCredentialProfileV1,
    ) -> Result<Self, ProtectedServiceProfileErrorV1> {
        observations::ProcessProfile::capture(credentials)
            .map(|observation| Self { observation })
            .map_err(map_observation_error)
    }

    pub const fn credentials(&self) -> ProtectedServiceCredentialProfileV1 {
        self.observation.credentials()
    }

    pub const fn cap_last_cap(&self) -> u32 {
        self.observation.cap_last_cap()
    }

    pub fn revalidate_current(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        self.observation
            .revalidate_current()
            .map_err(map_observation_error)
    }

    pub fn revalidate_process(
        &self,
        pid: rustix::process::Pid,
    ) -> Result<(), ProtectedServiceProfileErrorV1> {
        self.observation
            .revalidate_process(pid)
            .map_err(map_observation_error)
    }
}

/// Retained exact namespace identities for current-process and child revalidation.
pub struct ProtectedServiceNamespaceSetV1 {
    observation: observations::NamespaceSet,
}

impl ProtectedServiceNamespaceSetV1 {
    pub fn capture_self() -> Result<Self, ProtectedServiceProfileErrorV1> {
        observations::NamespaceSet::capture_self()
            .map(|observation| Self { observation })
            .map_err(map_observation_error)
    }

    pub fn revalidate_self(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        self.observation
            .revalidate_self()
            .map_err(map_observation_error)
    }

    pub fn revalidate_process(
        &self,
        pid: rustix::process::Pid,
    ) -> Result<(), ProtectedServiceProfileErrorV1> {
        self.observation
            .revalidate_process(pid)
            .map_err(map_observation_error)
    }
}

/// Validates the complete current locked service profile.
pub fn validate_current_protected_service_profile_v1(
    credentials: ProtectedServiceCredentialProfileV1,
) -> Result<(), ProtectedServiceProfileErrorV1> {
    ProtectedServiceProcessProfileV1::capture(credentials)?;
    require_owned_sigchld_v1()?;
    ProtectedServiceNamespaceSetV1::capture_self()?.revalidate_self()
}

/// Validates every proc-visible security field for one gated protected-service child.
///
/// This parent-side observation requires exact real, effective, saved, and filesystem IDs, no
/// supplementary groups, empty capability sets including bounding and ambient sets,
/// `no_new_privs=1`, no tracer, and umask `077`. The child must separately validate securebits,
/// dumpability, core limits, signal state, and namespace continuity before protected execution.
pub fn validate_protected_service_process_v1(
    credentials: ProtectedServiceCredentialProfileV1,
    pid: rustix::process::Pid,
) -> Result<(), ProtectedServiceProfileErrorV1> {
    observations::validate_process(credentials, pid).map_err(map_observation_error)
}

/// Requires the default `SIGCHLD` disposition used for exclusive pidfd reaping.
pub fn require_owned_sigchld_v1() -> Result<(), ProtectedServiceProfileErrorV1> {
    observations::require_owned_sigchld().map_err(map_observation_error)
}

fn map_observation_error(error: observations::Error) -> ProtectedServiceProfileErrorV1 {
    match error {
        observations::Error::ProcessProfile(reason) => {
            ProtectedServiceProfileErrorV1::ProcessProfile(reason)
        }
        observations::Error::Namespace(name) => ProtectedServiceProfileErrorV1::Namespace(name),
        observations::Error::InvalidState(reason) => {
            ProtectedServiceProfileErrorV1::InvalidState(reason)
        }
        observations::Error::Io { operation, source } => {
            let source = if operation == "read kernel capability ceiling"
                && source == rustix::io::Errno::ILSEQ
            {
                // Preserve the previous read_to_string UTF-8 failure at this boundary.
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "stream did not contain valid UTF-8",
                )
            } else {
                source.into()
            };
            ProtectedServiceProfileErrorV1::Io { operation, source }
        }
    }
}
