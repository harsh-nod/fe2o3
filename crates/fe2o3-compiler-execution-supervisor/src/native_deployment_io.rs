//! Fixed raw-source custody and bounded byte transport, not native admission.
//!
//! Startup creates Sources before its first fallible budget call. Its outer
//! original-ledger frame prepays SOURCE_WORK/SOURCE_SCRATCH and all input owners.
//! Capability/image duplication requires the caller's additional FULL overlapping
//! File/image charge; these helpers neither infer lengths nor retire reservations.

use crate::deployment::{self, ProtectedIssuerDeploymentErrorV1 as MechanicalError};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::{io::Errno, net::SendFlags};
use std::{
    error::Error as StdError,
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{AsFd, BorrowedFd, OwnedFd, RawFd},
    time::{Duration, Instant},
};

pub(crate) const INPUT_FDS: [RawFd; 11] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 220];
pub(crate) const LISTENER: usize = 0;
pub(crate) const ROOT: usize = 1;
pub(crate) const LAUNCHER: usize = 2;
pub(crate) const ISSUER: usize = 3;
pub(crate) const POLICY: usize = 4;
pub(crate) const KEY: usize = 5;
pub(crate) const PEER: usize = 6;
pub(crate) const PIDFD: usize = 7;
pub(crate) const BOOTSTRAP: usize = 8;
pub(crate) const LIFECYCLE: usize = 9;
pub(crate) const DEPLOYMENT: usize = 10;
pub(crate) const FILE_STORAGE: usize = size_of::<(File, usize)>();

/// One full-table validation, at most eleven duplicate/close operations, guard
/// cleanup, three close_range calls and the shared bootstrap predicate fit within
/// 128 fixed kernel operations. All are single attempts; caller admission is extra.
pub(crate) const SOURCE_WORK: usize = 8 + 128 * (1024 + 64) + 256;
pub(crate) const SOURCE_SCRATCH: usize = 4096 + 11 * FILE_STORAGE + 4 * size_of::<Sources>();

pub(crate) const READY_BYTES: usize =
    fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V2;
pub(crate) const MAX_SEND_ATTEMPTS: usize = 30_001;
const SEND_TIMEOUT: Duration = Duration::from_secs(30);
const PAUSE: Duration = Duration::from_millis(1);
const ENTRY: usize = 8;
/// Entry, initial deadline clock, fixed length/floor/control bookkeeping.
pub(crate) const SEND_BASE_WORK: usize = ENTRY + 1024 + 256;
/// One send, clocks before/after it, and a conservative full fixed-byte allowance.
pub(crate) const SEND_ATTEMPT_WORK: usize = 3 * (1024 + 64) + 64 * READY_BYTES + 256;
/// Single-attempt nanosleep and clocks before/after; EINTR never causes a sleep retry.
pub(crate) const SEND_PAUSE_WORK: usize = 3 * (1024 + 64) + 256;
/// Complete maximum, not a fresh sub-budget or a mandatory all-at-once charge.
pub(crate) const SEND_WORK: usize = SEND_BASE_WORK
    + MAX_SEND_ATTEMPTS * SEND_ATTEMPT_WORK
    + (MAX_SEND_ATTEMPTS - 1) * SEND_PAUSE_WORK;
/// Fixed logical transport frame, excluding the prepaid bootstrap and ready owner.
pub(crate) const SEND_SCRATCH: usize = 4096 + 4 * READY_BYTES + 8 * size_of::<Error>();

type Result<T> = std::result::Result<T, Error>;

