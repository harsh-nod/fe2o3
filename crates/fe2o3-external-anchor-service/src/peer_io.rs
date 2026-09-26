//! Shared peer syscalls and packet validation; the caller owns accounting and storage.
//!
//! A fallible charge runs before validation and every poll, receive or send attempt,
//! including retries. No ledger is created or reset here. A finite caller work budget
//! bounds attempts, not time blocked in a poll; legacy callers supply a no-op charge.

use std::io::{self, IoSliceMut};
use std::mem::{MaybeUninit, size_of};
use std::os::fd::{AsRawFd, OwnedFd, RawFd};
use std::time::{Duration, Instant};

use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1,
};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::OFlags;
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvFlags, ReturnFlags, SendFlags, SocketType, recvmsg,
    send,
};

use crate::service::ExternalAnchorDaemonErrorV1 as DaemonError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IoStep {
    Validate,
    Poll,
    Receive,
    Send,
}

impl IoStep {
    /// Fixed logical work, not elapsed syscall time or an allocation/stack bound.
    pub(crate) const fn work(self) -> usize {
        match self {
            // Two fcntls, domain/type queries and two fixed sockaddr name queries.
            Self::Validate => 8 + 6 * 1024 + 32 * (2 * size_of::<libc::sockaddr_un>()),
            Self::Poll => 8 + 1024,
            Self::Receive => 8 + 1024 + 32 * ANCHOR_CHALLENGE_WIRE_LEN_V1,
            Self::Send => 8 + 1024 + 32 * ANCHOR_OBSERVATION_WIRE_LEN_V1,
        }
    }
}

pub(crate) fn validate_peer<E: From<DaemonError>>(
    peer: &OwnedFd,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<(), E> {
    charge(IoStep::Validate)?;
    let descriptor_flags = rustix::io::fcntl_getfd(peer)
        .map_err(|source| io_error("inspect external-anchor peer descriptor flags", source))?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(DaemonError::PeerNotCloseOnExec.into());
    }
    let status = rustix::fs::fcntl_getfl(peer)
        .map_err(|source| io_error("inspect external-anchor peer status flags", source))?;
    if status != OFlags::RDWR | OFlags::NONBLOCK {
        return Err(DaemonError::InvalidPeerStatus.into());
    }
    let domain = rustix::net::sockopt::socket_domain(peer)
        .map_err(|source| io_error("inspect external-anchor peer domain", source))?;
    if domain != AddressFamily::UNIX {
        return Err(DaemonError::InvalidPeerDomain.into());
    }
    let socket_type = rustix::net::sockopt::socket_type(peer)
        .map_err(|source| io_error("inspect external-anchor peer socket type", source))?;
    if socket_type != SocketType::SEQPACKET {
        return Err(DaemonError::InvalidPeerSocketType.into());
    }
    require_unnamed(peer.as_raw_fd(), AddressSideV1::Local)?;
    require_unnamed(peer.as_raw_fd(), AddressSideV1::Remote)?;
    Ok(())
}

#[derive(Clone, Copy)]
enum AddressSideV1 {
    Local,
    Remote,
}

