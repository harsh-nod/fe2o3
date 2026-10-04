//! Shared Linux pidfd observations, not authentication or execution authority.
//!
//! The selected procfs mount must be compatible with the caller's PID namespace.
//! Numeric-self consistency is checked, but is not mount-namespace attestation.
//! These probes never open a pidfd, signal, wait for, or reap a process.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, OwnedFd};

/// Stable failure classification for an inert pidfd observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PidfdObservationErrorKindV1 {
    ExpectedPid,
    InspectPidfd,
    InspectStartTime,
    CloseOnExec,
    Thread,
    TargetMismatch,
    IdentityChanged,
    StartTimeChanged,
    AlreadyDead,
}

/// Error retaining the original syscall failure when one is available.
#[derive(Debug)]
pub struct PidfdObservationErrorV1 {
    kind: PidfdObservationErrorKindV1,
    message: String,
    source: Option<io::Error>,
}

impl PidfdObservationErrorV1 {
    fn new(kind: PidfdObservationErrorKindV1, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    fn io(
        kind: PidfdObservationErrorKindV1,
        message: impl Into<String>,
        source: io::Error,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source),
        }
    }

    pub const fn kind(&self) -> PidfdObservationErrorKindV1 {
        self.kind
    }

    /// Preserves the original syscall error when adapting to a caller's error vocabulary.
    pub fn into_parts(self) -> (PidfdObservationErrorKindV1, String, Option<io::Error>) {
        (self.kind, self.message, self.source)
    }
}

impl fmt::Display for PidfdObservationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for PidfdObservationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_ref().map(|e| e as &(dyn Error + 'static))
    }
}

const MAX_PIDFD_FDINFO_BYTES: u64 = 4096;
const MAX_PROC_STAT_BYTES: u64 = 4096;
const PIDFD_INFO_PID_V0: u64 = 1 << 0;
const PIDFS_IOCTL_MAGIC: u32 = 0xff;
const PIDFD_GET_INFO_NUMBER: u32 = 11;

// Linux UAPI pidfd_info version 0 is exactly 64 bytes. Keeping a local layout also keeps the
// ioctl opcode at the v0 size if a later libc exposes a larger structure version.
#[repr(C)]
struct PidfdInfoV0 {
    mask: u64,
    cgroupid: u64,
    pid: u32,
    tgid: u32,
    ppid: u32,
    ruid: u32,
    rgid: u32,
    euid: u32,
    egid: u32,
    suid: u32,
    sgid: u32,
    fsuid: u32,
    fsgid: u32,
    exit_code: i32,
}

const _: () = assert!(std::mem::size_of::<PidfdInfoV0>() == 64);
const PIDFD_GET_INFO_V0: libc::Ioctl =
    libc::_IOWR::<PidfdInfoV0>(PIDFS_IOCTL_MAGIC, PIDFD_GET_INFO_NUMBER);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PidfdIdentitySourceV1 {
    KernelIoctl,
    ProcfsFdinfo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PidfdTargetObservationV1 {
    pub pid: u32,
    pub source: PidfdIdentitySourceV1,
}

pub fn require_process_pidfd_mode(pidfd: &OwnedFd) -> Result<(), PidfdObservationErrorV1> {
    let flags = rustix::fs::fcntl_getfl(pidfd).map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot inspect process pidfd file status flags",
            io::Error::from(error),
        )
    })?;
    // Linux v6.12 UAPI defines PIDFD_THREAD as exactly O_EXCL. This contract rejects only that
    // identified process-vs-thread selector and does not require unrelated flag bits to be zero.
    if flags.contains(rustix::fs::OFlags::EXCL) {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::Thread,
            "process pidfd has Linux PIDFD_THREAD (O_EXCL) semantics",
        ));
    }
    Ok(())
}

