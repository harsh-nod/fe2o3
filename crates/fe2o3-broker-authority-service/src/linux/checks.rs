//! Shared allocation-free admission predicates and parsers, without live-owner orchestration.
use super::{
    AdmissionErrorKindV1 as Kind, ObjectIdentityV1, PeerCredentialsV1,
    ProtectedServiceAdmissionErrorV1, UnixAddressSideV1,
};
use rustix::fs::OFlags;
use rustix::net::{AddressFamily, SocketType};
use std::fmt;
use std::fs::File;
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};

#[cfg(test)]
#[path = "checks_tests.rs"]
mod tests;

pub(super) type Result<T> = std::result::Result<T, CheckError>;

#[derive(Debug)]
enum Message {
    Static(&'static str),
    Parts {
        prefix: &'static str,
        label: &'static str,
        suffix: &'static str,
    },
    PidMismatch {
        actual: u32,
        expected: u32,
    },
    PollEvents(i16),
}

#[derive(Debug)]
pub(super) struct CheckError {
    kind: Kind,
    message: Message,
    errno: Option<i32>,
}

impl CheckError {
    pub(super) const fn new(kind: Kind, message: &'static str) -> Self {
        Self {
            kind,
            message: Message::Static(message),
            errno: None,
        }
    }

    pub(super) fn io(kind: Kind, message: &'static str, error: rustix::io::Errno) -> Self {
        Self {
            kind,
            message: Message::Static(message),
            errno: Some(error.raw_os_error()),
        }
    }

    pub(super) const fn parts(
        kind: Kind,
        prefix: &'static str,
        label: &'static str,
        suffix: &'static str,
        errno: Option<i32>,
    ) -> Self {
        Self {
            kind,
            message: Message::Parts {
                prefix,
                label,
                suffix,
            },
            errno,
        }
    }

    pub(super) const fn kind(&self) -> Kind {
        self.kind
    }

    pub(super) const fn errno(&self) -> Option<i32> {
        self.errno
    }

    pub(super) const fn poll_events(revents: i16) -> Self {
        Self {
            kind: Kind::InspectClientPidfd,
            message: Message::PollEvents(revents),
            errno: None,
        }
    }
}

impl fmt::Display for CheckError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.message {
            Message::Static(message) => formatter.write_str(message),
            Message::Parts {
                prefix,
                label,
                suffix,
            } => {
                formatter.write_str(prefix)?;
                formatter.write_str(label)?;
                formatter.write_str(suffix)
            }
            Message::PidMismatch { actual, expected } => write!(
                formatter,
                "client pidfd targets PID {actual}, expected exact PID {expected}"
            ),
            Message::PollEvents(revents) => write!(
                formatter,
                "client pidfd returned unexpected poll events 0x{revents:x}"
            ),
        }
    }
}

impl std::error::Error for CheckError {}

impl From<CheckError> for ProtectedServiceAdmissionErrorV1 {
    fn from(error: CheckError) -> Self {
        // Only the legacy adapter owns a rendered String and std::io::Error.
        let message = error.to_string();
        match error.errno {
            Some(errno) => Self::io(error.kind, message, io::Error::from_raw_os_error(errno)),
            None => Self::new(error.kind, message),
        }
    }
}

pub(super) fn inspect_object(
    descriptor: &OwnedFd,
    kind: Kind,
    label: &'static str,
) -> Result<ObjectIdentityV1> {
    let stat = rustix::fs::fstat(descriptor).map_err(|error| {
        CheckError::parts(
            kind,
            "cannot inspect retained ",
            label,
            " descriptor",
            Some(error.raw_os_error()),
        )
    })?;
    Ok(ObjectIdentityV1 {
        device: stat.st_dev,
        inode: stat.st_ino,
        mode: stat.st_mode,
        uid: stat.st_uid,
        gid: stat.st_gid,
        links: stat.st_nlink,
    })
}

