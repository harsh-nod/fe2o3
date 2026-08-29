//! Exact bounded transport for one connected external-anchor peer.

use std::error::Error;
use std::fmt;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::fd::{AsRawFd, RawFd};
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

use crate::{DurableExternalAnchorV1, ExternalAnchorServiceErrorV1};

/// Maximum time allowed to publish one already-computed observation to the protected peer.
pub const EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1: Duration = Duration::from_secs(30);

/// Terminal report after the sole connected peer closes cleanly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorServiceReportV1 {
    exchanges: u64,
}

impl ExternalAnchorServiceReportV1 {
    pub const fn exchanges(self) -> u64 {
        self.exchanges
    }
}

/// Serves the sole connected protected peer until that peer closes.
///
/// The endpoint must be an unnamed, connected, nonblocking Unix `SOCK_SEQPACKET` with
/// `FD_CLOEXEC`. Each packet must contain exactly one canonical challenge and no ancillary data.
/// Invalid input terminates the service without a response. A successful response is sent only
/// after [`DurableExternalAnchorV1::exchange`] has completed its durable transition.
pub fn serve_connected_peer_v1(
    anchor: &mut DurableExternalAnchorV1,
    peer: OwnedFd,
) -> Result<ExternalAnchorServiceReportV1, ExternalAnchorDaemonErrorV1> {
    validate_peer(&peer)?;
    serve_connected_peer_with_timeout(anchor, peer, EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1)
}

fn serve_connected_peer_with_timeout(
    anchor: &mut DurableExternalAnchorV1,
    peer: OwnedFd,
    response_timeout: Duration,
) -> Result<ExternalAnchorServiceReportV1, ExternalAnchorDaemonErrorV1> {
    if response_timeout.is_zero() {
        return Err(ExternalAnchorDaemonErrorV1::InvalidResponseTimeout);
    }
    let mut exchanges = 0_u64;
    loop {
        let Some(challenge) = receive_challenge(&peer)? else {
            return Ok(ExternalAnchorServiceReportV1 { exchanges });
        };
        let observation = anchor.exchange(&challenge)?;
        send_observation(&peer, &observation, response_timeout)?;
        exchanges = exchanges
            .checked_add(1)
            .ok_or(ExternalAnchorDaemonErrorV1::ExchangeCountOverflow)?;
    }
}

fn validate_peer(peer: &OwnedFd) -> Result<(), ExternalAnchorDaemonErrorV1> {
    let descriptor_flags = rustix::io::fcntl_getfd(peer)
        .map_err(|source| io_error("inspect external-anchor peer descriptor flags", source))?;
    if !descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC) {
        return Err(ExternalAnchorDaemonErrorV1::PeerNotCloseOnExec);
    }
    let status = rustix::fs::fcntl_getfl(peer)
        .map_err(|source| io_error("inspect external-anchor peer status flags", source))?;
    if status != OFlags::RDWR | OFlags::NONBLOCK {
        return Err(ExternalAnchorDaemonErrorV1::InvalidPeerStatus);
    }
    let domain = rustix::net::sockopt::socket_domain(peer)
        .map_err(|source| io_error("inspect external-anchor peer domain", source))?;
    if domain != AddressFamily::UNIX {
        return Err(ExternalAnchorDaemonErrorV1::InvalidPeerDomain);
    }
    let socket_type = rustix::net::sockopt::socket_type(peer)
        .map_err(|source| io_error("inspect external-anchor peer socket type", source))?;
    if socket_type != SocketType::SEQPACKET {
        return Err(ExternalAnchorDaemonErrorV1::InvalidPeerSocketType);
    }
    require_unnamed(peer.as_raw_fd(), AddressSideV1::Local)?;
    require_unnamed(peer.as_raw_fd(), AddressSideV1::Remote)
}

#[derive(Clone, Copy)]
enum AddressSideV1 {
    Local,
    Remote,
}

