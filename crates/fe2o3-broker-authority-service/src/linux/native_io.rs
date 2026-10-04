//! Finite-attempt native procfs and poll leaves. The caller prepays their envelope.
use super::{
    AdmissionErrorKindV1 as Kind, MAX_PIDFD_FDINFO_BYTES, MAX_PROC_STAT_BYTES,
    PidfdIdentitySourceV1, PidfdTargetObservationV1,
    checks::{self, CheckError as Error, Result},
};
use rustix::{
    fs::{Mode, OFlags},
    io::Errno,
};
use std::{
    ffi::CStr,
    fs::File,
    mem::size_of,
    os::fd::{AsRawFd, OwnedFd},
};

pub(super) const RECORD_BYTES: usize = 4097;
pub(super) const MAX_READ_CALLS: usize = RECORD_BYTES;
pub(super) const PATH_BYTES: usize = 32;
const DECIMAL_BYTES: usize = 10;
/// Conservative logical leaf scratch, excluding the caller's live owner floor.
/// This is not a generated-stack, kernel-memory, instruction or latency bound.
pub(super) const SCRATCH_BYTES: usize = RECORD_BYTES + 2 * PATH_BYTES + DECIMAL_BYTES + 4096;

const _: () = {
    assert!(MAX_PIDFD_FDINFO_BYTES == 4096);
    assert!(MAX_PROC_STAT_BYTES == 4096);
    assert!(RECORD_BYTES == MAX_PIDFD_FDINFO_BYTES as usize + 1);
    assert!(PATH_BYTES > b"/proc/".len() + DECIMAL_BYTES + b"/stat".len());
    assert!(SCRATCH_BYTES == 8267);
    assert!(
        4 * size_of::<rustix::fs::Stat>()
            + 2 * size_of::<rustix::fs::StatFs>()
            + 6 * size_of::<File>()
            + 2 * size_of::<libc::pollfd>()
            + 8 * size_of::<Error>()
            + 64 * size_of::<usize>()
            + size_of::<Result<File>>()
            + size_of::<Result<PidfdTargetObservationV1>>()
            + size_of::<Result<u64>>()
            + size_of::<Result<(i32, i16)>>()
            <= 4096
    );
};

pub(super) fn inspect_pidfd_target_from_procfs(
    pidfd: &OwnedFd,
) -> Result<PidfdTargetObservationV1> {
    let self_entry = open_validated_procfs_self()?;
    let fdinfo = rustix::fs::openat(
        &self_entry,
        c"fdinfo",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|errno| {
        Error::io(
            Kind::InspectClientPidfd,
            "cannot open the retained procfs fdinfo directory",
            errno,
        )
    })?;
    checks::require_procfs(&fdinfo, "retained /proc/self/fdinfo directory")?;
    let fd = u32::try_from(pidfd.as_raw_fd()).map_err(|_| {
        Error::new(
            Kind::InspectClientPidfd,
            "invalid retained client pidfd descriptor",
        )
    })?;
    let mut path = [0; PATH_BYTES];
    let name = decimal_path(b"", fd, b"", &mut path)?;
    let record = rustix::fs::openat(
        &fdinfo,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|errno| {
        Error::io(
            Kind::InspectClientPidfd,
            "cannot open the bounded procfs client pidfd identity record",
            errno,
        )
    })?;
    checks::require_procfs(&record, "client pidfd identity record")?;
    let pid = read_pidfd_record_with(|bytes| rustix::io::read(&record, bytes))?;
    Ok(PidfdTargetObservationV1 {
        pid,
        source: PidfdIdentitySourceV1::ProcfsFdinfo,
    })
}

