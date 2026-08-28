#![doc = include_str!("../README.md")]

#[cfg(not(target_os = "linux"))]
compile_error!("fe2o3-compiler-execution-client requires Linux SOCK_SEQPACKET semantics");

use std::error::Error;
use std::fmt;
use std::io;
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationErrorV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, CompilerExecutionReceiptPublicationErrorV1,
    CompilerExecutionReceiptPublicationV1, CompilerExecutionServiceProtocolErrorV1,
    CompilerExecutionServiceRequestV1, CompilerExecutionServiceResponseKindV1,
    CompilerExecutionServiceResponseV1, MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1,
};

mod child_channel;
mod child_session;
mod receipt_return;
mod supervisor_handoff;

pub use child_channel::{
    CompilerExecutionChildChannelErrorV1, CompilerExecutionServiceLaunchV1,
    PendingCompilerExecutionChildChannelV1,
};
pub use child_session::{
    CompilerExecutionChildSessionErrorV1, CompilerExecutionChildSessionV1,
    CompletedCompilerExecutionChildSessionV1, PendingCompilerExecutionChildSessionV1,
    PendingCompilerExecutionChildSupervisorV1, ReadyCompilerExecutionChildSessionV1,
};
pub use fe2o3_compiler_execution_protocol::CompilerExecutionClientProcessIdentityV1;
pub use receipt_return::{
    COMPILER_EXECUTION_RECEIPT_RETURN_CHILD_FD_V1, CompilerExecutionReceiptReceiverV1,
    CompilerExecutionReceiptReturnErrorV1, CompilerExecutionReceiptSenderV1,
    PendingCompilerExecutionReceiptReturnV1,
};
pub use supervisor_handoff::{
    CompilerExecutionHandoffErrorV1, CompilerExecutionSupervisorCredentialsV1,
    PendingCompilerExecutionSupervisorV1,
};

/// Fixed rustc descriptor reserved for the compiler-execution service peer.
pub const COMPILER_EXECUTION_SERVICE_CHILD_FD_V1: i32 = 195;

/// Outcome of a recovery-only compiler-receipt session.
// Keep the complete bounded carriage inline so the authority boundary does not introduce a
// fallible heap allocation merely to report successful recovery.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum CompilerExecutionReceiptRecoveryV1 {
    /// The exact current Worker record was reacquired and returned in full.
    Recovered(CompilerExecutionReceiptCarriageV1),
    /// No canonical Worker record exists yet. This does not cover a different or damaged record.
    Absent {
        sequence: u64,
        rollback_anchor: [u8; 32],
    },
}

impl CompilerExecutionReceiptRecoveryV1 {
    /// Reports whether exact current-record recovery succeeded.
    pub const fn is_recovered(&self) -> bool {
        matches!(self, Self::Recovered(_))
    }

    /// Consumes a successful recovery result.
    pub fn into_carriage(self) -> Option<CompilerExecutionReceiptCarriageV1> {
        match self {
            Self::Recovered(carriage) => Some(carriage),
            Self::Absent { .. } => None,
        }
    }
}

/// One owned, bounded connection to the protected compiler-execution service.
///
/// Construction validates a connected unnamed Unix `SOCK_SEQPACKET` endpoint and enables
/// `FD_CLOEXEC`. Every exchange shares one absolute monotonic deadline. The value is deliberately
/// move-only and closes the peer on drop.
pub struct CompilerExecutionClientV1 {
    peer: OwnedFd,
    deadline: Instant,
}

impl fmt::Debug for CompilerExecutionClientV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionClientV1")
            .field("authority", &"none")
            .field("deadline", &self.deadline)
            .finish_non_exhaustive()
    }
}

