//! Finite policy-neutral acceptance. The containing native call prepays each turn.
use super::{SocketError, socket_io_error};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    io::Errno,
    net::{SocketFlags, accept_with},
};
use std::{
    os::fd::OwnedFd,
    time::{Duration, Instant},
};

pub(super) const MAX_ATTEMPTS: usize = 4096;
pub(super) const MAX_TIMEOUT: Duration = Duration::from_secs(120);
// One poll, one accept, bounded flag handling and descriptor retirement.
pub(super) const ATTEMPT_WORK: usize = 4 * 1024;

#[derive(Debug)]
pub(super) enum AcceptError {
    InvalidLimits,
    Timeout,
    Attempts,
    Socket(SocketError),
}

pub(super) fn accept(
    listener: &OwnedFd,
    attempts: usize,
    timeout: Duration,
) -> Result<OwnedFd, AcceptError> {
    bounded(attempts, timeout, |deadline| observe(listener, deadline))
}

fn bounded(
    attempts: usize,
    timeout: Duration,
    mut observe: impl FnMut(Instant) -> Result<Option<OwnedFd>, AcceptError>,
) -> Result<OwnedFd, AcceptError> {
    if attempts == 0 || attempts > MAX_ATTEMPTS || timeout.is_zero() || timeout > MAX_TIMEOUT {
        return Err(AcceptError::InvalidLimits);
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(AcceptError::InvalidLimits)?;
    for _ in 0..attempts {
        before_deadline(deadline)?;
        let control = observe(deadline)?;
        if let Some(control) = observed(control, deadline)? {
            return Ok(control);
        }
    }
    Err(AcceptError::Attempts)
}

fn observed(control: Option<OwnedFd>, deadline: Instant) -> Result<Option<OwnedFd>, AcceptError> {
    before_deadline(deadline)?;
    Ok(control)
}

fn before_deadline(deadline: Instant) -> Result<(), AcceptError> {
    if Instant::now() >= deadline {
        Err(AcceptError::Timeout)
    } else {
        Ok(())
    }
}

fn observe(listener: &OwnedFd, deadline: Instant) -> Result<Option<OwnedFd>, AcceptError> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let timeout = Timespec {
        tv_sec: remaining.as_secs() as i64,
        tv_nsec: i64::from(remaining.subsec_nanos()),
    };
    let mut descriptors = [PollFd::new(
        listener,
        PollFlags::IN | PollFlags::ERR | PollFlags::HUP,
    )];
    match poll(&mut descriptors, Some(&timeout)) {
        Ok(0) => return Err(AcceptError::Timeout),
        Err(Errno::INTR) => return Ok(None),
        Err(source) => return Err(io("poll protected issuer listener", source)),
        Ok(_) => {}
    }
    let events = descriptors[0].revents();
    if events.contains(PollFlags::NVAL) {
        return Err(AcceptError::Socket(SocketError::InvalidListener(
            "listener descriptor became invalid",
        )));
    }
    if events.intersects(PollFlags::ERR | PollFlags::HUP) {
        return Err(AcceptError::Socket(SocketError::InvalidListener(
            "listener reported an error or hangup",
        )));
    }
    if !events.contains(PollFlags::IN) {
        return Ok(None);
    }
    before_deadline(deadline)?;
    match accept_with(listener, SocketFlags::CLOEXEC | SocketFlags::NONBLOCK) {
        Ok(control) => Ok(Some(control)),
        Err(Errno::AGAIN | Errno::INTR) => Ok(None),
        Err(source) => Err(io("accept protected issuer control", source)),
    }
}

fn io(operation: &'static str, source: Errno) -> AcceptError {
    AcceptError::Socket(socket_io_error(operation, source.into()))
}

#[cfg(test)]
#[path = "listener_native_accept_tests.rs"]
mod tests;
