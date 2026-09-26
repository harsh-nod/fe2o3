//! Shared readiness mechanics, not child, deployment or endpoint admission.
//!
//! The caller retains the child and channels, prepays ATTEMPT_SCRATCH throughout
//! each phase, and reserves returned ready/descriptor storage before retaining it.
//! Profile reads require a nonblocking pipe; bootstrap receives use DONTWAIT.
//! Gate writes require the exclusively owned fresh release pipe. Logical attempt
//! limits and deadline checks do not bound a blocking syscall's duration.
//!
//! Unlike V1's old loops, deadlines are checked even on successful I/O, every
//! retry is finite, and a truncated one-byte packet cannot masquerade as a stage.

use fe2o3_external_anchor_provisioner::{
    EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1 as READY_BYTES,
    ExternalAnchorProvisioningReadyV1 as Ready,
};
use fe2o3_protected_service_spawn::{
    PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1,
};
use rustix::io::Errno;
use rustix::net::{RecvFlags, ReturnFlags};
use std::mem::size_of;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

pub(crate) const MAX_PHASE_ATTEMPTS: usize = 120_001;
pub(crate) const MAX_GATE_ATTEMPTS: usize = 64;
const MAX_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(1);
#[allow(unsafe_code)]
// SAFETY: the fixed payload size fits c_uint and CMSG alignment cannot overflow.
// Unlike rustix's unaligned byte-buffer allowance, this is the exact libc ABI size.
const CONTROL_BYTES: usize = unsafe { libc::CMSG_SPACE(size_of::<i32>() as u32) as usize };

/// Profile, ready and exec phases can each check liveness before all but their
/// last primary attempt. Observer::is_live has its own original-ledger quota.
pub(crate) const MAX_LIVENESS_CHECKS: usize = 3 * (MAX_PHASE_ATTEMPTS - 1);

/// Complete mechanical quota for one invocation of all four phases, using
/// Boundary::work(). Includes stage probes, pauses and failure FD disposal;
/// excludes liveness, caller validation, deadline construction and owner storage.
pub(crate) const MAX_WORK: usize = MAX_PHASE_ATTEMPTS
    * (Boundary::ProfileReady.work()
        + Boundary::ChildStage.work()
        + Boundary::ReadyTransfer.work()
        + Boundary::ExecEof.work())
    + MAX_LIVENESS_CHECKS * Boundary::Progress.work()
    + MAX_GATE_ATTEMPTS * Boundary::GateRelease.work();

/// Fixed logical frame, including payload/control, returned endpoint and failure
/// staging. Observer state/errors and nested liveness have separate caller charges.
pub(crate) const ATTEMPT_SCRATCH: usize = 4096
    + 8 * size_of::<Failure>()
    + 4 * size_of::<ReadyPacket>()
    + 4 * size_of::<(Ready, OwnedFd)>();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Boundary {
    ProfileReady,
    GateRelease,
    ReadyTransfer,
    ExecEof,
    ChildStage,
    Progress,
}

impl Boundary {
    /// One syscall attempt plus scalar/control allowance. ReadyTransfer also
    /// funds closing every FD that can fit in the fixed control buffer, even
    /// though at most one is retained. No Observer::is_live work is included.
    pub(crate) const fn work(self) -> usize {
        let (operations, bytes) = match self {
            Self::ReadyTransfer => (
                1 + CONTROL_BYTES / size_of::<i32>(),
                READY_BYTES + CONTROL_BYTES,
            ),
            Self::GateRelease => (1, 1),
            Self::Progress => (1, 0),
            Self::ProfileReady | Self::ExecEof | Self::ChildStage => (1, 2),
        };
        8 + operations * (1024 + 64) + bytes * 64 + 256
    }
}

pub(crate) trait Observer {
    type Error;

