//! Shared exact Linux process-profile admission for protected fe2o3 services.
//!
//! This crate validates identity and confinement facts only. Its values grant no signing,
//! compiler, publication, linking, loading, launch, execution, or GPU authority.

#![cfg(all(target_os = "linux", target_arch = "x86_64"))]

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};

const INVALID_ID: u32 = u32::MAX;
const MAX_PROC_STATUS_BYTES_V1: u64 = 64 * 1024;
const MAX_CAPABILITY_NUMBER_V1: u32 = 63;

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
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
}

impl ProtectedServiceProcessProfileV1 {
    pub fn capture(
        credentials: ProtectedServiceCredentialProfileV1,
    ) -> Result<Self, ProtectedServiceProfileErrorV1> {
        let profile = Self {
            credentials,
            cap_last_cap: read_cap_last_cap()?,
        };
        profile.revalidate_current()?;
        Ok(profile)
    }

    pub const fn credentials(&self) -> ProtectedServiceCredentialProfileV1 {
        self.credentials
    }

    pub const fn cap_last_cap(&self) -> u32 {
        self.cap_last_cap
    }

    pub fn revalidate_current(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        read_proc_status("/proc/self/status")?.require(self.credentials)?;
        let capabilities = rustix::thread::capabilities(None)
            .map_err(|source| io_error("inspect service capabilities", source.into()))?;
        if !capabilities.effective.is_empty()
            || !capabilities.permitted.is_empty()
            || !capabilities.inheritable.is_empty()
        {
            return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "effective, permitted, or inheritable capabilities are not empty",
            ));
        }
        let securebits = rustix::thread::capabilities_secure_bits()
            .map_err(|source| io_error("inspect service securebits", source.into()))?;
        if securebits.bits() != self.credentials.securebits() {
            return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "securebits are not exact and locked",
            ));
        }
        if !rustix::thread::no_new_privs()
            .map_err(|source| io_error("inspect service no_new_privs", source.into()))?
        {
            return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "no_new_privs is not set",
            ));
        }
        if rustix::process::dumpable_behavior()
            .map_err(|source| io_error("inspect service dumpability", source.into()))?
            != rustix::process::DumpableBehavior::NotDumpable
        {
            return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "process is dumpable",
            ));
        }
        let core = rustix::process::getrlimit(rustix::process::Resource::Core);
        if core.current != Some(0) || core.maximum != Some(0) {
            return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "core limit is not exactly zero",
            ));
        }
        if read_cap_last_cap()? != self.cap_last_cap {
            return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
                "kernel capability range changed",
            ));
        }
        Ok(())
    }

    pub fn revalidate_process(
        &self,
        pid: rustix::process::Pid,
    ) -> Result<(), ProtectedServiceProfileErrorV1> {
        validate_protected_service_process_v1(self.credentials, pid)
    }
}

/// Retained exact namespace identities for current-process and child revalidation.
pub struct ProtectedServiceNamespaceSetV1 {
    identities: [NamespaceIdentityV1; NAMESPACE_NAMES_V1.len()],
}

impl ProtectedServiceNamespaceSetV1 {
    pub fn capture_self() -> Result<Self, ProtectedServiceProfileErrorV1> {
        let identities = NAMESPACE_NAMES_V1
            .map(|name| namespace_identity(&format!("/proc/self/ns/{name}")))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .map_err(|_| {
                ProtectedServiceProfileErrorV1::InvalidState(
                    "namespace identity cardinality changed",
                )
            })?;
        let set = Self { identities };
        set.require_children_unchanged()?;
        Ok(set)
    }

    pub fn revalidate_self(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        for (index, name) in NAMESPACE_NAMES_V1.iter().enumerate() {
            if namespace_identity(&format!("/proc/self/ns/{name}"))? != self.identities[index] {
                return Err(ProtectedServiceProfileErrorV1::Namespace(name));
            }
        }
        self.require_children_unchanged()
    }

    pub fn revalidate_process(
        &self,
        pid: rustix::process::Pid,
    ) -> Result<(), ProtectedServiceProfileErrorV1> {
        for (index, name) in NAMESPACE_NAMES_V1.iter().enumerate() {
            if namespace_identity(&format!("/proc/{}/ns/{name}", pid.as_raw_pid()))?
                != self.identities[index]
            {
                return Err(ProtectedServiceProfileErrorV1::Namespace(name));
            }
        }
        Ok(())
    }