pub(super) fn inspect_peer_credentials(peer: &OwnedFd) -> Result<PeerCredentialsV1> {
    let credentials = rustix::net::sockopt::socket_peercred(peer).map_err(|error| {
        CheckError::io(
            Kind::InspectPeer,
            "cannot inspect retained peer SO_PEERCRED",
            error,
        )
    })?;
    let raw_pid = credentials.pid.as_raw_nonzero().get();
    let pid = u32::try_from(raw_pid).map_err(|_| {
        CheckError::new(
            Kind::ExpectedClientPid,
            "service peer SO_PEERCRED PID is not a positive u32",
        )
    })?;
    if pid == 0 {
        return Err(CheckError::new(
            Kind::ExpectedClientPid,
            "service peer SO_PEERCRED PID is zero",
        ));
    }
    Ok(PeerCredentialsV1 {
        pid,
        uid: credentials.uid.as_raw(),
        gid: credentials.gid.as_raw(),
    })
}

pub(super) fn require_close_on_exec(
    descriptor: &impl AsFd,
    kind: Kind,
    label: &'static str,
) -> Result<()> {
    let flags = rustix::io::fcntl_getfd(descriptor).map_err(|error| {
        CheckError::parts(
            kind,
            "cannot inspect ",
            label,
            " descriptor flags",
            Some(error.raw_os_error()),
        )
    })?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(CheckError::parts(
            kind,
            "retained ",
            label,
            " descriptor does not have FD_CLOEXEC",
            None,
        ));
    }
    Ok(())
}

pub(super) fn require_process_pidfd_mode(pidfd: &OwnedFd) -> Result<()> {
    let flags = rustix::fs::fcntl_getfl(pidfd).map_err(|error| {
        CheckError::io(
            Kind::InspectClientPidfd,
            "cannot inspect client pidfd file status flags",
            error,
        )
    })?;
    // Preserve the exact Linux PIDFD_THREAD selector without rejecting unrelated flags.
    if flags.contains(OFlags::EXCL) {
        return Err(CheckError::new(
            Kind::ClientPidfdThread,
            "client pidfd has Linux PIDFD_THREAD (O_EXCL) semantics",
        ));
    }
    Ok(())
}

pub(super) fn parse_pidfd_fdinfo(contents: &str) -> Result<u32> {
    let mut pid_value = None;
    let mut flags_value = None;
    for line in contents.lines() {
        if let Some(field) = line.strip_prefix("Pid:") {
            let value = field.strip_prefix('\t').ok_or_else(|| {
                CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed Pid field",
                )
            })?;
            if pid_value.is_some() {
                return Err(CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has duplicate Pid fields",
                ));
            }
            let canonical_positive_or_zero = !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value.len() == 1 || !value.starts_with('0'));
            if value != "-1" && !canonical_positive_or_zero {
                return Err(CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has a non-canonical decimal Pid field",
                ));
            }
            pid_value = Some(value.parse::<i64>().map_err(|_| {
                CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed Pid field",
                )
            })?);
        }
        if let Some(field) = line.strip_prefix("flags:") {
            let value = field.strip_prefix('\t').ok_or_else(|| {
                CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed flags field",
                )
            })?;
            if flags_value.is_some() {
                return Err(CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has duplicate flags fields",
                ));
            }
            if value.len() < 2
                || !value.starts_with('0')
                || !value.bytes().all(|byte| matches!(byte, b'0'..=b'7'))
            {
                return Err(CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has a malformed octal flags field",
                ));
            }
            flags_value = Some(u32::from_str_radix(value, 8).map_err(|_| {
                CheckError::new(
                    Kind::InspectClientPidfd,
                    "procfs client pidfd identity record has an out-of-range octal flags field",
                )
            })?);
        }
    }
    let pid_value = pid_value.ok_or_else(|| {
        CheckError::new(
            Kind::InspectClientPidfd,
            "descriptor is not a pidfd with a procfs Pid identity field",
        )
    })?;
    let flags_value = flags_value.ok_or_else(|| {
        CheckError::new(
            Kind::InspectClientPidfd,
            "descriptor has no exact procfs octal flags identity field",
        )
    })?;
    if flags_value & libc::PIDFD_THREAD != 0 {
        return Err(CheckError::new(
            Kind::ClientPidfdThread,
            "procfs client pidfd flags contain Linux PIDFD_THREAD (O_EXCL)",
        ));
    }
    if pid_value == -1 {
        return Err(CheckError::new(
            Kind::ClientAlreadyDead,
            "client pidfd target was already reaped",
        ));
    }
    let pid = u32::try_from(pid_value).map_err(|_| {
        CheckError::new(
            Kind::InspectClientPidfd,
            "procfs client pidfd identity is not positive in the selected procfs namespace view",
        )
    })?;
    if pid == 0 {
        return Err(CheckError::new(
            Kind::InspectClientPidfd,
            "procfs client pidfd identity is not positive in the selected procfs namespace view",
        ));
    }
    Ok(pid)
}

