//! Bounded `SOCK_SEQPACKET` service for protected compiler-execution attestation.

use std::error::Error;
use std::fmt;
use std::io;
use std::mem;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_runtime_protocol::{
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationRequestV1,
    CompilerExecutionCurrentRecordAttestationV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationAckV1,
    CompilerExecutionReceiptPublicationV1, CompilerExecutionServiceProtocolErrorV1,
    CompilerExecutionServicePublishDispositionV1, CompilerExecutionServiceRequestIdentityV1,
    CompilerExecutionServiceRequestKindV1, CompilerExecutionServiceRequestV1,
    CompilerExecutionServiceResponseV1, MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
};

use crate::{
    CompilerExecutionIssuerAckV1, CompilerExecutionIssuerRecoveryV1,
    ProtectedCompilerExecutionIssuerErrorV1, ProtectedCompilerExecutionIssuerV1,
};

/// Maximum packets one admitted connection may submit before the service fails closed.
pub const MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1: usize = 8;
/// Fixed absolute deadline applied to all socket waits in one admitted service session.
pub const COMPILER_EXECUTION_SERVICE_SESSION_TIMEOUT_V1: Duration = Duration::from_secs(300);

/// Terminal outcome of one bounded compiler-execution service connection.
// The complete bounded ACK stays inline so this authority boundary does not acquire a fallible
// allocation path merely to report its terminal result.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompilerExecutionServiceExitV1 {
    Published {
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        acknowledgment: CompilerExecutionReceiptPublicationAckV1,
        disposition: CompilerExecutionServicePublishDispositionV1,
    },
    Cancelled {
        request_identity: CompilerExecutionServiceRequestIdentityV1,
    },
    Recovered {
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        carriage: CompilerExecutionReceiptCarriageV1,
    },
    VerifiedCurrent {
        request_identity: CompilerExecutionServiceRequestIdentityV1,
        attestation: CompilerExecutionCurrentRecordAttestationV1,
    },
}

/// Consumes one admitted issuer and serves only its canonical bounded packet protocol.
///
/// The function polls the retained peer and client pidfd together under one absolute monotonic
/// deadline. Every packet is received and sent nonblocking, continuity is revalidated around each
/// operation, and the service terminates after publish, explicit cancellation, client exit,
/// timeout, malformed input, or the packet bound. A synchronous durable transition cannot be
/// interrupted; if it completes after the deadline, no response is sent and exact replay resolves
/// the committed state after restart.
pub fn serve_compiler_execution_v1(
    mut issuer: ProtectedCompilerExecutionIssuerV1,
) -> Result<CompilerExecutionServiceExitV1, CompilerExecutionServiceErrorV1> {
    serve_with_limits(
        &mut issuer,
        COMPILER_EXECUTION_SERVICE_SESSION_TIMEOUT_V1,
        MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1,
    )
}

trait CompilerExecutionServiceIssuerV1 {
    fn validate_continuity(&self) -> Result<(), CompilerExecutionServiceErrorV1>;
    fn policy(&self) -> &CompilerExecutionIssuerPolicyV1;
    fn peer(&self) -> BorrowedFd<'_>;
    fn client_pidfd(&self) -> BorrowedFd<'_>;
    fn recovery(&self) -> CompilerExecutionIssuerRecoveryV1;
    fn prepare(
        &mut self,
        expected_sequence: u64,
        expected_rollback_anchor: [u8; 32],
    ) -> Result<CompilerExecutionAttestationChallengeV1, CompilerExecutionServiceErrorV1>;
    fn issue(
        &mut self,
        request: &CompilerExecutionAttestationRequestV1,
    ) -> Result<CompilerExecutionReceiptPublicationV1, CompilerExecutionServiceErrorV1>;
    fn publish(
        &mut self,
        request: &CompilerExecutionAttestationRequestV1,
        publication: &CompilerExecutionReceiptPublicationV1,
    ) -> Result<
        (
            CompilerExecutionIssuerAckV1,
            CompilerExecutionReceiptPublicationAckV1,
        ),
        CompilerExecutionServiceErrorV1,
    >;
    fn recover(
        &self,
        subject: &InertCompilerExecutionSubjectV1,
    ) -> Result<Option<CompilerExecutionReceiptCarriageV1>, CompilerExecutionServiceErrorV1>;
    fn verify_current(
        &self,
        carriage: &CompilerExecutionReceiptCarriageV1,
        verification_challenge: [u8; 32],
    ) -> Result<CompilerExecutionCurrentRecordAttestationV1, CompilerExecutionServiceErrorV1>;
}

impl CompilerExecutionServiceIssuerV1 for ProtectedCompilerExecutionIssuerV1 {
    fn validate_continuity(&self) -> Result<(), CompilerExecutionServiceErrorV1> {
        self.validate_service_continuity().map_err(Into::into)
    }

    fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        self.service_policy()
    }

    fn peer(&self) -> BorrowedFd<'_> {
        self.service_peer()
    }

    fn client_pidfd(&self) -> BorrowedFd<'_> {
        ProtectedCompilerExecutionIssuerV1::client_pidfd(self)
    }

    fn recovery(&self) -> CompilerExecutionIssuerRecoveryV1 {
        ProtectedCompilerExecutionIssuerV1::recovery(self)
    }

    fn prepare(
        &mut self,
        expected_sequence: u64,
        expected_rollback_anchor: [u8; 32],
    ) -> Result<CompilerExecutionAttestationChallengeV1, CompilerExecutionServiceErrorV1> {
        Ok(self
            .prepare_challenge_for_service(expected_sequence, expected_rollback_anchor)?
            .challenge()
            .clone())
    }

    fn issue(
        &mut self,
        request: &CompilerExecutionAttestationRequestV1,
    ) -> Result<CompilerExecutionReceiptPublicationV1, CompilerExecutionServiceErrorV1> {
        Ok(self
            .issue_receipt_for_service(request.canonical_bytes())?
            .publication()
            .clone())
    }

    fn publish(
        &mut self,
        request: &CompilerExecutionAttestationRequestV1,
        publication: &CompilerExecutionReceiptPublicationV1,
    ) -> Result<
        (
            CompilerExecutionIssuerAckV1,
            CompilerExecutionReceiptPublicationAckV1,
        ),
        CompilerExecutionServiceErrorV1,
    > {
        self.publish_receipt_for_service(request.canonical_bytes(), publication.canonical_bytes())
            .map_err(Into::into)
    }

    fn recover(
        &self,
        subject: &InertCompilerExecutionSubjectV1,
    ) -> Result<Option<CompilerExecutionReceiptCarriageV1>, CompilerExecutionServiceErrorV1> {
        self.recover_current_carriage_for_service(subject)
            .map_err(Into::into)
    }

    fn verify_current(
        &self,
        carriage: &CompilerExecutionReceiptCarriageV1,
        verification_challenge: [u8; 32],
    ) -> Result<CompilerExecutionCurrentRecordAttestationV1, CompilerExecutionServiceErrorV1> {
        self.attest_current_carriage_for_service(carriage, verification_challenge)
            .map_err(Into::into)
    }
}