    /// Debit this attempt on the original ledger before any clock/I/O work.
    /// The caller keeps ATTEMPT_SCRATCH prepaid until the phase returns/unwinds.
    fn before_attempt(&mut self, boundary: Boundary) -> Result<(), Self::Error>;
    /// Native adapters invoke the real retained child's metered observation.
    fn is_live(&mut self) -> Result<bool, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Failure {
    Io {
        operation: &'static str,
        source: Errno,
    },
    InvalidTimeout,
    ChildStage(u8),
    ChildExited(&'static str),
    Timeout(&'static str),
    NoncanonicalProfileReady,
    NoncanonicalGateRelease,
    MalformedReadyTransfer,
    MalformedExecStatus,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Error<E> {
    Failure(Failure),
    Observer(E),
}

impl<E> From<Failure> for Error<E> {
    fn from(value: Failure) -> Self {
        Self::Failure(value)
    }
}

pub(crate) fn bounded_deadline(timeout: Duration) -> Result<Instant, Failure> {
    deadline_from(Instant::now(), timeout)
}

fn deadline_from(now: Instant, timeout: Duration) -> Result<Instant, Failure> {
    if timeout.is_zero() || timeout > MAX_TIMEOUT {
        return Err(Failure::InvalidTimeout);
    }
    now.checked_add(timeout).ok_or(Failure::InvalidTimeout)
}

pub(crate) fn await_profile_ready<O: Observer>(
    profile: BorrowedFd<'_>,
    bootstrap: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<(), Error<O::Error>> {
    Scheduler {
        observer,
        io: SystemIo,
        deadline,
    }
    .profile(profile, bootstrap)
}

pub(crate) fn release_child<O: Observer>(
    gate: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<(), Error<O::Error>> {
    Scheduler {
        observer,
        io: SystemIo,
        deadline,
    }
    .release(gate)
}

pub(crate) fn receive_ready<O: Observer>(
    bootstrap: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<(Ready, OwnedFd), Error<O::Error>> {
    Scheduler {
        observer,
        io: SystemIo,
        deadline,
    }
    .ready(bootstrap)
}

pub(crate) fn await_exec_eof<O: Observer>(
    bootstrap: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<(), Error<O::Error>> {
    Scheduler {
        observer,
        io: SystemIo,
        deadline,
    }
    .exec(bootstrap)
}

// The only production transport is SystemIo. This private test seam scripts
// syscall outcomes and time without constructing a public provider/authority API.
trait Io {
    fn now(&mut self) -> Instant;
    fn profile(&mut self, fd: BorrowedFd<'_>, bytes: &mut [u8; 2]) -> Result<usize, Errno>;
    fn gate(&mut self, fd: BorrowedFd<'_>) -> Result<usize, Errno>;
    fn status(&mut self, fd: BorrowedFd<'_>, bytes: &mut [u8; 2]) -> Result<(usize, usize), Errno>;
    fn ready(&mut self, fd: BorrowedFd<'_>) -> Result<ReadyPacket, Errno>;
    fn pause(&mut self, duration: Duration) -> Result<(), Errno>;
}

struct SystemIo;
impl Io for SystemIo {
    fn now(&mut self) -> Instant {
        Instant::now()
    }
    fn profile(&mut self, fd: BorrowedFd<'_>, bytes: &mut [u8; 2]) -> Result<usize, Errno> {
        rustix::io::read(fd, bytes)
    }
    fn gate(&mut self, fd: BorrowedFd<'_>) -> Result<usize, Errno> {
        rustix::io::write(fd, &[PROTECTED_SERVICE_GATE_RELEASE_V1])
    }
    fn status(&mut self, fd: BorrowedFd<'_>, bytes: &mut [u8; 2]) -> Result<(usize, usize), Errno> {
        rustix::net::recv(fd, bytes, RecvFlags::DONTWAIT)
    }
    fn ready(&mut self, fd: BorrowedFd<'_>) -> Result<ReadyPacket, Errno> {
        receive_packet(fd)
    }
    fn pause(&mut self, duration: Duration) -> Result<(), Errno> {
        let timeout = rustix::event::Timespec::try_from(duration).map_err(|_| Errno::INVAL)?;
        // A single poll, unlike std::thread::sleep's internal interruption loop.
        rustix::event::poll(&mut [], Some(&timeout)).map(|_| ())
    }
}

// Linux x86-64 CMSG_SPACE(one fd) holds one header and at most two fd integers;
// the alignment padding can contain a second unwanted right. It cannot contain
// two headers. Kernel-truncated undisclosed rights are closed by recvmsg itself.
#[repr(C)]
struct Control {
    length: usize,
    level: i32,
    kind: i32,
    descriptors: [i32; 2],
}
const _: () = assert!(size_of::<Control>() == CONTROL_BYTES);
const _: () = assert!(CONTROL_BYTES < 2 * size_of::<libc::cmsghdr>());
// Linux UAPI SCM_PIDFD, absent from the pinned libc/rustix ancillary enums.
const SCM_PIDFD: i32 = 0x04;

#[allow(unsafe_code)]
fn receive_packet(fd: BorrowedFd<'_>) -> Result<ReadyPacket, Errno> {
    let mut packet = ReadyPacket::empty();
    let mut control = Control {
        length: 0,
        level: 0,
        kind: 0,
        descriptors: [-1; 2],
    };
    let mut vector = libc::iovec {
        iov_base: packet.payload.as_mut_ptr().cast(),
        iov_len: READY_BYTES,
    };
    // SAFETY: all-zero msghdr is valid; this also initializes libc's musl padding.
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &raw mut vector;
    message.msg_iovlen = 1;
    message.msg_control = (&raw mut control).cast();
    message.msg_controllen = CONTROL_BYTES as _;
    // SAFETY: fd is borrowed live; every pointer names initialized writable
    // storage of its exact advertised size. There is one bounded syscall attempt.
    let received = unsafe {
        libc::recvmsg(
            fd.as_raw_fd(),
            &raw mut message,
            libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
        )
    };
    if received < 0 {
        // SAFETY: the failed syscall just set this thread's errno.
        return Err(Errno::from_raw_os_error(unsafe {
            *libc::__errno_location()
        }));
    }
    packet.bytes = received as usize;
    packet.flags = ReturnFlags::from_bits_retain(message.msg_flags as _);
    // SAFETY: only this successful kernel receive supplies descriptor integers.
    // No fallible observer or parser runs before all disclosed FDs enter owners.
    unsafe {
        take_control(
            &mut packet.rights,
            &control,
            message.msg_controllen as usize,
        );
    }
    Ok(packet)
}

/// Takes every disclosed FD, including rejected SCM_PIDFD, before shape checks.
/// The control header and FD integers must come from a successful recvmsg (or
/// ownership-transferring private tests), never caller-supplied untrusted bytes.
#[allow(unsafe_code)]
unsafe fn take_control(rights: &mut Rights, control: &Control, length: usize) {
    if length == 0 {
        return;
    }
    let header_bytes = size_of::<libc::cmsghdr>();
    let declared = control.length;
    if length > CONTROL_BYTES
        || length < header_bytes
        || declared < header_bytes
        || declared > length
    {
        rights.invalid = true;
    }
    let supported = control.level == libc::SOL_SOCKET && control.kind == libc::SCM_RIGHTS;
    if !supported {
        rights.invalid = true;
    }
    let owns_fds = control.level == libc::SOL_SOCKET
        && (control.kind == libc::SCM_RIGHTS || control.kind == SCM_PIDFD);
    if owns_fds {
        let payload = declared
            .min(length)
            .min(CONTROL_BYTES)
            .saturating_sub(header_bytes);
        if payload == 0 || payload % size_of::<i32>() != 0 {
            rights.invalid = true;
        }
        for &raw in control.descriptors.iter().take(payload / size_of::<i32>()) {
            if raw < 0 {
                rights.invalid = true;
                continue;
            }
            // SAFETY: successful recvmsg installed each disclosed descriptor once;
            // push owns it or closes it immediately when the message is rejected.
            rights.push(unsafe { OwnedFd::from_raw_fd(raw) });
        }
    }
}

#[derive(Default)]
struct Rights {
    fd: Option<OwnedFd>,
    invalid: bool,
}
impl Rights {
    fn push(&mut self, fd: OwnedFd) {
        if self.fd.is_some() || self.invalid {
            self.invalid = true;
            drop(fd);
        } else {
            self.fd = Some(fd);
        }
    }
}

struct ReadyPacket {
    payload: [u8; READY_BYTES],
    bytes: usize,
    flags: ReturnFlags,
    rights: Rights,
}
impl ReadyPacket {
    fn empty() -> Self {
        Self {
            payload: [0; READY_BYTES],
            bytes: 0,
            flags: ReturnFlags::empty(),
            rights: Rights::default(),
        }
    }
    fn validate(self) -> Result<(Ready, OwnedFd), Failure> {
        // Truncation/unknown ancillary rejection precedes even the stage shortcut.
        if self.rights.invalid
            || self
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(Failure::MalformedReadyTransfer);
        }
        if self.bytes == 1 && self.rights.fd.is_none() {
            return Err(Failure::ChildStage(self.payload[0]));
        }
        if self.bytes != READY_BYTES {
            return Err(Failure::MalformedReadyTransfer);
        }
        let ready = Ready::decode(&self.payload).map_err(|_| Failure::MalformedReadyTransfer)?;
        let fd = self.rights.fd.ok_or(Failure::MalformedReadyTransfer)?;
        Ok((ready, fd))
    }
}

struct Scheduler<'a, O, I> {
    observer: &'a mut O,
    io: I,
    deadline: Instant,
}

impl<O: Observer, I: Io> Scheduler<'_, O, I> {
    fn deadline(&mut self, phase: &'static str) -> Result<(), Error<O::Error>> {
        if self.io.now() >= self.deadline {
            Err(Failure::Timeout(phase).into())
        } else {
            Ok(())
        }
    }
    fn begin(&mut self, boundary: Boundary, phase: &'static str) -> Result<(), Error<O::Error>> {
        self.observer
            .before_attempt(boundary)
            .map_err(Error::Observer)?;
        self.deadline(phase)
    }
    fn progress(&mut self, phase: &'static str) -> Result<(), Error<O::Error>> {
        self.begin(Boundary::Progress, phase)?;
        if !self.observer.is_live().map_err(Error::Observer)? {
            return Err(Failure::ChildExited(phase).into());
        }
        let remaining = self.deadline.saturating_duration_since(self.io.now());
        if remaining.is_zero() {
            return Err(Failure::Timeout(phase).into());
        }
        match self.io.pause(remaining.min(POLL_INTERVAL)) {
            Ok(()) | Err(Errno::INTR) => self.deadline(phase),
            Err(source) => Err(Failure::Io {
                operation: "wait for child progress",
                source,
            }
            .into()),
        }
    }
    fn stage(&mut self, bootstrap: BorrowedFd<'_>) -> Result<Option<u8>, Error<O::Error>> {
        self.begin(Boundary::ChildStage, "child profile")?;
        let mut bytes = [0; 2];
        let result = self.io.status(bootstrap, &mut bytes);
        self.deadline("child profile")?;
        match result {
            Ok((0, 0)) | Err(Errno::AGAIN | Errno::INTR) => Ok(None),
            Ok((1, 1)) => Ok(Some(bytes[0])),
            Ok(_) => Err(Failure::MalformedExecStatus.into()),
            Err(source) => Err(Failure::Io {
                operation: "read child failure stage",
                source,
            }
            .into()),
        }
    }
    fn profile(
        &mut self,
        profile: BorrowedFd<'_>,
        bootstrap: BorrowedFd<'_>,
    ) -> Result<(), Error<O::Error>> {
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ProfileReady, "child profile")?;
            let mut bytes = [0; 2];
            let result = self.io.profile(profile, &mut bytes);
            self.deadline("child profile")?;
            match result {
                Ok(1) if bytes[0] == PROTECTED_SERVICE_PROFILE_READY_V1 => return Ok(()),
                Ok(0) => {
                    // Preserve V1 EOF fallback, but never swallow observer refusal
                    // or deadline exhaustion as a purported child exit.
                    return match self.stage(bootstrap) {
                        Ok(Some(stage)) => Err(Failure::ChildStage(stage).into()),
                        Err(e @ Error::Observer(_))
                        | Err(e @ Error::Failure(Failure::Timeout(_))) => Err(e),
                        _ => Err(Failure::ChildExited("child profile").into()),
                    };
                }
                Ok(_) => return Err(Failure::NoncanonicalProfileReady.into()),
                Err(Errno::AGAIN | Errno::INTR) => {
                    if let Some(stage) = self.stage(bootstrap)? {
                        return Err(Failure::ChildStage(stage).into());
                    }
                    if attempt + 1 == MAX_PHASE_ATTEMPTS {
                        break;
                    }
                    self.progress("child profile")?;
                }
                Err(source) => {
                    return Err(Failure::Io {
                        operation: "read child-profile record",
                        source,
                    }
                    .into());
                }
            }
        }
        Err(Failure::Timeout("child profile").into())
    }
    fn release(&mut self, gate: BorrowedFd<'_>) -> Result<(), Error<O::Error>> {
        for _ in 0..MAX_GATE_ATTEMPTS {
            self.begin(Boundary::GateRelease, "child release gate")?;
            let result = self.io.gate(gate);
            self.deadline("child release gate")?;
            match result {
                Ok(1) => return Ok(()),
                Ok(_) => return Err(Failure::NoncanonicalGateRelease.into()),
                Err(Errno::INTR) => {}
                Err(source) => {
                    return Err(Failure::Io {
                        operation: "release measured helper child",
                        source,
                    }
                    .into());
                }
            }
        }
        Err(Failure::Timeout("child release gate").into())
    }
    fn ready(&mut self, bootstrap: BorrowedFd<'_>) -> Result<(Ready, OwnedFd), Error<O::Error>> {
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ReadyTransfer, "helper-ready transfer")?;
            let result = self.io.ready(bootstrap);
            self.deadline("helper-ready transfer")?;
            match result {
                Ok(packet) => return packet.validate().map_err(Error::Failure),
                Err(Errno::AGAIN | Errno::INTR) => {
                    if attempt + 1 == MAX_PHASE_ATTEMPTS {
                        break;
                    }
                    self.progress("helper-ready transfer")?;
                }
                Err(source) => {
                    return Err(Failure::Io {
                        operation: "receive helper-ready transfer",
                        source,
                    }
                    .into());
                }
            }
        }
        Err(Failure::Timeout("helper-ready transfer").into())
    }
    fn exec(&mut self, bootstrap: BorrowedFd<'_>) -> Result<(), Error<O::Error>> {
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ExecEof, "daemon exec EOF")?;
            let mut bytes = [0; 2];
            let result = self.io.status(bootstrap, &mut bytes);
            self.deadline("daemon exec EOF")?;
            match result {
                Ok((0, 0)) => return Ok(()),
                Ok((1, 1)) => return Err(Failure::ChildStage(bytes[0]).into()),
                Ok(_) => return Err(Failure::MalformedExecStatus.into()),
                Err(Errno::AGAIN | Errno::INTR) => {
                    if attempt + 1 == MAX_PHASE_ATTEMPTS {
                        break;
                    }
                    self.progress("daemon exec EOF")?;
                }
                Err(source) => {
                    return Err(Failure::Io {
                        operation: "observe daemon exec EOF",
                        source,
                    }
                    .into());
                }
            }
        }
        Err(Failure::Timeout("daemon exec EOF").into())
    }
}

#[cfg(test)]
#[path = "launch_io_tests.rs"]
mod tests;