impl CompilerExecutionClientV1 {
    /// Admits and removes the exact compiler-service endpoint inherited by rustc at fixed FD 195.
    pub fn admit_inherited_child(
        timeout: Duration,
    ) -> Result<Self, CompilerExecutionClientErrorV1> {
        // SAFETY: F_GETFD consumes only the fixed scalar descriptor.
        let flags = unsafe { libc::fcntl(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, libc::F_GETFD) };
        if flags < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EBADF) {
                return Err(CompilerExecutionClientErrorV1::MissingInheritedPeer);
            }
            return Err(CompilerExecutionClientErrorV1::Descriptor(error));
        }
        if flags & libc::FD_CLOEXEC != 0 {
            return Err(CompilerExecutionClientErrorV1::InheritedPeerCloseOnExec);
        }
        // SAFETY: F_DUPFD_CLOEXEC returns one independent descriptor or reports failure.
        let retained = unsafe {
            libc::fcntl(
                COMPILER_EXECUTION_SERVICE_CHILD_FD_V1,
                libc::F_DUPFD_CLOEXEC,
                3,
            )
        };
        if retained < 0 {
            return Err(CompilerExecutionClientErrorV1::Descriptor(
                io::Error::last_os_error(),
            ));
        }
        // SAFETY: the fixed inherited descriptor is consumed exactly once by this operation.
        if unsafe { libc::close(COMPILER_EXECUTION_SERVICE_CHILD_FD_V1) } != 0 {
            let error = io::Error::last_os_error();
            // SAFETY: successful duplication returned one owned descriptor not yet wrapped.
            unsafe { libc::close(retained) };
            return Err(CompilerExecutionClientErrorV1::Descriptor(error));
        }
        // SAFETY: successful F_DUPFD_CLOEXEC returned one newly owned descriptor.
        Self::admit(unsafe { OwnedFd::from_raw_fd(retained) }, timeout)
    }

    /// Admits one owned connected peer and fixes the absolute deadline for its complete session.
    pub fn admit(peer: OwnedFd, timeout: Duration) -> Result<Self, CompilerExecutionClientErrorV1> {
        if timeout.is_zero() {
            return Err(CompilerExecutionClientErrorV1::InvalidTimeout);
        }
        set_close_on_exec(&peer)?;
        validate_seqpacket_peer(&peer)?;
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(CompilerExecutionClientErrorV1::DeadlineOverflow)?;
        Ok(Self { peer, deadline })
    }

    /// Recovers the exact current carriage or returns canonical absence, then terminates cleanly.
    pub fn recover_only(
        self,
        policy: &CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
    ) -> Result<CompilerExecutionReceiptRecoveryV1, CompilerExecutionClientErrorV1> {
        let recovery = self.recover_once(policy, subject)?;
        match recovery {
            RecoveryStepV1::Recovered(carriage) => {
                Ok(CompilerExecutionReceiptRecoveryV1::Recovered(carriage))
            }
            RecoveryStepV1::Absent {
                sequence,
                rollback_anchor,
            } => {
                let cancel = CompilerExecutionServiceRequestV1::cancel(policy);
                let response = self.exchange(policy, &cancel)?;
                require_kind(&response, CompilerExecutionServiceResponseKindV1::Cancelled)?;
                if response.sequence() != sequence || response.rollback_anchor() != rollback_anchor
                {
                    return Err(CompilerExecutionClientErrorV1::DurableStateChanged);
                }
                Ok(CompilerExecutionReceiptRecoveryV1::Absent {
                    sequence,
                    rollback_anchor,
                })
            }
        }
    }

    /// Recovers or completes one exact compiler receipt and returns its full carriage.
    ///
    /// The legal packet sequence is `Recover`; then, only after `ReceiptAbsent`, `Inspect` and the
    /// minimum suffix of `Prepare`, `Issue`, and `Publish` required by durable issuer state.
    pub fn acquire(
        self,
        policy: &CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
    ) -> Result<CompilerExecutionReceiptCarriageV1, CompilerExecutionClientErrorV1> {
        let absent_position = match self.recover_once(policy, subject.clone())? {
            RecoveryStepV1::Recovered(carriage) => return Ok(carriage),
            RecoveryStepV1::Absent {
                sequence,
                rollback_anchor,
            } => (sequence, rollback_anchor),
        };

        let inspect = CompilerExecutionServiceRequestV1::inspect(policy);
        let inspected = self.exchange(policy, &inspect)?;
        if (inspected.sequence(), inspected.rollback_anchor()) != absent_position {
            return Err(CompilerExecutionClientErrorV1::DurableStateChanged);
        }

        let (request, publication) = match inspected.kind() {
            CompilerExecutionServiceResponseKindV1::Ready => {
                let prepare = CompilerExecutionServiceRequestV1::prepare(
                    policy,
                    inspected.sequence(),
                    inspected.rollback_anchor(),
                )?;
                let prepared = self.exchange(policy, &prepare)?;
                require_kind(&prepared, CompilerExecutionServiceResponseKindV1::Prepared)?;
                if prepared.sequence() != inspected.sequence()
                    || prepared.rollback_anchor() != inspected.rollback_anchor()
                {
                    return Err(CompilerExecutionClientErrorV1::DurableStateChanged);
                }
                let challenge = prepared
                    .challenge()
                    .cloned()
                    .ok_or(CompilerExecutionClientErrorV1::MissingPayload("challenge"))?;
                self.issue_from_challenge(policy, subject, challenge)?
            }
            CompilerExecutionServiceResponseKindV1::Prepared => {
                let challenge = inspected
                    .challenge()
                    .cloned()
                    .ok_or(CompilerExecutionClientErrorV1::MissingPayload("challenge"))?;
                self.issue_from_challenge(policy, subject, challenge)?
            }
            CompilerExecutionServiceResponseKindV1::Issued => {
                let publication = inspected.publication().cloned().ok_or(
                    CompilerExecutionClientErrorV1::MissingPayload("receipt publication"),
                )?;
                let request = reconstruct_issued_request(policy, subject, &publication)?;
                (request, publication)
            }
            actual => {
                return Err(CompilerExecutionClientErrorV1::UnexpectedResponse {
                    expected: "Ready, Prepared, or Issued",
                    actual,
                });
            }
        };

        let publish = CompilerExecutionServiceRequestV1::publish(
            policy,
            request.clone(),
            publication.clone(),
        )?;
        let published = self.exchange(policy, &publish)?;
        require_kind(
            &published,
            CompilerExecutionServiceResponseKindV1::Published,
        )?;
        let acknowledgment = published.acknowledgment().cloned().ok_or(
            CompilerExecutionClientErrorV1::MissingPayload("publication acknowledgment"),
        )?;
        acknowledgment.matches_publication(&publication)?;
        CompilerExecutionReceiptCarriageV1::new(
            policy.clone(),
            request,
            publication,
            acknowledgment,
        )
        .map_err(Into::into)
    }

    fn recover_once(
        &self,
        policy: &CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
    ) -> Result<RecoveryStepV1, CompilerExecutionClientErrorV1> {
        let request = CompilerExecutionServiceRequestV1::recover(policy, subject.clone())?;
        let response = self.exchange(policy, &request)?;
        match response.kind() {
            CompilerExecutionServiceResponseKindV1::Recovered => {
                let carriage = response.carriage().cloned().ok_or(
                    CompilerExecutionClientErrorV1::MissingPayload("receipt carriage"),
                )?;
                if carriage.policy() != policy || carriage.request().subject() != &subject {
                    return Err(CompilerExecutionClientErrorV1::SubjectOrPolicyMismatch);
                }
                Ok(RecoveryStepV1::Recovered(carriage))
            }
            CompilerExecutionServiceResponseKindV1::ReceiptAbsent => Ok(RecoveryStepV1::Absent {
                sequence: response.sequence(),
                rollback_anchor: response.rollback_anchor(),
            }),
            actual => Err(CompilerExecutionClientErrorV1::UnexpectedResponse {
                expected: "Recovered or ReceiptAbsent",
                actual,
            }),
        }
    }

    fn issue_from_challenge(
        &self,
        policy: &CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
        challenge: CompilerExecutionAttestationChallengeV1,
    ) -> Result<
        (
            CompilerExecutionAttestationRequestV1,
            CompilerExecutionReceiptPublicationV1,
        ),
        CompilerExecutionClientErrorV1,
    > {
        if challenge.policy_identity() != policy.identity()
            || !challenge.subject().matches_subject(&subject)
        {
            return Err(CompilerExecutionClientErrorV1::SubjectOrPolicyMismatch);
        }
        let request = CompilerExecutionAttestationRequestV1::new(challenge, subject)?;
        let issue = CompilerExecutionServiceRequestV1::issue(policy, request.clone())?;
        let issued = self.exchange(policy, &issue)?;
        require_kind(&issued, CompilerExecutionServiceResponseKindV1::Issued)?;
        let publication =
            issued
                .publication()
                .cloned()
                .ok_or(CompilerExecutionClientErrorV1::MissingPayload(
                    "receipt publication",
                ))?;
        verify_publication(policy, &request, &publication)?;
        Ok((request, publication))
    }

    fn exchange(
        &self,
        policy: &CompilerExecutionIssuerPolicyV1,
        request: &CompilerExecutionServiceRequestV1,
    ) -> Result<CompilerExecutionServiceResponseV1, CompilerExecutionClientErrorV1> {
        send_packet(&self.peer, request.canonical_bytes(), self.deadline)?;
        let received = receive_packet(&self.peer, self.deadline)?;
        let response = CompilerExecutionServiceResponseV1::decode(received.as_slice())?;
        if response.request_identity() != request.identity() {
            return Err(CompilerExecutionClientErrorV1::RequestIdentityMismatch);
        }
        if response.policy_identity() != policy.identity() {
            return Err(CompilerExecutionClientErrorV1::SubjectOrPolicyMismatch);
        }
        Ok(response)
    }
}

