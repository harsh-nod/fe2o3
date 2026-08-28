//! Exact child-to-parent return channel for one inert compiler receipt carriage.

use std::error::Error;
use std::fmt;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1,
};
use rustix::net::{RecvAncillaryBuffer, RecvFlags, ReturnFlags, recvmsg};

use crate::child_channel::{
    child_create_and_transfer, duration_to_poll_millis, open_pidfd, peer_identity,
    receive_transferred_peer, require_close_on_exec, require_pidfd_live,
    require_reserved_descriptor_unused, seqpacket_pair, wait_for_transfer,
};
use crate::{
    CompilerExecutionChildChannelErrorV1, CompilerExecutionClientProcessIdentityV1,
    validate_seqpacket_peer,
};

const RECEIPT_TRANSFER_MAGIC: [u8; 8] = *b"FE2CER1\0";

/// Fixed rustc descriptor reserved for returning one compiler receipt carriage.
pub const COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1: RawFd = 196;

/// Prepared one-use receiver for a return endpoint created inside the rustc child.
///
/// The child-created socket makes `SO_PEERCRED` name the exact rustc process. This
/// value carries no policy, signing, compiler, publication, load, or launch authority.
pub struct PendingCompilerExecutionReceiptReturnV1 {
    receiver: OwnedFd,
}

impl fmt::Debug for PendingCompilerExecutionReceiptReturnV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PendingCompilerExecutionReceiptReturnV1")
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl PendingCompilerExecutionReceiptReturnV1 {
    /// Registers one child-only return endpoint on an unspawned rustc command.
    pub fn prepare(command: &mut Command) -> Result<Self, CompilerExecutionReceiptReturnErrorV1> {
        require_reserved_descriptor_unused(COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1)?;
        let (receiver, sender) = seqpacket_pair()?;
        // SAFETY: the callback performs only async-signal-safe descriptor and socket syscalls.
        unsafe {
            command.pre_exec(move || {
                child_create_and_transfer(
                    sender.as_raw_fd(),
                    COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
                    RECEIPT_TRANSFER_MAGIC,
                )
            });
        }
        Ok(Self { receiver })
    }

    /// Admits the exact return peer created by one still-live spawned rustc child.
    pub fn finish(
        self,
        child_pid: u32,
        timeout: Duration,
    ) -> Result<CompilerExecutionReceiptReceiverV1, CompilerExecutionReceiptReturnErrorV1> {
        if child_pid == 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidChildPid);
        }
        if timeout.is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidTimeout);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(CompilerExecutionReceiptReturnErrorV1::DeadlineOverflow)?;
        let client_pidfd = open_pidfd(child_pid)?;
        wait_for_transfer(&self.receiver, &client_pidfd, deadline)?;
        let (peer, transferred_pid, transferred_parent_pid) = receive_transferred_peer(
            &self.receiver,
            COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1,
            RECEIPT_TRANSFER_MAGIC,
        )?;
        if transferred_pid != child_pid {
            return Err(CompilerExecutionReceiptReturnErrorV1::ChildPidMismatch);
        }
        if transferred_parent_pid != std::process::id() {
            return Err(CompilerExecutionReceiptReturnErrorV1::ParentPidMismatch);
        }
        validate_seqpacket_peer(&peer)
            .map_err(|_| CompilerExecutionReceiptReturnErrorV1::InvalidPeer)?;
        require_close_on_exec(&peer)?;
        require_close_on_exec(&client_pidfd)?;
        require_pidfd_live(&client_pidfd)?;
        let client = peer_identity(&peer)?;
        if client.pid() != child_pid {
            return Err(CompilerExecutionReceiptReturnErrorV1::PeerCredentialsMismatch);
        }
        let receiver = Self::admit_receiver(peer, client_pidfd, client);
        receiver.revalidate_peer()?;
        Ok(receiver)
    }

    fn admit_receiver(
        peer: OwnedFd,
        client_pidfd: OwnedFd,
        client: CompilerExecutionClientProcessIdentityV1,
    ) -> CompilerExecutionReceiptReceiverV1 {
        CompilerExecutionReceiptReceiverV1 {
            peer,
            client_pidfd,
            client,
        }
    }
}

/// Parent owner of one exact rustc child's inert receipt-return endpoint.
///
/// ```compile_fail
/// fn require_clone<T: Clone>() {}
/// require_clone::<fe2o3_compiler_execution_client::CompilerExecutionReceiptReceiverV1>();
/// ```
pub struct CompilerExecutionReceiptReceiverV1 {
    peer: OwnedFd,
    client_pidfd: OwnedFd,
    client: CompilerExecutionClientProcessIdentityV1,
}