/// Bounded descriptor-custody or readiness-transport refusal, not authority admission.
#[derive(Debug)]
#[non_exhaustive]
pub enum NativeIssuerStartupIoErrorV2 {
    /// The original request ledger refused work, scratch or an input floor.
    Resource(Resource),
    /// One finite mechanical operating-system attempt failed.
    Io {
        /// Fixed operation label; no caller-supplied or allocated diagnostic.
        operation: &'static str,
        /// Original operating-system error code.
        errno: Errno,
    },
    /// A fixed inherited source unexpectedly has CLOEXEC set.
    SourceFlags(RawFd),
    /// Source custody is absent, consumed, unvalidated or otherwise inconsistent.
    SourceState,
    /// The shared bootstrap socket/peer predicate refused the descriptor.
    InvalidBootstrap,
    /// Readiness bytes do not have the fixed native wire length.
    ReadyLength,
    /// The checked readiness deadline overflowed or expired, including after send.
    ReadyTimeout,
    /// Every finite send attempt was consumed, even if the clock did not advance.
    ReadyAttempts,
    /// The seqpacket send did not report exactly one complete readiness record.
    ReadyPartial,
}
pub(crate) use NativeIssuerStartupIoErrorV2 as Error;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<MechanicalError> for Error {
    fn from(value: MechanicalError) -> Self {
        match value {
            MechanicalError::Descriptor { operation, source } => Self::Io {
                operation,
                errno: Errno::from_raw_os_error(source.raw_os_error().unwrap_or(libc::EIO)),
            },
            MechanicalError::UnexpectedCloseOnExec(fd) => Self::SourceFlags(fd),
            MechanicalError::InvalidBootstrap => Self::InvalidBootstrap,
            // Only the descriptor helpers feed this conversion; no authority
            // variant is constructed or accepted by this mechanical module.
            _ => Self::SourceState,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Io { operation, errno } => write!(f, "{operation}: {errno}"),
            Self::SourceFlags(fd) => write!(f, "native supervisor source {fd} is CLOEXEC"),
            Self::SourceState => f.write_str("native supervisor source table is not available"),
            Self::InvalidBootstrap => f.write_str("invalid native supervisor bootstrap channel"),
            Self::ReadyLength => f.write_str("invalid native supervisor readiness length"),
            Self::ReadyTimeout => f.write_str("native supervisor readiness deadline expired"),
            Self::ReadyAttempts => f.write_str("native supervisor readiness attempts exhausted"),
            Self::ReadyPartial => f.write_str("partial native supervisor readiness send"),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Io { errno, .. } => Some(errno),
            _ => None,
        }
    }
}

pub(crate) struct Sources {
    live: [bool; 11],
    validated: bool,
}
impl Sources {
    /// Takes exclusive raw-slot cleanup custody, without any fallible operation.
    ///
    /// # Safety
    /// Sole startup in a dedicated process: no Rust FD owner or concurrent actor
    /// may own, close, replace or allocate into these slots until this guard has
    /// consumed them. Missing slots are permitted only for fail-closed validation.
    /// Create before budget refusal, but validate the whole table before any
    /// admission/duplication can reuse a missing slot. This proves no object role.
    pub(crate) unsafe fn new() -> Self {
        Self {
            live: [true; 11],
            validated: false,
        }
    }

    pub(crate) fn validate(&mut self) -> Result<()> {
        self.validated = false;
        if !self.live.iter().all(|live| *live) {
            return Err(Error::SourceState);
        }
        for fd in INPUT_FDS {
            deployment::require_inherited(fd)?;
        }
        self.validated = true;
        Ok(())
    }

    /// Duplicates at >=256 with CLOEXEC, then consumes the fixed raw slot.
    /// Caller prepays simultaneous full source/duplicate charges before this call.
    pub(crate) fn take(&mut self, role: usize) -> Result<File> {
        self.take_with(role, deployment::close_inherited)
    }

    fn take_with(
        &mut self,
        role: usize,
        close: impl FnOnce(RawFd) -> std::result::Result<(), MechanicalError>,
    ) -> Result<File> {
        if !self.validated || self.live.get(role) != Some(&true) {
            return Err(Error::SourceState);
        }
        let retained = deployment::duplicate_inherited(INPUT_FDS[role])?;
        self.close_with(role, close)?;
        Ok(retained.into())
    }