// This private step preserves the same allocation-free carriage ownership as the public result.
#[allow(clippy::large_enum_variant)]
enum RecoveryStepV1 {
    Recovered(CompilerExecutionReceiptCarriageV1),
    Absent {
        sequence: u64,
        rollback_anchor: [u8; 32],
    },
}

fn reconstruct_issued_request(
    policy: &CompilerExecutionIssuerPolicyV1,
    subject: InertCompilerExecutionSubjectV1,
    publication: &CompilerExecutionReceiptPublicationV1,
) -> Result<CompilerExecutionAttestationRequestV1, CompilerExecutionClientErrorV1> {
    let receipt = publication.receipt();
    if publication.policy_identity() != policy.identity()
        || !receipt.subject().matches_subject(&subject)
    {
        return Err(CompilerExecutionClientErrorV1::SubjectOrPolicyMismatch);
    }
    let challenge = CompilerExecutionAttestationChallengeV1::new(
        policy,
        &subject,
        receipt.challenge_nonce(),
        receipt.sequence(),
        receipt.prior_rollback_anchor(),
    )?;
    if challenge.identity() != receipt.challenge_identity() {
        return Err(CompilerExecutionClientErrorV1::IssuedStateMismatch);
    }
    let request = CompilerExecutionAttestationRequestV1::new(challenge, subject)?;
    verify_publication(policy, &request, publication)?;
    Ok(request)
}