impl fmt::Debug for CompilerExecutionReceiptReceiverV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionReceiptReceiverV1")
            .field("authority", &"none")
            .field("client", &self.client)
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionReceiptReceiverV1 {
    /// Returns the exact rustc identity admitted before receipt delivery.
    pub const fn client(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.client
    }

    /// Receives and validates one complete carriage against protected caller expectations.
    ///
    /// A packet already queued by the admitted child remains eligible if the child exits before
    /// the parent is scheduled. Child exit without a complete packet fails closed.
    pub fn receive_exact(
        self,
        expected_policy: &CompilerExecutionIssuerPolicyV1,
        expected_subject: &InertCompilerExecutionSubjectV1,
        timeout: Duration,
    ) -> Result<CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptReturnErrorV1> {
        if timeout.is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidTimeout);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(CompilerExecutionReceiptReturnErrorV1::DeadlineOverflow)?;
        wait_for_receipt(&self.peer, &self.client_pidfd, deadline)?;
        let carriage = receive_carriage(&self.peer)?;
        if carriage.policy() != expected_policy {
            return Err(CompilerExecutionReceiptReturnErrorV1::PolicyMismatch);
        }
        if carriage.request().subject() != expected_subject {
            return Err(CompilerExecutionReceiptReturnErrorV1::SubjectMismatch);
        }
        require_clean_eof(&self.peer, &self.client_pidfd, deadline)?;
        self.revalidate_peer()?;
        Ok(carriage)
    }

    fn revalidate_peer(&self) -> Result<(), CompilerExecutionReceiptReturnErrorV1> {
        validate_seqpacket_peer(&self.peer)
            .map_err(|_| CompilerExecutionReceiptReturnErrorV1::InvalidPeer)?;
        require_close_on_exec(&self.peer)?;
        require_close_on_exec(&self.client_pidfd)?;
        if peer_identity(&self.peer)? != self.client {
            return Err(CompilerExecutionReceiptReturnErrorV1::PeerCredentialsMismatch);
        }
        Ok(())
    }
}

/// Child owner of the fixed receipt-return endpoint.
pub struct CompilerExecutionReceiptSenderV1 {
    peer: OwnedFd,
}

impl fmt::Debug for CompilerExecutionReceiptSenderV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionReceiptSenderV1")
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionReceiptSenderV1 {
    /// Admits and removes the inherited fixed child descriptor.
    pub fn from_inherited_child() -> Result<Self, CompilerExecutionReceiptReturnErrorV1> {
        let peer = take_inherited(COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1)?;
        validate_seqpacket_peer(&peer)
            .map_err(|_| CompilerExecutionReceiptReturnErrorV1::InvalidPeer)?;
        require_close_on_exec(&peer)?;
        Ok(Self { peer })
    }

    /// Sends exactly one canonical carriage after repeating its policy and subject join.
    pub fn send_exact(
        self,
        expected_policy: &CompilerExecutionIssuerPolicyV1,
        expected_subject: &InertCompilerExecutionSubjectV1,
        carriage: CompilerExecutionReceiptCarriageV1,
        timeout: Duration,
    ) -> Result<(), CompilerExecutionReceiptReturnErrorV1> {
        if timeout.is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidTimeout);
        }
        let decoded = CompilerExecutionReceiptCarriageV1::decode(carriage.canonical_bytes())
            .map_err(|error| {
                CompilerExecutionReceiptReturnErrorV1::InvalidCarriage(error.to_string())
            })?;
        if decoded != carriage {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidCarriage(
                "canonical carriage changed during local revalidation".to_owned(),
            ));
        }
        if carriage.policy() != expected_policy {
            return Err(CompilerExecutionReceiptReturnErrorV1::PolicyMismatch);
        }
        if carriage.request().subject() != expected_subject {
            return Err(CompilerExecutionReceiptReturnErrorV1::SubjectMismatch);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(CompilerExecutionReceiptReturnErrorV1::DeadlineOverflow)?;
        send_carriage(&self.peer, carriage.canonical_bytes(), deadline)?;
        // SAFETY: shutdown consumes only the live socket descriptor and a scalar direction.
        if unsafe { libc::shutdown(self.peer.as_raw_fd(), libc::SHUT_WR) } != 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::Io(
                io::Error::last_os_error(),
            ));
        }
        Ok(())
    }
}

