//! Shared readiness mechanics, not child, deployment or endpoint admission.
//!
//! The caller retains the child and channels, prepays ATTEMPT_SCRATCH (or
//! PIPE_ATTEMPT_SCRATCH for readiness pipes) throughout each phase, and reserves
//! returned ready/descriptor storage before retaining it. Profile reads require
//! a nonblocking pipe; bootstrap I/O uses DONTWAIT.
//! Gate writes require the exclusively owned fresh release pipe. Logical attempt
//! limits and deadline checks do not bound a blocking syscall's duration.
//!
//! Unlike V1's old loops, deadlines are checked even on successful I/O, every
//! retry is finite, and a truncated one-byte packet cannot masquerade as a stage.
//! Terminal status enables SO_PASSCRED before receive. Linux marks even queued
//! empty records with SCM_CREDENTIALS; EOF carries no record. These credentials
//! distinguish framing only. Credential-bound readiness additionally compares the
//! actual packet sender with the owning coordinator's child and deployment IDs;
//! it does not admit those IDs, an image, or a deployment. Failure to enable the
//! option refuses, without a hangup-only or unmarked-record fallback.

use crate::{PROTECTED_SERVICE_GATE_RELEASE_V1, PROTECTED_SERVICE_PROFILE_READY_V1};
use rustix::io::Errno;
use rustix::net::ReturnFlags;
use std::mem::size_of;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

#[path = "launch_io_pipe.rs"]
mod pipe;
pub use pipe::{ExactPipeFrame, PipeFrameError};

/// Finite primary attempts in each readiness phase.
pub const MAX_PHASE_ATTEMPTS: usize = 120_001;
/// Finite interrupted writes to the exclusive release gate.
pub const MAX_GATE_ATTEMPTS: usize = 64;
const MAX_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_millis(1);
/// Largest inert readiness payload accepted by this fixed-size transport.
pub const MAX_READY_BYTES: usize = 88;
/// Largest inert exact pipe frame; independent of the SEQPACKET limit.
pub const MAX_PIPE_READY_BYTES: usize = 120;
/// Per pipe attempt: fstat, F_GETFL, and one nonblocking read, without retries.
pub const MAX_PIPE_ATTEMPT_SYSCALLS: usize = 3;
/// Separate original-ledger liveness allowance, not included in MAX_LIVENESS_CHECKS.
pub const MAX_PIPE_LIVENESS_CHECKS: usize = MAX_PHASE_ATTEMPTS - 1;
/// Transport and pause syscalls; excludes clock and separately metered liveness.
pub const MAX_PIPE_SYSCALLS: usize =
    MAX_PHASE_ATTEMPTS * MAX_PIPE_ATTEMPT_SYSCALLS + MAX_PIPE_LIVENESS_CHECKS;
/// Complete pipe-phase mechanical work, not included in MAX_WORK. Excludes
/// nested liveness, deadline construction, caller validation and retained storage.
pub const MAX_PIPE_WORK: usize = MAX_PHASE_ATTEMPTS * Boundary::ReadyPipe.work()
    + MAX_PIPE_LIVENESS_CHECKS * Boundary::Progress.work();
/// Prepaid logical pipe frame/stat/result scratch; not RSS or a stack bound.
/// Observer state/errors and nested liveness require separate caller charges.
pub const PIPE_ATTEMPT_SCRATCH: usize = 8192;
#[allow(unsafe_code)]
// SAFETY: the fixed payload size fits c_uint and CMSG alignment cannot overflow.
// Unlike rustix's unaligned byte-buffer allowance, this is the exact libc ABI size.
const CONTROL_BYTES: usize = unsafe {
    libc::CMSG_SPACE(size_of::<libc::ucred>() as u32) as usize
        + libc::CMSG_SPACE(size_of::<i32>() as u32) as usize
};

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

/// Separate liveness allowance for one send phase, not included in MAX_LIVENESS_CHECKS.
/// Observer::is_live has its own original-ledger quota.
pub const MAX_SEND_LIVENESS_CHECKS: usize = MAX_PHASE_ATTEMPTS - 1;