fn require_unnamed(peer: RawFd, side: AddressSideV1) -> Result<(), ExternalAnchorDaemonErrorV1> {
    let mut address = MaybeUninit::<libc::sockaddr_un>::zeroed();
    let mut length = libc::socklen_t::try_from(std::mem::size_of::<libc::sockaddr_un>())
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
            Err(ExternalAnchorDaemonErrorV1::PeerNotConnected)
        } else {
            Err(ExternalAnchorDaemonErrorV1::Io {
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
        return Err(ExternalAnchorDaemonErrorV1::InvalidPeerDomain);
    }
    let unnamed_length = std::mem::offset_of!(libc::sockaddr_un, sun_path);
    if usize::try_from(length).ok() == Some(unnamed_length) {
        Ok(())
    } else {
        Err(match side {
            AddressSideV1::Local => ExternalAnchorDaemonErrorV1::NamedLocalAddress,
            AddressSideV1::Remote => ExternalAnchorDaemonErrorV1::NamedRemoteAddress,
        })
    }
}

fn receive_challenge(
    peer: &OwnedFd,
) -> Result<Option<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]>, ExternalAnchorDaemonErrorV1> {
    let mut bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
    loop {
        wait_for(peer, PollFlags::IN, None)?;
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
                    return Err(ExternalAnchorDaemonErrorV1::AncillaryData);
                }
                if message.flags.contains(ReturnFlags::TRUNC)
                    || message.bytes > ANCHOR_CHALLENGE_WIRE_LEN_V1
                {
                    return Err(ExternalAnchorDaemonErrorV1::PacketTruncated);
                }
                if message.bytes == 0 {
                    return Ok(None);
                }
                if message.bytes != ANCHOR_CHALLENGE_WIRE_LEN_V1 {
                    return Err(ExternalAnchorDaemonErrorV1::InvalidChallengeLength {
                        actual: message.bytes,
                    });
                }
                return Ok(Some(bytes));
            }
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Err(rustix::io::Errno::CONNRESET | rustix::io::Errno::NOTCONN) => return Ok(None),
            Err(source) => {
                return Err(io_error("receive external-anchor challenge", source));
            }
        }
    }
}

fn send_observation(
    peer: &OwnedFd,
    observation: &[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
    timeout: Duration,
) -> Result<(), ExternalAnchorDaemonErrorV1> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(ExternalAnchorDaemonErrorV1::DeadlineOverflow)?;
    loop {
        wait_for(peer, PollFlags::OUT, Some(deadline))?;
        match send(peer, observation, SendFlags::DONTWAIT | SendFlags::NOSIGNAL) {
            Ok(count) if count == observation.len() => return Ok(()),
            Ok(_) => return Err(ExternalAnchorDaemonErrorV1::PartialSend),
            Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => {}
            Err(
                rustix::io::Errno::PIPE | rustix::io::Errno::CONNRESET | rustix::io::Errno::NOTCONN,
            ) => {
                return Err(ExternalAnchorDaemonErrorV1::PeerClosed);
            }
            Err(source) => return Err(io_error("send external-anchor observation", source)),
        }
    }
}

fn wait_for(
    peer: &OwnedFd,
    wanted: PollFlags,
    deadline: Option<Instant>,
) -> Result<(), ExternalAnchorDaemonErrorV1> {
    loop {
        let timeout = match deadline {
            Some(deadline) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(ExternalAnchorDaemonErrorV1::ResponseTimeout);
                }
                Some(
                    rustix::event::Timespec::try_from(remaining)
                        .map_err(|_| ExternalAnchorDaemonErrorV1::DeadlineOverflow)?,
                )
            }
            None => None,
        };
        let mut descriptors = [PollFd::new(
            peer,
            wanted | PollFlags::ERR | PollFlags::HUP | PollFlags::RDHUP,
        )];
        match poll(&mut descriptors, timeout.as_ref()) {
            Ok(0) => return Err(ExternalAnchorDaemonErrorV1::ResponseTimeout),
            Ok(_) => {
                let ready = descriptors[0].revents();
                if ready.contains(PollFlags::NVAL) {
                    return Err(ExternalAnchorDaemonErrorV1::InvalidPeer);
                }
                if ready.contains(PollFlags::ERR) {
                    return Err(ExternalAnchorDaemonErrorV1::PeerFailed);
                }
                if ready.contains(wanted) {
                    return Ok(());
                }
                if ready.intersects(PollFlags::HUP | PollFlags::RDHUP) {
                    if wanted == PollFlags::IN {
                        return Ok(());
                    }
                    return Err(ExternalAnchorDaemonErrorV1::PeerClosed);
                }
            }
            Err(rustix::io::Errno::INTR) => {}
            Err(source) => return Err(io_error("poll external-anchor peer", source)),
        }
    }
}