pub(super) fn parse_process_start_time_ticks(contents: &[u8], expected_pid: u32) -> Result<u64> {
    let close = contents
        .iter()
        .rposition(|byte| *byte == b')')
        .ok_or_else(|| {
            CheckError::new(
                Kind::InspectClientStartTime,
                "client procfs stat identity has no command terminator",
            )
        })?;
    let first_space = contents
        .iter()
        .position(|byte| *byte == b' ')
        .ok_or_else(|| {
            CheckError::new(
                Kind::InspectClientStartTime,
                "client procfs stat identity has no PID terminator",
            )
        })?;
    if contents.get(first_space + 1) != Some(&b'(') || close <= first_space + 1 {
        return Err(CheckError::new(
            Kind::InspectClientStartTime,
            "client procfs stat identity has a malformed command field",
        ));
    }
    let pid_bytes = &contents[..first_space];
    if pid_bytes.is_empty()
        || (pid_bytes.len() > 1 && pid_bytes.starts_with(b"0"))
        || !pid_bytes.iter().all(u8::is_ascii_digit)
    {
        return Err(CheckError::new(
            Kind::InspectClientStartTime,
            "client procfs stat identity has a noncanonical PID",
        ));
    }
    let recorded_pid = std::str::from_utf8(pid_bytes)
        .ok()
        .and_then(|value| value.parse::<u32>().ok());
    if recorded_pid != Some(expected_pid) {
        return Err(CheckError::new(
            Kind::InspectClientStartTime,
            "client procfs stat PID does not match the retained pidfd target",
        ));
    }
    let mut fields = contents
        .get(close + 1..)
        .ok_or_else(|| {
            CheckError::new(
                Kind::InspectClientStartTime,
                "client procfs stat identity ended at its command field",
            )
        })?
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty());
    let start_time = fields.nth(19).ok_or_else(|| {
        CheckError::new(
            Kind::InspectClientStartTime,
            "client procfs stat identity has no start-time field",
        )
    })?;
    if start_time.is_empty()
        || (start_time.len() > 1 && start_time.starts_with(b"0"))
        || !start_time.iter().all(u8::is_ascii_digit)
    {
        return Err(CheckError::new(
            Kind::InspectClientStartTime,
            "client procfs stat identity has a noncanonical start time",
        ));
    }
    let start_time = std::str::from_utf8(start_time)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| {
            CheckError::new(
                Kind::InspectClientStartTime,
                "client procfs stat identity has an invalid start time",
            )
        })?;
    Ok(start_time)
}

pub(super) fn require_procfs(file: &File, label: &'static str) -> Result<()> {
    let filesystem = rustix::fs::fstatfs(file).map_err(|error| {
        CheckError::parts(
            Kind::InspectClientPidfd,
            "cannot inspect filesystem type for ",
            label,
            "",
            Some(error.raw_os_error()),
        )
    })?;
    if filesystem.f_type != rustix::fs::PROC_SUPER_MAGIC {
        return Err(CheckError::parts(
            Kind::InspectClientPidfd,
            "",
            label,
            " is not backed by procfs",
            None,
        ));
    }
    Ok(())
}

pub(super) fn require_client_start_time(actual: u64, expected: u64) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(CheckError::new(
            Kind::ClientStartTimeChanged,
            "retained client process start time changed",
        ))
    }
}

pub(super) fn require_pidfd_target(actual_pid: u32, expected_pid: u32) -> Result<()> {
    if actual_pid != expected_pid {
        return Err(CheckError {
            kind: Kind::ClientPidfdTargetMismatch,
            message: Message::PidMismatch {
                actual: actual_pid,
                expected: expected_pid,
            },
            errno: None,
        });
    }
    Ok(())
}