fn verify_publication(
    policy: &CompilerExecutionIssuerPolicyV1,
    request: &CompilerExecutionAttestationRequestV1,
    publication: &CompilerExecutionReceiptPublicationV1,
) -> Result<(), CompilerExecutionClientErrorV1> {
    if publication.policy_identity() != policy.identity()
        || publication.receipt().request_sha256() != request.identity().as_bytes()
        || publication.receipt().challenge_identity() != request.challenge().identity()
    {
        return Err(CompilerExecutionClientErrorV1::IssuedStateMismatch);
    }
    publication.receipt().clone().verify(
        policy,
        request,
        request.challenge().prior_rollback_anchor(),
    )?;
    Ok(())
}

fn require_kind(
    response: &CompilerExecutionServiceResponseV1,
    expected: CompilerExecutionServiceResponseKindV1,
) -> Result<(), CompilerExecutionClientErrorV1> {
    if response.kind() != expected {
        return Err(CompilerExecutionClientErrorV1::UnexpectedResponse {
            expected: match expected {
                CompilerExecutionServiceResponseKindV1::Ready => "Ready",
                CompilerExecutionServiceResponseKindV1::Prepared => "Prepared",
                CompilerExecutionServiceResponseKindV1::Issued => "Issued",
                CompilerExecutionServiceResponseKindV1::Published => "Published",
                CompilerExecutionServiceResponseKindV1::Cancelled => "Cancelled",
                CompilerExecutionServiceResponseKindV1::Recovered => "Recovered",
                CompilerExecutionServiceResponseKindV1::ReceiptAbsent => "ReceiptAbsent",
            },
            actual: response.kind(),
        });
    }
    Ok(())
}