fn io_error(operation: &'static str, source: rustix::io::Errno) -> ExternalAnchorDaemonErrorV1 {
    ExternalAnchorDaemonErrorV1::Io {
        operation,
        source: io::Error::from(source),
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum ExternalAnchorDaemonErrorV1 {
    Anchor(ExternalAnchorServiceErrorV1),
    PeerNotCloseOnExec,
    InvalidPeerStatus,
    InvalidPeerDomain,
    InvalidPeerSocketType,
    NamedLocalAddress,
    NamedRemoteAddress,
    PeerNotConnected,
    InvalidPeer,
    PeerFailed,
    PeerClosed,
    PacketTruncated,
    AncillaryData,
    InvalidChallengeLength {
        actual: usize,
    },
    PartialSend,
    InvalidResponseTimeout,
    DeadlineOverflow,
    ResponseTimeout,
    ExchangeCountOverflow,
    Io {
        operation: &'static str,
        source: io::Error,
    },
}

impl fmt::Display for ExternalAnchorDaemonErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Anchor(error) => write!(formatter, "external-anchor transition failed: {error}"),
            Self::PeerNotCloseOnExec => {
                formatter.write_str("external-anchor peer is not close-on-exec")
            }
            Self::InvalidPeerStatus => formatter
                .write_str("external-anchor peer is not an exact nonblocking read-write endpoint"),
            Self::InvalidPeerDomain => {
                formatter.write_str("external-anchor peer is not a Unix socket")
            }
            Self::InvalidPeerSocketType => {
                formatter.write_str("external-anchor peer is not SOCK_SEQPACKET")
            }
            Self::NamedLocalAddress => {
                formatter.write_str("external-anchor peer local address is named")
            }
            Self::NamedRemoteAddress => {
                formatter.write_str("external-anchor peer remote address is named")
            }
            Self::PeerNotConnected => formatter.write_str("external-anchor peer is not connected"),
            Self::InvalidPeer => formatter.write_str("external-anchor peer descriptor is invalid"),
            Self::PeerFailed => formatter.write_str("external-anchor peer reported an error"),
            Self::PeerClosed => formatter.write_str("external-anchor peer closed before response"),
            Self::PacketTruncated => {
                formatter.write_str("external-anchor challenge exceeded the fixed packet bound")
            }
            Self::AncillaryData => {
                formatter.write_str("external-anchor challenge carried ancillary data")
            }
            Self::InvalidChallengeLength { actual } => write!(
                formatter,
                "external-anchor challenge length must be {ANCHOR_CHALLENGE_WIRE_LEN_V1}, got {actual}"
            ),
            Self::PartialSend => {
                formatter.write_str("external-anchor observation send was not atomic")
            }
            Self::InvalidResponseTimeout => {
                formatter.write_str("external-anchor response timeout is zero")
            }
            Self::DeadlineOverflow => {
                formatter.write_str("external-anchor response deadline overflowed")
            }
            Self::ResponseTimeout => {
                formatter.write_str("external-anchor response publication timed out")
            }
            Self::ExchangeCountOverflow => {
                formatter.write_str("external-anchor exchange count overflowed")
            }
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ExternalAnchorDaemonErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Anchor(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<ExternalAnchorServiceErrorV1> for ExternalAnchorDaemonErrorV1 {
    fn from(error: ExternalAnchorServiceErrorV1) -> Self {
        Self::Anchor(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::fd::AsFd;
    use std::os::unix::fs::PermissionsExt;
    use std::thread;

    use ed25519_dalek::SigningKey;
    use fe2o3_broker_authority_service::{
        ProtectedCompilerExecutionExternalAnchorV1, ProtectedExternalAnchorServiceAdmissionV1,
    };
    use fe2o3_external_anchor_protocol::{
        AnchorPositionV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1, PinnedAnchorKeyV1,
        TransactionDigestV1,
    };
    use fe2o3_runtime_protocol::CompilerExecutionExternalAnchorServiceIdentityV1;
    use rustix::net::{AddressFamily, SocketFlags, socketpair};
    use rustix::process::{PidfdFlags, getpid, pidfd_open};

    use super::*;

    fn root() -> (tempfile::TempDir, OwnedFd) {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let root = File::open(directory.path()).unwrap().into();
        (directory, root)
    }

    fn endpoint_pair(flags: SocketFlags) -> (OwnedFd, OwnedFd) {
        socketpair(AddressFamily::UNIX, SocketType::SEQPACKET, flags, None).unwrap()
    }

    fn keys(seed: u8) -> (SigningKey, PinnedAnchorKeyV1) {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let pinned = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
        (signing, pinned)
    }

    fn challenge(
        state: AnchoredStateV1,
        transaction: u8,
        nonce: u8,
        key: &PinnedAnchorKeyV1,
    ) -> fe2o3_external_anchor_protocol::AnchorChallengeV1 {
        state
            .prepare(TransactionDigestV1::from_bytes([transaction; 32]), key)
            .unwrap()
            .begin_advance(CallerNonceV1::from_bytes([nonce; 32]), key)
            .unwrap()
            .challenge()
            .clone()
    }

    #[test]
    fn existing_protected_transport_drives_durable_service_end_to_end() {
        let (_directory, root) = root();
        let (signing, pinned) = keys(21);
        let mut anchor = DurableExternalAnchorV1::initialize(root, signing).unwrap();
        let (service_peer, client_peer) =
            endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
        let service = thread::spawn(move || serve_connected_peer_v1(&mut anchor, service_peer));

        let identity = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap();
        let admission =
            ProtectedExternalAnchorServiceAdmissionV1::admit_non_authoritative_same_uid_test(
                client_peer,
                pidfd_open(getpid(), PidfdFlags::empty()).unwrap(),
                identity,
            )
            .unwrap();
        let mut transport =
            ProtectedCompilerExecutionExternalAnchorV1::new(admission, pinned).unwrap();
        let challenge_key = PinnedAnchorKeyV1::from_bytes(
            SigningKey::from_bytes(&[21; 32]).verifying_key().to_bytes(),
        )
        .unwrap();

        let first = challenge(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            61,
            31,
            &challenge_key,
        );
        let first_receipt = transport.exchange(&first).unwrap();
        assert_eq!(first_receipt.position(), AnchorPositionV1::Proposed);
        let second = challenge(
            AnchoredStateV1::from_local_state(1, first.proposed_head()),
            62,
            32,
            &challenge_key,
        );
        let second_receipt = transport.exchange(&second).unwrap();
        assert_eq!(second_receipt.position(), AnchorPositionV1::Proposed);
        drop(transport);

        let report = service.join().unwrap().unwrap();
        assert_eq!(report.exchanges(), 2);
    }

    #[test]
    fn wrong_endpoint_shapes_fail_before_reading() {
        let (_directory, root) = root();
        let (signing, _) = keys(22);
        let mut anchor = DurableExternalAnchorV1::initialize(root, signing).unwrap();
        let (blocking, _other) = endpoint_pair(SocketFlags::CLOEXEC);
        assert!(matches!(
            serve_connected_peer_v1(&mut anchor, blocking),
            Err(ExternalAnchorDaemonErrorV1::InvalidPeerStatus)
        ));

        let stream = socketpair(
            AddressFamily::UNIX,
            SocketType::STREAM,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap()
        .0;
        assert!(matches!(
            serve_connected_peer_v1(&mut anchor, stream),
            Err(ExternalAnchorDaemonErrorV1::InvalidPeerSocketType)
        ));
    }

    #[test]
    fn short_oversized_and_ancillary_packets_fail_closed() {
        for hostile in 0..3 {
            let (_directory, root) = root();
            let (signing, _) = keys(23 + hostile);
            let mut anchor = DurableExternalAnchorV1::initialize(root, signing).unwrap();
            let (service_peer, client_peer) =
                endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
            let service = thread::spawn(move || serve_connected_peer_v1(&mut anchor, service_peer));
            match hostile {
                0 => {
                    send(&client_peer, &[7; 183], SendFlags::NOSIGNAL).unwrap();
                }
                1 => {
                    send(&client_peer, &[8; 185], SendFlags::NOSIGNAL).unwrap();
                }
                _ => {
                    let passed = File::open("/dev/null").unwrap();
                    let mut ancillary_bytes =
                        [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
                    let mut ancillary = rustix::net::SendAncillaryBuffer::new(&mut ancillary_bytes);
                    let rights = [passed.as_fd()];
                    assert!(ancillary.push(rustix::net::SendAncillaryMessage::ScmRights(&rights)));
                    rustix::net::sendmsg(
                        &client_peer,
                        &[std::io::IoSlice::new(&[9; ANCHOR_CHALLENGE_WIRE_LEN_V1])],
                        &mut ancillary,
                        SendFlags::NOSIGNAL,
                    )
                    .unwrap();
                }
            }
            let error = service.join().unwrap().unwrap_err();
            assert!(matches!(
                (hostile, error),
                (
                    0,
                    ExternalAnchorDaemonErrorV1::InvalidChallengeLength { actual: 183 }
                ) | (1, ExternalAnchorDaemonErrorV1::PacketTruncated)
                    | (2, ExternalAnchorDaemonErrorV1::AncillaryData)
            ));
        }
    }

    #[test]
    fn clean_peer_close_returns_zero_exchange_report() {
        let (_directory, root) = root();
        let (signing, _) = keys(27);
        let mut anchor = DurableExternalAnchorV1::initialize(root, signing).unwrap();
        let (service_peer, client_peer) =
            endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
        drop(client_peer);
        assert_eq!(
            serve_connected_peer_v1(&mut anchor, service_peer).unwrap(),
            ExternalAnchorServiceReportV1 { exchanges: 0 }
        );
    }

    #[test]
    fn zero_response_timeout_is_rejected() {
        let (_directory, root) = root();
        let (signing, _) = keys(28);
        let mut anchor = DurableExternalAnchorV1::initialize(root, signing).unwrap();
        let (service_peer, _client_peer) =
            endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
        assert!(matches!(
            serve_connected_peer_with_timeout(&mut anchor, service_peer, Duration::ZERO),
            Err(ExternalAnchorDaemonErrorV1::InvalidResponseTimeout)
        ));
    }
}