    /// Single close after capability admission retained its own charged descriptor.
    /// Clear custody BEFORE close: Linux close errors must never lead to a retry.
    pub(crate) fn close(&mut self, role: usize) -> Result<()> {
        self.close_with(role, deployment::close_inherited)
    }

    fn close_with(
        &mut self,
        role: usize,
        close: impl FnOnce(RawFd) -> std::result::Result<(), MechanicalError>,
    ) -> Result<()> {
        let live = self.live.get_mut(role).ok_or(Error::SourceState)?;
        if !*live {
            return Err(Error::SourceState);
        }
        *live = false;
        close(INPUT_FDS[role]).map_err(Into::into)
    }
}
impl Drop for Sources {
    fn drop(&mut self) {
        for (fd, live) in INPUT_FDS.into_iter().zip(self.live) {
            if live {
                // SAFETY: exclusive raw custody was transferred at construction.
                // Close is one attempt even on error; no later guard can retry it.
                unsafe {
                    libc::close(fd);
                }
            }
        }
    }
}

/// Removes every descriptor outside the eleven-role input ABI, including stdio.
///
/// # Safety
/// Dedicated startup only, before ANY descriptor-owning admission/duplication.
/// No ambient Rust FD owners, pending child cleanup or concurrent descriptor
/// users may exist. A failure may have closed earlier ranges; never retry it.
pub(crate) unsafe fn close_unrelated() -> Result<()> {
    for (low, high) in [(0_u32, 2_u32), (13, 219), (221, u32::MAX)] {
        // SAFETY: the caller exclusively owns this dedicated descriptor table.
        if unsafe { libc::syscall(libc::SYS_close_range, low, high, 0) } != 0 {
            return Err(Error::Io {
                operation: "close unrelated native supervisor descriptors",
                errno: Errno::from_raw_os_error(
                    std::io::Error::last_os_error()
                        .raw_os_error()
                        .unwrap_or(libc::EIO),
                ),
            });
        }
    }
    Ok(())
}

/// Sends already canonical native readiness bytes, not a parser or admission.
/// Prepay FILE_STORAGE + bytes.len(); a containing readiness owner's FULL charge
/// remains live. The bootstrap must already pass the shared root-peer predicate.
/// Charges every send and pause before clocks/syscalls; finite attempts also bound
/// a frozen clock. A send observed at/after the deadline fails even if delivered.
/// Entry storage/history survive all exits, including unwind; owners stay borrowed.
pub(crate) fn send_ready(bootstrap: &OwnedFd, bytes: &[u8], b: &mut Budget<'_>) -> Result<()> {
    send_ready_with(bootstrap.as_fd(), bytes, b, &mut SystemReadyIo)
}

trait ReadyIo {
    fn now(&mut self) -> Instant;
    fn send(
        &mut self,
        bootstrap: BorrowedFd<'_>,
        bytes: &[u8],
    ) -> std::result::Result<usize, Errno>;
    fn pause(&mut self) -> std::result::Result<(), Errno>;
}
struct SystemReadyIo;
impl ReadyIo for SystemReadyIo {
    fn now(&mut self) -> Instant {
        Instant::now()
    }
    fn send(
        &mut self,
        bootstrap: BorrowedFd<'_>,
        bytes: &[u8],
    ) -> std::result::Result<usize, Errno> {
        rustix::net::send(bootstrap, bytes, SendFlags::DONTWAIT | SendFlags::NOSIGNAL)
    }
    fn pause(&mut self) -> std::result::Result<(), Errno> {
        let interval = libc::timespec {
            tv_sec: 0,
            tv_nsec: PAUSE.as_nanos() as libc::c_long,
        };
        // SAFETY: fixed valid timespec, no remaining-time pointer or retry loop.
        if unsafe { libc::nanosleep(&interval, std::ptr::null_mut()) } == 0 {
            Ok(())
        } else {
            Err(Errno::from_raw_os_error(
                std::io::Error::last_os_error()
                    .raw_os_error()
                    .unwrap_or(libc::EIO),
            ))
        }
    }
}