pub fn inspect_pidfd_target(
    pidfd: &OwnedFd,
) -> Result<PidfdTargetObservationV1, PidfdObservationErrorV1> {
    // SAFETY: PidfdInfoV0 contains only integer fields, so all-zero is a valid request. The ioctl
    // reads the request mask and initializes fields indicated by the returned mask.
    let mut info = unsafe { MaybeUninit::<PidfdInfoV0>::zeroed().assume_init() };
    info.mask = PIDFD_INFO_PID_V0;
    // SAFETY: `info` is writable for the exact 64-byte v0 type encoded by PIDFD_GET_INFO_V0 and the
    // descriptor remains borrowed for the call.
    let result = unsafe { libc::ioctl(pidfd.as_raw_fd(), PIDFD_GET_INFO_V0, &mut info) };
    if result == 0 {
        if info.mask & PIDFD_INFO_PID_V0 == 0 || info.pid == 0 || info.tgid != info.pid {
            return Err(PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::InspectPidfd,
                "PIDFD_GET_INFO omitted a usable process-leader target PID",
            ));
        }
        return Ok(PidfdTargetObservationV1 {
            pid: info.pid,
            source: PidfdIdentitySourceV1::KernelIoctl,
        });
    }

    dispatch_pidfd_get_info_error(pidfd, io::Error::last_os_error())
}

fn dispatch_pidfd_get_info_error(
    pidfd: &OwnedFd,
    error: io::Error,
) -> Result<PidfdTargetObservationV1, PidfdObservationErrorV1> {
    match error.raw_os_error() {
        // Linux v6.12 checks for a nonzero pidfd ioctl argument before its command switch, so the
        // pointer-bearing v0 info request returns EINVAL. Linux v6.13 dispatches PIDFD_GET_INFO
        // before that check. Neither errno proves descriptor type: only strict kernel procfs
        // inspection below can make this fallback succeed.
        Some(libc::ENOTTY) | Some(libc::EINVAL) => inspect_pidfd_target_from_procfs(pidfd),
        Some(libc::ESRCH) => Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::AlreadyDead,
            "process pidfd target exited before identity inspection",
        )),
        _ => Err(PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot inspect process pidfd with PIDFD_GET_INFO",
            error,
        )),
    }
}

fn inspect_pidfd_target_from_procfs(
    pidfd: &OwnedFd,
) -> Result<PidfdTargetObservationV1, PidfdObservationErrorV1> {
    let self_entry = open_validated_procfs_self()?;
    let directory_flags = rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
        | rustix::fs::OFlags::CLOEXEC;
    let fdinfo: File = rustix::fs::openat(
        &self_entry,
        "fdinfo",
        directory_flags,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot open the retained procfs fdinfo directory",
            io::Error::from(error),
        )
    })?;
    require_procfs(&fdinfo, "retained /proc/self/fdinfo directory")?;
    let record_flags =
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC;
    let mut record: File = rustix::fs::openat(
        &fdinfo,
        pidfd.as_raw_fd().to_string(),
        record_flags,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot open the bounded procfs process pidfd identity record",
            io::Error::from(error),
        )
    })?;
    require_procfs(&record, "process pidfd identity record")?;

    let mut contents = String::new();
    record
        .by_ref()
        .take(MAX_PIDFD_FDINFO_BYTES + 1)
        .read_to_string(&mut contents)
        .map_err(|error| {
            PidfdObservationErrorV1::io(
                PidfdObservationErrorKindV1::InspectPidfd,
                "cannot read the bounded procfs process pidfd identity record",
                error,
            )
        })?;
    if contents.len() as u64 > MAX_PIDFD_FDINFO_BYTES {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectPidfd,
            "procfs process pidfd identity record exceeds 4096 bytes",
        ));
    }

    let pid = parse_pidfd_fdinfo(&contents)?;
    Ok(PidfdTargetObservationV1 {
        pid,
        source: PidfdIdentitySourceV1::ProcfsFdinfo,
    })
}

