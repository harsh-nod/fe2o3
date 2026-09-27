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
//! Terminal status enables SO_PASSCRED before receive. Linux marks even queued
//! empty records with SCM_CREDENTIALS; EOF carries no record. These credentials
//! distinguish framing only and never authenticate service identity. Failure to
//! enable the option refuses, without a hangup-only or unmarked-record fallback.

use crate::{PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1};
use rustix::io::Errno;
use rustix::net::ReturnFlags;
use std::mem::size_of;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

/// Finite primary attempts in each readiness phase.
pub const MAX_PHASE_ATTEMPTS: usize = 120_001;
/// Finite interrupted writes to the exclusive release gate.
pub const MAX_GATE_ATTEMPTS: usize = 64;
const MAX_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(1);
/// Largest inert readiness payload accepted by this fixed-size transport.
pub const MAX_READY_BYTES: usize = 88;
#[allow(unsafe_code)]
// SAFETY: the fixed payload size fits c_uint and CMSG alignment cannot overflow.
// Unlike rustix's unaligned byte-buffer allowance, this is the exact libc ABI size.
const CONTROL_BYTES: usize = unsafe { libc::CMSG_SPACE(size_of::<libc::ucred>() as u32) as usize };

/// Profile, ready and exec phases can each check liveness before all but their
/// last primary attempt. Observer::is_live has its own original-ledger quota.
pub const MAX_LIVENESS_CHECKS: usize = 3 * (MAX_PHASE_ATTEMPTS - 1);

/// Complete mechanical quota for one invocation of all four phases, using
/// Boundary::work(). Includes stage probes, pauses and failure FD disposal;
/// excludes liveness, caller validation, deadline construction and owner storage.
pub const MAX_WORK: usize = MAX_PHASE_ATTEMPTS
    * (Boundary::ProfileReady.work()
        + Boundary::ChildStage.work()
        + Boundary::ReadyTransfer.work()
        + Boundary::ExecEof.work())
    + MAX_LIVENESS_CHECKS * Boundary::Progress.work()
    + MAX_GATE_ATTEMPTS * Boundary::GateRelease.work();

/// Fixed logical frame, including payload/control, returned endpoint and failure
/// staging. Observer state/errors and nested liveness have separate caller charges.
pub const ATTEMPT_SCRATCH: usize = 4096
    + 8 * size_of::<Failure>()
    + 4 * size_of::<ReadyPacket>()
    + 4 * size_of::<([u8; MAX_READY_BYTES], Option<OwnedFd>)>();

/// Mechanical attempt charged before clock or descriptor observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Boundary {
    /// Read the two-byte profile pipe.
    ProfileReady,
    /// Write the exclusive one-byte gate.
    GateRelease,
    /// Receive a bounded payload and control record.
    ReadyTransfer,
    /// Enable kernel record credentials and distinguish EOF from an empty packet.
    ExecEof,
    /// Probe a child failure stage, including control disposal.
    ChildStage,
    /// Observe child liveness and perform one bounded pause.
    Progress,
}

impl Boundary {
    /// One receive (plus terminal socket setup) and scalar/control allowance. Also
    /// funds closing every FD that can fit in the fixed control buffer, even
    /// though at most one is retained. No Observer::is_live work is included.
    pub const fn work(self) -> usize {
        let (operations, bytes) = match self {
            Self::ReadyTransfer => (
                1 + CONTROL_BYTES / size_of::<i32>(),
                MAX_READY_BYTES + CONTROL_BYTES,
            ),
            Self::GateRelease => (1, 1),
            Self::Progress => (1, 0),
            Self::ProfileReady => (1, 2),
            Self::ExecEof | Self::ChildStage => (
                2 + CONTROL_BYTES / size_of::<i32>(),
                MAX_READY_BYTES + CONTROL_BYTES,
            ),
        };
        8 + operations * (1024 + 64) + bytes * 64 + 256
    }
}

/// Metering and retained-child observations supplied by the owning coordinator.
/// This interface and its results grant no child or deployment authority.
pub trait Observer {
    /// Caller-owned refusal type, propagated without losing its category.
    type Error;