fn require_unnamed(peer: RawFd, side: AddressSideV1) -> Result<(), DaemonError> {
    let mut address = MaybeUninit::<libc::sockaddr_un>::zeroed();
    let mut length = libc::socklen_t::try_from(size_of::<libc::sockaddr_un>())
        .expect("sockaddr_un size fits socklen_t");
    // SAFETY: the address buffer is writable for its declared size, `length` is initialized to
    // that size, and `peer` remains owned by the caller throughout this inspection.
    let result = unsafe {
        match side {
            AddressSideV1::Local => libc::getsockname(
                peer,
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
            AddressSideV1::Remote => libc::getpeername(
                peer,
                address.as_mut_ptr().cast::<libc::sockaddr>(),
                &mut length,
            ),
        }
    };
    if result != 0 {
        let source = io::Error::last_os_error();
        return if matches!(side, AddressSideV1::Remote)
            && matches!(source.raw_os_error(), Some(libc::ENOTCONN))
        {
            Err(DaemonError::PeerNotConnected)
        } else {
            Err(DaemonError::Io {
                operation: match side {
                    AddressSideV1::Local => "inspect external-anchor local address",
                    AddressSideV1::Remote => "inspect external-anchor remote address",
                },
                source,
            })
        };
    }
    // SAFETY: a successful name query initialized at least the family field and the buffer was
    // zeroed before the kernel wrote it.
    let address = unsafe { address.assume_init() };
    if i32::from(address.sun_family) != libc::AF_UNIX {
        return Err(DaemonError::InvalidPeerDomain);
    }
    let unnamed_length = std::mem::offset_of!(libc::sockaddr_un, sun_path);
    if usize::try_from(length).ok() == Some(unnamed_length) {
        Ok(())
    } else {
        Err(match side {
            AddressSideV1::Local => DaemonError::NamedLocalAddress,
            AddressSideV1::Remote => DaemonError::NamedRemoteAddress,
        })
    }
}

pub(crate) fn receive_challenge<E: From<DaemonError>>(
    peer: &OwnedFd,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<Option<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]>, E> {
    loop {
        wait_for(peer, PollFlags::IN, None, charge)?;
        charge(IoStep::Receive)?;
        let mut bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        let mut vectors = [IoSliceMut::new(&mut bytes)];
        let mut ancillary = RecvAncillaryBuffer::default();
        match recvmsg(
            peer,
            &mut vectors,
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::TRUNC | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(message) => {
                if message.flags.contains(ReturnFlags::CTRUNC) || ancillary.drain().next().is_some()
                {
                    return Err(DaemonError::AncillaryData.into());
                }
                if message.flags.contains(ReturnFlags::TRUNC)
                    || message.bytes > ANCHOR_CHALLENGE_WIRE_LEN_V1
                {
                    return Err(DaemonError::PacketTruncated.into());
                }
                if message.bytes == 0 {
                    return Ok(None);
                }
                if message.bytes != ANCHOR_CHALLENGE_WIRE_LEN_V1 {
                    return Err(DaemonError::InvalidChallengeLength {
                        actual: message.bytes,
                    }
                    .into());
                }
                return Ok(Some(bytes));
            }
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Err(rustix::io::Errno::CONNRESET | rustix::io::Errno::NOTCONN) => return Ok(None),
            Err(source) => {
                return Err(io_error("receive external-anchor challenge", source).into());
            }
        }
    }
}

pub(crate) fn send_observation<E: From<DaemonError>>(
    peer: &OwnedFd,
    observation: &[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
    timeout: Duration,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<(), E> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(DaemonError::DeadlineOverflow)?;
    loop {
        wait_for(peer, PollFlags::OUT, Some(deadline), charge)?;
        charge(IoStep::Send)?;
        match send(peer, observation, SendFlags::DONTWAIT | SendFlags::NOSIGNAL) {
            Ok(count) if count == observation.len() => return Ok(()),
            Ok(_) => return Err(DaemonError::PartialSend.into()),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Err(
                rustix::io::Errno::PIPE | rustix::io::Errno::CONNRESET | rustix::io::Errno::NOTCONN,
            ) => {
                return Err(DaemonError::PeerClosed.into());
            }
            Err(source) => return Err(io_error("send external-anchor observation", source).into()),
        }
    }
}

fn wait_for<E: From<DaemonError>>(
    peer: &OwnedFd,
    wanted: PollFlags,
    deadline: Option<Instant>,
    charge: &mut impl FnMut(IoStep) -> Result<(), E>,
) -> Result<(), E> {
    loop {
        charge(IoStep::Poll)?;
        let timeout = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(DaemonError::ResponseTimeout.into());
                }
                Some(
                    rustix::event::Timespec::try_from(remaining)
                        .map_err(|_| DaemonError::DeadlineOverflow)?,
                )
            }
            None => None,
        };
        let mut descriptors = [PollFd::new(
            peer,
            wanted | PollFlags::ERR | PollFlags::HUP | PollFlags::RDHUP,
        )];
        match poll(&mut descriptors, timeout.as_ref()) {
            Ok(0) => return Err(DaemonError::ResponseTimeout.into()),
            Ok(_) => {
                let ready = descriptors[0].revents();
                if ready.contains(PollFlags::NVAL) {
                    return Err(DaemonError::InvalidPeer.into());
                }
                if ready.contains(PollFlags::ERR) {
                    return Err(DaemonError::PeerFailed.into());
                }
                if ready.contains(wanted) {
                    return Ok(());
                }
                if ready.intersects(PollFlags::HUP | PollFlags::RDHUP) {
                    if wanted == PollFlags::IN {
                        return Ok(());
                    }
                    return Err(DaemonError::PeerClosed.into());
                }
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => return Err(io_error("poll external-anchor peer", source).into()),
        }
    }
}

fn io_error(operation: &'static str, source: rustix::io::Errno) -> DaemonError {
    DaemonError::Io {
        operation,
        source: io::Error::from(source),
    }
}
