use super::*;
use rustix::event::{Timespec, poll};
use std::time::{Duration, Instant};

/// Inert name of a finite native process-protocol boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedIssuerBoundaryV2 {
    /// Direct child profile acknowledgement before gate release.
    Profile,
    /// Static-launcher exec-status pipe completion, not issuer readiness.
    Exec,
    /// Exact issuer readiness frame and EOF on its private pipe.
    Readiness,
    /// Atomic readiness publication on the Cargo control socket.
    Publication,
    /// Natural consuming terminal wait on the exact child pidfd.
    Exit,
}

/// Finite attempt count and deadline; neither grants process authority.
///
/// EINTR, EAGAIN, short reads, and pending observations consume attempts.
/// These are logical operation bounds, not kernel scheduling/latency guarantees.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerWaitV2 {
    attempts: usize,
    timeout: Duration,
}
impl ProtectedIssuerWaitV2 {
    /// Largest accepted protocol-attempt count per boundary.
    pub const MAX_ATTEMPTS: usize = 4096;
    /// Longest accepted observation deadline per boundary.
    pub const MAX_TIMEOUT: Duration = Duration::from_secs(120);
    /// Four weighted syscalls cover read/send/wait, liveness, a failure probe and
    /// optional poll; byte/field work includes the complete fixed readiness frame.
    pub const ATTEMPT_WORK: usize = 4 * 1024 + 8 * READY_BYTES + 256;

    /// Admits bounded inert limits without any I/O or budget creation.
    pub fn new(attempts: usize, timeout: Duration) -> Result<Self> {
        if attempts == 0
            || attempts > Self::MAX_ATTEMPTS
            || timeout.is_zero()
            || timeout > Self::MAX_TIMEOUT
        {
            return Err(Error::InvalidWait);
        }
        Ok(Self { attempts, timeout })
    }
    /// Total prepaid logical work for all attempts, including setup/final checks.
    pub const fn work(self) -> usize {
        ENTRY + Self::ATTEMPT_WORK * self.attempts
    }
    /// Maximum number of protocol observations before refusal.
    pub const fn attempts(self) -> usize {
        self.attempts
    }
    /// Absolute observation interval, excluding scheduler and mutex guarantees.
    pub const fn timeout(self) -> Duration {
        self.timeout
    }

    pub(super) fn deadline(self) -> Result<Instant> {
        Instant::now()
            .checked_add(self.timeout)
            .ok_or(Error::InvalidWait)
    }
}

pub(super) fn before_deadline(deadline: Instant, boundary: Boundary) -> Result<()> {
    if Instant::now() >= deadline {
        Err(Error::Timeout(boundary))
    } else {
        Ok(())
    }
}

// All work is prepaid by the containing lifecycle operation. The callback is
// crate-private and performs at most the fixed schedule described by ATTEMPT_WORK.
pub(super) fn attempts<T>(
    limits: Wait,
    deadline: Instant,
    boundary: Boundary,
    mut observe: impl FnMut() -> Result<Option<T>>,
) -> Result<T> {
    for turn in 0..limits.attempts {
        before_deadline(deadline, boundary)?;
        if let Some(value) = observed(observe()?, deadline, boundary)? {
            return Ok(value);
        }
        if turn + 1 < limits.attempts {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let nanos = remaining.min(Duration::from_millis(1)).as_nanos() as i64;
            let delay = Timespec {
                tv_sec: 0,
                tv_nsec: nanos,
            };
            match poll(&mut [], Some(&delay)) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(errno) => {
                    return Err(Error::Io {
                        operation: "poll native issuer boundary",
                        errno,
                    });
                }
            }
        }
    }
    Err(Error::Attempts(boundary))
}

// Publication and terminal reap commit externally visible outcomes. A clock
// observation after success cannot undo them or turn them into a timeout.
pub(super) fn observed<T>(
    result: Option<T>,
    deadline: Instant,
    boundary: Boundary,
) -> Result<Option<T>> {
    if result.is_none() || !matches!(boundary, Boundary::Publication | Boundary::Exit) {
        before_deadline(deadline, boundary)?;
    }
    Ok(result)
}

pub(super) fn live(process: &super::super::IssuerChild, boundary: Boundary) -> Result<()> {
    if process.observe_live()? {
        Ok(())
    } else {
        Err(Error::ChildExited(boundary))
    }
}
