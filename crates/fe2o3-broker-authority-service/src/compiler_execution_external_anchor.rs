use std::error::Error;
use std::fmt;
use std::io;
use std::mem;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::{Duration, Instant};

use fe2o3_external_anchor_protocol::{
    ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorChallengeV1, AnchorProtocolErrorV1,
    AnchorTransitionReceiptV1, PinnedAnchorKeyV1,
};
use fe2o3_runtime_protocol::CompilerExecutionIssuerPolicyV1;

use crate::{ProtectedExternalAnchorServiceAdmissionV1, ProtectedServiceAdmissionErrorV1};

/// Fixed production deadline for one compiler external-anchor request and response.
pub const COMPILER_EXECUTION_EXTERNAL_ANCHOR_TIMEOUT_V1: Duration = Duration::from_secs(30);

/// Retained protected endpoint for one-at-a-time compiler external-anchor transitions.
///
/// The endpoint is supervisor-provisioned and already bound to one credential identity and live
/// pidfd. Every exchange revalidates that admission, sends exactly one canonical challenge, and
/// returns only an exact signed observation verified under the separately pinned key. A response
/// already queued after process recovery is consumed before any challenge is retransmitted.
/// This object does not implement the external monotonic store or establish independent operation.
pub struct ProtectedCompilerExecutionExternalAnchorV1 {
    admission: ProtectedExternalAnchorServiceAdmissionV1,
    key: PinnedAnchorKeyV1,
    timeout: Duration,
    poisoned: bool,
}

impl fmt::Debug for ProtectedCompilerExecutionExternalAnchorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedCompilerExecutionExternalAnchorV1")
            .field("service_identity", &self.admission.service_identity())
            .field("anchor_key_identity", &self.key.identity())
            .field("timeout", &self.timeout)
            .field("poisoned", &self.poisoned)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl ProtectedCompilerExecutionExternalAnchorV1 {
    /// Binds a retained service admission to one exact external-anchor verification key.
    pub fn new(
        admission: ProtectedExternalAnchorServiceAdmissionV1,
        key: PinnedAnchorKeyV1,
    ) -> Result<Self, ProtectedCompilerExecutionExternalAnchorErrorV1> {
        Self::new_with_timeout(
            admission,
            key,
            COMPILER_EXECUTION_EXTERNAL_ANCHOR_TIMEOUT_V1,
        )
    }

    /// Binds a retained service admission directly to the external key in one issuer policy.
    pub fn from_issuer_policy(
        admission: ProtectedExternalAnchorServiceAdmissionV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, ProtectedCompilerExecutionExternalAnchorErrorV1> {
        let key = PinnedAnchorKeyV1::from_bytes(*policy.external_anchor_verifying_key())?;
        Self::new(admission, key)
    }

    fn new_with_timeout(
        admission: ProtectedExternalAnchorServiceAdmissionV1,
        key: PinnedAnchorKeyV1,
        timeout: Duration,
    ) -> Result<Self, ProtectedCompilerExecutionExternalAnchorErrorV1> {
        if timeout.is_zero() {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::InvalidTimeout);
        }
        admission.validate_continuity()?;
        Ok(Self {
            admission,
            key,
            timeout,
            poisoned: false,
        })
    }

    /// Performs one exact bounded request-response exchange and verifies its signed observation.
    pub fn exchange(
        &mut self,
        challenge: &AnchorChallengeV1,
    ) -> Result<AnchorTransitionReceiptV1, ProtectedCompilerExecutionExternalAnchorErrorV1> {
        if self.poisoned {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::Poisoned);
        }
        let result = self.exchange_once(challenge);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    pub(crate) fn validate_continuity(&self) -> Result<(), ProtectedServiceAdmissionErrorV1> {
        self.admission.validate_continuity()
    }

    pub(crate) fn matches_policy(&self, policy: &CompilerExecutionIssuerPolicyV1) -> bool {
        self.key.to_bytes() == *policy.external_anchor_verifying_key()
    }

    fn exchange_once(
        &self,
        challenge: &AnchorChallengeV1,
    ) -> Result<AnchorTransitionReceiptV1, ProtectedCompilerExecutionExternalAnchorErrorV1> {
        self.admission.validate_continuity()?;
        if challenge.anchor_key_identity() != self.key.identity() {
            return Err(AnchorProtocolErrorV1::AnchorKeyIdentityMismatch.into());
        }
        let deadline = Instant::now()
            .checked_add(self.timeout)
            .ok_or(ProtectedCompilerExecutionExternalAnchorErrorV1::DeadlineOverflow)?;

        if let Some(observation) = receive_observation_nonblocking(self.service_peer())? {
            let receipt = self.verify_observation(challenge, observation)?;
            self.reject_queued_response()?;
            self.admission.validate_continuity()?;
            return Ok(receipt);
        }

        wait_for_service(
            self.service_peer(),
            self.service_pidfd(),
            libc::POLLOUT,
            deadline,
        )?;
        self.admission.validate_continuity()?;
        send_challenge(self.service_peer(), challenge)?;

        wait_for_service(
            self.service_peer(),
            self.service_pidfd(),
            libc::POLLIN,
            deadline,
        )?;
        self.admission.validate_continuity()?;
        let observation = receive_observation_nonblocking(self.service_peer())?
            .ok_or(ProtectedCompilerExecutionExternalAnchorErrorV1::EndpointNotReady)?;
        let receipt = self.verify_observation(challenge, observation)?;
        self.reject_queued_response()?;
        self.admission.validate_continuity()?;
        Ok(receipt)
    }

    fn verify_observation(
        &self,
        challenge: &AnchorChallengeV1,
        observation: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
    ) -> Result<AnchorTransitionReceiptV1, ProtectedCompilerExecutionExternalAnchorErrorV1> {
        AnchorTransitionReceiptV1::new(challenge.clone(), &observation, &self.key)
            .map_err(Into::into)
    }

    fn reject_queued_response(
        &self,
    ) -> Result<(), ProtectedCompilerExecutionExternalAnchorErrorV1> {
        if receive_observation_nonblocking(self.service_peer())?.is_some() {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::DuplicateResponse);
        }
        Ok(())
    }

    fn service_peer(&self) -> BorrowedFd<'_> {
        self.admission.service_peer()
    }

    fn service_pidfd(&self) -> BorrowedFd<'_> {
        self.admission.service_pidfd()
    }

    #[cfg(test)]
    fn new_for_test(
        admission: ProtectedExternalAnchorServiceAdmissionV1,
        key: PinnedAnchorKeyV1,
        timeout: Duration,
    ) -> Result<Self, ProtectedCompilerExecutionExternalAnchorErrorV1> {
        Self::new_with_timeout(admission, key, timeout)
    }
}

