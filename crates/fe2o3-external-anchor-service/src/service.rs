//! Exact bounded transport for one connected external-anchor peer.

use std::error::Error;
use std::fmt;
use std::io;
#[cfg(test)]
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::time::Duration;

use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1,
};
#[cfg(test)]
use rustix::net::{SendFlags, SocketType, send};

use crate::{DurableExternalAnchorV1, ExternalAnchorServiceErrorV1, peer_io};

/// Maximum time allowed to publish one already-computed observation to the protected peer.
pub const EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1: Duration = Duration::from_secs(30);

/// Terminal report after the sole connected peer closes cleanly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorServiceReportV1 {
    pub(crate) exchanges: u64,
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
    serve_connected_peer_with_hooks(anchor, peer, response_timeout, &mut NoopServiceHooksV1)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServiceBoundaryV1 {
    BeforeReceive,
    AfterReceive,
    BeforeExchange,
    AfterExchange,
    BeforeSend,
    AfterSend,
}

impl ServiceBoundaryV1 {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 6] = [
        Self::BeforeReceive,
        Self::AfterReceive,
        Self::BeforeExchange,
        Self::AfterExchange,
        Self::BeforeSend,
        Self::AfterSend,
    ];

    const fn operation(self) -> &'static str {
        match self {
            Self::BeforeReceive => "before receiving external-anchor challenge",
            Self::AfterReceive => "after receiving external-anchor challenge",
            Self::BeforeExchange => "before applying external-anchor challenge",
            Self::AfterExchange => "after applying external-anchor challenge",
            Self::BeforeSend => "before sending external-anchor observation",
            Self::AfterSend => "after sending external-anchor observation",
        }
    }
}

pub(crate) trait ServiceHooksV1 {
    fn checkpoint(&mut self, boundary: ServiceBoundaryV1) -> io::Result<()>;
}

pub(crate) struct NoopServiceHooksV1;

impl ServiceHooksV1 for NoopServiceHooksV1 {
    fn checkpoint(&mut self, _boundary: ServiceBoundaryV1) -> io::Result<()> {
        Ok(())
    }
}

fn serve_connected_peer_with_hooks<H: ServiceHooksV1>(
    anchor: &mut DurableExternalAnchorV1,
    peer: OwnedFd,
    response_timeout: Duration,
    hooks: &mut H,
) -> Result<ExternalAnchorServiceReportV1, ExternalAnchorDaemonErrorV1> {
    crate::peer_loop::run(&mut LegacyPeer(anchor), &peer, response_timeout, hooks)
}

struct LegacyPeer<'a>(&'a mut DurableExternalAnchorV1);

impl crate::peer_loop::PeerSession for LegacyPeer<'_> {
    type Error = ExternalAnchorDaemonErrorV1;
    type Observation = [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1];

    fn receive(
        &mut self,
        peer: &OwnedFd,
    ) -> Result<Option<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]>, Self::Error> {
        receive_challenge(peer)
    }
    fn exchange(
        &mut self,
        challenge: &[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1],
    ) -> Result<Self::Observation, Self::Error> {
        Ok(self.0.exchange(challenge)?)
    }
    fn send(
        &mut self,
        peer: &OwnedFd,
        observation: &Self::Observation,
        timeout: Duration,
    ) -> Result<(), Self::Error> {
        send_observation(peer, observation, timeout)
    }
    fn retire(&mut self, _observation: Self::Observation) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub(crate) fn service_checkpoint<H: ServiceHooksV1>(
    hooks: &mut H,
    boundary: ServiceBoundaryV1,
) -> Result<(), ExternalAnchorDaemonErrorV1> {
    hooks
        .checkpoint(boundary)
        .map_err(|source| ExternalAnchorDaemonErrorV1::Io {
            operation: boundary.operation(),
            source,
        })
}

pub(crate) fn validate_peer(peer: &OwnedFd) -> Result<(), ExternalAnchorDaemonErrorV1> {
    peer_io::validate_peer(peer, &mut |_| Ok(()))
}

pub(crate) fn receive_challenge(
    peer: &OwnedFd,
) -> Result<Option<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1]>, ExternalAnchorDaemonErrorV1> {
    peer_io::receive_challenge(peer, &mut |_| Ok(()))
}