    fn require_children_unchanged(&self) -> Result<(), ProtectedServiceProfileErrorV1> {
        if self.identities[2] != self.identities[3] {
            return Err(ProtectedServiceProfileErrorV1::Namespace(
                "pid-for-children",
            ));
        }
        if self.identities[8] != self.identities[9] {
            return Err(ProtectedServiceProfileErrorV1::Namespace(
                "time-for-children",
            ));
        }
        Ok(())
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
    read_proc_status(&format!("/proc/{}/status", pid.as_raw_pid()))?.require(credentials)
}

/// Requires the default `SIGCHLD` disposition used for exclusive pidfd reaping.
#[allow(unsafe_code)]
pub fn require_owned_sigchld_v1() -> Result<(), ProtectedServiceProfileErrorV1> {
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::uninit();
    // SAFETY: sigaction with a null new action initializes exactly one old-action record.
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        return Err(io_error(
            "inspect service SIGCHLD ownership",
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: successful sigaction initialized the record.
    let action = unsafe { action.assume_init() };
    if action.sa_sigaction != libc::SIG_DFL
        || action.sa_flags & (libc::SA_NOCLDWAIT | libc::SA_NOCLDSTOP) != 0
    {
        return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
            "SIGCHLD disposition does not permit exclusive pidfd reaping",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcStatusProfileV1 {
    uid: [u32; 4],
    gid: [u32; 4],
    groups_empty: bool,
    capabilities_zero: bool,
    no_new_privs: u32,
    tracer_pid: u32,
    umask: u32,
}

impl ProcStatusProfileV1 {
    fn parse(bytes: &[u8]) -> Result<Self, ProtectedServiceProfileErrorV1> {
        let text = std::str::from_utf8(bytes).map_err(|_| {
            ProtectedServiceProfileErrorV1::ProcessProfile("proc status is not UTF-8")
        })?;
        let mut uid = None;
        let mut gid = None;
        let mut groups_empty = None;
        let mut capabilities = [None; 5];
        let mut no_new_privs = None;
        let mut tracer_pid = None;
        let mut umask = None;
        for line in text.lines() {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match name {
                "Uid" => set_once(&mut uid, parse_four_decimal(value)?)?,
                "Gid" => set_once(&mut gid, parse_four_decimal(value)?)?,
                "Groups" => set_once(&mut groups_empty, value.is_empty())?,
                "CapInh" => set_once(&mut capabilities[0], parse_hex_u64(value)?)?,
                "CapPrm" => set_once(&mut capabilities[1], parse_hex_u64(value)?)?,
                "CapEff" => set_once(&mut capabilities[2], parse_hex_u64(value)?)?,
                "CapBnd" => set_once(&mut capabilities[3], parse_hex_u64(value)?)?,
                "CapAmb" => set_once(&mut capabilities[4], parse_hex_u64(value)?)?,
                "NoNewPrivs" => set_once(&mut no_new_privs, parse_decimal(value)?)?,
                "TracerPid" => set_once(&mut tracer_pid, parse_decimal(value)?)?,
                "Umask" => set_once(&mut umask, parse_octal(value)?)?,
                _ => {}
            }
        }
        Ok(Self {
            uid: uid.ok_or(ProtectedServiceProfileErrorV1::ProcessProfile(
                "proc status lacks Uid",
            ))?,
            gid: gid.ok_or(ProtectedServiceProfileErrorV1::ProcessProfile(
                "proc status lacks Gid",
            ))?,
            groups_empty: groups_empty.ok_or(ProtectedServiceProfileErrorV1::ProcessProfile(
                "proc status lacks Groups",
            ))?,
            capabilities_zero: capabilities
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .ok_or(ProtectedServiceProfileErrorV1::ProcessProfile(
                    "proc status lacks a capability set",
                ))?
                .into_iter()
                .all(|value| value == 0),
            no_new_privs: no_new_privs.ok_or(ProtectedServiceProfileErrorV1::ProcessProfile(
                "proc status lacks NoNewPrivs",
            ))?,
            tracer_pid: tracer_pid.ok_or(ProtectedServiceProfileErrorV1::ProcessProfile(
                "proc status lacks TracerPid",
            ))?,
            umask: umask.ok_or(ProtectedServiceProfileErrorV1::ProcessProfile(
                "proc status lacks Umask",
            ))?,
        })
    }

    fn require(
        self,
        credentials: ProtectedServiceCredentialProfileV1,
    ) -> Result<(), ProtectedServiceProfileErrorV1> {
        let failure = if self.uid != [credentials.uid(); 4] {
            Some("real, effective, saved, or filesystem UID differs")
        } else if self.gid != [credentials.gid(); 4] {
            Some("real, effective, saved, or filesystem GID differs")
        } else if !self.groups_empty {
            Some("supplementary group set is not empty")
        } else if !self.capabilities_zero {
            Some("a capability set is not empty")
        } else if self.no_new_privs != 1 {
            Some("no_new_privs is not set")
        } else if self.tracer_pid != 0 {
            Some("service process is traced")
        } else if self.umask != 0o077 {
            Some("umask is not 077")
        } else {
            None
        };
        match failure {
            Some(reason) => Err(ProtectedServiceProfileErrorV1::ProcessProfile(reason)),
            None => Ok(()),
        }
    }
}

fn read_proc_status(path: &str) -> Result<ProcStatusProfileV1, ProtectedServiceProfileErrorV1> {
    let file = File::open(path).map_err(|source| io_error("open proc process status", source))?;
    let mut bytes = Vec::new();
    file.take(MAX_PROC_STATUS_BYTES_V1 + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io_error("read proc process status", source))?;
    if bytes.len() as u64 > MAX_PROC_STATUS_BYTES_V1 {
        return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
            "proc status exceeds the fixed bound",
        ));
    }
    ProcStatusProfileV1::parse(&bytes)
}

fn read_cap_last_cap() -> Result<u32, ProtectedServiceProfileErrorV1> {
    let text = std::fs::read_to_string("/proc/sys/kernel/cap_last_cap")
        .map_err(|source| io_error("read kernel capability ceiling", source))?;
    let value = text.trim().parse::<u32>().map_err(|_| {
        ProtectedServiceProfileErrorV1::ProcessProfile("kernel capability ceiling is malformed")
    })?;
    if value > MAX_CAPABILITY_NUMBER_V1 {
        return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
            "kernel capability ceiling exceeds the supported 64-bit set",
        ));
    }
    Ok(value)
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), ProtectedServiceProfileErrorV1> {
    if slot.replace(value).is_some() {
        return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
            "proc status duplicates a security field",
        ));
    }
    Ok(())
}