fn take_inherited(descriptor: RawFd) -> Result<OwnedFd, CompilerExecutionReceiptReturnErrorV1> {
    // SAFETY: F_GETFD consumes only the scalar descriptor.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(CompilerExecutionReceiptReturnErrorV1::MissingInheritedDescriptor);
    }
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(CompilerExecutionReceiptReturnErrorV1::UnexpectedCloseOnExec);
    }
    // SAFETY: F_DUPFD_CLOEXEC returns one independently owned descriptor.
    let retained = unsafe { libc::fcntl(descriptor, libc::F_DUPFD_CLOEXEC, 3) };
    if retained < 0 {
        return Err(CompilerExecutionReceiptReturnErrorV1::Io(
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: the inherited fixed descriptor is consumed exactly once by this operation.
    if unsafe { libc::close(descriptor) } != 0 {
        // SAFETY: successful duplication returned one owned descriptor not yet wrapped.
        unsafe { libc::close(retained) };
        return Err(CompilerExecutionReceiptReturnErrorV1::Io(
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: successful F_DUPFD_CLOEXEC returned one newly owned descriptor.
    Ok(unsafe { OwnedFd::from_raw_fd(retained) })
}

fn wait_for_receipt(
    peer: &OwnedFd,
    client_pidfd: &OwnedFd,
    deadline: Instant,
) -> Result<(), CompilerExecutionReceiptReturnErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::Timeout);
        }
        let mut descriptors = [
            libc::pollfd {
                fd: peer.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: client_pidfd.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
        ];
        // SAFETY: descriptors is a live two-element pollfd array.
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
            return Err(CompilerExecutionReceiptReturnErrorV1::Io(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::Timeout);
        }
        if descriptors
            .iter()
            .any(|descriptor| descriptor.revents & libc::POLLNVAL != 0)
        {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidPeer);
        }
        if descriptors[0].revents & libc::POLLIN != 0 {
            return Ok(());
        }
        if descriptors[0].revents & libc::POLLERR != 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::PeerFailed);
        }
        if descriptors[0].revents & libc::POLLHUP != 0 || descriptors[1].revents != 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::ChildExitedWithoutReceipt);
        }
    }
}

fn receive_carriage(
    peer: &OwnedFd,
) -> Result<CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptReturnErrorV1> {
    let mut payload = [0_u8; COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1];
    let mut vectors = [IoSliceMut::new(&mut payload)];
    let mut control = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut control);
    let received = recvmsg(
        peer,
        &mut vectors,
        &mut ancillary,
        RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
    )
    .map_err(|error| CompilerExecutionReceiptReturnErrorV1::Io(error.into()))?;
    if received.flags.contains(ReturnFlags::TRUNC) {
        return Err(CompilerExecutionReceiptReturnErrorV1::TruncatedPacket);
    }
    if received.flags.contains(ReturnFlags::CTRUNC) {
        return Err(CompilerExecutionReceiptReturnErrorV1::AncillaryData);
    }
    if received.bytes != payload.len() {
        if received.bytes == 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::ChildExitedWithoutReceipt);
        }
        return Err(CompilerExecutionReceiptReturnErrorV1::WrongPacketLength {
            actual: received.bytes,
        });
    }
    if ancillary.drain().next().is_some() {
        return Err(CompilerExecutionReceiptReturnErrorV1::AncillaryData);
    }
    CompilerExecutionReceiptCarriageV1::decode(&payload)
        .map_err(|error| CompilerExecutionReceiptReturnErrorV1::InvalidCarriage(error.to_string()))
}

fn require_clean_eof(
    peer: &OwnedFd,
    client_pidfd: &OwnedFd,
    deadline: Instant,
) -> Result<(), CompilerExecutionReceiptReturnErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::Timeout);
        }
        let mut descriptors = [
            libc::pollfd {
                fd: peer.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: client_pidfd.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
        ];
        // SAFETY: descriptors is a live two-element pollfd array.
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
            return Err(CompilerExecutionReceiptReturnErrorV1::Io(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::Timeout);
        }
        if descriptors
            .iter()
            .any(|descriptor| descriptor.revents & libc::POLLNVAL != 0)
        {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidPeer);
        }
        if descriptors[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            let mut payload = [0_u8; 1];
            let mut vectors = [IoSliceMut::new(&mut payload)];
            let mut control = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut ancillary = RecvAncillaryBuffer::new(&mut control);
            let received = match recvmsg(
                peer,
                &mut vectors,
                &mut ancillary,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(received) => received,
                Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {
                    continue;
                }
                Err(error) => {
                    return Err(CompilerExecutionReceiptReturnErrorV1::Io(error.into()));
                }
            };
            if received.bytes != 0
                || received
                    .flags
                    .intersects(ReturnFlags::EOR | ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                || ancillary.drain().next().is_some()
            {
                return Err(CompilerExecutionReceiptReturnErrorV1::TrailingPacket);
            }
            return Ok(());
        }
        if descriptors[0].revents & libc::POLLERR != 0 || descriptors[1].revents != 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::PeerFailed);
        }
    }
}