pub(crate) fn send_observation(
    peer: &OwnedFd,
    observation: &[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
    timeout: Duration,
) -> Result<(), ExternalAnchorDaemonErrorV1> {
    peer_io::send_observation(peer, observation, timeout, &mut |_| Ok(()))
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
    use std::io;
    use std::os::fd::{AsFd, AsRawFd};
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

    struct CrashAtServiceBoundaryV1 {
        target: ServiceBoundaryV1,
        fired: bool,
    }

    impl CrashAtServiceBoundaryV1 {
        const fn new(target: ServiceBoundaryV1) -> Self {
            Self {
                target,
                fired: false,
            }
        }
    }

    impl ServiceHooksV1 for CrashAtServiceBoundaryV1 {
        fn checkpoint(&mut self, boundary: ServiceBoundaryV1) -> io::Result<()> {
            if !self.fired && boundary == self.target {
                self.fired = true;
                return Err(io::Error::other(
                    "injected external-anchor daemon process crash",
                ));
            }
            Ok(())
        }
    }

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
    fn response_delivery_failure_restarts_and_replays_exact_committed_advance() {
        let (directory, root) = root();
        let (signing, pinned) = keys(29);
        let mut anchor = DurableExternalAnchorV1::initialize(root, signing).unwrap();
        let advance = challenge(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            63,
            33,
            &pinned,
        );
        let proposed_head = advance.proposed_head();
        let (service_peer, client_peer) =
            endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);

        // Keep the request direction live while making response delivery fail deterministically.
        // SAFETY: `client_peer` is a live connected socket and shutdown consumes no memory.
        assert_eq!(
            unsafe { libc::shutdown(client_peer.as_raw_fd(), libc::SHUT_RD) },
            0
        );
        let service = thread::spawn(move || serve_connected_peer_v1(&mut anchor, service_peer));
        assert_eq!(
            send(&client_peer, advance.as_bytes(), SendFlags::NOSIGNAL).unwrap(),
            advance.as_bytes().len()
        );
        assert!(matches!(
            service.join().unwrap(),
            Err(ExternalAnchorDaemonErrorV1::PeerClosed)
        ));
        drop(client_peer);

        let root = File::open(directory.path()).unwrap().into();
        let (signing, _) = keys(29);
        let mut restarted = DurableExternalAnchorV1::open(root, signing).unwrap();
        assert_eq!(restarted.sequence(), 1);
        assert_eq!(restarted.head(), proposed_head);

        let (service_peer, client_peer) =
            endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
        let service = thread::spawn(move || serve_connected_peer_v1(&mut restarted, service_peer));
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
        let receipt = transport.exchange(&advance).unwrap();
        assert_eq!(receipt.position(), AnchorPositionV1::Proposed);
        assert_eq!(receipt.challenge().proposed_head(), proposed_head);
        drop(transport);
        assert_eq!(service.join().unwrap().unwrap().exchanges(), 1);

        let root = File::open(directory.path()).unwrap().into();
        let (signing, _) = keys(29);
        let reopened = DurableExternalAnchorV1::open(root, signing).unwrap();
        assert_eq!(reopened.sequence(), 1);
        assert_eq!(reopened.head(), proposed_head);
    }

    #[test]
    fn every_transport_boundary_restarts_at_prior_or_exact_proposed_state() {
        for boundary in ServiceBoundaryV1::ALL {
            let (directory, root) = root();
            let (signing, pinned) = keys(30);
            let mut anchor = DurableExternalAnchorV1::initialize(root, signing).unwrap();
            let advance = challenge(
                AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
                64,
                34,
                &pinned,
            );
            let proposed_head = advance.proposed_head();
            let (service_peer, client_peer) =
                endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
            assert_eq!(
                send(&client_peer, advance.as_bytes(), SendFlags::NOSIGNAL).unwrap(),
                advance.as_bytes().len()
            );
            let mut fault = CrashAtServiceBoundaryV1::new(boundary);
            assert!(matches!(
                serve_connected_peer_with_hooks(
                    &mut anchor,
                    service_peer,
                    EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1,
                    &mut fault,
                ),
                Err(ExternalAnchorDaemonErrorV1::Io { .. })
            ));
            assert!(fault.fired, "fault did not fire at {boundary:?}");
            drop(anchor);
            drop(client_peer);

            let root = File::open(directory.path()).unwrap().into();
            let (signing, _) = keys(30);
            let mut restarted = DurableExternalAnchorV1::open(root, signing).unwrap();
            let persisted_proposed = matches!(
                boundary,
                ServiceBoundaryV1::AfterExchange
                    | ServiceBoundaryV1::BeforeSend
                    | ServiceBoundaryV1::AfterSend
            );
            assert_eq!(
                restarted.sequence(),
                u64::from(persisted_proposed),
                "unexpected recovered sequence at {boundary:?}"
            );
            assert_eq!(
                restarted.head(),
                if persisted_proposed {
                    proposed_head
                } else {
                    HashChainHeadV1::from_bytes([0; 32])
                },
                "unexpected recovered head at {boundary:?}"
            );

            let (service_peer, client_peer) =
                endpoint_pair(SocketFlags::CLOEXEC | SocketFlags::NONBLOCK);
            let service =
                thread::spawn(move || serve_connected_peer_v1(&mut restarted, service_peer));
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
            let receipt = transport.exchange(&advance).unwrap();
            assert_eq!(receipt.position(), AnchorPositionV1::Proposed);
            assert_eq!(receipt.challenge().proposed_head(), proposed_head);
            drop(transport);
            assert_eq!(service.join().unwrap().unwrap().exchanges(), 1);

            let root = File::open(directory.path()).unwrap().into();
            let (signing, _) = keys(30);
            let reopened = DurableExternalAnchorV1::open(root, signing).unwrap();
            assert_eq!(reopened.sequence(), 1);
            assert_eq!(reopened.head(), proposed_head);
        }
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