fn parse_four_decimal(value: &str) -> Result<[u32; 4], ProtectedServiceProfileErrorV1> {
    value
        .split_ascii_whitespace()
        .map(parse_decimal)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| {
            ProtectedServiceProfileErrorV1::ProcessProfile(
                "proc identity does not have four fields",
            )
        })
}

fn parse_decimal(value: &str) -> Result<u32, ProtectedServiceProfileErrorV1> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
            "proc decimal field is malformed",
        ));
    }
    value
        .parse()
        .map_err(|_| ProtectedServiceProfileErrorV1::ProcessProfile("proc decimal field overflows"))
}

fn parse_hex_u64(value: &str) -> Result<u64, ProtectedServiceProfileErrorV1> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
            "proc capability field is malformed",
        ));
    }
    u64::from_str_radix(value, 16).map_err(|_| {
        ProtectedServiceProfileErrorV1::ProcessProfile("proc capability field overflows")
    })
}

fn parse_octal(value: &str) -> Result<u32, ProtectedServiceProfileErrorV1> {
    if value.is_empty() || !value.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err(ProtectedServiceProfileErrorV1::ProcessProfile(
            "proc umask field is malformed",
        ));
    }
    u32::from_str_radix(value, 8)
        .map_err(|_| ProtectedServiceProfileErrorV1::ProcessProfile("proc umask field overflows"))
}