fn parse_pidfd_fdinfo(contents: &str) -> Result<u32, PidfdObservationErrorV1> {
    let mut pid_value = None;
    let mut flags_value = None;
    for line in contents.lines() {
        if let Some(field) = line.strip_prefix("Pid:") {
            let value = field.strip_prefix('\t').ok_or_else(|| {
                PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has a malformed Pid field",
                )
            })?;
            if pid_value.is_some() {
                return Err(PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has duplicate Pid fields",
                ));
            }
            let canonical_positive_or_zero = !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value.len() == 1 || !value.starts_with('0'));
            if value != "-1" && !canonical_positive_or_zero {
                return Err(PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has a non-canonical decimal Pid field",
                ));
            }
            pid_value = Some(value.parse::<i64>().map_err(|_| {
                PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has a malformed Pid field",
                )
            })?);
        }
        if let Some(field) = line.strip_prefix("flags:") {
            let value = field.strip_prefix('\t').ok_or_else(|| {
                PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has a malformed flags field",
                )
            })?;
            if flags_value.is_some() {
                return Err(PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has duplicate flags fields",
                ));
            }
            if value.len() < 2
                || !value.starts_with('0')
                || !value.bytes().all(|byte| matches!(byte, b'0'..=b'7'))
            {
                return Err(PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has a malformed octal flags field",
                ));
            }
            flags_value = Some(u32::from_str_radix(value, 8).map_err(|_| {
                PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::InspectPidfd,
                    "procfs process pidfd identity record has an out-of-range octal flags field",
                )
            })?);
        }
    }
    let pid_value = pid_value.ok_or_else(|| {
        PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectPidfd,
            "descriptor is not a pidfd with a procfs Pid identity field",
        )
    })?;
    let flags_value = flags_value.ok_or_else(|| {
        PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectPidfd,
            "descriptor has no exact procfs octal flags identity field",
        )
    })?;
    // Linux v6.12 fs/proc/fd.c emits file->f_flags in octal, and pidfd.h defines PIDFD_THREAD as
    // O_EXCL. Reject only that exact bit so unrelated current or future flags remain admissible.
    if flags_value & libc::PIDFD_THREAD != 0 {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::Thread,
            "procfs process pidfd flags contain Linux PIDFD_THREAD (O_EXCL)",
        ));
    }
    if pid_value == -1 {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::AlreadyDead,
            "process pidfd target was already reaped",
        ));
    }
    let pid = u32::try_from(pid_value).map_err(|_| {
        PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectPidfd,
            "procfs process pidfd identity is not positive in the selected procfs namespace view",
        )
    })?;
    if pid == 0 {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectPidfd,
            "procfs process pidfd identity is not positive in the selected procfs namespace view",
        ));
    }
    Ok(pid)
}

fn open_validated_procfs_self() -> Result<File, PidfdObservationErrorV1> {
    let self_entry = File::open("/proc/self").map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot open /proc/self for pidfd fallback validation",
            error,
        )
    })?;
    let numeric_entry = File::open(format!("/proc/{}", std::process::id())).map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "selected procfs mount has no numeric entry for the service getpid value",
            error,
        )
    })?;
    require_procfs(&self_entry, "/proc/self")?;
    require_procfs(&numeric_entry, "numeric /proc self entry")?;
    let self_stat = rustix::fs::fstat(&self_entry).map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot inspect /proc/self",
            io::Error::from(error),
        )
    })?;
    let numeric_stat = rustix::fs::fstat(&numeric_entry).map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot inspect numeric /proc self entry",
            io::Error::from(error),
        )
    })?;
    if (self_stat.st_dev, self_stat.st_ino) != (numeric_stat.st_dev, numeric_stat.st_ino) {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectPidfd,
            "/proc/self and /proc/<getpid> do not name the same process in the selected procfs mount",
        ));
    }
    Ok(self_entry)
}

pub fn require_procfs(file: &File, label: &'static str) -> Result<(), PidfdObservationErrorV1> {
    let filesystem = rustix::fs::fstatfs(file).map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            format!("cannot inspect filesystem type for {label}"),
            io::Error::from(error),
        )
    })?;
    if filesystem.f_type != rustix::fs::PROC_SUPER_MAGIC {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectPidfd,
            format!("{label} is not backed by procfs"),
        ));
    }
    Ok(())
}

pub fn inspect_process_start_time_ticks(pid: u32) -> Result<u64, PidfdObservationErrorV1> {
    parse_process_start_time_ticks(&read_process_stat(pid)?, pid)
}

