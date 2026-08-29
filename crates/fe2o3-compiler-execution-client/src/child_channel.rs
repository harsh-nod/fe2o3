//! Exact post-fork rustc service-channel construction and parent handoff.

use std::error::Error;
use std::fmt;
use std::io::{self, IoSliceMut};
use std::mem;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::ptr;
use std::time::{Duration, Instant};

use fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1;
use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg};

use crate::{COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, validate_seqpacket_peer};

const TRANSFER_MAGIC: [u8; 8] = *b"FE2CEC2\0";
const TRANSFER_VERSION: u32 = 2;
const TRANSFER_BYTES: usize = 24;

/// Move-only service launch inputs bound to one still-live rustc child.
///
/// The value grants no issuer, signing, compilation, publication, loading, or execution
/// authority. A protected supervisor must transfer both descriptors and this exact identity to
/// the distinct-UID issuer, which repeats its own admission.
///
/// ```compile_fail
/// fn require_clone<T: Clone>() {}
/// require_clone::<fe2o3_compiler_execution_client::CompilerExecutionServiceLaunchV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::CompilerExecutionServiceLaunchV1;
/// fn decompose(launch: CompilerExecutionServiceLaunchV1) {
///     let _ = launch.into_descriptors();
/// }
/// ```
pub struct CompilerExecutionServiceLaunchV1 {
    service_peer: OwnedFd,
    client_pidfd: OwnedFd,
    client: CompilerExecutionClientProcessIdentityV1,
    submitter: CompilerExecutionClientProcessIdentityV1,
}

impl fmt::Debug for CompilerExecutionServiceLaunchV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionServiceLaunchV1")
            .field("authority", &"none")
            .field("client", &self.client)
            .field("submitter", &self.submitter)
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionServiceLaunchV1 {
    /// Returns the exact rustc process identity associated with both retained descriptors.
    pub const fn client(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.client
    }

    /// Returns the exact direct-parent identity that received this child-created channel.
    pub const fn submitter(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.submitter
    }

    /// Consumes the inert launch inputs for one protected-supervisor transfer.
    pub(crate) fn into_descriptors(self) -> (OwnedFd, OwnedFd) {
        (self.service_peer, self.client_pidfd)
    }

    /// Decomposes launch inputs for cross-crate adversarial protocol tests only.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    pub fn into_test_descriptors(self) -> (OwnedFd, OwnedFd) {
        (self.service_peer, self.client_pidfd)
    }

    pub(crate) fn revalidate_for_supervisor_handoff(
        &self,
    ) -> Result<(), CompilerExecutionChildChannelErrorV1> {
        if self.submitter.pid() != std::process::id()
            || self.submitter.uid() != rustix::process::geteuid().as_raw()
            || self.submitter.gid() != rustix::process::getegid().as_raw()
        {
            return Err(CompilerExecutionChildChannelErrorV1::ParentCredentialsMismatch);
        }
        validate_seqpacket_peer(&self.service_peer)
            .map_err(|_| CompilerExecutionChildChannelErrorV1::InvalidServicePeer)?;
        require_close_on_exec(&self.service_peer)?;
        require_close_on_exec(&self.client_pidfd)?;
        if peer_identity(&self.service_peer)? != self.client {
            return Err(CompilerExecutionChildChannelErrorV1::PeerCredentialsMismatch);
        }
        require_service_peer_live(&self.service_peer)?;
        require_pidfd_live(&self.client_pidfd)
    }
}

/// Prepared one-use parent receiver for a socketpair created by the rustc child itself.
///
/// Preparation registers one async-signal-safe `pre_exec` callback. The command is one-use after
/// preparation: a second spawn cannot produce another admitted handoff after this receiver is
/// consumed. The pending value reserves FD 195 with the exact private control socket until the
/// child has crossed `fork`, preventing an unrelated concurrent descriptor allocation from
/// occupying the fixed target. The command retains the control sender until it is dropped; all
/// waits are therefore bounded by the caller-supplied absolute deadline rather than EOF.
pub struct PendingCompilerExecutionChildChannelV1 {
    receiver: OwnedFd,
    _reserved_child_fd: OwnedFd,
}

