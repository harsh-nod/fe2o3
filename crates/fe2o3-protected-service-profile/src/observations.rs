//! Bounded, allocation-free mechanical observations of Linux process confinement.
//!
//! These snapshots accept inert credential configuration and grant no authority.
//! They never construct a legacy admitted owner. Each syscall is attempted once;
//! interruption fails closed. Concurrent process changes are not made atomic.
//!
//! WORK counts logical byte/field operations and weighted syscall attempts,
//! including descriptor cleanup. SCRATCH counts logical staging bytes, including
//! conservative fixed control/temporary allowances. Neither bounds generated
//! stack size, elapsed time, RSS, kernel memory, or kernel-side syscall work.
//! Callers must prepay the corresponding bound before entering an observation.

use crate::ProtectedServiceCredentialProfileV1;
use rustix::{io::Errno, process::Pid};
use std::{fmt, mem::size_of};

#[path = "observation_io.rs"]
mod bounded_io;
#[path = "observation_status.rs"]
mod status;

use bounded_io::{PROC_PATH_BYTES, ProcPath};
use status::{ProcStatusProfile, read_proc_status};

/// Maximum accepted status snapshot; its fixed buffer has one extra sentinel byte.
pub const MAX_PROC_STATUS_BYTES: usize = 64 * 1024;
/// Maximum accepted capability ceiling text, including whitespace and newline.
pub const MAX_CAP_LAST_CAP_BYTES: usize = 64;
const MAX_CAPABILITY_NUMBER: u32 = 63;

// One logical syscall attempt has weight 1024, independent of its latency.
// A bounded N-byte file needs at most N+1 reads, plus open and close.
// Status parsing: 64 units per byte cover zeroing, UTF-8, line/name scans,
// trimming, whitespace splitting, numeric validation/conversion, and field
// dispatch (12 names), presence checks and comparisons. A token/line cannot
// outnumber bytes. 8 units per path byte cover build/validation/decimal format;
// 256 extra units cover fixed control and final profile comparisons.
const SYSCALL_WORK: usize = 1024;
const STATUS_WORK: usize = 64 * (MAX_PROC_STATUS_BYTES + 1)
    + (MAX_PROC_STATUS_BYTES + 3) * SYSCALL_WORK
    + 8 * PROC_PATH_BYTES
    + 256;
// Ceiling parsing has no field dispatch: 16 units per byte cover buffer
// zeroing, reading, UTF-8, trimming and decimal conversion; 256 cover control.
const CEILING_WORK: usize =
    16 * (MAX_CAP_LAST_CAP_BYTES + 1) + (MAX_CAP_LAST_CAP_BYTES + 3) * SYSCALL_WORK + 256;
// Eight profile/error slots cover accumulators, returned values and temporaries;
// 1024 covers scalar counters, references, the fd and read/parser control.
const STATUS_SCRATCH: usize = MAX_PROC_STATUS_BYTES
    + 1
    + PROC_PATH_BYTES
    + size_of::<rustix::path::DecInt>()
    + 8 * size_of::<ProcStatusProfile>()
    + 8 * size_of::<Error>()
    + 1024;
const CEILING_SCRATCH: usize = MAX_CAP_LAST_CAP_BYTES + 1 + 1024;

/// Worst-case work for `validate_process` and `ProcessProfile::revalidate_process`.
pub const PROCESS_VALIDATE_WORK: usize = STATUS_WORK;
/// Scratch for `validate_process` and `ProcessProfile::revalidate_process`.
pub const PROCESS_VALIDATE_SCRATCH: usize = STATUS_SCRATCH;
/// Status, capability ceiling and five calls: capget, three prctls, getrlimit.
pub const PROCESS_CURRENT_WORK: usize = STATUS_WORK + CEILING_WORK + 5 * SYSCALL_WORK + 256;
/// Sum of both staging phases, plus capability/rlimit/scalar temporaries.
pub const PROCESS_CURRENT_SCRATCH: usize = STATUS_SCRATCH + CEILING_SCRATCH + 1024;
/// Initial ceiling observation followed by full current-process revalidation.
pub const PROCESS_CAPTURE_WORK: usize = CEILING_WORK + PROCESS_CURRENT_WORK + 256;
/// Both observation allowances plus the retained profile under construction.
pub const PROCESS_CAPTURE_SCRATCH: usize =
    CEILING_SCRATCH + PROCESS_CURRENT_SCRATCH + size_of::<ProcessProfile>();

// Each namespace needs open, fstat and close, one bounded path and an identity
// comparison. The fixed array has ten entries. 256 covers child comparisons
// and loop control. Scratch includes the array, one observation and temporaries.
const NAMESPACE_WORK: usize =
    NAMESPACES.len() * (3 * SYSCALL_WORK + 8 * PROC_PATH_BYTES + 64) + 256;