pub(super) fn inspect_process_start_time_ticks(pid: u32) -> Result<u64> {
    let _validated_self = open_validated_procfs_self()?;
    let mut path = [0; PATH_BYTES];
    let path = decimal_path(b"/proc/", pid, b"/stat", &mut path)?;
    // Match the legacy absolute path and symlink-following semantics.
    let record = rustix::fs::open(path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
        .map(File::from)
        .map_err(|errno| {
            Error::io(
                Kind::InspectClientStartTime,
                "cannot open bounded client procfs stat identity",
                errno,
            )
        })?;
    checks::require_procfs(&record, "client process stat identity")?;
    read_stat_record_with(pid, |bytes| rustix::io::read(&record, bytes))
}

fn open_validated_procfs_self() -> Result<File> {
    let self_entry = rustix::fs::open(
        c"/proc/self",
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|errno| {
        Error::io(
            Kind::InspectClientPidfd,
            "cannot open /proc/self for pidfd fallback validation",
            errno,
        )
    })?;
    let mut path = [0; PATH_BYTES];
    let path = decimal_path(b"/proc/", std::process::id(), b"", &mut path)?;
    let numeric_entry = rustix::fs::open(path, OFlags::RDONLY | OFlags::CLOEXEC, Mode::empty())
        .map(File::from)
        .map_err(|errno| {
            Error::io(
                Kind::InspectClientPidfd,
                "selected procfs mount has no numeric entry for the service getpid value",
                errno,
            )
        })?;
    checks::require_procfs(&self_entry, "/proc/self")?;
    checks::require_procfs(&numeric_entry, "numeric /proc self entry")?;
    let self_stat = rustix::fs::fstat(&self_entry)
        .map_err(|errno| Error::io(Kind::InspectClientPidfd, "cannot inspect /proc/self", errno))?;
    let numeric_stat = rustix::fs::fstat(&numeric_entry).map_err(|errno| {
        Error::io(
            Kind::InspectClientPidfd,
            "cannot inspect numeric /proc self entry",
            errno,
        )
    })?;
    if (self_stat.st_dev, self_stat.st_ino) != (numeric_stat.st_dev, numeric_stat.st_ino) {
        return Err(Error::new(
            Kind::InspectClientPidfd,
            "/proc/self and /proc/<getpid> do not name the same process in the selected procfs mount",
        ));
    }
    Ok(self_entry)
}

fn read_pidfd_record_with(read: impl FnMut(&mut [u8]) -> rustix::io::Result<usize>) -> Result<u32> {
    let mut bytes = [0; RECORD_BYTES];
    let length = read_record_with(
        &mut bytes,
        Kind::InspectClientPidfd,
        "cannot read the bounded procfs client pidfd identity record",
        read,
    )?;
    // Legacy read_to_string validates UTF-8 before its caller checks length.
    let contents = std::str::from_utf8(&bytes[..length]).map_err(|_| {
        Error::new(
            Kind::InspectClientPidfd,
            "procfs client pidfd identity record is not valid UTF-8",
        )
    })?;
    if length > MAX_PIDFD_FDINFO_BYTES as usize {
        return Err(Error::new(
            Kind::InspectClientPidfd,
            "procfs client pidfd identity record exceeds 4096 bytes",
        ));
    }
    checks::parse_pidfd_fdinfo(contents)
}

fn read_stat_record_with(
    pid: u32,
    read: impl FnMut(&mut [u8]) -> rustix::io::Result<usize>,
) -> Result<u64> {
    let mut bytes = [0; RECORD_BYTES];
    let length = read_record_with(
        &mut bytes,
        Kind::InspectClientStartTime,
        "cannot read bounded client procfs stat identity",
        read,
    )?;
    if length == 0 || length > MAX_PROC_STAT_BYTES as usize {
        return Err(Error::new(
            Kind::InspectClientStartTime,
            "client procfs stat identity is empty or exceeds 4096 bytes",
        ));
    }
    checks::parse_process_start_time_ticks(&bytes[..length], pid)
}

fn read_record_with(
    bytes: &mut [u8; RECORD_BYTES],
    kind: Kind,
    operation: &'static str,
    mut read: impl FnMut(&mut [u8]) -> rustix::io::Result<usize>,
) -> Result<usize> {
    let mut filled = 0;
    for _ in 0..MAX_READ_CALLS {
        let remaining = bytes.len() - filled;
        let count =
            read(&mut bytes[filled..]).map_err(|errno| Error::io(kind, operation, errno))?;
        if count > remaining {
            return Err(Error::new(
                kind,
                "procfs read returned an invalid byte count",
            ));
        }
        filled += count;
        // The full buffer is an oversize sentinel, checked before either parser.
        if count == 0 || filled == bytes.len() {
            return Ok(filled);
        }
    }
    Err(Error::new(kind, "bounded procfs read attempts exhausted"))
}

fn decimal_path<'a>(
    prefix: &[u8],
    mut value: u32,
    suffix: &[u8],
    storage: &'a mut [u8; PATH_BYTES],
) -> Result<&'a CStr> {
    let mut digits = [0; DECIMAL_BYTES];
    let mut start = digits.len();
    for _ in 0..DECIMAL_BYTES {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let length = prefix
        .len()
        .checked_add(digits.len() - start)
        .and_then(|length| length.checked_add(suffix.len()))
        .filter(|length| *length < storage.len())
        .ok_or_else(|| Error::new(Kind::InspectClientPidfd, "bounded procfs path is too long"))?;
    let end_digits = prefix.len() + digits.len() - start;
    storage[..prefix.len()].copy_from_slice(prefix);
    storage[prefix.len()..end_digits].copy_from_slice(&digits[start..]);
    storage[end_digits..length].copy_from_slice(suffix);
    storage[length] = 0;
    CStr::from_bytes_with_nul(&storage[..=length])
        .map_err(|_| Error::new(Kind::InspectClientPidfd, "bounded procfs path contains NUL"))
}

/// One zero-timeout poll. The shared checks layer classifies readiness/revents.
pub(super) fn poll_once(pidfd: &OwnedFd) -> Result<(libc::c_int, libc::c_short)> {
    poll_once_with(pidfd, |descriptor| {
        // SAFETY: one initialized, writable pollfd is borrowed for this call;
        // timeout zero does not wait, and this leaf never retries an error.
        let ready = unsafe { libc::poll(descriptor, 1, 0) };
        if ready < 0 {
            let errno = std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO);
            Err(Errno::from_raw_os_error(errno))
        } else {
            Ok(ready)
        }
    })
}

fn poll_once_with(
    pidfd: &OwnedFd,
    poll: impl FnOnce(&mut libc::pollfd) -> rustix::io::Result<libc::c_int>,
) -> Result<(libc::c_int, libc::c_short)> {
    let mut descriptor = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ready = poll(&mut descriptor).map_err(|errno| {
        Error::io(
            Kind::InspectClientPidfd,
            "cannot poll client pidfd for liveness",
            errno,
        )
    })?;
    Ok((ready, descriptor.revents))
}

#[cfg(test)]
#[path = "native_io_tests.rs"]
mod tests;