impl fmt::Debug for PendingCompilerExecutionChildChannelV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingCompilerExecutionChildChannelV1")
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl PendingCompilerExecutionChildChannelV1 {
    /// Registers exact child-side channel creation on one rustc command.
    pub fn prepare(command: &mut Command) -> Result<Self, CompilerExecutionChildChannelErrorV1> {
        require_reserved_descriptor_unused()?;
        let (mut receiver, mut sender) = seqpacket_pair()?;
        if receiver.as_raw_fd() == COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 {
            mem::swap(&mut receiver, &mut sender);
        }
        let reserved_child_fd = if sender.as_raw_fd() == COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 {
            let control = rustix::io::fcntl_dupfd_cloexec(&sender, 0).map_err(|error| {
                CompilerExecutionChildChannelErrorV1::Descriptor(io::Error::from(error))
            })?;
            let reserved = sender;
            sender = control;
            reserved
        } else {
            let reserved =
                rustix::io::fcntl_dupfd_cloexec(&sender, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1)
                    .map_err(|error| {
                        CompilerExecutionChildChannelErrorV1::Descriptor(io::Error::from(error))
                    })?;
            if reserved.as_raw_fd() != COMPILER_EXECUTION_SERVICE_CHILD_FD_V1 {
                return Err(CompilerExecutionChildChannelErrorV1::ReservedDescriptorInUse);
            }
            reserved
        };
        // The command owns this descriptor through every spawn. The callback creates the actual
        // service channel after fork, so SO_PEERCRED records the rustc child's PID rather than its
        // parent wrapper. Every operation below is an async-signal-safe Linux descriptor syscall.
        unsafe {
            command.pre_exec(move || child_create_and_transfer(sender.as_raw_fd()));
        }
        Ok(Self {
            receiver,
            _reserved_child_fd: reserved_child_fd,
        })
    }

    /// Receives and validates the exact service endpoint for one still-live spawned rustc PID.
    pub fn finish(
        self,
        child_pid: u32,
        timeout: Duration,
    ) -> Result<CompilerExecutionServiceLaunchV1, CompilerExecutionChildChannelErrorV1> {
        if child_pid == 0 {
            return Err(CompilerExecutionChildChannelErrorV1::InvalidChildPid);
        }
        if timeout.is_zero() {
            return Err(CompilerExecutionChildChannelErrorV1::InvalidTimeout);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(CompilerExecutionChildChannelErrorV1::DeadlineOverflow)?;
        self.finish_until(child_pid, deadline)
    }

    /// Receives and validates the exact service endpoint before one absolute deadline.
    pub fn finish_until(
        self,
        child_pid: u32,
        deadline: Instant,
    ) -> Result<CompilerExecutionServiceLaunchV1, CompilerExecutionChildChannelErrorV1> {
        if child_pid == 0 {
            return Err(CompilerExecutionChildChannelErrorV1::InvalidChildPid);
        }
        require_child_channel_deadline(deadline)?;
        let client_pidfd = open_pidfd(child_pid)?;
        wait_for_transfer(&self.receiver, &client_pidfd, deadline)?;
        let (service_peer, transferred_pid, transferred_parent_pid) =
            receive_service_peer(&self.receiver)?;
        if transferred_pid != child_pid {
            return Err(CompilerExecutionChildChannelErrorV1::ChildPidMismatch);
        }
        if transferred_parent_pid != std::process::id() {
            return Err(CompilerExecutionChildChannelErrorV1::ParentPidMismatch);
        }
        validate_seqpacket_peer(&service_peer)
            .map_err(|_| CompilerExecutionChildChannelErrorV1::InvalidServicePeer)?;
        require_close_on_exec(&service_peer)?;
        let client = peer_identity(&service_peer)?;
        if client.pid() != child_pid {
            return Err(CompilerExecutionChildChannelErrorV1::PeerCredentialsMismatch);
        }
        require_service_peer_live(&service_peer)?;
        require_pidfd_live(&client_pidfd)?;
        let submitter = CompilerExecutionClientProcessIdentityV1::new(
            transferred_parent_pid,
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .map_err(|_| CompilerExecutionChildChannelErrorV1::ParentPidMismatch)?;
        if submitter.uid() != client.uid() || submitter.gid() != client.gid() {
            return Err(CompilerExecutionChildChannelErrorV1::ParentCredentialsMismatch);
        }
        require_child_channel_deadline(deadline)?;
        Ok(CompilerExecutionServiceLaunchV1 {
            service_peer,
            client_pidfd,
            client,
            submitter,
        })
    }
}

fn require_child_channel_deadline(
    deadline: Instant,
) -> Result<(), CompilerExecutionChildChannelErrorV1> {
    if deadline.saturating_duration_since(Instant::now()).is_zero() {
        Err(CompilerExecutionChildChannelErrorV1::Timeout)
    } else {
        Ok(())
    }
}

fn require_reserved_descriptor_unused() -> Result<(), CompilerExecutionChildChannelErrorV1> {
    // SAFETY: F_GETFD uses only the scalar descriptor and reports absence through EBADF.
    let result = unsafe { libc::fcntl(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, libc::F_GETFD) };
    if result >= 0 {
        return Err(CompilerExecutionChildChannelErrorV1::ReservedDescriptorInUse);
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() != Some(libc::EBADF) {
        return Err(CompilerExecutionChildChannelErrorV1::Descriptor(error));
    }
    Ok(())
}

fn seqpacket_pair() -> Result<(OwnedFd, OwnedFd), CompilerExecutionChildChannelErrorV1> {
    let mut descriptors = [-1_i32; 2];
    // SAFETY: successful socketpair initializes both output descriptor slots.
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            descriptors.as_mut_ptr(),
        )
    } != 0
    {
        return Err(CompilerExecutionChildChannelErrorV1::Descriptor(
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: socketpair returned two independently owned descriptors.
    Ok(unsafe {
        (
            OwnedFd::from_raw_fd(descriptors[0]),
            OwnedFd::from_raw_fd(descriptors[1]),
        )
    })
}

fn child_create_and_transfer(control: RawFd) -> io::Result<()> {
    require_exact_child_reservation(control)?;

    let mut peers = [-1_i32; 2];
    // SAFETY: successful socketpair initializes both child-local output slots.
    if unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            peers.as_mut_ptr(),
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    let (service, client) = (peers[0], peers[1]);

    let result = (|| {
        // SAFETY: dup3 atomically replaces the exact validated reservation with the live endpoint.
        if unsafe { libc::dup3(client, COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, 0) }
            != COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
        {
            return Err(io::Error::last_os_error());
        }
        send_service_peer(control, service)
    })();

    // SAFETY: these are child-local descriptors returned by socketpair. The validated reservation
    // prevents either new endpoint from occupying FD 195 before dup3 replaces it.
    unsafe {
        libc::close(service);
        libc::close(client);
        if result.is_err() {
            libc::close(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1);
        }
    }
    result
}

fn require_exact_child_reservation(control: RawFd) -> io::Result<()> {
    // The reservation must remain close-on-exec until this callback replaces it. A later callback
    // cannot substitute an inherited object without changing the exact socket identity below.
    // SAFETY: F_GETFD consumes only the scalar descriptor value.
    let flags = unsafe { libc::fcntl(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, libc::F_GETFD) };
    if flags < 0 || flags & libc::FD_CLOEXEC == 0 {
        return Err(io::Error::from_raw_os_error(libc::EBUSY));
    }

    // SAFETY: zero is a valid initial representation and fstat initializes each output on success.
    let mut expected = unsafe { mem::zeroed::<libc::stat>() };
    // SAFETY: see above; control is retained by the command callback.
    if unsafe { libc::fstat(control, &mut expected) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: see above; FD 195 was validated as open immediately before this call.
    let mut actual = unsafe { mem::zeroed::<libc::stat>() };
    if unsafe { libc::fstat(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, &mut actual) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if actual.st_dev != expected.st_dev
        || actual.st_ino != expected.st_ino
        || actual.st_mode != expected.st_mode
        || actual.st_rdev != expected.st_rdev
    {
        return Err(io::Error::from_raw_os_error(libc::EBUSY));
    }
    Ok(())
}

fn send_service_peer(control: RawFd, service: RawFd) -> io::Result<()> {
    let pid = u32::try_from(unsafe { libc::getpid() })
        .map_err(|_| io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    let parent_pid = u32::try_from(unsafe { libc::getppid() })
        .map_err(|_| io::Error::from_raw_os_error(libc::EOVERFLOW))?;
    if parent_pid == 0 {
        return Err(io::Error::from_raw_os_error(libc::ESRCH));
    }
    let mut payload = [0_u8; TRANSFER_BYTES];
    payload[..8].copy_from_slice(&TRANSFER_MAGIC);
    payload[8..12].copy_from_slice(&TRANSFER_VERSION.to_le_bytes());
    payload[12..16].copy_from_slice(&pid.to_le_bytes());
    payload[16..20].copy_from_slice(&COMPILER_EXECUTION_SERVICE_CHILD_FD_V1.to_le_bytes());
    payload[20..24].copy_from_slice(&parent_pid.to_le_bytes());

    let mut vector = libc::iovec {
        iov_base: payload.as_mut_ptr().cast(),
        iov_len: payload.len(),
    };
    let mut control_storage = [0_usize; 8];
    // SAFETY: zero is a valid initial msghdr representation.
    let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
    header.msg_iov = &mut vector;
    header.msg_iovlen = 1;
    header.msg_control = control_storage.as_mut_ptr().cast();
    header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
    // SAFETY: the aligned storage is large enough for one SCM_RIGHTS descriptor.
    unsafe {
        let message = libc::CMSG_FIRSTHDR(&header);
        (*message).cmsg_level = libc::SOL_SOCKET;
        (*message).cmsg_type = libc::SCM_RIGHTS;
        (*message).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize;
        ptr::write_unaligned(libc::CMSG_DATA(message).cast::<RawFd>(), service);
    }
    // SAFETY: every msghdr pointer names live stack storage for the complete call.
    let sent = unsafe { libc::sendmsg(control, &header, libc::MSG_NOSIGNAL) };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    if usize::try_from(sent).ok() != Some(payload.len()) {
        return Err(io::Error::from_raw_os_error(libc::EIO));
    }
    Ok(())
}

fn open_pidfd(child_pid: u32) -> Result<OwnedFd, CompilerExecutionChildChannelErrorV1> {
    // SAFETY: pidfd_open consumes one positive scalar PID and zero flags.
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, child_pid, 0) };
    if descriptor < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Err(CompilerExecutionChildChannelErrorV1::ChildExited);
        }
        return Err(CompilerExecutionChildChannelErrorV1::Pidfd(error));
    }
    let descriptor = i32::try_from(descriptor).map_err(|_| {
        CompilerExecutionChildChannelErrorV1::Pidfd(io::Error::from_raw_os_error(libc::EOVERFLOW))
    })?;
    // SAFETY: successful pidfd_open returned one newly owned descriptor.
    let pidfd = unsafe { OwnedFd::from_raw_fd(descriptor) };
    require_close_on_exec(&pidfd)?;
    Ok(pidfd)
}

fn wait_for_transfer(
    receiver: &OwnedFd,
    pidfd: &OwnedFd,
    deadline: Instant,
) -> Result<(), CompilerExecutionChildChannelErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CompilerExecutionChildChannelErrorV1::Timeout);
        }
        let mut descriptors = [
            libc::pollfd {
                fd: receiver.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: pidfd.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
        ];
        // SAFETY: descriptors names a live two-element pollfd array.
        let result = unsafe {
            libc::poll(
                descriptors.as_mut_ptr(),
                descriptors.len() as libc::nfds_t,
                duration_to_poll_millis(remaining),
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(CompilerExecutionChildChannelErrorV1::Poll(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(CompilerExecutionChildChannelErrorV1::Timeout);
        }
        if descriptors
            .iter()
            .any(|descriptor| descriptor.revents & libc::POLLNVAL != 0)
        {
            return Err(CompilerExecutionChildChannelErrorV1::InvalidDescriptor);
        }
        if descriptors[1].revents != 0 {
            return Err(CompilerExecutionChildChannelErrorV1::ChildExited);
        }
        if descriptors[0].revents & libc::POLLIN != 0 {
            return Ok(());
        }
        if descriptors[0].revents & libc::POLLERR != 0 {
            return Err(CompilerExecutionChildChannelErrorV1::ControlFailed);
        }
        if descriptors[0].revents & libc::POLLHUP != 0 {
            return Err(CompilerExecutionChildChannelErrorV1::ControlClosed);
        }
    }
}

fn receive_service_peer(
    receiver: &OwnedFd,
) -> Result<(OwnedFd, u32, u32), CompilerExecutionChildChannelErrorV1> {
    let mut payload = [0_u8; TRANSFER_BYTES];
    let mut vectors = [IoSliceMut::new(&mut payload)];
    let mut control = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut control);
    let received = recvmsg(
        receiver,
        &mut vectors,
        &mut ancillary,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    )
    .map_err(|error| CompilerExecutionChildChannelErrorV1::Receive(io::Error::from(error)))?;
    let malformed_header = received.bytes != payload.len()
        || received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC);
    let mut descriptor = None;
    let mut malformed_ancillary = false;
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(descriptors) => {
                for received in descriptors {
                    if descriptor.is_some() {
                        malformed_ancillary = true;
                    } else {
                        descriptor = Some(received);
                    }
                }
            }
            _ => malformed_ancillary = true,
        }
    }
    if malformed_header || malformed_ancillary {
        return Err(CompilerExecutionChildChannelErrorV1::MalformedTransfer);
    }
    let descriptor = descriptor.ok_or(CompilerExecutionChildChannelErrorV1::MalformedTransfer)?;
    if payload[..8] != TRANSFER_MAGIC
        || u32::from_le_bytes(payload[8..12].try_into().unwrap()) != TRANSFER_VERSION
        || i32::from_le_bytes(payload[16..20].try_into().unwrap())
            != COMPILER_EXECUTION_SERVICE_CHILD_FD_V1
    {
        return Err(CompilerExecutionChildChannelErrorV1::MalformedTransfer);
    }
    let child_pid = u32::from_le_bytes(payload[12..16].try_into().unwrap());
    if child_pid == 0 {
        return Err(CompilerExecutionChildChannelErrorV1::MalformedTransfer);
    }
    let parent_pid = u32::from_le_bytes(payload[20..24].try_into().unwrap());
    if parent_pid == 0 || parent_pid == child_pid {
        return Err(CompilerExecutionChildChannelErrorV1::MalformedTransfer);
    }

    Ok((descriptor, child_pid, parent_pid))
}