pub fn read_process_stat(pid: u32) -> Result<Vec<u8>, PidfdObservationErrorV1> {
    // Validate that the selected procfs mount maps the service's numeric getpid consistently
    // before trusting a numeric client entry. This remains a trusted compatible-procfs
    // precondition; the check does not prove mount-namespace provenance.
    let _validated_self = open_validated_procfs_self()?;
    let mut record = File::open(format!("/proc/{pid}/stat")).map_err(|error| {
        PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectStartTime,
            "cannot open bounded process procfs stat identity",
            error,
        )
    })?;
    require_procfs(&record, "retained process stat identity")?;
    let mut contents = Vec::new();
    record
        .by_ref()
        .take(MAX_PROC_STAT_BYTES + 1)
        .read_to_end(&mut contents)
        .map_err(|error| {
            PidfdObservationErrorV1::io(
                PidfdObservationErrorKindV1::InspectStartTime,
                "cannot read bounded process procfs stat identity",
                error,
            )
        })?;
    if contents.is_empty() || contents.len() as u64 > MAX_PROC_STAT_BYTES {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectStartTime,
            "process procfs stat identity is empty or exceeds 4096 bytes",
        ));
    }
    Ok(contents)
}

/// Returns the current process's exact Linux procfs `starttime` tick field.
///
/// The observation first proves that `/proc/self` and `/proc/<getpid>` name the
/// same process entry in the selected procfs mount, then applies the same
/// bounded and strict parser used for retained pidfd identities. The result is
/// inert process identity data and grants no process or descriptor authority.
pub fn current_process_start_time_ticks_v1() -> Result<u64, PidfdObservationErrorV1> {
    inspect_process_start_time_ticks(std::process::id())
}

pub fn parse_process_start_time_ticks(
    contents: &[u8],
    expected_pid: u32,
) -> Result<u64, PidfdObservationErrorV1> {
    let close = contents
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| {
            PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::InspectStartTime,
                "process procfs stat identity has no command terminator",
            )
        })?;
    let first_space = contents
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or_else(|| {
            PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::InspectStartTime,
                "process procfs stat identity has no PID terminator",
            )
        })?;
    if contents.get(first_space + 1) != Some(&b'(') || close <= first_space + 1 {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectStartTime,
            "process procfs stat identity has a malformed command field",
        ));
    }
    let pid_bytes = &contents[..first_space];
    if pid_bytes.is_empty()
        || (pid_bytes.len() > 1 && pid_bytes.starts_with(b"0"))
        || !pid_bytes.iter().all(u8::is_ascii_digit)
    {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectStartTime,
            "process procfs stat identity has a noncanonical PID",
        ));
    }
    let recorded_pid = std::str::from_utf8(pid_bytes)
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    if recorded_pid != Some(expected_pid) {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectStartTime,
            "process procfs stat PID does not match the retained pidfd target",
        ));
    }
    let mut fields = contents
        .get(close + 1..)
        .ok_or_else(|| {
            PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::InspectStartTime,
                "process procfs stat identity ended at its command field",
            )
        })?
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty());
    let start_time = fields.nth(19).ok_or_else(|| {
        PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectStartTime,
            "process procfs stat identity has no start-time field",
        )
    })?;
    if start_time.is_empty()
        || (start_time.len() > 1 && start_time.starts_with(b"0"))
        || !start_time.iter().all(u8::is_ascii_digit)
    {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::InspectStartTime,
            "process procfs stat identity has a noncanonical start time",
        ));
    }
    let start_time = std::str::from_utf8(start_time)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::InspectStartTime,
                "process procfs stat identity has an invalid start time",
            )
        })?;
    Ok(start_time)
}

fn require_client_start_time(actual: u64, expected: u64) -> Result<(), PidfdObservationErrorV1> {
    if actual == expected {
        Ok(())
    } else {
        Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::StartTimeChanged,
            "retained retained process start time changed",
        ))
    }
}

fn require_pidfd_target(actual_pid: u32, expected_pid: u32) -> Result<(), PidfdObservationErrorV1> {
    if actual_pid != expected_pid {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::TargetMismatch,
            format!("process pidfd targets PID {actual_pid}, expected exact PID {expected_pid}"),
        ));
    }
    Ok(())
}