fn serve_with_limits<I: CompilerExecutionServiceIssuerV1>(
    issuer: &mut I,
    timeout: Duration,
    max_packets: usize,
) -> Result<CompilerExecutionServiceExitV1, CompilerExecutionServiceErrorV1> {
    if timeout.is_zero()
        || max_packets == 0
        || max_packets > MAX_COMPILER_EXECUTION_SERVICE_PACKETS_V1
    {
        return Err(CompilerExecutionServiceErrorV1::InvalidLimits);
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(CompilerExecutionServiceErrorV1::DeadlineOverflow)?;
    for _ in 0..max_packets {
        issuer.validate_continuity()?;
        let packet = receive_packet(issuer.peer(), issuer.client_pidfd(), deadline)?;
        issuer.validate_continuity()?;
        let request = CompilerExecutionServiceRequestV1::decode(packet.as_slice())?;
        let dispatch = dispatch_request(issuer, request)?;
        issuer.validate_continuity()?;
        send_packet(
            issuer.peer(),
            issuer.client_pidfd(),
            dispatch.response.canonical_bytes(),
            deadline,
        )?;
        issuer.validate_continuity()?;
        if let Some(exit) = dispatch.exit {
            return Ok(exit);
        }
    }
    Err(CompilerExecutionServiceErrorV1::PacketLimit)
}

struct DispatchV1 {
    response: CompilerExecutionServiceResponseV1,
    exit: Option<CompilerExecutionServiceExitV1>,
}

fn dispatch_request<I: CompilerExecutionServiceIssuerV1>(
    issuer: &mut I,
    request: CompilerExecutionServiceRequestV1,
) -> Result<DispatchV1, CompilerExecutionServiceErrorV1> {
    let policy = issuer.policy().clone();
    if request.policy_identity() != policy.identity() {
        return Err(CompilerExecutionServiceErrorV1::PolicyMismatch);
    }
    let request_identity = request.identity();
    let (response, exit) = match request.kind() {
        CompilerExecutionServiceRequestKindV1::Inspect => (
            response_from_recovery(request_identity, &policy, issuer.recovery())?,
            None,
        ),
        CompilerExecutionServiceRequestKindV1::Prepare => {
            let challenge = issuer.prepare(
                request.expected_sequence(),
                request.expected_rollback_anchor(),
            )?;
            (
                CompilerExecutionServiceResponseV1::prepared(request_identity, &policy, challenge)?,
                None,
            )
        }
        CompilerExecutionServiceRequestKindV1::Issue => {
            let attestation_request = request
                .request()
                .ok_or(CompilerExecutionServiceErrorV1::PayloadMismatch)?;
            let publication = issuer.issue(attestation_request)?;
            (
                CompilerExecutionServiceResponseV1::issued(request_identity, &policy, publication)?,
                None,
            )
        }
        CompilerExecutionServiceRequestKindV1::Publish => {
            let attestation_request = request
                .request()
                .ok_or(CompilerExecutionServiceErrorV1::PayloadMismatch)?;
            let publication = request
                .publication()
                .ok_or(CompilerExecutionServiceErrorV1::PayloadMismatch)?;
            let (outcome, acknowledgment) = issuer.publish(attestation_request, publication)?;
            acknowledgment.matches_publication(publication)?;
            let disposition = match outcome {
                CompilerExecutionIssuerAckV1::Advanced => {
                    CompilerExecutionServicePublishDispositionV1::Advanced
                }
                CompilerExecutionIssuerAckV1::AlreadyAcknowledged => {
                    CompilerExecutionServicePublishDispositionV1::AlreadyAcknowledged
                }
            };
            let response = CompilerExecutionServiceResponseV1::published(
                request_identity,
                &policy,
                acknowledgment.clone(),
                disposition,
            )?;
            let exit = CompilerExecutionServiceExitV1::Published {
                request_identity,
                acknowledgment,
                disposition,
            };
            (response, Some(exit))
        }
        CompilerExecutionServiceRequestKindV1::Cancel => {
            let (sequence, rollback_anchor) = recovery_position(&issuer.recovery());
            (
                CompilerExecutionServiceResponseV1::cancelled(
                    request_identity,
                    &policy,
                    sequence,
                    rollback_anchor,
                )?,
                Some(CompilerExecutionServiceExitV1::Cancelled { request_identity }),
            )
        }
        CompilerExecutionServiceRequestKindV1::Recover => {
            let subject = request
                .subject()
                .ok_or(CompilerExecutionServiceErrorV1::PayloadMismatch)?;
            match issuer.recover(subject)? {
                Some(carriage) => {
                    if carriage.request().subject() != subject {
                        return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
                    }
                    let response = CompilerExecutionServiceResponseV1::recovered(
                        request_identity,
                        carriage.clone(),
                    )?;
                    let exit = CompilerExecutionServiceExitV1::Recovered {
                        request_identity,
                        carriage,
                    };
                    (response, Some(exit))
                }
                None => {
                    let (sequence, rollback_anchor) = recovery_position(&issuer.recovery());
                    (
                        CompilerExecutionServiceResponseV1::receipt_absent(
                            request_identity,
                            &policy,
                            sequence,
                            rollback_anchor,
                        )?,
                        None,
                    )
                }
            }
        }
        CompilerExecutionServiceRequestKindV1::VerifyCurrent => {
            let carriage = request
                .carriage()
                .ok_or(CompilerExecutionServiceErrorV1::PayloadMismatch)?;
            let verification_challenge = request
                .verification_challenge()
                .ok_or(CompilerExecutionServiceErrorV1::PayloadMismatch)?;
            let attestation = issuer.verify_current(carriage, verification_challenge)?;
            let verification = attestation.verification();
            if verification.policy_identity() != *policy.identity().as_bytes()
                || verification.carriage_identity() != *carriage.identity().as_bytes()
                || verification.sequence() != request.expected_sequence()
                || verification.current_rollback_anchor() != request.expected_rollback_anchor()
                || attestation.challenge() != verification_challenge
                || attestation.verifying_key() != *policy.verifying_key()
            {
                return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
            }
            let response = CompilerExecutionServiceResponseV1::verified_current(
                request_identity,
                attestation.clone(),
            )?;
            let exit = CompilerExecutionServiceExitV1::VerifiedCurrent {
                request_identity,
                attestation,
            };
            (response, Some(exit))
        }
    };
    Ok(DispatchV1 { response, exit })
}

fn response_from_recovery(
    request_identity: CompilerExecutionServiceRequestIdentityV1,
    policy: &CompilerExecutionIssuerPolicyV1,
    recovery: CompilerExecutionIssuerRecoveryV1,
) -> Result<CompilerExecutionServiceResponseV1, CompilerExecutionServiceProtocolErrorV1> {
    match recovery {
        CompilerExecutionIssuerRecoveryV1::Ready {
            next_sequence,
            current_rollback_anchor,
        } => CompilerExecutionServiceResponseV1::ready(
            request_identity,
            policy,
            next_sequence,
            current_rollback_anchor,
        ),
        CompilerExecutionIssuerRecoveryV1::Prepared { challenge } => {
            CompilerExecutionServiceResponseV1::prepared(request_identity, policy, challenge)
        }
        CompilerExecutionIssuerRecoveryV1::Issued { publication } => {
            CompilerExecutionServiceResponseV1::issued(request_identity, policy, publication)
        }
    }
}

fn recovery_position(recovery: &CompilerExecutionIssuerRecoveryV1) -> (u64, [u8; 32]) {
    match recovery {
        CompilerExecutionIssuerRecoveryV1::Ready {
            next_sequence,
            current_rollback_anchor,
        } => (*next_sequence, *current_rollback_anchor),
        CompilerExecutionIssuerRecoveryV1::Prepared { challenge } => {
            (challenge.sequence(), challenge.prior_rollback_anchor())
        }
        CompilerExecutionIssuerRecoveryV1::Issued { publication } => (
            publication.receipt().sequence(),
            publication.receipt().prior_rollback_anchor(),
        ),
    }
}

fn receive_packet(
    peer: BorrowedFd<'_>,
    client_pidfd: BorrowedFd<'_>,
    deadline: Instant,
) -> Result<ReceivedPacketV1, CompilerExecutionServiceErrorV1> {
    let mut bytes = [0_u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1];
    loop {
        wait_for_peer(peer, client_pidfd, libc::POLLIN, deadline)?;
        let mut vector = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        // SAFETY: An all-zero msghdr is a valid empty header and its initialized iovec fields are
        // installed immediately below before the header is passed to the kernel.
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut vector;
        header.msg_iovlen = 1;
        // No ancillary data is accepted. Linux closes SCM_RIGHTS descriptors that do not fit the
        // zero-length control buffer and reports MSG_CTRUNC, which is terminal below.
        // SAFETY: `header` names the live stack buffer through one initialized iovec for the
        // duration of the call, and `peer` remains borrowed for that duration.
        let received = unsafe {
            libc::recvmsg(
                peer.as_raw_fd(),
                &mut header,
                libc::MSG_DONTWAIT | libc::MSG_CMSG_CLOEXEC,
            )
        };
        if received < 0 {
            let error = io::Error::last_os_error();
            if matches!(
                error.kind(),
                io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
            ) {
                continue;
            }
            return Err(CompilerExecutionServiceErrorV1::Receive(error));
        }
        if header.msg_flags & libc::MSG_CTRUNC != 0 {
            return Err(CompilerExecutionServiceErrorV1::AncillaryData);
        }
        if header.msg_flags & libc::MSG_TRUNC != 0 {
            return Err(CompilerExecutionServiceErrorV1::PacketTruncated);
        }
        let received = usize::try_from(received)
            .map_err(|_| CompilerExecutionServiceErrorV1::PacketTruncated)?;
        if received == 0 {
            return Err(CompilerExecutionServiceErrorV1::PeerClosed);
        }
        return Ok(ReceivedPacketV1 {
            bytes,
            len: received,
        });
    }
}

struct ReceivedPacketV1 {
    bytes: [u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1],
    len: usize,
}

impl ReceivedPacketV1 {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn send_packet(
    peer: BorrowedFd<'_>,
    client_pidfd: BorrowedFd<'_>,
    bytes: &[u8],
    deadline: Instant,
) -> Result<(), CompilerExecutionServiceErrorV1> {
    loop {
        wait_for_peer(peer, client_pidfd, libc::POLLOUT, deadline)?;
        // SAFETY: `bytes` is readable for `bytes.len()` bytes and `peer` remains borrowed for the
        // duration of the call.
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
            if matches!(
                error.kind(),
                io::ErrorKind::Interrupted | io::ErrorKind::WouldBlock
            ) {
                continue;
            }
            return Err(CompilerExecutionServiceErrorV1::Send(error));
        }
        if usize::try_from(sent).ok() != Some(bytes.len()) {
            return Err(CompilerExecutionServiceErrorV1::PartialSend);
        }
        return Ok(());
    }
}

fn wait_for_peer(
    peer: BorrowedFd<'_>,
    client_pidfd: BorrowedFd<'_>,
    wanted: i16,
    deadline: Instant,
) -> Result<(), CompilerExecutionServiceErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CompilerExecutionServiceErrorV1::Timeout);
        }
        let mut descriptors = [
            libc::pollfd {
                fd: client_pidfd.as_raw_fd(),
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
            libc::pollfd {
                fd: peer.as_raw_fd(),
                events: wanted | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            },
        ];
        // SAFETY: `descriptors` is a live two-element pollfd array for the duration of the call.
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
            return Err(CompilerExecutionServiceErrorV1::Poll(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(CompilerExecutionServiceErrorV1::Timeout);
        }
        if descriptors[0].revents & libc::POLLNVAL != 0 {
            return Err(CompilerExecutionServiceErrorV1::InvalidClientPidfd);
        }
        if descriptors[0].revents != 0 {
            return Err(CompilerExecutionServiceErrorV1::ClientExited);
        }
        if descriptors[1].revents & libc::POLLNVAL != 0 {
            return Err(CompilerExecutionServiceErrorV1::InvalidPeer);
        }
        if descriptors[1].revents & libc::POLLERR != 0 {
            return Err(CompilerExecutionServiceErrorV1::PeerFailed);
        }
        if descriptors[1].revents & libc::POLLHUP != 0 {
            return Err(CompilerExecutionServiceErrorV1::PeerClosed);
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

/// Protected compiler-execution service failure.
#[derive(Debug)]
pub enum CompilerExecutionServiceErrorV1 {
    Issuer(ProtectedCompilerExecutionIssuerErrorV1),
    Protocol(CompilerExecutionServiceProtocolErrorV1),
    Publication(fe2o3_runtime_protocol::CompilerExecutionReceiptPublicationErrorV1),
    Poll(io::Error),
    Receive(io::Error),
    Send(io::Error),
    InvalidLimits,
    DeadlineOverflow,
    Timeout,
    PacketLimit,
    PacketTruncated,
    AncillaryData,
    PeerClosed,
    PeerFailed,
    InvalidPeer,
    ClientExited,
    InvalidClientPidfd,
    PartialSend,
    PolicyMismatch,
    PayloadMismatch,
}

impl fmt::Display for CompilerExecutionServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Issuer(error) => write!(formatter, "compiler-execution issuer failed: {error}"),
            Self::Protocol(error) => write!(formatter, "compiler-execution packet failed: {error}"),
            Self::Publication(error) => {
                write!(formatter, "compiler-execution publication failed: {error}")
            }
            Self::Poll(error) => {
                write!(formatter, "compiler-execution service poll failed: {error}")
            }
            Self::Receive(error) => write!(
                formatter,
                "compiler-execution service receive failed: {error}"
            ),
            Self::Send(error) => {
                write!(formatter, "compiler-execution service send failed: {error}")
            }
            Self::InvalidLimits => {
                formatter.write_str("compiler-execution service limits are invalid")
            }
            Self::DeadlineOverflow => {
                formatter.write_str("compiler-execution service deadline overflowed")
            }
            Self::Timeout => formatter.write_str("compiler-execution service deadline expired"),
            Self::PacketLimit => {
                formatter.write_str("compiler-execution service packet limit reached")
            }
            Self::PacketTruncated => {
                formatter.write_str("compiler-execution service packet was truncated")
            }
            Self::AncillaryData => {
                formatter.write_str("compiler-execution service rejects ancillary data")
            }
            Self::PeerClosed => formatter.write_str("compiler-execution service peer closed"),
            Self::PeerFailed => {
                formatter.write_str("compiler-execution service peer reported an error")
            }
            Self::InvalidPeer => {
                formatter.write_str("compiler-execution service peer descriptor is invalid")
            }
            Self::ClientExited => formatter.write_str("compiler-execution service client exited"),
            Self::InvalidClientPidfd => {
                formatter.write_str("compiler-execution service client pidfd is invalid")
            }
            Self::PartialSend => {
                formatter.write_str("compiler-execution service response was partially sent")
            }
            Self::PolicyMismatch => {
                formatter.write_str("compiler-execution service policy mismatch")
            }
            Self::PayloadMismatch => {
                formatter.write_str("compiler-execution service request payload is absent")
            }
        }
    }
}