fn peer_identity(
    service_peer: &OwnedFd,
) -> Result<CompilerExecutionClientProcessIdentityV1, CompilerExecutionChildChannelErrorV1> {
    let credentials = rustix::net::sockopt::socket_peercred(service_peer).map_err(|error| {
        CompilerExecutionChildChannelErrorV1::PeerCredentials(io::Error::from(error))
    })?;
    let pid = u32::try_from(credentials.pid.as_raw_nonzero().get())
        .map_err(|_| CompilerExecutionChildChannelErrorV1::PeerCredentialsMismatch)?;
    CompilerExecutionClientProcessIdentityV1::new(
        pid,
        credentials.uid.as_raw(),
        credentials.gid.as_raw(),
    )
    .map_err(|_| CompilerExecutionChildChannelErrorV1::PeerCredentialsMismatch)
}

fn require_close_on_exec(descriptor: &OwnedFd) -> Result<(), CompilerExecutionChildChannelErrorV1> {
    let flags = rustix::io::fcntl_getfd(descriptor).map_err(|error| {
        CompilerExecutionChildChannelErrorV1::Descriptor(io::Error::from(error))
    })?;
    if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(CompilerExecutionChildChannelErrorV1::MissingCloseOnExec);
    }
    Ok(())
}

fn require_pidfd_live(pidfd: &OwnedFd) -> Result<(), CompilerExecutionChildChannelErrorV1> {
    require_not_poll_ready(pidfd, CompilerExecutionChildChannelErrorV1::ChildExited)
}