fn send_ready_with(
    bootstrap: BorrowedFd<'_>,
    bytes: &[u8],
    b: &mut Budget<'_>,
    io: &mut impl ReadyIo,
) -> Result<()> {
    let floor = FILE_STORAGE
        .checked_add(bytes.len())
        .ok_or(Resource::Arithmetic)?;
    b.with_prepaid_scope(floor, ENTRY, SEND_BASE_WORK, SEND_SCRATCH, |b| {
        if bytes.len() != READY_BYTES {
            return Err(Error::ReadyLength);
        }
        let deadline = io
            .now()
            .checked_add(SEND_TIMEOUT)
            .ok_or(Error::ReadyTimeout)?;
        for attempt in 0..MAX_SEND_ATTEMPTS {
            b.charge_work(SEND_ATTEMPT_WORK)?;
            if io.now() >= deadline {
                return Err(Error::ReadyTimeout);
            }
            let sent = io.send(bootstrap, bytes);
            if io.now() >= deadline {
                return Err(Error::ReadyTimeout);
            }
            match sent {
                Ok(n) if n == bytes.len() => return Ok(()),
                Ok(_) => return Err(Error::ReadyPartial),
                Err(Errno::AGAIN | Errno::INTR) => {}
                Err(errno) => {
                    return Err(Error::Io {
                        operation: "send native supervisor readiness",
                        errno,
                    });
                }
            }
            if attempt + 1 == MAX_SEND_ATTEMPTS {
                return Err(Error::ReadyAttempts);
            }
            b.charge_work(SEND_PAUSE_WORK)?;
            if io.now() >= deadline {
                return Err(Error::ReadyTimeout);
            }
            match io.pause() {
                Ok(()) | Err(Errno::INTR) => {}
                Err(errno) => {
                    return Err(Error::Io {
                        operation: "pause native supervisor readiness",
                        errno,
                    });
                }
            }
            if io.now() >= deadline {
                return Err(Error::ReadyTimeout);
            }
        }
        Err(Error::ReadyAttempts)
    })
}

const _: () = {
    use crate::deployment::*;
    assert!(INPUT_FDS[LISTENER] == COMPILER_EXECUTION_SUPERVISOR_LISTENER_FD_V1);
    assert!(INPUT_FDS[ROOT] == COMPILER_EXECUTION_SUPERVISOR_ROOT_FD_V1);
    assert!(INPUT_FDS[LAUNCHER] == COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_FD_V1);
    assert!(INPUT_FDS[ISSUER] == COMPILER_EXECUTION_SUPERVISOR_ISSUER_FD_V1);
    assert!(INPUT_FDS[POLICY] == COMPILER_EXECUTION_SUPERVISOR_POLICY_FD_V1);
    assert!(INPUT_FDS[KEY] == COMPILER_EXECUTION_SUPERVISOR_SIGNING_KEY_FD_V1);
    assert!(INPUT_FDS[PEER] == COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PEER_FD_V1);
    assert!(INPUT_FDS[PIDFD] == COMPILER_EXECUTION_SUPERVISOR_EXTERNAL_ANCHOR_PIDFD_V1);
    assert!(INPUT_FDS[BOOTSTRAP] == COMPILER_EXECUTION_SUPERVISOR_BOOTSTRAP_FD_V1);
    assert!(INPUT_FDS[LIFECYCLE] == COMPILER_EXECUTION_SUPERVISOR_LIFECYCLE_FD_V1);
    assert!(
        INPUT_FDS[DEPLOYMENT]
            == fe2o3_compiler_closure_capability::COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_FD_V1
    );
    assert!(
        READY_BYTES
            == fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V3
    );
};

#[cfg(test)]
#[path = "native_deployment_io_tests.rs"]
mod tests;