const NAMESPACE_SCRATCH: usize = size_of::<NamespaceSet>()
    + PROC_PATH_BYTES
    + size_of::<rustix::path::DecInt>()
    + size_of::<rustix::fs::Stat>()
    + 8 * size_of::<NamespaceIdentity>()
    + 8 * size_of::<Error>()
    + 1024;
/// Ten namespace identities followed by PID/time child-namespace checks.
pub const NAMESPACE_CAPTURE_WORK: usize = NAMESPACE_WORK;
/// Fixed namespace capture staging.
pub const NAMESPACE_CAPTURE_SCRATCH: usize = NAMESPACE_SCRATCH;
/// Ten identity comparisons followed by PID/time child-namespace checks.
pub const NAMESPACE_SELF_WORK: usize = NAMESPACE_WORK;
/// Fixed current-process namespace staging.
pub const NAMESPACE_SELF_SCRATCH: usize = NAMESPACE_SCRATCH;
/// Ten process namespace identity comparisons.
pub const NAMESPACE_PROCESS_WORK: usize = NAMESPACE_WORK;
/// Fixed other-process namespace staging.
pub const NAMESPACE_PROCESS_SCRATCH: usize = NAMESPACE_SCRATCH;
/// One sigaction observation and fixed disposition/flag checks.
pub const SIGCHLD_WORK: usize = SYSCALL_WORK + 256;
/// One sigaction result plus fixed error/scalar/control staging.
pub const SIGCHLD_SCRATCH: usize = size_of::<libc::sigaction>() + size_of::<Error>() + 256;