/// Complete mechanical quota for one send phase, not included in MAX_WORK.
/// Excludes nested liveness, caller validation, deadline construction and storage.
pub const MAX_SEND_WORK: usize = MAX_PHASE_ATTEMPTS * Boundary::ReadySend.work()
    + MAX_SEND_LIVENESS_CHECKS * Boundary::Progress.work();

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
    /// Send one bounded payload without descriptor rights.
    ReadySend,
    /// Validate the nonblocking pipe read end, then read frame bytes or exact EOF.
    ReadyPipe,
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
                2 + CONTROL_BYTES / size_of::<i32>(),
                MAX_READY_BYTES + CONTROL_BYTES,
            ),
            Self::ReadySend => (1, MAX_READY_BYTES),
            Self::ReadyPipe => (
                MAX_PIPE_ATTEMPT_SYSCALLS,
                MAX_PIPE_READY_BYTES + 1 + size_of::<rustix::fs::Stat>(),
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
    /// The caller keeps ATTEMPT_SCRATCH (PIPE_ATTEMPT_SCRATCH for the pipe phase)
    /// prepaid until the phase returns/unwinds. ReadyPipe's exact mechanical quote
    /// is 8 + 3 * (1024 + 64) + (121 + size_of::<rustix::fs::Stat>()) * 64 + 256.
    fn before_attempt(&mut self, boundary: Boundary) -> Result<(), Self::Error>;
    /// Native adapters invoke the real retained child's metered observation.
    fn is_live(&mut self) -> Result<bool, Self::Error>;
}