fn require_service_peer_live(
    service_peer: &OwnedFd,
) -> Result<(), CompilerExecutionChildChannelErrorV1> {
    require_not_poll_ready(
        service_peer,
        CompilerExecutionChildChannelErrorV1::ServicePeerClosed,
    )
}

fn require_not_poll_ready(
    descriptor: &OwnedFd,
    ready_error: CompilerExecutionChildChannelErrorV1,
) -> Result<(), CompilerExecutionChildChannelErrorV1> {
    let mut descriptor = libc::pollfd {
        fd: descriptor.as_raw_fd(),
        events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
        revents: 0,
    };
    // SAFETY: descriptor is a live one-element pollfd array for the complete nonblocking call.
    let result = unsafe { libc::poll(&mut descriptor, 1, 0) };
    if result < 0 {
        return Err(CompilerExecutionChildChannelErrorV1::Poll(
            io::Error::last_os_error(),
        ));
    }
    if result != 0 {
        return Err(ready_error);
    }
    Ok(())
}

fn duration_to_poll_millis(duration: Duration) -> i32 {
    let millis = duration.as_millis();
    let rounded = if duration.subsec_nanos().is_multiple_of(1_000_000) {
        millis
    } else {
        millis.saturating_add(1)
    };
    rounded.clamp(1, i32::MAX as u128) as i32
}