    /// Debit this attempt on the original ledger before any clock/I/O work.
    /// The caller keeps ATTEMPT_SCRATCH prepaid until the phase returns/unwinds.
    fn before_attempt(&mut self, boundary: Boundary) -> Result<(), Self::Error>;
    /// Native adapters invoke the real retained child's metered observation.
    fn is_live(&mut self) -> Result<bool, Self::Error>;
}

/// Fixed mechanical failure categories, with no owned diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// A single syscall attempt failed.
    Io {
        /// Fixed operation name.
        operation: &'static str,
        /// Kernel errno.
        source: Errno,
    },
    /// Timeout is zero, too large, or overflows the clock.
    InvalidTimeout,
    /// A canonical child failure byte.
    ChildStage(u8),
    /// The retained child exited before this phase.
    ChildExited(&'static str),
    /// Deadline or finite attempts exhausted.
    Timeout(&'static str),
    /// Profile pipe record was not the exact token.
    NoncanonicalProfileReady,
    /// Gate write did not write the exact token.
    NoncanonicalGateRelease,
    /// Ready length, rights or truncation violated the expected shape.
    MalformedReadyTransfer,
    /// Status was neither a canonical stage nor actual EOF.
    MalformedExecStatus,
}

/// Transport or original-observer refusal; no success evidence on error.
#[derive(Debug, Eq, PartialEq)]
pub enum Error<E> {
    /// Mechanical transport failure.
    Failure(Failure),
    /// Original caller refusal, unchanged.
    Observer(E),
}

impl<E> From<Failure> for Error<E> {
    fn from(value: Failure) -> Self {
        Self::Failure(value)
    }
}

/// Builds one shared deadline, limited to 120 seconds.
pub fn bounded_deadline(timeout: Duration) -> Result<Instant, Failure> {
    deadline_from(Instant::now(), timeout)
}

fn deadline_from(now: Instant, timeout: Duration) -> Result<Instant, Failure> {
    if timeout.is_zero() || timeout > MAX_TIMEOUT {
        return Err(Failure::InvalidTimeout);
    }
    now.checked_add(timeout).ok_or(Failure::InvalidTimeout)
}

/// Waits for the exact profile token while checking child stages and liveness.
pub fn await_profile_ready<O: Observer>(
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

/// Releases the exclusively owned fresh gate using finite interrupted writes.
pub fn release_child<O: Observer>(
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

/// Receives exactly N inert payload bytes and either one or zero descriptor rights.
/// The caller must decode the family record against its actual admitted context.
/// Invalid shapes and all deadline/refusal paths close every received descriptor.
pub fn receive_ready<const N: usize, const RIGHTS: bool, O: Observer>(
    bootstrap: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<([u8; N], Option<OwnedFd>), Error<O::Error>> {
    if !(2..=MAX_READY_BYTES).contains(&N) {
        return Err(Failure::MalformedReadyTransfer.into());
    }
    let (payload, fd) = Scheduler {
        observer,
        io: SystemIo,
        deadline,
    }
    .ready(bootstrap, N, RIGHTS)?;
    let mut bytes = [0; N];
    bytes.copy_from_slice(&payload[..N]);
    Ok((bytes, fd))
}

/// Waits for canonical exec EOF, refusing stages, payloads and ancillary data.
pub fn await_exec_eof<O: Observer>(
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
    fn status(
        &mut self,
        fd: BorrowedFd<'_>,
        bytes: &mut [u8; 2],
        eof: bool,
    ) -> Result<(usize, usize), Errno>;
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
    fn status(
        &mut self,
        fd: BorrowedFd<'_>,
        bytes: &mut [u8; 2],
        eof: bool,
    ) -> Result<(usize, usize), Errno> {
        if eof {
            // Linux supplies SCM_CREDENTIALS for every queued record, including
            // zero-length records. True EOF has none. This setting is confined
            // to the terminal phase of the exclusively owned bootstrap receiver.
            rustix::net::sockopt::set_socket_passcred(fd, true)?;
        }
        Ok(receive_packet(fd)?.status(bytes, eof))
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

// One credential record, or one header plus at most four descriptor integers.
// Two payload-bearing headers cannot fit. Kernel-truncated undisclosed rights
// are closed by recvmsg itself. Credentials carry framing only, never authority.
#[repr(C)]
struct Control {
    length: usize,
    level: i32,
    kind: i32,
    descriptors: [i32; 4],
}
const _: () = assert!(size_of::<Control>() == CONTROL_BYTES);
const _: () = assert!(CONTROL_BYTES <= 2 * size_of::<libc::cmsghdr>());
// Linux UAPI SCM_PIDFD, absent from the pinned libc/rustix ancillary enums.
const SCM_PIDFD: i32 = 0x04;

#[allow(unsafe_code)]
fn receive_packet(fd: BorrowedFd<'_>) -> Result<ReadyPacket, Errno> {
    let mut packet = ReadyPacket::empty();
    let mut control = Control {
        length: 0,
        level: 0,
        kind: 0,
        descriptors: [-1; 4],
    };
    let mut vector = libc::iovec {
        iov_base: packet.payload.as_mut_ptr().cast(),
        iov_len: MAX_READY_BYTES,
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
    let credentials = control.level == libc::SOL_SOCKET && control.kind == libc::SCM_CREDENTIALS;
    if credentials {
        rights.credentials = true;
        if declared != header_bytes + size_of::<libc::ucred>() {
            rights.invalid = true;
        }
    } else if !supported {
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
    credentials: bool,
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
    payload: [u8; MAX_READY_BYTES],
    bytes: usize,
    flags: ReturnFlags,
    rights: Rights,
}
impl ReadyPacket {
    fn empty() -> Self {
        Self {
            payload: [0; MAX_READY_BYTES],
            bytes: 0,
            flags: ReturnFlags::empty(),
            rights: Rights::default(),
        }
    }
    fn status(self, bytes: &mut [u8; 2], eof: bool) -> (usize, usize) {
        if self.rights.invalid
            || self.rights.fd.is_some()
            || self
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || (self.rights.credentials && (!eof || self.bytes == 0))
            || (eof && self.bytes != 0 && !self.rights.credentials)
        {
            return (3, 3);
        }
        bytes.copy_from_slice(&self.payload[..2]);
        (self.bytes.min(2), self.bytes)
    }
    fn validate(
        self,
        bytes: usize,
        rights: bool,
    ) -> Result<([u8; MAX_READY_BYTES], Option<OwnedFd>), Failure> {
        // Dispose all received ownership before rejecting shape or returning a stage.
        if self.rights.invalid
            || self.rights.credentials
            || self
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(Failure::MalformedReadyTransfer);
        }
        if self.bytes == 1 && self.rights.fd.is_none() {
            return Err(Failure::ChildStage(self.payload[0]));
        }
        if self.bytes != bytes || self.rights.fd.is_some() != rights {
            return Err(Failure::MalformedReadyTransfer);
        }
        Ok((self.payload, self.rights.fd))
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
        let result = self.io.status(bootstrap, &mut bytes, false);
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
                        operation: "release measured service child",
                        source,
                    }
                    .into());
                }
            }
        }
        Err(Failure::Timeout("child release gate").into())
    }
    fn ready(
        &mut self,
        bootstrap: BorrowedFd<'_>,
        bytes: usize,
        rights: bool,
    ) -> Result<([u8; MAX_READY_BYTES], Option<OwnedFd>), Error<O::Error>> {
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ReadyTransfer, "service-ready transfer")?;
            let result = self.io.ready(bootstrap);
            self.deadline("service-ready transfer")?;
            match result {
                Ok(packet) => return packet.validate(bytes, rights).map_err(Error::Failure),
                Err(Errno::AGAIN | Errno::INTR) => {
                    if attempt + 1 == MAX_PHASE_ATTEMPTS {
                        break;
                    }
                    self.progress("service-ready transfer")?;
                }
                Err(source) => {
                    return Err(Failure::Io {
                        operation: "receive service-ready transfer",
                        source,
                    }
                    .into());
                }
            }
        }
        Err(Failure::Timeout("service-ready transfer").into())
    }
    fn exec(&mut self, bootstrap: BorrowedFd<'_>) -> Result<(), Error<O::Error>> {
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ExecEof, "daemon exec EOF")?;
            let mut bytes = [0; 2];
            let result = self.io.status(bootstrap, &mut bytes, true);
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
