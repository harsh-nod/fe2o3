use super::{Error, io_error};
use rustix::{io::Errno, path::DecInt, process::Pid};
use std::ffi::CStr;

// /proc/ + signed i32 PID + /ns/ + time_for_children + NUL fits in 39 bytes.
pub(super) const PROC_PATH_BYTES: usize = 64;

pub(super) struct ProcPath {
    bytes: [u8; PROC_PATH_BYTES],
    len: usize,
}

impl ProcPath {
    pub(super) fn new(pid: Option<Pid>, suffix: &str) -> Result<Self, Error> {
        let decimal = DecInt::new(pid.map_or(0, Pid::as_raw_pid));
        let identity = if pid.is_some() {
            decimal.as_bytes()
        } else {
            b"self"
        };
        let mut path = Self {
            bytes: [0; PROC_PATH_BYTES],
            len: 0,
        };
        for part in [b"/proc/".as_slice(), identity, b"/", suffix.as_bytes()] {
            if part.len() >= PROC_PATH_BYTES - path.len {
                return Err(Error::InvalidState("proc path exceeds the fixed bound"));
            }
            path.bytes[path.len..path.len + part.len()].copy_from_slice(part);
            path.len += part.len();
        }
        Ok(path)
    }

    pub(super) fn as_c_str(&self) -> Result<&CStr, Error> {
        CStr::from_bytes_with_nul(&self.bytes[..=self.len])
            .map_err(|_| Error::InvalidState("proc path contains an interior NUL"))
    }
}

/// N includes the over-limit sentinel. Success requires an explicit EOF read;
/// a short positive read is progress, not EOF. Every continuing call adds at
/// least one byte, so there are at most N calls, including EOF or refusal.
/// EINTR and every other read error return immediately without a retry.
pub(super) fn read_bounded<const N: usize>(
    bytes: &mut [u8; N],
    operation: &'static str,
    overflow: &'static str,
    mut read: impl FnMut(&mut [u8]) -> Result<usize, Errno>,
) -> Result<usize, Error> {
    let mut used = 0;
    for _ in 0..N {
        let available = N - used;
        let count = read(&mut bytes[used..]).map_err(|source| io_error(operation, source))?;
        if count > available {
            return Err(Error::InvalidState(
                "proc read exceeded the supplied buffer",
            ));
        }
        if count == 0 {
            return Ok(used);
        }
        used += count;
        if used == N {
            return Err(Error::ProcessProfile(overflow));
        }
    }
    Err(Error::InvalidState("proc read has no sentinel capacity"))
}

pub(super) fn open(path: &CStr, operation: &'static str) -> Result<rustix::fd::OwnedFd, Error> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map_err(|source| io_error(operation, source))
}