/// Stable failure for exact child-channel construction and transfer.
#[derive(Debug)]
pub enum CompilerExecutionChildChannelErrorV1 {
    InvalidChildPid,
    InvalidTimeout,
    DeadlineOverflow,
    ReservedDescriptorInUse,
    Descriptor(io::Error),
    Pidfd(io::Error),
    Poll(io::Error),
    Receive(io::Error),
    Timeout,
    InvalidDescriptor,
    ChildExited,
    ControlFailed,
    ControlClosed,
    MalformedTransfer,
    ChildPidMismatch,
    ParentPidMismatch,
    ParentCredentialsMismatch,
    InvalidServicePeer,
    ServicePeerClosed,
    PeerCredentials(io::Error),
    PeerCredentialsMismatch,
    MissingCloseOnExec,
}

impl fmt::Display for CompilerExecutionChildChannelErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChildPid => formatter.write_str("rustc child PID must be nonzero"),
            Self::InvalidTimeout => formatter.write_str("rustc channel timeout must be nonzero"),
            Self::DeadlineOverflow => formatter.write_str("rustc channel deadline overflowed"),
            Self::ReservedDescriptorInUse => write!(
                formatter,
                "reserved rustc compiler-service descriptor {COMPILER_EXECUTION_SERVICE_CHILD_FD_V1} is already in use"
            ),
            Self::Descriptor(error) => {
                write!(formatter, "rustc channel descriptor failed: {error}")
            }
            Self::Pidfd(error) => write!(formatter, "rustc child pidfd failed: {error}"),
            Self::Poll(error) => write!(formatter, "rustc channel poll failed: {error}"),
            Self::Receive(error) => write!(formatter, "rustc channel receive failed: {error}"),
            Self::Timeout => formatter.write_str("rustc channel absolute deadline expired"),
            Self::InvalidDescriptor => {
                formatter.write_str("rustc channel descriptor became invalid")
            }
            Self::ChildExited => formatter.write_str("rustc child exited before channel admission"),
            Self::ControlFailed => formatter.write_str("rustc channel control peer failed"),
            Self::ControlClosed => formatter.write_str("rustc channel control peer closed"),
            Self::MalformedTransfer => formatter.write_str("rustc channel transfer is malformed"),
            Self::ChildPidMismatch => {
                formatter.write_str("rustc channel transfer names another PID")
            }
            Self::ParentPidMismatch => {
                formatter.write_str("rustc channel transfer names another direct parent")
            }
            Self::ParentCredentialsMismatch => formatter
                .write_str("rustc child credentials differ from its direct-parent credentials"),
            Self::InvalidServicePeer => {
                formatter.write_str("rustc service endpoint has the wrong socket shape")
            }
            Self::ServicePeerClosed => {
                formatter.write_str("rustc service endpoint lost its client before admission")
            }
            Self::PeerCredentials(error) => {
                write!(formatter, "rustc service peer credentials failed: {error}")
            }
            Self::PeerCredentialsMismatch => {
                formatter.write_str("rustc service peer credentials name another process")
            }
            Self::MissingCloseOnExec => {
                formatter.write_str("retained rustc channel descriptor lacks close-on-exec")
            }
        }
    }
}

impl Error for CompilerExecutionChildChannelErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Descriptor(error)
            | Self::Pidfd(error)
            | Self::Poll(error)
            | Self::Receive(error)
            | Self::PeerCredentials(error) => Some(error),
            _ => None,
        }
    }
}