fn send_challenge(
    peer: BorrowedFd<'_>,
    challenge: &AnchorChallengeV1,
) -> Result<(), ProtectedCompilerExecutionExternalAnchorErrorV1> {
    let bytes = challenge.as_bytes();
    loop {
        // SAFETY: `bytes` remains readable for its fixed length and `peer` remains borrowed for
        // the duration of this nonblocking, signal-suppressed send.
        let sent = unsafe {
            libc::send(
                peer.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
            )
        };
        if sent < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.kind() == io::ErrorKind::WouldBlock {
                return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::EndpointNotReady);
            }
            if is_peer_closed_error(&error) {
                return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::PeerClosed);
            }
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::Send(error));
        }
        if usize::try_from(sent).ok() != Some(bytes.len()) {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::PartialSend);
        }
        return Ok(());
    }
}

fn receive_observation_nonblocking(
    peer: BorrowedFd<'_>,
) -> Result<
    Option<[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1]>,
    ProtectedCompilerExecutionExternalAnchorErrorV1,
> {
    let mut bytes = [0_u8; ANCHOR_OBSERVATION_WIRE_LEN_V1];
    loop {
        let mut vector = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        // SAFETY: all-zero is a valid empty msghdr and the initialized iovec is installed before
        // the kernel observes it. A zero-length control buffer rejects all ancillary data.
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut vector;
        header.msg_iovlen = 1;
        // SAFETY: `header` references the live fixed buffer through one initialized iovec and the
        // retained endpoint remains borrowed throughout the nonblocking receive.
        let received = unsafe {
            libc::recvmsg(
                peer.as_raw_fd(),
                &mut header,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if received < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            if error.kind() == io::ErrorKind::WouldBlock {
                return Ok(None);
            }
            if is_peer_closed_error(&error) {
                return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::PeerClosed);
            }
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::Receive(
                error,
            ));
        }
        if header.msg_flags & libc::MSG_CTRUNC != 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::AncillaryData);
        }
        if header.msg_flags & libc::MSG_TRUNC != 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::PacketTruncated);
        }
        let actual = usize::try_from(received)
            .map_err(|_| ProtectedCompilerExecutionExternalAnchorErrorV1::PacketTruncated)?;
        if actual == 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::PeerClosed);
        }
        if actual != bytes.len() {
            return Err(
                ProtectedCompilerExecutionExternalAnchorErrorV1::InvalidObservationLength {
                    expected: bytes.len(),
                    actual,
                },
            );
        }
        return Ok(Some(bytes));
    }
}