/// Inert expected message credentials, not authority to admit or signal a child.
/// The native caller derives these from its retained child and admitted profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageSender {
    pid: i32,
    uid: u32,
    gid: u32,
}
impl MessageSender {
    /// Supplies comparison values only. Each receive requires SCM_CREDENTIALS.
    pub const fn new(pid: i32, uid: u32, gid: u32) -> Self {
        Self { pid, uid, gid }
    }
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
    /// The retained child exited, or its credential-bound bootstrap reached EOF
    /// before readiness. Bootstrap EOF alone proves neither exit nor reaping.
    ChildExited(&'static str),
    /// Deadline or finite attempts exhausted.
    Timeout(&'static str),
    /// Profile pipe record was not the exact token.
    NoncanonicalProfileReady,
    /// Gate write did not write the exact token.
    NoncanonicalGateRelease,
    /// Ready length, rights, truncation or short send violated the expected shape.
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

/// Sends exactly N inert bytes in one nonblocking, SIGPIPE-suppressed packet,
/// without descriptor rights. Requires 0 < N <= MAX_READY_BYTES and a caller-owned
/// SEQPACKET endpoint; this transport does not admit its peer or payload.
/// Keep ATTEMPT_SCRATCH prepaid and budget MAX_SEND_WORK plus separately metered
/// liveness for at most MAX_SEND_LIVENESS_CHECKS observations on the original ledger.
/// A deadline refusal after a successful syscall can follow delivery; do not replay
/// the record on error. No receipt or positive authority is returned.
pub fn send_ready<const N: usize, O: Observer>(
    bootstrap: BorrowedFd<'_>,
    payload: &[u8; N],
    observer: &mut O,
    deadline: Instant,
) -> Result<(), Error<O::Error>> {
    Scheduler {
        observer,
        io: SystemIo,
        deadline,
    }
    .send(bootstrap, payload)
}

/// Receives exactly N inert pipe bytes followed by EOF, with 0 < N <= 120.
/// Requires a private pipe read end whose aliases cannot change status flags
/// during this call. Each attempt validates FIFO, RDONLY and NONBLOCK before
/// reading; the transport does not admit the payload, pipe provenance or child.
/// Keep PIPE_ATTEMPT_SCRATCH prepaid and budget MAX_PIPE_WORK plus separately
/// metered MAX_PIPE_LIVENESS_CHECKS observations on the original account.
/// Partial bytes, errors and late success never return readiness evidence.
pub fn receive_ready_pipe<const N: usize, O: Observer>(
    reader: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<[u8; N], Error<O::Error>> {
    Scheduler {
        observer,
        io: SystemIo,
        deadline,
    }
    .pipe(reader)
}

/// Receives exactly N inert payload bytes and either one or zero descriptor rights.
/// The caller must decode the family record against its actual admitted context.
/// Invalid shapes and all deadline/refusal paths close every received descriptor.
pub fn receive_ready<const N: usize, const RIGHTS: bool, O: Observer>(
    bootstrap: BorrowedFd<'_>,
    observer: &mut O,
    deadline: Instant,
) -> Result<([u8; N], Option<OwnedFd>), Error<O::Error>> {
    receive_ready_inner::<N, RIGHTS, O>(bootstrap, None, observer, deadline)
}

/// Receives readiness only from the exact per-message PID/UID/GID. SO_PASSCRED
/// must already be enabled before the child can send. Ancillary credentials are
/// required even for failure stages; SO_PEERCRED's socket creator is not a writer.
/// Payload decoding and image/profile/child custody remain caller obligations.
pub fn receive_ready_from<const N: usize, const RIGHTS: bool, O: Observer>(
    bootstrap: BorrowedFd<'_>,
    sender: MessageSender,
    observer: &mut O,
    deadline: Instant,
) -> Result<([u8; N], Option<OwnedFd>), Error<O::Error>> {
    receive_ready_inner::<N, RIGHTS, O>(bootstrap, Some(sender), observer, deadline)
}

fn receive_ready_inner<const N: usize, const RIGHTS: bool, O: Observer>(
    bootstrap: BorrowedFd<'_>,
    sender: Option<MessageSender>,
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
    .ready_from(bootstrap, N, RIGHTS, sender)?;
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
    fn send(&mut self, fd: BorrowedFd<'_>, payload: &[u8]) -> Result<usize, Errno>;
    fn status(
        &mut self,
        fd: BorrowedFd<'_>,
        bytes: &mut [u8; 2],
        eof: bool,
    ) -> Result<(usize, usize), Errno>;
    fn ready(&mut self, fd: BorrowedFd<'_>, credentials: bool) -> Result<ReadyPacket, Errno>;
    fn pipe(&mut self, fd: BorrowedFd<'_>, bytes: &mut [u8]) -> Result<usize, Errno> {
        pipe::read_nonblocking(fd, bytes)
    }
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
    fn send(&mut self, fd: BorrowedFd<'_>, payload: &[u8]) -> Result<usize, Errno> {
        rustix::net::send(
            fd,
            payload,
            rustix::net::SendFlags::DONTWAIT | rustix::net::SendFlags::NOSIGNAL,
        )
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
    fn ready(&mut self, fd: BorrowedFd<'_>, credentials: bool) -> Result<ReadyPacket, Errno> {
        if credentials && !rustix::net::sockopt::socket_passcred(fd)? {
            return Err(Errno::INVAL);
        }
        receive_packet(fd)
    }
    fn pause(&mut self, duration: Duration) -> Result<(), Errno> {
        let timeout = rustix::event::Timespec::try_from(duration).map_err(|_| Errno::INVAL)?;
        // A single poll, unlike std::thread::sleep's internal interruption loop.
        rustix::event::poll(&mut [], Some(&timeout)).map(|_| ())
    }
}

// Exactly enough for credentials plus one right (alignment may fit a second
// right, which is owned and rejected). recvmsg closes undisclosed truncated FDs.
#[repr(C)]
struct Control {
    words: [usize; CONTROL_BYTES / size_of::<usize>()],
}
const _: () = assert!(size_of::<Control>() == CONTROL_BYTES);
// Linux UAPI SCM_PIDFD, absent from the pinned libc/rustix ancillary enums.
const SCM_PIDFD: i32 = 0x04;

#[allow(unsafe_code)]
fn receive_packet(fd: BorrowedFd<'_>) -> Result<ReadyPacket, Errno> {
    let mut packet = ReadyPacket::empty();
    let mut control = Control {
        words: [0; CONTROL_BYTES / size_of::<usize>()],
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
    let header_bytes = size_of::<libc::cmsghdr>();
    if length > CONTROL_BYTES {
        rights.invalid = true;
    }
    let length = length.min(CONTROL_BYTES);
    let base = (control as *const Control).cast::<u8>();
    let mut offset = 0;
    while offset < length {
        if length - offset < header_bytes {
            rights.invalid = true;
            break;
        }
        // SAFETY: the checked remaining range contains this complete initialized
        // header. All reads are unaligned, independent of ancillary padding.
        let header = unsafe { base.add(offset).cast::<libc::cmsghdr>().read_unaligned() };
        let declared = header.cmsg_len as usize;
        let remaining = length - offset;
        if declared < header_bytes || declared > remaining {
            rights.invalid = true;
        }
        let payload = declared.min(remaining).saturating_sub(header_bytes);
        // SAFETY: offset + header_bytes is within the initialized control buffer.
        let data = unsafe { base.add(offset + header_bytes) };
        match (header.cmsg_level, header.cmsg_type) {
            (libc::SOL_SOCKET, libc::SCM_CREDENTIALS) => {
                if payload != size_of::<libc::ucred>() || rights.credentials.is_some() {
                    rights.invalid = true;
                } else {
                    // SAFETY: payload has the exact checked ucred size.
                    let cred = unsafe { data.cast::<libc::ucred>().read_unaligned() };
                    rights.credentials = Some(MessageSender::new(cred.pid, cred.uid, cred.gid));
                }
            }
            (libc::SOL_SOCKET, kind @ (libc::SCM_RIGHTS | SCM_PIDFD)) => {
                if kind != libc::SCM_RIGHTS || payload == 0 || payload % size_of::<i32>() != 0 {
                    rights.invalid = true;
                }
                for index in 0..payload / size_of::<i32>() {
                    // SAFETY: each complete integer is within the checked payload.
                    let raw = unsafe { data.cast::<i32>().add(index).read_unaligned() };
                    if raw < 0 {
                        rights.invalid = true;
                    } else {
                        // SAFETY: recvmsg installed each disclosed FD exactly once.
                        rights.push(unsafe { OwnedFd::from_raw_fd(raw) });
                    }
                }
            }
            _ => rights.invalid = true,
        }
        if declared < header_bytes || declared > remaining {
            break;
        }
        let aligned = declared.next_multiple_of(size_of::<usize>());
        if aligned > remaining {
            if declared != remaining {
                rights.invalid = true;
            }
            break;
        }
        offset += aligned;
    }
}

#[derive(Default)]
struct Rights {
    fd: Option<OwnedFd>,
    invalid: bool,
    credentials: Option<MessageSender>,
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
            || (self.rights.credentials.is_some() && self.bytes == 0)
            || (eof && self.bytes != 0 && self.rights.credentials.is_none())
        {
            return (3, 3);
        }
        bytes.copy_from_slice(&self.payload[..2]);
        (self.bytes.min(2), self.bytes)
    }
    #[cfg(test)]
    fn validate(
        self,
        bytes: usize,
        rights: bool,
    ) -> Result<([u8; MAX_READY_BYTES], Option<OwnedFd>), Failure> {
        self.validate_from(bytes, rights, None)
    }
    fn validate_from(
        self,
        bytes: usize,
        rights: bool,
        sender: Option<MessageSender>,
    ) -> Result<([u8; MAX_READY_BYTES], Option<OwnedFd>), Failure> {
        // Dispose all received ownership before rejecting shape or returning a stage.
        if self.rights.invalid
            || self
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(Failure::MalformedReadyTransfer);
        }
        // Only credential-bound receives require SO_PASSCRED before I/O. There,
        // even an empty queued record has credentials; clean EOF has none.
        if sender.is_some()
            && self.bytes == 0
            && self.rights.credentials.is_none()
            && self.rights.fd.is_none()
        {
            return Err(Failure::ChildExited(
                "bootstrap EOF before service-ready transfer",
            ));
        }
        if self.rights.credentials != sender {
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
    fn send<const N: usize>(
        &mut self,
        bootstrap: BorrowedFd<'_>,
        payload: &[u8; N],
    ) -> Result<(), Error<O::Error>> {
        if !(1..=MAX_READY_BYTES).contains(&N) {
            return Err(Failure::MalformedReadyTransfer.into());
        }
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ReadySend, "service-ready send")?;
            let result = self.io.send(bootstrap, payload);
            self.deadline("service-ready send")?;
            match result {
                Ok(bytes) if bytes == N => return Ok(()),
                Ok(_) => return Err(Failure::MalformedReadyTransfer.into()),
                Err(Errno::AGAIN | Errno::INTR) => {
                    if attempt + 1 == MAX_PHASE_ATTEMPTS {
                        break;
                    }
                    self.progress("service-ready send")?;
                }
                Err(source) => {
                    return Err(Failure::Io {
                        operation: "send service-ready record",
                        source,
                    }
                    .into());
                }
            }
        }
        Err(Failure::Timeout("service-ready send").into())
    }
    #[cfg(test)]
    fn ready(
        &mut self,
        bootstrap: BorrowedFd<'_>,
        bytes: usize,
        rights: bool,
    ) -> Result<([u8; MAX_READY_BYTES], Option<OwnedFd>), Error<O::Error>> {
        self.ready_from(bootstrap, bytes, rights, None)
    }
    fn ready_from(
        &mut self,
        bootstrap: BorrowedFd<'_>,
        bytes: usize,
        rights: bool,
        sender: Option<MessageSender>,
    ) -> Result<([u8; MAX_READY_BYTES], Option<OwnedFd>), Error<O::Error>> {
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ReadyTransfer, "service-ready transfer")?;
            let result = self.io.ready(bootstrap, sender.is_some());
            self.deadline("service-ready transfer")?;
            match result {
                Ok(packet) => {
                    return packet
                        .validate_from(bytes, rights, sender)
                        .map_err(Error::Failure);
                }
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

#[cfg(test)]
#[path = "launch_io_credential_tests.rs"]
mod credential_tests;