const NAMESPACE_NAMES_V1: [&str; 10] = [
    "user",
    "mnt",
    "pid",
    "pid_for_children",
    "net",
    "ipc",
    "uts",
    "cgroup",
    "time",
    "time_for_children",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NamespaceIdentityV1 {
    device: u64,
    inode: u64,
}

fn namespace_identity(path: &str) -> Result<NamespaceIdentityV1, ProtectedServiceProfileErrorV1> {
    let namespace = File::open(path).map_err(|source| io_error("open proc namespace", source))?;
    let stat = rustix::fs::fstat(&namespace)
        .map_err(|source| io_error("inspect proc namespace", source.into()))?;
    Ok(NamespaceIdentityV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

fn io_error(operation: &'static str, source: io::Error) -> ProtectedServiceProfileErrorV1 {
    ProtectedServiceProfileErrorV1::Io { operation, source }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXACT: &[u8] = b"Name:\ttest\nUmask:\t0077\nTracerPid:\t0\nUid:\t1000\t1000\t1000\t1000\nGid:\t1001\t1001\t1001\t1001\nGroups:\t\nCapInh:\t0000000000000000\nCapPrm:\t0000000000000000\nCapEff:\t0000000000000000\nCapBnd:\t0000000000000000\nCapAmb:\t0000000000000000\nNoNewPrivs:\t1\n";

    #[test]
    fn credential_profile_rejects_root_and_sentinel_identities() {
        assert_eq!(
            ProtectedServiceCredentialProfileV1::new(0, 1),
            Err(ProtectedServiceCredentialProfileErrorV1::InvalidUid)
        );
        assert_eq!(
            ProtectedServiceCredentialProfileV1::new(u32::MAX, 1),
            Err(ProtectedServiceCredentialProfileErrorV1::InvalidUid)
        );
        assert_eq!(
            ProtectedServiceCredentialProfileV1::new(1, 0),
            Err(ProtectedServiceCredentialProfileErrorV1::InvalidGid)
        );
        assert_eq!(
            ProtectedServiceCredentialProfileV1::new(1, u32::MAX),
            Err(ProtectedServiceCredentialProfileErrorV1::InvalidGid)
        );
    }

    #[test]
    fn strict_proc_profile_parser_accepts_only_the_exact_shape() {
        let credentials = ProtectedServiceCredentialProfileV1::new(1000, 1001).unwrap();
        ProcStatusProfileV1::parse(EXACT)
            .unwrap()
            .require(credentials)
            .unwrap();
        for hostile in [
            replace(EXACT, b"Umask:\t0077", b"Umask:\t0022"),
            replace(EXACT, b"TracerPid:\t0", b"TracerPid:\t9"),
            replace(
                EXACT,
                b"Uid:\t1000\t1000\t1000\t1000",
                b"Uid:\t1000\t1000\t1000\t1002",
            ),
            replace(
                EXACT,
                b"Gid:\t1001\t1001\t1001\t1001",
                b"Gid:\t1001\t1001\t1001\t1002",
            ),
            replace(EXACT, b"Groups:\t\n", b"Groups:\t1001\n"),
            replace(
                EXACT,
                b"CapEff:\t0000000000000000",
                b"CapEff:\t0000000000000001",
            ),
            replace(EXACT, b"NoNewPrivs:\t1", b"NoNewPrivs:\t0"),
        ] {
            assert!(
                ProcStatusProfileV1::parse(&hostile)
                    .and_then(|value| value.require(credentials))
                    .is_err()
            );
        }
    }

    #[test]
    fn duplicate_missing_and_malformed_fields_fail_closed() {
        let credentials = ProtectedServiceCredentialProfileV1::new(1000, 1001).unwrap();
        for hostile in [
            [EXACT, b"Uid:\t1000\t1000\t1000\t1000\n"].concat(),
            replace(EXACT, b"Groups:\t\n", b""),
            replace(EXACT, b"NoNewPrivs:\t1", b"NoNewPrivs:\tx"),
            replace(EXACT, b"CapBnd:\t0000000000000000", b"CapBnd:\tnot-hex"),
            replace(EXACT, b"Umask:\t0077", b"Umask:\t0088"),
        ] {
            assert!(
                ProcStatusProfileV1::parse(&hostile)
                    .and_then(|value| value.require(credentials))
                    .is_err()
            );
        }
    }

    #[test]
    fn current_namespace_snapshot_revalidates_without_drift() {
        let namespaces = ProtectedServiceNamespaceSetV1::capture_self().unwrap();
        namespaces.revalidate_self().unwrap();
    }

    fn replace(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
        let offset = bytes
            .windows(from.len())
            .position(|part| part == from)
            .expect("test field exists");
        let mut replaced = Vec::with_capacity(bytes.len() - from.len() + to.len());
        replaced.extend_from_slice(&bytes[..offset]);
        replaced.extend_from_slice(to);
        replaced.extend_from_slice(&bytes[offset + from.len()..]);
        replaced
    }
}