impl Error for CompilerExecutionServiceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Issuer(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Publication(error) => Some(error),
            Self::Poll(error) | Self::Receive(error) | Self::Send(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProtectedCompilerExecutionIssuerErrorV1> for CompilerExecutionServiceErrorV1 {
    fn from(error: ProtectedCompilerExecutionIssuerErrorV1) -> Self {
        Self::Issuer(error)
    }
}

impl From<CompilerExecutionServiceProtocolErrorV1> for CompilerExecutionServiceErrorV1 {
    fn from(error: CompilerExecutionServiceProtocolErrorV1) -> Self {
        Self::Protocol(error)
    }
}

impl From<fe2o3_runtime_protocol::CompilerExecutionReceiptPublicationErrorV1>
    for CompilerExecutionServiceErrorV1
{
    fn from(error: fe2o3_runtime_protocol::CompilerExecutionReceiptPublicationErrorV1) -> Self {
        Self::Publication(error)
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::{AsFd, FromRawFd, OwnedFd, RawFd};
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use ed25519_dalek::SigningKey;
    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
    };
    use fe2o3_runtime_protocol::{
        CompilerExecutionAttestationReceiptV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionServiceResponseKindV1, MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1,
    };
    use sha2::{Digest, Sha256};

    use super::*;

    const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
    const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

    struct FakeIssuer {
        service: OwnedFd,
        pidfd: OwnedFd,
        signing_key: SigningKey,
        policy: CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
        acknowledgment: CompilerExecutionReceiptPublicationAckV1,
        recovery: CompilerExecutionIssuerRecoveryV1,
        published: bool,
        continuity_checks: Arc<AtomicUsize>,
    }

    impl FakeIssuer {
        fn new() -> (Self, OwnedFd) {
            let (service, client) = socket_pair();
            let pidfd = pidfd(std::process::id());
            let key = SigningKey::from_bytes(&[0x51; 32]);
            let policy = CompilerExecutionIssuerPolicyV1::new(
                1,
                CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 123).unwrap(),
                CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 456).unwrap(),
                key.verifying_key().to_bytes(),
                SigningKey::from_bytes(&[0x52; 32])
                    .verifying_key()
                    .to_bytes(),
            )
            .unwrap();
            let challenge = CompilerExecutionAttestationChallengeV1::new(
                &policy,
                &subject(0x20),
                [0x63; 32],
                1,
                [0; 32],
            )
            .unwrap();
            let request =
                CompilerExecutionAttestationRequestV1::new(challenge, subject(0x20)).unwrap();
            let receipt =
                CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &key).unwrap();
            let publication =
                CompilerExecutionReceiptPublicationV1::new([0x64; 32], [0x65; 32], receipt)
                    .unwrap();
            let acknowledgment =
                CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x66; 32]).unwrap();
            (
                Self {
                    service,
                    pidfd,
                    signing_key: key,
                    policy,
                    request,
                    publication,
                    acknowledgment,
                    recovery: CompilerExecutionIssuerRecoveryV1::Ready {
                        next_sequence: 1,
                        current_rollback_anchor: [0; 32],
                    },
                    published: false,
                    continuity_checks: Arc::new(AtomicUsize::new(0)),
                },
                client,
            )
        }
    }

    impl CompilerExecutionServiceIssuerV1 for FakeIssuer {
        fn validate_continuity(&self) -> Result<(), CompilerExecutionServiceErrorV1> {
            self.continuity_checks.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
            &self.policy
        }

        fn peer(&self) -> BorrowedFd<'_> {
            self.service.as_fd()
        }

        fn client_pidfd(&self) -> BorrowedFd<'_> {
            self.pidfd.as_fd()
        }

        fn recovery(&self) -> CompilerExecutionIssuerRecoveryV1 {
            self.recovery.clone()
        }

        fn prepare(
            &mut self,
            expected_sequence: u64,
            expected_rollback_anchor: [u8; 32],
        ) -> Result<CompilerExecutionAttestationChallengeV1, CompilerExecutionServiceErrorV1>
        {
            if expected_sequence != 1 || expected_rollback_anchor != [0; 32] {
                return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
            }
            let challenge = self.request.challenge().clone();
            match &self.recovery {
                CompilerExecutionIssuerRecoveryV1::Ready { .. } => {
                    self.recovery = CompilerExecutionIssuerRecoveryV1::Prepared {
                        challenge: challenge.clone(),
                    };
                }
                CompilerExecutionIssuerRecoveryV1::Prepared {
                    challenge: retained,
                } if retained == &challenge => {}
                _ => return Err(CompilerExecutionServiceErrorV1::PayloadMismatch),
            }
            Ok(challenge)
        }

        fn issue(
            &mut self,
            request: &CompilerExecutionAttestationRequestV1,
        ) -> Result<CompilerExecutionReceiptPublicationV1, CompilerExecutionServiceErrorV1>
        {
            if request != &self.request {
                return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
            }
            match &self.recovery {
                CompilerExecutionIssuerRecoveryV1::Prepared { .. } => {
                    self.recovery = CompilerExecutionIssuerRecoveryV1::Issued {
                        publication: self.publication.clone(),
                    };
                }
                CompilerExecutionIssuerRecoveryV1::Issued { publication }
                    if publication == &self.publication => {}
                _ => return Err(CompilerExecutionServiceErrorV1::PayloadMismatch),
            }
            Ok(self.publication.clone())
        }

        fn publish(
            &mut self,
            request: &CompilerExecutionAttestationRequestV1,
            publication: &CompilerExecutionReceiptPublicationV1,
        ) -> Result<
            (
                CompilerExecutionIssuerAckV1,
                CompilerExecutionReceiptPublicationAckV1,
            ),
            CompilerExecutionServiceErrorV1,
        > {
            if request != &self.request || publication != &self.publication {
                return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
            }
            let outcome = if self.published {
                CompilerExecutionIssuerAckV1::AlreadyAcknowledged
            } else {
                self.published = true;
                self.recovery = CompilerExecutionIssuerRecoveryV1::Ready {
                    next_sequence: 2,
                    current_rollback_anchor: self.publication.receipt().next_rollback_anchor(),
                };
                CompilerExecutionIssuerAckV1::Advanced
            };
            Ok((outcome, self.acknowledgment.clone()))
        }

        fn recover(
            &self,
            expected_subject: &InertCompilerExecutionSubjectV1,
        ) -> Result<Option<CompilerExecutionReceiptCarriageV1>, CompilerExecutionServiceErrorV1>
        {
            if !self.published {
                return Ok(None);
            }
            if self.request.subject() != expected_subject {
                return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
            }
            CompilerExecutionReceiptCarriageV1::new(
                self.policy.clone(),
                self.request.clone(),
                self.publication.clone(),
                self.acknowledgment.clone(),
            )
            .map(Some)
            .map_err(Into::into)
        }

        fn verify_current(
            &self,
            carriage: &CompilerExecutionReceiptCarriageV1,
            verification_challenge: [u8; 32],
        ) -> Result<CompilerExecutionCurrentRecordAttestationV1, CompilerExecutionServiceErrorV1>
        {
            if !self.published {
                return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
            }
            let expected = CompilerExecutionReceiptCarriageV1::new(
                self.policy.clone(),
                self.request.clone(),
                self.publication.clone(),
                self.acknowledgment.clone(),
            )?;
            if carriage != &expected {
                return Err(CompilerExecutionServiceErrorV1::PayloadMismatch);
            }
            let verification =
                fe2o3_runtime_protocol::CompilerExecutionCurrentRecordVerificationV1::new(
                    self.request.subject(),
                    carriage,
                    [0x91; 32],
                    [0x92; 32],
                )
                .map_err(CompilerExecutionServiceProtocolErrorV1::from)?;
            CompilerExecutionCurrentRecordAttestationV1::issue(
                &self.policy,
                verification,
                verification_challenge,
                &self.signing_key,
            )
            .map_err(CompilerExecutionServiceProtocolErrorV1::from)
            .map_err(Into::into)
        }
    }

    fn socket_pair() -> (OwnedFd, OwnedFd) {
        let mut descriptors = [-1; 2];
        let result = unsafe {
            libc::socketpair(
                libc::AF_UNIX,
                libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                0,
                descriptors.as_mut_ptr(),
            )
        };
        assert_eq!(result, 0);
        unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        }
    }

    fn pidfd(pid: u32) -> OwnedFd {
        let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
        assert!(descriptor >= 0);
        unsafe { OwnedFd::from_raw_fd(descriptor as RawFd) }
    }

    fn policy() -> CompilerExecutionIssuerPolicyV1 {
        let key = SigningKey::from_bytes(&[0x51; 32]);
        CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[0x52; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn send_raw(peer: BorrowedFd<'_>, bytes: &[u8]) {
        let sent = unsafe {
            libc::send(
                peer.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_NOSIGNAL,
            )
        };
        assert_eq!(sent, bytes.len() as isize);
    }

    fn receive_raw(peer: BorrowedFd<'_>) -> Vec<u8> {
        let mut bytes = [0_u8; MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1];
        let received =
            unsafe { libc::recv(peer.as_raw_fd(), bytes.as_mut_ptr().cast(), bytes.len(), 0) };
        assert!(received > 0);
        bytes[..received as usize].to_vec()
    }

    #[test]
    fn dispatches_all_operations_and_exact_replays() {
        let (mut issuer, _client) = FakeIssuer::new();
        let absent_request = CompilerExecutionServiceRequestV1::recover(
            &issuer.policy,
            issuer.request.subject().clone(),
        )
        .unwrap();
        let absent = dispatch_request(&mut issuer, absent_request).unwrap();
        assert_eq!(
            absent.response.kind(),
            CompilerExecutionServiceResponseKindV1::ReceiptAbsent
        );
        assert!(absent.exit.is_none());

        let inspect = CompilerExecutionServiceRequestV1::inspect(&issuer.policy);
        let response = dispatch_request(&mut issuer, inspect).unwrap();
        assert_eq!(
            response.response.kind(),
            CompilerExecutionServiceResponseKindV1::Ready
        );
        assert!(response.exit.is_none());

        let prepare =
            CompilerExecutionServiceRequestV1::prepare(&issuer.policy, 1, [0; 32]).unwrap();
        let prepared = dispatch_request(&mut issuer, prepare.clone()).unwrap();
        assert_eq!(
            prepared.response.kind(),
            CompilerExecutionServiceResponseKindV1::Prepared
        );
        let replayed = dispatch_request(&mut issuer, prepare).unwrap();
        assert_eq!(replayed.response.challenge(), prepared.response.challenge());

        let issue =
            CompilerExecutionServiceRequestV1::issue(&issuer.policy, issuer.request.clone())
                .unwrap();
        let issued = dispatch_request(&mut issuer, issue.clone()).unwrap();
        assert_eq!(
            issued.response.kind(),
            CompilerExecutionServiceResponseKindV1::Issued
        );
        let replayed = dispatch_request(&mut issuer, issue).unwrap();
        assert_eq!(
            replayed.response.publication(),
            issued.response.publication()
        );

        let publish = CompilerExecutionServiceRequestV1::publish(
            &issuer.policy,
            issuer.request.clone(),
            issuer.publication.clone(),
        )
        .unwrap();
        let published = dispatch_request(&mut issuer, publish.clone()).unwrap();
        assert_eq!(
            published.response.disposition(),
            Some(CompilerExecutionServicePublishDispositionV1::Advanced)
        );
        assert!(matches!(
            published.exit,
            Some(CompilerExecutionServiceExitV1::Published { .. })
        ));
        let replayed = dispatch_request(&mut issuer, publish).unwrap();
        assert_eq!(
            replayed.response.disposition(),
            Some(CompilerExecutionServicePublishDispositionV1::AlreadyAcknowledged)
        );
        assert_eq!(
            replayed.response.acknowledgment(),
            published.response.acknowledgment()
        );

        let recover = CompilerExecutionServiceRequestV1::recover(
            &issuer.policy,
            issuer.request.subject().clone(),
        )
        .unwrap();
        let recovered = dispatch_request(&mut issuer, recover).unwrap();
        assert_eq!(
            recovered.response.kind(),
            CompilerExecutionServiceResponseKindV1::Recovered
        );
        assert_eq!(
            recovered
                .response
                .carriage()
                .expect("recovered response carries the receipt")
                .request()
                .subject(),
            issuer.request.subject()
        );
        assert!(matches!(
            recovered.exit,
            Some(CompilerExecutionServiceExitV1::Recovered { .. })
        ));

        let carriage = recovered.response.carriage().unwrap().clone();
        let verify_current =
            CompilerExecutionServiceRequestV1::verify_current(&issuer.policy, carriage, [0xa1; 32])
                .unwrap();
        let verified = dispatch_request(&mut issuer, verify_current).unwrap();
        assert_eq!(
            verified.response.kind(),
            CompilerExecutionServiceResponseKindV1::VerifiedCurrent
        );
        assert!(matches!(
            verified.exit,
            Some(CompilerExecutionServiceExitV1::VerifiedCurrent { .. })
        ));

        let wrong =
            CompilerExecutionServiceRequestV1::recover(&issuer.policy, subject(0x21)).unwrap();
        assert!(matches!(
            dispatch_request(&mut issuer, wrong),
            Err(CompilerExecutionServiceErrorV1::PayloadMismatch)
        ));
    }

    #[test]
    fn dispatch_rejects_a_canonical_request_for_an_unpinned_policy() {
        let (mut issuer, _client) = FakeIssuer::new();
        let wrong_key = SigningKey::from_bytes(&[0x71; 32]);
        let wrong_policy = CompilerExecutionIssuerPolicyV1::new(
            2,
            CompilerExecutionIssuerMeasurementV1::new([0x72; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x73; 32], 456).unwrap(),
            wrong_key.verifying_key().to_bytes(),
            *issuer.policy.external_anchor_verifying_key(),
        )
        .unwrap();
        let request = CompilerExecutionServiceRequestV1::inspect(&wrong_policy);
        assert!(matches!(
            dispatch_request(&mut issuer, request),
            Err(CompilerExecutionServiceErrorV1::PolicyMismatch)
        ));
        assert_eq!(
            issuer.recovery,
            CompilerExecutionIssuerRecoveryV1::Ready {
                next_sequence: 1,
                current_rollback_anchor: [0; 32],
            }
        );
    }

    #[test]
    fn bounded_loop_sends_cancel_response_and_enforces_packet_limit() {
        let (mut issuer, client) = FakeIssuer::new();
        let policy = issuer.policy.clone();
        let continuity_checks = issuer.continuity_checks.clone();
        let handle =
            thread::spawn(move || serve_with_limits(&mut issuer, Duration::from_secs(1), 2));
        let inspect = CompilerExecutionServiceRequestV1::inspect(&policy);
        send_raw(client.as_fd(), inspect.canonical_bytes());
        let response =
            CompilerExecutionServiceResponseV1::decode(&receive_raw(client.as_fd())).unwrap();
        assert_eq!(
            response.kind(),
            CompilerExecutionServiceResponseKindV1::Ready
        );
        let cancel = CompilerExecutionServiceRequestV1::cancel(&policy);
        send_raw(client.as_fd(), cancel.canonical_bytes());
        let response =
            CompilerExecutionServiceResponseV1::decode(&receive_raw(client.as_fd())).unwrap();
        assert_eq!(
            response.kind(),
            CompilerExecutionServiceResponseKindV1::Cancelled
        );
        assert!(matches!(
            handle.join().unwrap().unwrap(),
            CompilerExecutionServiceExitV1::Cancelled { .. }
        ));
        assert_eq!(continuity_checks.load(Ordering::SeqCst), 8);

        let (mut issuer, client) = FakeIssuer::new();
        let policy = issuer.policy.clone();
        let handle =
            thread::spawn(move || serve_with_limits(&mut issuer, Duration::from_secs(1), 1));
        let inspect = CompilerExecutionServiceRequestV1::inspect(&policy);
        send_raw(client.as_fd(), inspect.canonical_bytes());
        CompilerExecutionServiceResponseV1::decode(&receive_raw(client.as_fd())).unwrap();
        assert!(matches!(
            handle.join().unwrap(),
            Err(CompilerExecutionServiceErrorV1::PacketLimit)
        ));
    }

    #[test]
    fn bounded_service_returns_the_complete_current_carriage() {
        let (mut issuer, client) = FakeIssuer::new();
        issuer.published = true;
        let policy = issuer.policy.clone();
        let expected_subject = issuer.request.subject().clone();
        let expected_acknowledgment = issuer.acknowledgment.clone();
        let handle =
            thread::spawn(move || serve_with_limits(&mut issuer, Duration::from_secs(1), 1));

        let recover =
            CompilerExecutionServiceRequestV1::recover(&policy, expected_subject.clone()).unwrap();
        send_raw(client.as_fd(), recover.canonical_bytes());
        let response =
            CompilerExecutionServiceResponseV1::decode(&receive_raw(client.as_fd())).unwrap();
        assert_eq!(
            response.kind(),
            CompilerExecutionServiceResponseKindV1::Recovered
        );
        let carriage = response.carriage().unwrap();
        assert_eq!(carriage.policy(), &policy);
        assert_eq!(carriage.request().subject(), &expected_subject);
        assert_eq!(carriage.acknowledgment(), &expected_acknowledgment);
        match handle.join().unwrap().unwrap() {
            CompilerExecutionServiceExitV1::Recovered {
                request_identity,
                carriage,
            } => {
                assert_eq!(request_identity, recover.identity());
                assert_eq!(carriage, response.carriage().unwrap().clone());
            }
            other => panic!("unexpected service exit: {other:?}"),
        }
    }

    #[test]
    fn bounded_service_verifies_the_complete_exact_current_carriage() {
        let (mut issuer, client) = FakeIssuer::new();
        issuer.published = true;
        let policy = issuer.policy.clone();
        let carriage = issuer.recover(issuer.request.subject()).unwrap().unwrap();
        let handle =
            thread::spawn(move || serve_with_limits(&mut issuer, Duration::from_secs(1), 1));

        let request = CompilerExecutionServiceRequestV1::verify_current(
            &policy,
            carriage.clone(),
            [0xa1; 32],
        )
        .unwrap();
        send_raw(client.as_fd(), request.canonical_bytes());
        let response =
            CompilerExecutionServiceResponseV1::decode(&receive_raw(client.as_fd())).unwrap();
        assert_eq!(
            response.kind(),
            CompilerExecutionServiceResponseKindV1::VerifiedCurrent
        );
        let verification = response.current_record_verification().unwrap();
        assert_eq!(
            verification.carriage_identity(),
            *carriage.identity().as_bytes()
        );
        assert_eq!(
            verification.protected_policy_verification_identity(),
            [0x91; 32]
        );
        assert_eq!(
            verification.protected_worker_ledger_verification_identity(),
            [0x92; 32]
        );
        match handle.join().unwrap().unwrap() {
            CompilerExecutionServiceExitV1::VerifiedCurrent {
                request_identity,
                attestation: exited,
            } => {
                assert_eq!(request_identity, request.identity());
                assert_eq!(exited.verification(), verification);
                assert_eq!(exited.challenge(), [0xa1; 32]);
            }
            other => panic!("unexpected service exit: {other:?}"),
        }
    }

    #[test]
    fn absent_receipt_keeps_the_bounded_session_open() {
        let (mut issuer, client) = FakeIssuer::new();
        let policy = issuer.policy.clone();
        let subject = issuer.request.subject().clone();
        let handle =
            thread::spawn(move || serve_with_limits(&mut issuer, Duration::from_secs(1), 2));

        let recover = CompilerExecutionServiceRequestV1::recover(&policy, subject).unwrap();
        send_raw(client.as_fd(), recover.canonical_bytes());
        let response =
            CompilerExecutionServiceResponseV1::decode(&receive_raw(client.as_fd())).unwrap();
        assert_eq!(
            response.kind(),
            CompilerExecutionServiceResponseKindV1::ReceiptAbsent
        );
        assert_eq!(response.request_identity(), recover.identity());

        let cancel = CompilerExecutionServiceRequestV1::cancel(&policy);
        send_raw(client.as_fd(), cancel.canonical_bytes());
        let response =
            CompilerExecutionServiceResponseV1::decode(&receive_raw(client.as_fd())).unwrap();
        assert_eq!(
            response.kind(),
            CompilerExecutionServiceResponseKindV1::Cancelled
        );
        assert!(matches!(
            handle.join().unwrap().unwrap(),
            CompilerExecutionServiceExitV1::Cancelled { .. }
        ));
    }

    #[test]
    fn exact_request_and_response_packets_preserve_boundaries() {
        let (service, client) = socket_pair();
        let live = pidfd(std::process::id());
        let policy = policy();
        let request = CompilerExecutionServiceRequestV1::inspect(&policy);
        send_raw(client.as_fd(), request.canonical_bytes());
        let received = receive_packet(
            service.as_fd(),
            live.as_fd(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert_eq!(received.as_slice(), request.canonical_bytes());

        let response =
            CompilerExecutionServiceResponseV1::ready(request.identity(), &policy, 1, [0; 32])
                .unwrap();
        send_packet(
            service.as_fd(),
            live.as_fd(),
            response.canonical_bytes(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        let mut bytes = [0_u8; 1024];
        let received = unsafe {
            libc::recv(
                client.as_raw_fd(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                0,
            )
        };
        assert_eq!(received, response.canonical_bytes().len() as isize);
        assert_eq!(&bytes[..received as usize], response.canonical_bytes());
    }

    #[test]
    fn oversized_and_ancillary_packets_fail_closed() {
        let (service, client) = socket_pair();
        let live = pidfd(std::process::id());
        let oversized = vec![0_u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1 + 1];
        send_raw(client.as_fd(), &oversized);
        assert!(matches!(
            receive_packet(
                service.as_fd(),
                live.as_fd(),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(CompilerExecutionServiceErrorV1::PacketTruncated)
        ));

        let (service, client) = socket_pair();
        let pipe = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        send_one_descriptor(client.as_fd(), pipe.0.as_raw_fd());
        assert!(matches!(
            receive_packet(
                service.as_fd(),
                live.as_fd(),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(CompilerExecutionServiceErrorV1::AncillaryData)
        ));
    }

    #[test]
    fn absolute_deadline_and_client_pidfd_cancel_waits() {
        let (service, _client) = socket_pair();
        let live = pidfd(std::process::id());
        assert!(matches!(
            receive_packet(
                service.as_fd(),
                live.as_fd(),
                Instant::now() + Duration::from_millis(5)
            ),
            Err(CompilerExecutionServiceErrorV1::Timeout)
        ));

        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let child_pidfd = pidfd(child.id());
        child.kill().unwrap();
        assert!(matches!(
            receive_packet(
                service.as_fd(),
                child_pidfd.as_fd(),
                Instant::now() + Duration::from_secs(1)
            ),
            Err(CompilerExecutionServiceErrorV1::ClientExited)
        ));
        child.wait().unwrap();
    }

    #[test]
    fn blocked_response_send_obeys_the_same_absolute_deadline() {
        let (service, _client) = socket_pair();
        let live = pidfd(std::process::id());
        let bytes = [0x5a_u8; MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1];
        loop {
            let sent = unsafe {
                libc::send(
                    service.as_raw_fd(),
                    bytes.as_ptr().cast(),
                    bytes.len(),
                    libc::MSG_DONTWAIT | libc::MSG_NOSIGNAL,
                )
            };
            if sent < 0 {
                assert_eq!(io::Error::last_os_error().kind(), io::ErrorKind::WouldBlock);
                break;
            }
            assert_eq!(sent, bytes.len() as isize);
        }
        assert!(matches!(
            send_packet(
                service.as_fd(),
                live.as_fd(),
                &bytes,
                Instant::now() + Duration::from_millis(5)
            ),
            Err(CompilerExecutionServiceErrorV1::Timeout)
        ));
    }

    fn send_one_descriptor(socket: BorrowedFd<'_>, descriptor: RawFd) {
        let mut byte = [0x7f_u8];
        let mut vector = libc::iovec {
            iov_base: byte.as_mut_ptr().cast(),
            iov_len: byte.len(),
        };
        let mut control = [0_usize; 8];
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut vector;
        header.msg_iovlen = 1;
        header.msg_control = control.as_mut_ptr().cast();
        header.msg_controllen = unsafe { libc::CMSG_SPACE(mem::size_of::<RawFd>() as _) } as usize;
        unsafe {
            let message = libc::CMSG_FIRSTHDR(&header);
            (*message).cmsg_level = libc::SOL_SOCKET;
            (*message).cmsg_type = libc::SCM_RIGHTS;
            (*message).cmsg_len = libc::CMSG_LEN(mem::size_of::<RawFd>() as _) as usize;
            std::ptr::write_unaligned(libc::CMSG_DATA(message).cast::<RawFd>(), descriptor);
        }
        let sent = unsafe { libc::sendmsg(socket.as_raw_fd(), &header, libc::MSG_NOSIGNAL) };
        assert_eq!(sent, 1);
    }

    fn subject(seed: u8) -> InertCompilerExecutionSubjectV1 {
        let closure_pins = [
            [seed; 32],
            [seed + 1; 32],
            [seed + 2; 32],
            [seed + 3; 32],
            [seed + 4; 32],
            [seed + 5; 32],
        ];
        let mut closure_digest = Sha256::new();
        closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
        closure_digest.update(1_u16.to_le_bytes());
        for pin in closure_pins {
            closure_digest.update(pin);
        }
        let closure_identity: [u8; 32] = closure_digest.finalize().into();
        let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
        let mut offset = 0;
        fixture_put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        );
        fixture_put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
        );
        fixture_put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
        fixture_put(
            &mut bytes,
            &mut offset,
            &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
        );
        fixture_put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
        fixture_put(&mut bytes, &mut offset, &9_u64.to_le_bytes());
        fixture_put(&mut bytes, &mut offset, &[seed + 6; 16]);
        fixture_put(&mut bytes, &mut offset, &[seed + 7; 32]);
        bytes[offset] = 0;
        offset += 8;
        fixture_put(&mut bytes, &mut offset, &[seed + 8; 32]);
        fixture_put(&mut bytes, &mut offset, &[seed + 9; 32]);
        for pin in closure_pins {
            fixture_put(&mut bytes, &mut offset, &pin);
        }
        fixture_put(&mut bytes, &mut offset, &1_u16.to_le_bytes());
        fixture_put(&mut bytes, &mut offset, &closure_identity);
        for axis in 0_u8..7 {
            fixture_put(&mut bytes, &mut offset, &[seed + 10 + axis; 32]);
            fixture_put(
                &mut bytes,
                &mut offset,
                &(1_000_u64 + u64::from(axis)).to_le_bytes(),
            );
        }
        let identity = subject_digest(&bytes[..offset]);
        fixture_put(&mut bytes, &mut offset, &identity);
        assert_eq!(offset, bytes.len());
        InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
    }

    fn fixture_put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
        let end = *offset + value.len();
        output[*offset..end].copy_from_slice(value);
        *offset = end;
    }

    fn subject_digest(bytes: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(SUBJECT_IDENTITY_DOMAIN);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        digest.finalize().into()
    }
}