pub fn require_pidfd_not_pollable(pidfd: &OwnedFd) -> Result<(), PidfdObservationErrorV1> {
    let mut poll_descriptor = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: one valid borrowed pollfd; timeout zero never waits. Interrupted probes
    // fail closed so a signal storm cannot extend an external protocol deadline.
    let ready = unsafe { libc::poll(&mut poll_descriptor, 1, 0) };
    if ready < 0 {
        return Err(PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "cannot poll process pidfd for liveness",
            io::Error::last_os_error(),
        ));
    }
    if ready == 0 {
        return Ok(());
    }
    if poll_descriptor.revents & (libc::POLLIN | libc::POLLHUP) != 0 {
        return Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::AlreadyDead,
            "process pidfd reports process exit",
        ));
    }
    Err(PidfdObservationErrorV1::new(
        PidfdObservationErrorKindV1::InspectPidfd,
        format!(
            "process pidfd returned unexpected poll events 0x{:x}",
            poll_descriptor.revents
        ),
    ))
}

/// Poll-only owner of an original descriptor received from a separately authenticated sender.
///
/// Admission binds the kernel target, full descriptor metadata, observation source and process
/// start time. Callers must authenticate the sender and its role separately. A successful check
/// is point-in-time only; this object is not a proof session or execution permission.
///
/// No descriptor export, duplication, signal, wait, reap, or numeric-PID reopening is exposed.
///
/// ```compile_fail
/// use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
/// use std::os::fd::AsFd;
/// fn export(owner: &ReceivedProcessPidfdV1) { let _ = owner.as_fd(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
/// fn duplicate(owner: ReceivedProcessPidfdV1) { let _ = owner.clone(); }
/// ```
pub struct ReceivedProcessPidfdV1 {
    pidfd: OwnedFd,
    target: PidfdTargetObservationV1,
    object: PidfdObjectV1,
    start_time_ticks: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PidfdObjectV1 {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
}

impl PidfdObjectV1 {
    fn inspect(fd: &OwnedFd) -> Result<Self, PidfdObservationErrorV1> {
        let flags = rustix::io::fcntl_getfd(fd).map_err(|error| {
            PidfdObservationErrorV1::io(
                PidfdObservationErrorKindV1::InspectPidfd,
                "cannot inspect received pidfd descriptor flags",
                error.into(),
            )
        })?;
        if flags != rustix::io::FdFlags::CLOEXEC {
            return Err(PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::CloseOnExec,
                "received pidfd must retain exact CLOEXEC",
            ));
        }
        let stat = rustix::fs::fstat(fd).map_err(|error| {
            PidfdObservationErrorV1::io(
                PidfdObservationErrorKindV1::InspectPidfd,
                "cannot inspect received pidfd object",
                error.into(),
            )
        })?;
        Ok(Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            gid: stat.st_gid,
            links: stat.st_nlink,
        })
    }
}

impl fmt::Debug for ReceivedProcessPidfdV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReceivedProcessPidfdV1")
            .field("pid", &self.target.pid)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl ReceivedProcessPidfdV1 {
    /// Consumes an original received pidfd, never reopening the numeric PID.
    pub fn admit_received(
        pidfd: OwnedFd,
        expected_pid: u32,
    ) -> Result<Self, PidfdObservationErrorV1> {
        if expected_pid == 0 {
            return Err(PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::ExpectedPid,
                "expected received process PID must be nonzero",
            ));
        }
        let object = PidfdObjectV1::inspect(&pidfd)?;
        require_process_pidfd_mode(&pidfd)?;
        let target = inspect_pidfd_target(&pidfd)?;
        require_pidfd_target(target.pid, expected_pid)?;
        require_pidfd_not_pollable(&pidfd)?;
        let owner = Self {
            start_time_ticks: inspect_process_start_time_ticks(expected_pid)?,
            pidfd,
            target,
            object,
        };
        owner.revalidate()?;
        Ok(owner)
    }

    /// Rechecks the original process without waitid or any process-control syscall.
    pub fn revalidate(&self) -> Result<(), PidfdObservationErrorV1> {
        for _ in 0..2 {
            require_pidfd_not_pollable(&self.pidfd)?;
            require_process_pidfd_mode(&self.pidfd)?;
            if PidfdObjectV1::inspect(&self.pidfd)? != self.object
                || inspect_pidfd_target(&self.pidfd)? != self.target
            {
                return Err(PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::IdentityChanged,
                    "received pidfd object, target or observation source changed",
                ));
            }
            require_client_start_time(
                inspect_process_start_time_ticks(self.target.pid)?,
                self.start_time_ticks,
            )?;
        }
        require_pidfd_not_pollable(&self.pidfd)
    }

    pub const fn pid(&self) -> u32 {
        self.target.pid
    }

    pub const fn start_time_ticks(&self) -> u64 {
        self.start_time_ticks
    }
}