fn send_carriage(
    peer: &OwnedFd,
    bytes: &[u8; COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1],
    deadline: Instant,
) -> Result<(), CompilerExecutionReceiptReturnErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::Timeout);
        }
        let mut descriptor = libc::pollfd {
            fd: peer.as_raw_fd(),
            events: libc::POLLOUT | libc::POLLERR | libc::POLLHUP,
            revents: 0,
        };
        // SAFETY: descriptor is a live one-element pollfd array.
        let result = unsafe { libc::poll(&mut descriptor, 1, duration_to_poll_millis(remaining)) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(CompilerExecutionReceiptReturnErrorV1::Io(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(CompilerExecutionReceiptReturnErrorV1::Timeout);
        }
        if descriptor.revents & libc::POLLNVAL != 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::InvalidPeer);
        }
        if descriptor.revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::PeerFailed);
        }
        if descriptor.revents & libc::POLLOUT == 0 {
            continue;
        }
        // SAFETY: bytes names an immutable fixed-size packet and peer remains owned.
        let sent = unsafe {
            libc::send(
                peer.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if sent == bytes.len() as isize {
            return Ok(());
        }
        if sent >= 0 {
            return Err(CompilerExecutionReceiptReturnErrorV1::PartialSend);
        }
        let error = io::Error::last_os_error();
        if matches!(error.raw_os_error(), Some(libc::EINTR) | Some(libc::EAGAIN)) {
            continue;
        }
        return Err(CompilerExecutionReceiptReturnErrorV1::Io(error));
    }
}

/// Stable failure preparing, admitting, or using the inert receipt-return channel.
#[derive(Debug)]
pub enum CompilerExecutionReceiptReturnErrorV1 {
    InvalidChildPid,
    InvalidTimeout,
    DeadlineOverflow,
    ChildChannel(CompilerExecutionChildChannelErrorV1),
    ChildPidMismatch,
    ParentPidMismatch,
    InvalidPeer,
    PeerCredentialsMismatch,
    MissingInheritedDescriptor,
    UnexpectedCloseOnExec,
    Timeout,
    PeerFailed,
    ChildExitedWithoutReceipt,
    WrongPacketLength { actual: usize },
    TruncatedPacket,
    AncillaryData,
    TrailingPacket,
    PartialSend,
    InvalidCarriage(String),
    PolicyMismatch,
    SubjectMismatch,
    Io(io::Error),
}

impl fmt::Display for CompilerExecutionReceiptReturnErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChildPid => formatter.write_str("rustc child PID must be nonzero"),
            Self::InvalidTimeout => formatter.write_str("receipt return timeout must be nonzero"),
            Self::DeadlineOverflow => formatter.write_str("receipt return deadline overflowed"),
            Self::ChildChannel(error) => write!(formatter, "receipt return setup failed: {error}"),
            Self::ChildPidMismatch => formatter.write_str("receipt return names another child PID"),
            Self::ParentPidMismatch => {
                formatter.write_str("receipt return names another direct parent")
            }
            Self::InvalidPeer => formatter.write_str("receipt return peer is invalid"),
            Self::PeerCredentialsMismatch => {
                formatter.write_str("receipt return peer names another process")
            }
            Self::MissingInheritedDescriptor => formatter
                .write_str("rustc child has no inherited compiler receipt-return descriptor"),
            Self::UnexpectedCloseOnExec => {
                formatter.write_str("inherited compiler receipt-return descriptor is close-on-exec")
            }
            Self::Timeout => formatter.write_str("receipt return absolute deadline expired"),
            Self::PeerFailed => formatter.write_str("receipt return peer failed"),
            Self::ChildExitedWithoutReceipt => {
                formatter.write_str("rustc child exited without a complete compiler receipt")
            }
            Self::WrongPacketLength { actual } => write!(
                formatter,
                "compiler receipt-return packet has length {actual}, expected {COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V1}"
            ),
            Self::TruncatedPacket => {
                formatter.write_str("compiler receipt-return packet truncated")
            }
            Self::AncillaryData => {
                formatter.write_str("compiler receipt-return packet carried ancillary data")
            }
            Self::TrailingPacket => {
                formatter.write_str("compiler receipt-return channel contained a second packet")
            }
            Self::PartialSend => formatter.write_str("compiler receipt-return send was partial"),
            Self::InvalidCarriage(error) => {
                write!(formatter, "invalid compiler receipt carriage: {error}")
            }
            Self::PolicyMismatch => {
                formatter.write_str("compiler receipt carriage names another issuer policy")
            }
            Self::SubjectMismatch => {
                formatter.write_str("compiler receipt carriage names another compiler subject")
            }
            Self::Io(error) => write!(formatter, "compiler receipt return failed: {error}"),
        }
    }
}

impl Error for CompilerExecutionReceiptReturnErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ChildChannel(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerExecutionChildChannelErrorV1> for CompilerExecutionReceiptReturnErrorV1 {
    fn from(error: CompilerExecutionChildChannelErrorV1) -> Self {
        Self::ChildChannel(error)
    }
}