fn set_close_on_exec(peer: &OwnedFd) -> Result<(), CompilerExecutionClientErrorV1> {
    // SAFETY: `peer` is a live owned descriptor and F_GETFD has no pointer arguments.
    let flags = unsafe { libc::fcntl(peer.as_raw_fd(), libc::F_GETFD) };
    if flags < 0 {
        return Err(CompilerExecutionClientErrorV1::Descriptor(
            io::Error::last_os_error(),
        ));
    }
    // SAFETY: F_SETFD consumes only the scalar descriptor flags supplied here.
    if unsafe { libc::fcntl(peer.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } != 0 {
        return Err(CompilerExecutionClientErrorV1::Descriptor(
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

fn validate_seqpacket_peer(peer: &OwnedFd) -> Result<(), CompilerExecutionClientErrorV1> {
    let mut socket_type = 0_i32;
    let mut socket_type_len = mem::size_of::<i32>() as libc::socklen_t;
    // SAFETY: the output pointers name initialized writable scalar storage of the declared length.
    if unsafe {
        libc::getsockopt(
            peer.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_TYPE,
            (&mut socket_type as *mut i32).cast(),
            &mut socket_type_len,
        )
    } != 0
    {
        return Err(CompilerExecutionClientErrorV1::Descriptor(
            io::Error::last_os_error(),
        ));
    }
    if socket_type_len as usize != mem::size_of::<i32>() || socket_type != libc::SOCK_SEQPACKET {
        return Err(CompilerExecutionClientErrorV1::NotSeqpacket);
    }
    validate_unnamed_address(peer, false)?;
    validate_unnamed_address(peer, true)
}

fn validate_unnamed_address(
    peer: &OwnedFd,
    remote: bool,
) -> Result<(), CompilerExecutionClientErrorV1> {
    // SAFETY: zero is a valid initial sockaddr_storage representation and the kernel writes the
    // returned address and length before they are inspected.
    let mut address = unsafe { mem::zeroed::<libc::sockaddr_storage>() };
    let mut length = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    // SAFETY: the address and length pointers name writable storage of the declared capacity.
    let result = unsafe {
        if remote {
            libc::getpeername(
                peer.as_raw_fd(),
                (&mut address as *mut libc::sockaddr_storage).cast(),
                &mut length,
            )
        } else {
            libc::getsockname(
                peer.as_raw_fd(),
                (&mut address as *mut libc::sockaddr_storage).cast(),
                &mut length,
            )
        }
    };
    if result != 0 {
        let error = io::Error::last_os_error();
        if remote && error.raw_os_error() == Some(libc::ENOTCONN) {
            return Err(CompilerExecutionClientErrorV1::NamedOrNonUnixPeer);
        }
        return Err(CompilerExecutionClientErrorV1::Descriptor(error));
    }
    if address.ss_family as i32 != libc::AF_UNIX
        || length as usize != mem::size_of::<libc::sa_family_t>()
    {
        return Err(CompilerExecutionClientErrorV1::NamedOrNonUnixPeer);
    }
    Ok(())
}

fn send_packet(
    peer: &OwnedFd,
    bytes: &[u8],
    deadline: Instant,
) -> Result<(), CompilerExecutionClientErrorV1> {
    loop {
        wait_for_peer(peer, libc::POLLOUT, deadline)?;
        // SAFETY: `bytes` is readable for its complete length and `peer` remains owned throughout.
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
            return Err(CompilerExecutionClientErrorV1::Send(error));
        }
        if usize::try_from(sent).ok() != Some(bytes.len()) {
            return Err(CompilerExecutionClientErrorV1::PartialSend);
        }
        return Ok(());
    }
}

fn receive_packet(
    peer: &OwnedFd,
    deadline: Instant,
) -> Result<ReceivedPacketV1, CompilerExecutionClientErrorV1> {
    let mut bytes = [0_u8; MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1];
    loop {
        wait_for_peer(peer, libc::POLLIN, deadline)?;
        let mut vector = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        // SAFETY: an all-zero msghdr is a valid empty header and its iovec is installed below.
        let mut header = unsafe { mem::zeroed::<libc::msghdr>() };
        header.msg_iov = &mut vector;
        header.msg_iovlen = 1;
        // SAFETY: the header names the live stack buffer through one initialized iovec and no
        // ancillary buffer. The owned peer remains valid for the duration of the call.
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
            return Err(CompilerExecutionClientErrorV1::Receive(error));
        }
        if header.msg_flags & libc::MSG_CTRUNC != 0 {
            return Err(CompilerExecutionClientErrorV1::AncillaryData);
        }
        if header.msg_flags & libc::MSG_TRUNC != 0 {
            return Err(CompilerExecutionClientErrorV1::PacketTruncated);
        }
        let received = usize::try_from(received)
            .map_err(|_| CompilerExecutionClientErrorV1::PacketTruncated)?;
        if received == 0 {
            return Err(CompilerExecutionClientErrorV1::PeerClosed);
        }
        return Ok(ReceivedPacketV1 {
            bytes,
            len: received,
        });
    }
}

struct ReceivedPacketV1 {
    bytes: [u8; MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V1],
    len: usize,
}

impl ReceivedPacketV1 {
    fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

fn wait_for_peer(
    peer: &OwnedFd,
    wanted: i16,
    deadline: Instant,
) -> Result<(), CompilerExecutionClientErrorV1> {
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(CompilerExecutionClientErrorV1::Timeout);
        }
        let mut descriptor = libc::pollfd {
            fd: peer.as_raw_fd(),
            events: wanted | libc::POLLERR | libc::POLLHUP,
            revents: 0,
        };
        // SAFETY: `descriptor` is a live one-element pollfd array for the complete call.
        let result = unsafe { libc::poll(&mut descriptor, 1, duration_to_poll_millis(remaining)) };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(CompilerExecutionClientErrorV1::Poll(error));
        }
        if result == 0 || deadline.saturating_duration_since(Instant::now()).is_zero() {
            return Err(CompilerExecutionClientErrorV1::Timeout);
        }
        if descriptor.revents & libc::POLLNVAL != 0 {
            return Err(CompilerExecutionClientErrorV1::InvalidPeer);
        }
        if descriptor.revents & wanted != 0 {
            return Ok(());
        }
        if descriptor.revents & libc::POLLERR != 0 {
            return Err(CompilerExecutionClientErrorV1::PeerFailed);
        }
        if descriptor.revents & libc::POLLHUP != 0 {
            return Err(CompilerExecutionClientErrorV1::PeerClosed);
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

/// Bounded client admission, transport, correlation, or state-machine failure.
#[derive(Debug)]
pub enum CompilerExecutionClientErrorV1 {
    InvalidTimeout,
    DeadlineOverflow,
    MissingInheritedPeer,
    InheritedPeerCloseOnExec,
    Descriptor(io::Error),
    NotSeqpacket,
    NamedOrNonUnixPeer,
    Poll(io::Error),
    Send(io::Error),
    Receive(io::Error),
    Timeout,
    InvalidPeer,
    PeerFailed,
    PeerClosed,
    PacketTruncated,
    AncillaryData,
    PartialSend,
    Protocol(CompilerExecutionServiceProtocolErrorV1),
    Attestation(CompilerExecutionAttestationErrorV1),
    Publication(CompilerExecutionReceiptPublicationErrorV1),
    RequestIdentityMismatch,
    SubjectOrPolicyMismatch,
    DurableStateChanged,
    IssuedStateMismatch,
    MissingPayload(&'static str),
    UnexpectedResponse {
        expected: &'static str,
        actual: CompilerExecutionServiceResponseKindV1,
    },
}

impl fmt::Display for CompilerExecutionClientErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimeout => formatter.write_str("compiler service timeout must be nonzero"),
            Self::DeadlineOverflow => formatter.write_str("compiler service deadline overflowed"),
            Self::MissingInheritedPeer => formatter.write_str(
                "rustc child has no inherited compiler-service peer at fixed descriptor 195",
            ),
            Self::InheritedPeerCloseOnExec => formatter
                .write_str("inherited rustc compiler-service peer is unexpectedly close-on-exec"),
            Self::Descriptor(error) => {
                write!(formatter, "compiler service peer is invalid: {error}")
            }
            Self::NotSeqpacket => {
                formatter.write_str("compiler service peer is not SOCK_SEQPACKET")
            }
            Self::NamedOrNonUnixPeer => {
                formatter.write_str("compiler service peer is not a connected unnamed Unix socket")
            }
            Self::Poll(error) => write!(formatter, "compiler service poll failed: {error}"),
            Self::Send(error) => write!(formatter, "compiler service send failed: {error}"),
            Self::Receive(error) => write!(formatter, "compiler service receive failed: {error}"),
            Self::Timeout => formatter.write_str("compiler service absolute deadline expired"),
            Self::InvalidPeer => formatter.write_str("compiler service peer descriptor is invalid"),
            Self::PeerFailed => formatter.write_str("compiler service peer reported an error"),
            Self::PeerClosed => formatter.write_str("compiler service peer closed"),
            Self::PacketTruncated => formatter.write_str("compiler service packet was truncated"),
            Self::AncillaryData => formatter.write_str("compiler service sent ancillary data"),
            Self::PartialSend => formatter.write_str("compiler service packet send was partial"),
            Self::Protocol(error) => write!(formatter, "compiler service protocol failed: {error}"),
            Self::Attestation(error) => {
                write!(formatter, "compiler receipt attestation failed: {error}")
            }
            Self::Publication(error) => {
                write!(formatter, "compiler receipt publication failed: {error}")
            }
            Self::RequestIdentityMismatch => {
                formatter.write_str("compiler service response names another request")
            }
            Self::SubjectOrPolicyMismatch => {
                formatter.write_str("compiler service subject or pinned policy changed")
            }
            Self::DurableStateChanged => {
                formatter.write_str("compiler service durable state changed within one session")
            }
            Self::IssuedStateMismatch => {
                formatter.write_str("compiler service issued state cannot reconstruct its request")
            }
            Self::MissingPayload(payload) => {
                write!(formatter, "compiler service response is missing {payload}")
            }
            Self::UnexpectedResponse { expected, actual } => {
                write!(
                    formatter,
                    "expected compiler service {expected}; received {actual:?}"
                )
            }
        }
    }
}

impl Error for CompilerExecutionClientErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Descriptor(error)
            | Self::Poll(error)
            | Self::Send(error)
            | Self::Receive(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::Attestation(error) => Some(error),
            Self::Publication(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerExecutionServiceProtocolErrorV1> for CompilerExecutionClientErrorV1 {
    fn from(error: CompilerExecutionServiceProtocolErrorV1) -> Self {
        Self::Protocol(error)
    }
}

impl From<CompilerExecutionAttestationErrorV1> for CompilerExecutionClientErrorV1 {
    fn from(error: CompilerExecutionAttestationErrorV1) -> Self {
        Self::Attestation(error)
    }
}

impl From<CompilerExecutionReceiptPublicationErrorV1> for CompilerExecutionClientErrorV1 {
    fn from(error: CompilerExecutionReceiptPublicationErrorV1) -> Self {
        Self::Publication(error)
    }
}