#[cfg(test)]
mod owner_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn pidfd_for(pid: u32) -> OwnedFd {
        use std::os::fd::FromRawFd;
        // SAFETY: pidfd_open has no pointer arguments and returns a newly owned descriptor.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        assert!(fd >= 0, "pidfd_open: {}", io::Error::last_os_error());
        // SAFETY: the successful syscall returned an exclusively owned descriptor.
        unsafe { OwnedFd::from_raw_fd(fd as i32) }
    }
    fn proc_stat_fixture(pid: u32, start_time_ticks: &str) -> Vec<u8> {
        let mut fields = vec!["R"; 19];
        fields.push(start_time_ticks);
        format!("{pid} (command with ) delimiters) {}\n", fields.join(" ")).into_bytes()
    }

    #[test]
    fn proc_stat_parser_binds_exact_pid_and_start_time() {
        let pid = std::process::id();
        let bytes = proc_stat_fixture(pid, "987654321");
        assert_eq!(
            parse_process_start_time_ticks(&bytes, pid).unwrap(),
            987_654_321
        );

        for wrong_pid in [pid.saturating_add(1), pid.saturating_sub(1)] {
            if wrong_pid != 0 && wrong_pid != pid {
                assert_eq!(
                    parse_process_start_time_ticks(&bytes, wrong_pid)
                        .unwrap_err()
                        .kind(),
                    PidfdObservationErrorKindV1::InspectStartTime
                );
            }
        }
    }

    #[test]
    fn proc_stat_parser_rejects_noncanonical_or_missing_start_time() {
        let pid = std::process::id();
        for start_time in ["0", "01", "-1", "+1", "x", "18446744073709551616"] {
            assert_eq!(
                parse_process_start_time_ticks(&proc_stat_fixture(pid, start_time), pid)
                    .unwrap_err()
                    .kind(),
                PidfdObservationErrorKindV1::InspectStartTime,
                "start time {start_time}"
            );
        }
        for malformed in [
            format!("{pid} command R 1 2 3"),
            format!("{pid} (command) R 1 2 3"),
            format!("0{pid} (command) {}", vec!["1"; 20].join(" ")),
            format!(
                "{} (command) {}",
                pid.saturating_add(1),
                vec!["1"; 20].join(" ")
            ),
        ] {
            assert_eq!(
                parse_process_start_time_ticks(malformed.as_bytes(), pid)
                    .unwrap_err()
                    .kind(),
                PidfdObservationErrorKindV1::InspectStartTime
            );
        }
    }

    #[test]
    fn injected_legacy_ioctl_errnos_route_to_strict_procfs_fallback() {
        let pidfd = pidfd_for(std::process::id());
        for errno in [libc::ENOTTY, libc::EINVAL] {
            assert_eq!(
                dispatch_pidfd_get_info_error(&pidfd, io::Error::from_raw_os_error(errno)).unwrap(),
                PidfdTargetObservationV1 {
                    pid: std::process::id(),
                    source: PidfdIdentitySourceV1::ProcfsFdinfo,
                }
            );
        }
    }

    #[test]
    fn injected_legacy_ioctl_errnos_reject_an_ordinary_descriptor() {
        let descriptor: OwnedFd = File::open("/dev/null").unwrap().into();
        for errno in [libc::ENOTTY, libc::EINVAL] {
            assert_eq!(
                dispatch_pidfd_get_info_error(&descriptor, io::Error::from_raw_os_error(errno),)
                    .unwrap_err()
                    .kind(),
                PidfdObservationErrorKindV1::InspectPidfd
            );
        }
    }

    #[test]
    fn injected_nonfallback_ioctl_errnos_remain_errors() {
        let pidfd = pidfd_for(std::process::id());
        for errno in [libc::EACCES, libc::EFAULT, libc::EIO, libc::EPERM] {
            assert_eq!(
                dispatch_pidfd_get_info_error(&pidfd, io::Error::from_raw_os_error(errno))
                    .unwrap_err()
                    .kind(),
                PidfdObservationErrorKindV1::InspectPidfd
            );
        }
        assert_eq!(
            dispatch_pidfd_get_info_error(&pidfd, io::Error::from_raw_os_error(libc::ESRCH))
                .unwrap_err()
                .kind(),
            PidfdObservationErrorKindV1::AlreadyDead
        );
    }

    #[test]
    fn pidfd_fdinfo_parser_accepts_one_exact_positive_pid() {
        let base_flags = u32::try_from(libc::O_RDWR | libc::O_CLOEXEC).unwrap();
        let nonblocking = base_flags | u32::try_from(libc::O_NONBLOCK).unwrap();
        let future_non_thread = base_flags | (1 << 30);
        for flags in [base_flags, nonblocking, future_non_thread] {
            let record = format!("pos:\t0\nflags:\t0{flags:o}\nPid:\t1234\nNSpid:\t1234\n");
            assert_eq!(parse_pidfd_fdinfo(&record).unwrap(), 1234);
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_rejects_thread_flag() {
        let flags = u32::try_from(libc::O_RDWR | libc::O_CLOEXEC).unwrap() | libc::PIDFD_THREAD;
        for flags in [flags, flags | (1 << 30)] {
            let record = format!("flags:\t0{flags:o}\nPid:\t1234\n");
            assert_eq!(
                parse_pidfd_fdinfo(&record).unwrap_err().kind(),
                PidfdObservationErrorKindV1::Thread
            );
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_rejects_missing_duplicate_and_malformed_flags() {
        for record in [
            "Pid:\t1\n",
            "flags:\t02000002\n",
            "flags:\t02000002\nflags:\t02000002\nPid:\t1\n",
            "flags:\t02000002\nflags: 02000002\nPid:\t1\n",
            "flags: 02000002\nPid:\t1\n",
            "flags:\t\nPid:\t1\n",
            "flags:\t0\nPid:\t1\n",
            "flags:\t2000002\nPid:\t1\n",
            "flags:\t02000008\nPid:\t1\n",
            "flags:\t02000002 \nPid:\t1\n",
            "flags:\t077777777777\nPid:\t1\n",
        ] {
            assert_eq!(
                parse_pidfd_fdinfo(record).unwrap_err().kind(),
                PidfdObservationErrorKindV1::InspectPidfd
            );
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_rejects_missing_duplicate_and_malformed_pid() {
        for record in [
            "flags:\t02000002\n",
            "flags:\t02000002\nPid:\t1\nPid:\t1\n",
            "flags:\t02000002\nPid:\t1\nPid: 1\n",
            "flags:\t02000002\nPid: 1\n",
            "flags:\t02000002\nPid:\t 1\n",
            "flags:\t02000002\nPid:\t1 \n",
            "flags:\t02000002\nPid:\t+1\n",
            "flags:\t02000002\nPid:\t01\n",
            "flags:\t02000002\nPid:\t-2\n",
            "flags:\t02000002\nPid:\t0\n",
            "flags:\t02000002\nPid:\t4294967296\n",
        ] {
            assert_eq!(
                parse_pidfd_fdinfo(record).unwrap_err().kind(),
                PidfdObservationErrorKindV1::InspectPidfd
            );
        }
    }

    #[test]
    fn pidfd_fdinfo_parser_classifies_reaped_target_as_dead() {
        assert_eq!(
            parse_pidfd_fdinfo("flags:\t02000002\nPid:\t-1\n")
                .unwrap_err()
                .kind(),
            PidfdObservationErrorKindV1::AlreadyDead
        );
    }
}