pub(super) fn validate_peer_shape(peer: &OwnedFd) -> Result<ObjectIdentityV1> {
    let identity = inspect_object(peer, Kind::InspectPeer, "peer")?;
    let domain = rustix::net::sockopt::socket_domain(peer).map_err(|error| {
        CheckError::io(
            Kind::PeerDomain,
            "retained service peer is not a socket with an inspectable domain",
            error,
        )
    })?;
    if domain != AddressFamily::UNIX {
        return Err(CheckError::new(
            Kind::PeerDomain,
            "retained service peer is not an AF_UNIX socket",
        ));
    }
    let socket_type = rustix::net::sockopt::socket_type(peer).map_err(|error| {
        CheckError::io(
            Kind::PeerSocketType,
            "cannot inspect retained service peer socket type",
            error,
        )
    })?;
    if socket_type != SocketType::SEQPACKET {
        return Err(CheckError::new(
            Kind::PeerSocketType,
            "retained service peer is not SOCK_SEQPACKET",
        ));
    }
    require_unnamed_unix_address(
        peer,
        UnixAddressSideV1::Remote,
        Kind::PeerRemoteAddress,
        "remote",
    )?;
    require_unnamed_unix_address(
        peer,
        UnixAddressSideV1::Local,
        Kind::PeerLocalAddress,
        "local",
    )?;
    let final_identity = inspect_object(peer, Kind::InspectPeer, "peer")?;
    if final_identity != identity {
        return Err(CheckError::new(
            Kind::PeerIdentityChanged,
            "retained service peer identity changed while checking socket shape",
        ));
    }
    Ok(final_identity)
}

pub(super) fn validate_external_anchor_peer_status(peer: &OwnedFd) -> Result<()> {
    let status = rustix::fs::fcntl_getfl(peer).map_err(|error| {
        CheckError::io(
            Kind::PeerStatusFlags,
            "cannot inspect external-anchor peer status flags",
            error,
        )
    })?;
    if status != OFlags::RDWR | OFlags::NONBLOCK {
        return Err(CheckError::new(
            Kind::PeerStatusFlags,
            "external-anchor peer is not an exact nonblocking read-write endpoint",
        ));
    }
    Ok(())
}

pub(super) fn require_unnamed_unix_address(
    peer: &OwnedFd,
    side: UnixAddressSideV1,
    kind: Kind,
    label: &'static str,
) -> Result<()> {
    let mut address = MaybeUninit::<libc::sockaddr_un>::zeroed();
    let mut length = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_un>())
        .expect("sockaddr_un length fits socklen_t");
    // SAFETY: the zeroed address buffer and length describe writable storage,
    // and the descriptor stays borrowed throughout this single syscall.
    let result = unsafe {
        match side {
            UnixAddressSideV1::Local => libc::getsockname(
                peer.as_raw_fd(),
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
            UnixAddressSideV1::Remote => libc::getpeername(
                peer.as_raw_fd(),
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
        }
    };
    if result != 0 {
        let error_kind = match side {
            UnixAddressSideV1::Local => kind,
            UnixAddressSideV1::Remote => Kind::PeerNotConnected,
        };
        return Err(CheckError::parts(
            error_kind,
            "cannot inspect retained service peer ",
            label,
            " address",
            io::Error::last_os_error().raw_os_error(),
        ));
    }
    // SAFETY: the kernel wrote into fully zero-initialized sockaddr_un storage.
    let address = unsafe { address.assume_init() };
    if i32::from(address.sun_family) != libc::AF_UNIX {
        return Err(CheckError::parts(
            kind,
            "retained service peer ",
            label,
            " address is not AF_UNIX",
            None,
        ));
    }
    let unnamed_length = std::mem::offset_of!(libc::sockaddr_un, sun_path);
    if usize::try_from(length).ok() != Some(unnamed_length) {
        return Err(CheckError::parts(
            kind,
            "retained service peer ",
            label,
            " address is named",
            None,
        ));
    }
    Ok(())
}

pub(super) fn require_distinct_peer_and_pidfd(
    peer: ObjectIdentityV1,
    pidfd: ObjectIdentityV1,
) -> Result<()> {
    if peer.object() == pidfd.object() {
        return Err(CheckError::new(
            Kind::DuplicateDescriptors,
            "external-anchor peer and pidfd resolve to the same object",
        ));
    }
    Ok(())
}