fn is_peer_closed_error(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(libc::EPIPE) | Some(libc::ECONNRESET) | Some(libc::ENOTCONN)
    )
}

fn wait_for_service(
    peer: BorrowedFd<'_>,
    service_pidfd: BorrowedFd<'_>,
    wanted: i16,
    deadline: Instant,
) -> Result<(), ProtectedCompilerExecutionExternalAnchorErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::Timeout);
        }
        let mut descriptors = [
            libc::pollfd {
                fd: service_pidfd.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: peer.as_raw_fd(),
                events: wanted | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
        ];
        // SAFETY: `descriptors` is a live two-element pollfd array for the bounded call.
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
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::Poll(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::Timeout);
        }
        if descriptors[0].revents & libc::POLLNVAL != 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::InvalidServicePidfd);
        }
        if descriptors[0].revents != 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::ServiceExited);
        }
        if descriptors[1].revents & libc::POLLNVAL != 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::InvalidPeer);
        }
        if descriptors[1].revents & libc::POLLHUP != 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::PeerClosed);
        }
        if descriptors[1].revents & libc::POLLERR != 0 {
            return Err(ProtectedCompilerExecutionExternalAnchorErrorV1::PeerFailed);
        }
        if descriptors[1].revents & wanted != 0 {
            return Ok(());
        }
    }
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

/// Failure from one protected compiler external-anchor exchange.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedCompilerExecutionExternalAnchorErrorV1 {
    /// Retained endpoint or process admission failed continuity validation.
    Admission(ProtectedServiceAdmissionErrorV1),
    /// Signed anchor protocol validation failed.
    Protocol(AnchorProtocolErrorV1),
    /// The configured bounded timeout was zero.
    InvalidTimeout,
    /// The monotonic deadline could not be represented.
    DeadlineOverflow,
    /// Polling the retained endpoint failed.
    Poll(io::Error),
    /// Sending the fixed challenge failed.
    Send(io::Error),
    /// Receiving the fixed observation failed.
    Receive(io::Error),
    /// The bounded exchange deadline elapsed.
    Timeout,
    /// The retained external-anchor process exited.
    ServiceExited,
    /// The retained external-anchor pidfd became invalid.
    InvalidServicePidfd,
    /// The retained endpoint became invalid.
    InvalidPeer,
    /// The retained endpoint reported an asynchronous error.
    PeerFailed,
    /// The retained endpoint closed.
    PeerClosed,
    /// Poll readiness did not survive the nonblocking operation.
    EndpointNotReady,
    /// A seqpacket challenge send was not atomic and complete.
    PartialSend,
    /// The observation packet exceeded the fixed wire bound.
    PacketTruncated,
    /// The observation carried forbidden ancillary data.
    AncillaryData,
    /// The observation packet had the wrong exact byte length.
    InvalidObservationLength { expected: usize, actual: usize },
    /// More than one response was already queued for one request.
    DuplicateResponse,
    /// A prior failed exchange made the live endpoint state uncertain.
    Poisoned,
}