/// Fixed failures, with no owned strings, boxes, or heap-backed I/O errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    ProcessProfile(&'static str),
    Namespace(&'static str),
    InvalidState(&'static str),
    Io {
        operation: &'static str,
        source: Errno,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProcessProfile(reason) => {
                write!(
                    formatter,
                    "protected service process profile mismatch: {reason}"
                )
            }
            Self::Namespace(name) => {
                write!(formatter, "protected service namespace changed: {name}")
            }
            Self::InvalidState(reason) => {
                write!(
                    formatter,
                    "invalid protected service profile state: {reason}"
                )
            }
            Self::Io { operation, source } => {
                write!(formatter, "{operation}: OS error {}", source.raw_os_error())
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Retained mechanical facts, independent of native or legacy admitted owners.
pub struct ProcessProfile {
    credentials: ProtectedServiceCredentialProfileV1,
    cap_last_cap: u32,
}

impl ProcessProfile {
    /// Observes the ceiling, then revalidates the complete current profile.
    pub fn capture(credentials: ProtectedServiceCredentialProfileV1) -> Result<Self, Error> {
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

    pub fn revalidate_current(&self) -> Result<(), Error> {
        read_proc_status(c"/proc/self/status")?.require(self.credentials)?;
        let capabilities = rustix::thread::capabilities(None)
            .map_err(|source| io_error("inspect service capabilities", source))?;
        if !capabilities.effective.is_empty()
            || !capabilities.permitted.is_empty()
            || !capabilities.inheritable.is_empty()
        {
            return Err(Error::ProcessProfile(
                "effective, permitted, or inheritable capabilities are not empty",
            ));
        }
        let securebits = rustix::thread::capabilities_secure_bits()
            .map_err(|source| io_error("inspect service securebits", source))?;
        if securebits.bits() != self.credentials.securebits() {
            return Err(Error::ProcessProfile("securebits are not exact and locked"));
        }
        if !rustix::thread::no_new_privs()
            .map_err(|source| io_error("inspect service no_new_privs", source))?
        {
            return Err(Error::ProcessProfile("no_new_privs is not set"));
        }
        if rustix::process::dumpable_behavior()
            .map_err(|source| io_error("inspect service dumpability", source))?
            != rustix::process::DumpableBehavior::NotDumpable
        {
            return Err(Error::ProcessProfile("process is dumpable"));
        }
        let core = rustix::process::getrlimit(rustix::process::Resource::Core);
        if core.current != Some(0) || core.maximum != Some(0) {
            return Err(Error::ProcessProfile("core limit is not exactly zero"));
        }
        if read_cap_last_cap()? != self.cap_last_cap {
            return Err(Error::ProcessProfile("kernel capability range changed"));
        }
        Ok(())
    }

    /// Observes proc-visible fields only; the child must check its local state.
    pub fn revalidate_process(&self, pid: Pid) -> Result<(), Error> {
        validate_process(self.credentials, pid)
    }
}

/// Retained identities in the established user/mount/PID/network/IPC/UTS/cgroup/time order.
pub struct NamespaceSet {
    identities: [NamespaceIdentity; NAMESPACES.len()],
}

impl NamespaceSet {
    pub fn capture_self() -> Result<Self, Error> {
        let mut identities = [NamespaceIdentity {
            device: 0,
            inode: 0,
        }; NAMESPACES.len()];
        for (index, (_, suffix)) in NAMESPACES.iter().enumerate() {
            identities[index] = namespace_identity(None, suffix)?;
        }
        let set = Self { identities };
        set.require_children_unchanged()?;
        Ok(set)
    }

    pub fn revalidate_self(&self) -> Result<(), Error> {
        self.revalidate(None)?;
        self.require_children_unchanged()
    }

    pub fn revalidate_process(&self, pid: Pid) -> Result<(), Error> {
        self.revalidate(Some(pid))
    }

    fn revalidate(&self, pid: Option<Pid>) -> Result<(), Error> {
        for (index, (name, suffix)) in NAMESPACES.iter().enumerate() {
            if namespace_identity(pid, suffix)? != self.identities[index] {
                return Err(Error::Namespace(name));
            }
        }
        Ok(())
    }

    fn require_children_unchanged(&self) -> Result<(), Error> {
        if self.identities[2] != self.identities[3] {
            return Err(Error::Namespace("pid-for-children"));
        }
        if self.identities[8] != self.identities[9] {
            return Err(Error::Namespace("time-for-children"));
        }
        Ok(())
    }
}

/// Checks the same proc-visible security fields as process-profile revalidation.
pub fn validate_process(
    credentials: ProtectedServiceCredentialProfileV1,
    pid: Pid,
) -> Result<(), Error> {
    let path = ProcPath::new(Some(pid), "status")?;
    read_proc_status(path.as_c_str()?)?.require(credentials)
}

/// Requires default SIGCHLD without SA_NOCLDWAIT or SA_NOCLDSTOP.
#[allow(unsafe_code)]
pub fn require_owned_sigchld() -> Result<(), Error> {
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::uninit();
    // SAFETY: a null new action only observes and initializes the old action.
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        // SAFETY: Linux libc supplies this thread's errno slot; the failed call
        // set it, and no intervening libc call has changed it.
        let source = Errno::from_raw_os_error(unsafe { *libc::__errno_location() });
        return Err(io_error("inspect service SIGCHLD ownership", source));
    }
    // SAFETY: successful sigaction initialized the old-action record.
    let action = unsafe { action.assume_init() };
    if action.sa_sigaction != libc::SIG_DFL
        || action.sa_flags & (libc::SA_NOCLDWAIT | libc::SA_NOCLDSTOP) != 0
    {
        return Err(Error::ProcessProfile(
            "SIGCHLD disposition does not permit exclusive pidfd reaping",
        ));
    }
    Ok(())
}

fn read_cap_last_cap() -> Result<u32, Error> {
    let file = bounded_io::open(
        c"/proc/sys/kernel/cap_last_cap",
        "read kernel capability ceiling",
    )?;
    let mut bytes = [0; MAX_CAP_LAST_CAP_BYTES + 1];
    let len = bounded_io::read_bounded(
        &mut bytes,
        "read kernel capability ceiling",
        "kernel capability ceiling exceeds the fixed bound",
        |buffer| rustix::io::read(&file, buffer),
    )?;
    parse_cap_last_cap(&bytes[..len])
}

fn parse_cap_last_cap(bytes: &[u8]) -> Result<u32, Error> {
    // EILSEQ retains an allocation-free encoding error; the legacy adapter
    // restores read_to_string's InvalidData error category and message.
    let text = std::str::from_utf8(bytes)
        .map_err(|_| io_error("read kernel capability ceiling", Errno::ILSEQ))?;
    let value = text
        .trim()
        .parse::<u32>()
        .map_err(|_| Error::ProcessProfile("kernel capability ceiling is malformed"))?;
    if value > MAX_CAPABILITY_NUMBER {
        return Err(Error::ProcessProfile(
            "kernel capability ceiling exceeds the supported 64-bit set",
        ));
    }
    Ok(value)
}

const NAMESPACES: [(&str, &str); 10] = [
    ("user", "ns/user"),
    ("mnt", "ns/mnt"),
    ("pid", "ns/pid"),
    ("pid_for_children", "ns/pid_for_children"),
    ("net", "ns/net"),
    ("ipc", "ns/ipc"),
    ("uts", "ns/uts"),
    ("cgroup", "ns/cgroup"),
    ("time", "ns/time"),
    ("time_for_children", "ns/time_for_children"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NamespaceIdentity {
    device: u64,
    inode: u64,
}

fn namespace_identity(pid: Option<Pid>, suffix: &str) -> Result<NamespaceIdentity, Error> {
    let path = ProcPath::new(pid, suffix)?;
    let namespace = bounded_io::open(path.as_c_str()?, "open proc namespace")?;
    let stat = rustix::fs::fstat(&namespace)
        .map_err(|source| io_error("inspect proc namespace", source))?;
    Ok(NamespaceIdentity {
        device: stat.st_dev,
        inode: stat.st_ino,
    })
}

fn io_error(operation: &'static str, source: Errno) -> Error {
    Error::Io { operation, source }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