impl fmt::Display for ProtectedCompilerExecutionExternalAnchorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission(error) => write!(formatter, "external-anchor admission: {error}"),
            Self::Protocol(error) => write!(formatter, "external-anchor protocol: {error}"),
            Self::InvalidTimeout => formatter.write_str("external-anchor timeout is zero"),
            Self::DeadlineOverflow => formatter.write_str("external-anchor deadline overflowed"),
            Self::Poll(error) => write!(formatter, "external-anchor poll failed: {error}"),
            Self::Send(error) => write!(formatter, "external-anchor send failed: {error}"),
            Self::Receive(error) => write!(formatter, "external-anchor receive failed: {error}"),
            Self::Timeout => formatter.write_str("external-anchor exchange timed out"),
            Self::ServiceExited => formatter.write_str("external-anchor service exited"),
            Self::InvalidServicePidfd => formatter.write_str("external-anchor pidfd is invalid"),
            Self::InvalidPeer => formatter.write_str("external-anchor endpoint is invalid"),
            Self::PeerFailed => formatter.write_str("external-anchor endpoint reported an error"),
            Self::PeerClosed => formatter.write_str("external-anchor endpoint closed"),
            Self::EndpointNotReady => {
                formatter.write_str("external-anchor endpoint lost readiness")
            }
            Self::PartialSend => formatter.write_str("external-anchor challenge send was partial"),
            Self::PacketTruncated => {
                formatter.write_str("external-anchor observation was truncated")
            }
            Self::AncillaryData => {
                formatter.write_str("external-anchor observation carried ancillary data")
            }
            Self::InvalidObservationLength { expected, actual } => write!(
                formatter,
                "external-anchor observation length mismatch: expected {expected}, got {actual}"
            ),
            Self::DuplicateResponse => {
                formatter.write_str("external-anchor returned more than one response")
            }
            Self::Poisoned => formatter.write_str("external-anchor transport is poisoned"),
        }
    }
}

impl Error for ProtectedCompilerExecutionExternalAnchorErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Poll(error) | Self::Send(error) | Self::Receive(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProtectedServiceAdmissionErrorV1> for ProtectedCompilerExecutionExternalAnchorErrorV1 {
    fn from(error: ProtectedServiceAdmissionErrorV1) -> Self {
        Self::Admission(error)
    }
}

impl From<AnchorProtocolErrorV1> for ProtectedCompilerExecutionExternalAnchorErrorV1 {
    fn from(error: AnchorProtocolErrorV1) -> Self {
        Self::Protocol(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::fd::{AsFd, FromRawFd, OwnedFd, RawFd};
    use std::process::Command;
    use std::ptr;
    use std::sync::mpsc;
    use std::thread;

    use ed25519_dalek::{Signer, SigningKey};
    use fe2o3_external_anchor_protocol::{
        AnchorPositionV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1, TransactionDigestV1,
        UnsignedAnchorObservationV1,
    };
    use fe2o3_runtime_protocol::CompilerExecutionExternalAnchorServiceIdentityV1;
    use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};

    fn pidfd_for(pid: u32) -> OwnedFd {
        // SAFETY: pidfd_open receives one positive PID and zero flags. Success returns one
        // fresh owned close-on-exec descriptor.
        let descriptor =
            unsafe { libc::syscall(libc::SYS_pidfd_open, libc::pid_t::try_from(pid).unwrap(), 0) };
        assert!(
            descriptor >= 0,
            "pidfd_open failed: {}",
            io::Error::last_os_error()
        );
        // SAFETY: successful pidfd_open returned a fresh owned descriptor.
        unsafe { OwnedFd::from_raw_fd(descriptor as RawFd) }
    }

    fn pidfd_for_current_process() -> OwnedFd {
        pidfd_for(std::process::id())
    }

    fn service_identity() -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        CompilerExecutionExternalAnchorServiceIdentityV1::new(
            rustix::process::geteuid().as_raw(),
            rustix::process::getegid().as_raw(),
        )
        .unwrap()
    }

    fn endpoint_pair() -> (OwnedFd, OwnedFd) {
        socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap()
    }

    fn fixture(
        timeout: Duration,
    ) -> (
        ProtectedCompilerExecutionExternalAnchorV1,
        OwnedFd,
        AnchorChallengeV1,
        SigningKey,
    ) {
        let signing_key = SigningKey::from_bytes(&[0xa3; 32]);
        let pinned = PinnedAnchorKeyV1::from_bytes(signing_key.verifying_key().to_bytes()).unwrap();
        let challenge = challenge_for(&pinned, [0x72; 32], [0x51; 32], false);
        let (peer, service) = endpoint_pair();
        let admission =
            ProtectedExternalAnchorServiceAdmissionV1::admit_non_authoritative_same_uid_test(
                peer,
                pidfd_for_current_process(),
                service_identity(),
            )
            .unwrap();
        let transport =
            ProtectedCompilerExecutionExternalAnchorV1::new_for_test(admission, pinned, timeout)
                .unwrap();
        (transport, service, challenge, signing_key)
    }

    fn challenge_for(
        pinned: &PinnedAnchorKeyV1,
        nonce: [u8; 32],
        transaction: [u8; 32],
        recovery: bool,
    ) -> AnchorChallengeV1 {
        let stable = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0_u8; 32]));
        let prepared = stable
            .prepare(TransactionDigestV1::from_bytes(transaction), pinned)
            .unwrap();
        let pending = if recovery {
            prepared.begin_recovery(CallerNonceV1::from_bytes(nonce), pinned)
        } else {
            prepared.begin_advance(CallerNonceV1::from_bytes(nonce), pinned)
        }
        .unwrap();
        pending.challenge().clone()
    }

    fn receive_challenge(service: &OwnedFd) -> AnchorChallengeV1 {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut bytes = [0_u8; fe2o3_external_anchor_protocol::ANCHOR_CHALLENGE_WIRE_LEN_V1];
        loop {
            // SAFETY: the fixed byte array is writable and the descriptor is retained by `service`.
            let received = unsafe {
                libc::recv(
                    service.as_raw_fd(),
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            if received == bytes.len() as isize {
                return AnchorChallengeV1::decode(&bytes).unwrap();
            }
            if received < 0 && io::Error::last_os_error().kind() == io::ErrorKind::WouldBlock {
                assert!(Instant::now() < deadline, "challenge receive timed out");
                thread::yield_now();
                continue;
            }
            panic!("unexpected challenge receive result: {received}");
        }
    }

    fn signed_observation(
        challenge: &AnchorChallengeV1,
        position: AnchorPositionV1,
        signing_key: &SigningKey,
    ) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
        let unsigned = UnsignedAnchorObservationV1::from_challenge(challenge, position);
        let signature = signing_key.sign(&unsigned.signing_bytes()).to_bytes();
        unsigned.attach_signature(signature)
    }

    fn send_packet(service: &OwnedFd, bytes: &[u8]) {
        // SAFETY: `bytes` is readable for its length and the service descriptor remains owned.
        let sent = unsafe {
            libc::send(
                service.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_NOSIGNAL,
            )
        };
        assert_eq!(sent, bytes.len() as isize);
    }

    #[test]
    fn exact_proposed_observation_is_verified() {
        let (mut transport, service, challenge, signing_key) = fixture(Duration::from_secs(5));
        let expected = challenge.clone();
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let received = receive_challenge(&service);
            assert_eq!(received, expected);
            send_packet(
                &service,
                &signed_observation(&received, AnchorPositionV1::Proposed, &signing_key),
            );
            retain.recv().unwrap();
        });
        let result = transport.exchange(&challenge);
        release.send(()).unwrap();
        let receipt = result.unwrap();
        assert_eq!(receipt.challenge(), &challenge);
        assert_eq!(receipt.position(), AnchorPositionV1::Proposed);
        worker.join().unwrap();
    }

    #[test]
    fn queued_recovery_response_is_consumed_without_retransmission() {
        let (mut transport, service, challenge, signing_key) = fixture(Duration::from_secs(5));
        send_packet(
            &service,
            &signed_observation(&challenge, AnchorPositionV1::Proposed, &signing_key),
        );
        let receipt = transport.exchange(&challenge).unwrap();
        assert_eq!(receipt.position(), AnchorPositionV1::Proposed);
        let mut byte = 0_u8;
        // SAFETY: one byte of writable storage is provided for a nonblocking receive probe.
        let received = unsafe {
            libc::recv(
                service.as_raw_fd(),
                ptr::from_mut(&mut byte).cast(),
                1,
                libc::MSG_DONTWAIT,
            )
        };
        assert_eq!(received, -1);
        assert_eq!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn wrong_signature_is_rejected() {
        let (mut transport, service, challenge, _signing_key) = fixture(Duration::from_secs(5));
        let wrong = SigningKey::from_bytes(&[0xb4; 32]);
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let received = receive_challenge(&service);
            send_packet(
                &service,
                &signed_observation(&received, AnchorPositionV1::Proposed, &wrong),
            );
            retain.recv().unwrap();
        });
        let error = transport.exchange(&challenge).unwrap_err();
        release.send(()).unwrap();
        assert!(matches!(
            error,
            ProtectedCompilerExecutionExternalAnchorErrorV1::Protocol(
                AnchorProtocolErrorV1::SignatureRejected
            )
        ));
        assert!(matches!(
            transport.exchange(&challenge).unwrap_err(),
            ProtectedCompilerExecutionExternalAnchorErrorV1::Poisoned
        ));
        worker.join().unwrap();
    }

    #[test]
    fn nonce_and_challenge_substitution_is_rejected() {
        let (mut transport, service, challenge, signing_key) = fixture(Duration::from_secs(5));
        let pinned = PinnedAnchorKeyV1::from_bytes(signing_key.verifying_key().to_bytes()).unwrap();
        let substituted = challenge_for(&pinned, [0x73; 32], [0x51; 32], false);
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = receive_challenge(&service);
            send_packet(
                &service,
                &signed_observation(&substituted, AnchorPositionV1::Proposed, &signing_key),
            );
            retain.recv().unwrap();
        });
        let error = transport.exchange(&challenge).unwrap_err();
        release.send(()).unwrap();
        assert!(matches!(
            error,
            ProtectedCompilerExecutionExternalAnchorErrorV1::Protocol(
                AnchorProtocolErrorV1::ChallengeMismatch
            )
        ));
        worker.join().unwrap();
    }

    #[test]
    fn challenge_phase_substitution_is_rejected() {
        let (mut transport, service, challenge, signing_key) = fixture(Duration::from_secs(5));
        let pinned = PinnedAnchorKeyV1::from_bytes(signing_key.verifying_key().to_bytes()).unwrap();
        let substituted = challenge_for(&pinned, [0x72; 32], [0x51; 32], true);
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = receive_challenge(&service);
            send_packet(
                &service,
                &signed_observation(&substituted, AnchorPositionV1::Proposed, &signing_key),
            );
            retain.recv().unwrap();
        });
        let error = transport.exchange(&challenge).unwrap_err();
        release.send(()).unwrap();
        assert!(matches!(
            error,
            ProtectedCompilerExecutionExternalAnchorErrorV1::Protocol(
                AnchorProtocolErrorV1::ChallengeMismatch
            )
        ));
        worker.join().unwrap();
    }

    #[test]
    fn challenge_for_another_pinned_key_is_rejected_before_send() {
        let (mut transport, service, _challenge, _signing_key) = fixture(Duration::from_secs(5));
        let other = SigningKey::from_bytes(&[0xc5; 32]);
        let other_pinned = PinnedAnchorKeyV1::from_bytes(other.verifying_key().to_bytes()).unwrap();
        let other_challenge = challenge_for(&other_pinned, [0x72; 32], [0x51; 32], false);
        assert!(matches!(
            transport.exchange(&other_challenge).unwrap_err(),
            ProtectedCompilerExecutionExternalAnchorErrorV1::Protocol(
                AnchorProtocolErrorV1::AnchorKeyIdentityMismatch
            )
        ));
        let mut byte = 0_u8;
        // SAFETY: one writable byte is provided for a nonblocking no-send assertion.
        let received = unsafe {
            libc::recv(
                service.as_raw_fd(),
                ptr::from_mut(&mut byte).cast(),
                1,
                libc::MSG_DONTWAIT,
            )
        };
        assert_eq!(received, -1);
        assert_eq!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock);
    }

    #[test]
    fn short_observation_is_rejected() {
        let (mut transport, service, challenge, _signing_key) = fixture(Duration::from_secs(5));
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = receive_challenge(&service);
            send_packet(&service, &[0x11; 17]);
            retain.recv().unwrap();
        });
        let error = transport.exchange(&challenge).unwrap_err();
        release.send(()).unwrap();
        assert!(matches!(
            error,
            ProtectedCompilerExecutionExternalAnchorErrorV1::InvalidObservationLength {
                expected: ANCHOR_OBSERVATION_WIRE_LEN_V1,
                actual: 17
            }
        ));
        worker.join().unwrap();
    }

    #[test]
    fn oversized_observation_is_rejected() {
        let (mut transport, service, challenge, _signing_key) = fixture(Duration::from_secs(5));
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = receive_challenge(&service);
            send_packet(&service, &[0x22; ANCHOR_OBSERVATION_WIRE_LEN_V1 + 1]);
            retain.recv().unwrap();
        });
        let result = transport.exchange(&challenge);
        release.send(()).unwrap();
        assert!(matches!(
            result.unwrap_err(),
            ProtectedCompilerExecutionExternalAnchorErrorV1::PacketTruncated
        ));
        worker.join().unwrap();
    }

    #[test]
    fn duplicate_queued_observation_is_rejected() {
        let (mut transport, service, challenge, signing_key) = fixture(Duration::from_secs(5));
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let received = receive_challenge(&service);
            let response = signed_observation(&received, AnchorPositionV1::Proposed, &signing_key);
            send_packet(&service, &response);
            send_packet(&service, &response);
            retain.recv().unwrap();
        });
        let result = transport.exchange(&challenge);
        release.send(()).unwrap();
        assert!(matches!(
            result.unwrap_err(),
            ProtectedCompilerExecutionExternalAnchorErrorV1::DuplicateResponse
        ));
        worker.join().unwrap();
    }

    #[test]
    fn timeout_is_bounded() {
        let (mut transport, _service, challenge, _signing_key) = fixture(Duration::from_millis(20));
        let started = Instant::now();
        assert!(matches!(
            transport.exchange(&challenge).unwrap_err(),
            ProtectedCompilerExecutionExternalAnchorErrorV1::Timeout
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn peer_close_is_terminal() {
        let (mut transport, service, challenge, _signing_key) = fixture(Duration::from_secs(5));
        drop(service);
        let error = transport.exchange(&challenge).unwrap_err();
        assert!(
            matches!(
                error,
                ProtectedCompilerExecutionExternalAnchorErrorV1::PeerClosed
            ),
            "unexpected closed-peer result: {error:?}"
        );
    }

    #[test]
    fn exited_service_pidfd_is_terminal() {
        let (peer, _service) = endpoint_pair();
        let mut command = Command::new("/bin/true");
        let mut child = crate::test_process_execution::spawn(&mut command).unwrap();
        let pidfd = pidfd_for(child.id());
        assert!(child.wait().unwrap().success());
        assert!(matches!(
            wait_for_service(
                peer.as_fd(),
                pidfd.as_fd(),
                libc::POLLIN,
                Instant::now() + Duration::from_secs(5),
            )
            .unwrap_err(),
            ProtectedCompilerExecutionExternalAnchorErrorV1::ServiceExited
        ));
    }

    #[test]
    fn ancillary_observation_is_rejected() {
        let (mut transport, service, challenge, _signing_key) = fixture(Duration::from_secs(5));
        let (release, retain) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _ = receive_challenge(&service);
            let descriptor: OwnedFd = File::open("/dev/null").unwrap().into();
            let mut byte = 0x61_u8;
            let mut vector = libc::iovec {
                iov_base: ptr::from_mut(&mut byte).cast(),
                iov_len: 1,
            };
            let mut control = [0_usize; 8];
            // SAFETY: zero is the valid empty initialization for msghdr.
            let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
            header.msg_iov = &mut vector;
            header.msg_iovlen = 1;
            header.msg_control = control.as_mut_ptr().cast();
            header.msg_controllen =
                unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
            // SAFETY: aligned control storage has capacity for one SCM_RIGHTS descriptor.
            unsafe {
                let message = libc::CMSG_FIRSTHDR(&header);
                (*message).cmsg_level = libc::SOL_SOCKET;
                (*message).cmsg_type = libc::SCM_RIGHTS;
                (*message).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize;
                ptr::write_unaligned(
                    libc::CMSG_DATA(message).cast::<RawFd>(),
                    descriptor.as_raw_fd(),
                );
            }
            // SAFETY: header references live iovec and aligned ancillary storage for this call.
            let sent = unsafe { libc::sendmsg(service.as_raw_fd(), &header, libc::MSG_NOSIGNAL) };
            assert_eq!(sent, 1);
            retain.recv().unwrap();
        });
        let result = transport.exchange(&challenge);
        release.send(()).unwrap();
        assert!(matches!(
            result.unwrap_err(),
            ProtectedCompilerExecutionExternalAnchorErrorV1::AncillaryData
        ));
        worker.join().unwrap();
    }
}
