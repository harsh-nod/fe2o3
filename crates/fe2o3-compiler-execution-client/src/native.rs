//! Consuming native exchanges on the existing inherited service channel.
//! The endpoint response is cryptographic evidence, not protected-key or GPU authority.
use super::{
    CompilerExecutionClientErrorV1 as TransportError, receive_packet_with_io, send_packet_with_io,
    set_close_on_exec, validate_seqpacket_peer,
};
use fe2o3_artifact_transaction::{
    InertCompilerExecutionSubjectStorageV2 as SubjectStorage,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationChallengeV2 as Challenge,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionAttestationStorageV2 as Storage, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionNativeJournalErrorV2 as JournalError,
    CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionReceiptPublicationErrorV2 as PublicationError,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServiceProtocolErrorV2 as ProtocolError,
    CompilerExecutionServiceRequestPayloadV2 as Payload,
    CompilerExecutionServiceRequestV2 as ServiceRequest,
    CompilerExecutionServiceResponseKindV2 as Kind, CompilerExecutionServiceResponseV2 as Response,
    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V2 as MAX_REQUEST,
    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_BYTES_V2 as MAX_RESPONSE,
    VerifiedCompilerExecutionCurrentRecordV3 as VerifiedCurrent,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    os::fd::OwnedFd,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub enum CompilerExecutionClientErrorV2 {
    Resource(Resource),
    Transport(TransportError),
    Protocol(ProtocolError),
    Attestation(AttestationError),
    Publication(PublicationError),
    Journal(JournalError),
    Mismatch(&'static str),
}
type ClientError = CompilerExecutionClientErrorV2;
type Result<T> = std::result::Result<T, ClientError>;

/// Full logical output charge, returned unreserved on the original ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionClientStorageV2(usize);
impl CompilerExecutionClientStorageV2 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
macro_rules! from_error {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for ClientError {
            fn from(error: $ty) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(TransportError, Transport);
from_error!(ProtocolError, Protocol);
from_error!(AttestationError, Attestation);
from_error!(PublicationError, Publication);
from_error!(JournalError, Journal);
impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch(reason) => f.write_str(reason),
            Self::Resource(e) => fmt::Display::fmt(e, f),
            Self::Transport(e) => fmt::Display::fmt(e, f),
            Self::Protocol(e) => fmt::Display::fmt(e, f),
            Self::Attestation(e) => fmt::Display::fmt(e, f),
            Self::Publication(e) => fmt::Display::fmt(e, f),
            Self::Journal(e) => fmt::Display::fmt(e, f),
        }
    }
}
impl Error for ClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Transport(e) => Some(e),
            Self::Protocol(e) => Some(e),
            Self::Attestation(e) => Some(e),
            Self::Publication(e) => Some(e),
            Self::Journal(e) => Some(e),
            Self::Mismatch(_) => None,
        }
    }
}

/// One terminal native session, exclusively borrowing the original ledger.
/// Policy, subject/carriage, and peer input storage must be prepaid before
/// admission. The peer charge transfers to this owner and is released on drop.
/// Native packets are never retried as V1 and no decoded V1 issuer owner is used.
/// This diagnostic API does not activate the production issuer or prove its
/// protected key custody, live-rustc observation, durability, or GPU authority.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_client::CompilerExecutionClientV2 as Client;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn replace<'a>(peer: std::os::fd::OwnedFd, b: &'a mut Budget<'a>, other: Budget<'a>) {
///     let client = Client::admit(peer, std::time::Duration::from_secs(1), b).unwrap();
///     *b = other;
///     drop(client);
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_client::CompilerExecutionClientV2;
/// fn duplicate<T: Clone>() {}
/// duplicate::<CompilerExecutionClientV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_client::CompilerExecutionClientV2;
/// fn expose<T: std::os::fd::AsFd>() {}
/// expose::<CompilerExecutionClientV2<'static, 'static>>();
/// ```
pub struct CompilerExecutionClientV2<'budget, 'work> {
    peer: Option<OwnedFd>,
    deadline: Instant,
    budget: &'budget mut Budget<'work>,
    retained: usize,
}

impl<'budget, 'work> CompilerExecutionClientV2<'budget, 'work> {
    pub const PEER_STORAGE: usize = size_of::<OwnedFd>();
    pub const SUBJECT_STORAGE: usize = size_of::<Subject>() + size_of::<SubjectStorage>();
    const RETAINED: usize = size_of::<Self>();
    const SESSION_SCRATCH: usize = 16 * MAX_RESPONSE + 8 * MAX_REQUEST + 8192;

    /// Consumes a prepaid unnamed connected SOCK_SEQPACKET peer.
    /// On admission failure the descriptor is closed but its input reservation
    /// remains caller-owned. Successful admission releases that charge on drop.
    pub fn admit(
        peer: OwnedFd,
        timeout: Duration,
        budget: &'budget mut Budget<'work>,
    ) -> Result<Self> {
        let deadline = budget.with_prepaid_scope(Self::PEER_STORAGE, 8, 64 * 1024, 8192, |_| {
            if timeout.is_zero() || timeout > Duration::from_secs(300) {
                return Err(ClientError::Transport(TransportError::InvalidTimeout));
            }
            set_close_on_exec(&peer)?;
            validate_seqpacket_peer(&peer)?;
            Instant::now()
                .checked_add(timeout)
                .ok_or_else(|| TransportError::DeadlineOverflow.into())
        })?;
        budget.reserve_storage(Self::RETAINED - Self::PEER_STORAGE)?;
        Ok(Self {
            peer: Some(peer),
            deadline,
            budget,
            retained: Self::RETAINED,
        })
    }

    /// Executes Recover/Inspect and the required Prepare/Issue/Publish suffix.
    /// The subject is an expected input, never evidence that this process compiled it.
    /// Returns a full carriage charge; retire the consumed subject's charge separately.
    pub fn acquire(
        self,
        policy: &Policy,
        subject: Subject,
    ) -> Result<(Carriage, CompilerExecutionClientStorageV2)> {
        let floor = input_floor(
            self.retained,
            policy.retained_storage(),
            Self::SUBJECT_STORAGE,
        )?;
        let peer = self
            .peer
            .as_ref()
            .ok_or(ClientError::Mismatch("closed native peer"))?;
        let deadline = self.deadline;
        let carriage = self
            .budget
            .with_prepaid_scope(floor, 8, 8, Self::SESSION_SCRATCH, |b| {
                Session::new(peer, deadline, b).acquire(policy, subject)
            })?;
        let charge = CompilerExecutionClientStorageV2(carriage.retained_storage());
        Ok((carriage, charge))
    }

    /// Consumes a ready session by exchanging one exact acknowledged Cancel.
    /// This does not authenticate readiness or a compiler result. The caller
    /// must first validate the supervisor's readiness/launch binding. Policy
    /// and request joins, framing, deadline and all I/O use the retained budget.
    pub fn cancel(self, policy: &Policy) -> Result<()> {
        let floor = input_floor(self.retained, policy.retained_storage(), 0)?;
        let peer = self
            .peer
            .as_ref()
            .ok_or(ClientError::Mismatch("closed native peer"))?;
        let deadline = self.deadline;
        self.budget
            .with_prepaid_scope(floor, 8, 8, Self::SESSION_SCRATCH, |b| {
                let response = Session::new(peer, deadline, b).exchange(policy, Payload::Cancel)?;
                require_kind(&response, Kind::Cancelled)
            })
    }

    /// Generates a fresh OS challenge and authenticates both signatures over the
    /// exact native carriage. This is terminal even when the peer refuses it.
    pub fn verify_current_only(
        self,
        policy: &Policy,
        carriage: &Carriage,
    ) -> Result<(VerifiedCurrent, CompilerExecutionClientStorageV2)> {
        let floor = input_floor(
            self.retained,
            policy.retained_storage(),
            carriage.retained_storage(),
        )?;
        let peer = self
            .peer
            .as_ref()
            .ok_or(ClientError::Mismatch("closed native peer"))?;
        let deadline = self.deadline;
        self.budget
            .with_prepaid_scope(floor, 8, 8, Self::SESSION_SCRATCH, |b| {
                let mut session = Session::new(peer, deadline, b);
                if policy.canonical_bytes() != carriage.policy().canonical_bytes() {
                    return Err(ClientError::Mismatch("native currentness policy mismatch"));
                }
                let challenge =
                    super::fresh_verification_challenge_with_io(&mut || session.attempt())?;
                let response = session.exchange(
                    policy,
                    Payload::VerifyCurrent {
                        carriage,
                        verification_challenge: *challenge.as_bytes(),
                    },
                )?;
                require_kind(&response, Kind::VerifiedCurrent)?;
                let attestation = retain(
                    response.decode_current_record_attestation(session.budget)?,
                    session.budget,
                )?;
                let (verified, growth) = attestation.verify_native(
                    policy,
                    carriage,
                    *challenge.as_bytes(),
                    session.budget,
                )?;
                session
                    .budget
                    .reserve_storage(growth.additional_storage())?;
                Ok((
                    verified,
                    CompilerExecutionClientStorageV2(size_of::<(VerifiedCurrent, Storage)>()),
                ))
            })
    }
}

impl Drop for CompilerExecutionClientV2<'_, '_> {
    fn drop(&mut self) {
        drop(self.peer.take());
        // Exclusive ledger custody and restoring scopes preserve this reservation.
        let _ = self.budget.release_storage(self.retained);
    }
}

struct Session<'a, 'work> {
    peer: &'a OwnedFd,
    deadline: Instant,
    budget: &'a mut Budget<'work>,
    packets: usize,
    attempts: usize,
}
impl<'a, 'work> Session<'a, 'work> {
    fn new(peer: &'a OwnedFd, deadline: Instant, budget: &'a mut Budget<'work>) -> Self {
        Self {
            peer,
            deadline,
            budget,
            packets: 0,
            attempts: 0,
        }
    }
    fn attempt(&mut self) -> Result<()> {
        self.budget
            .charge_work(1024 + MAX_RESPONSE.max(MAX_REQUEST))?;
        self.attempts = self.attempts.checked_add(1).ok_or(Resource::Arithmetic)?;
        if self.attempts > 256 {
            return Err(ClientError::Mismatch("native client I/O attempt limit"));
        }
        Ok(())
    }
    fn exchange(&mut self, policy: &Policy, payload: Payload<'_>) -> Result<Response> {
        self.budget.charge_work(8)?;
        if self.packets == 8 {
            return Err(ClientError::Mismatch("native client packet limit"));
        }
        self.packets += 1;
        let request = retain(
            ServiceRequest::new(policy, payload, self.budget)?,
            self.budget,
        )?;
        let peer = self.peer;
        let deadline = self.deadline;
        send_packet_with_io(peer, request.canonical_bytes(), deadline, &mut || {
            self.attempt()
        })?;
        let packet =
            receive_packet_with_io::<MAX_RESPONSE, ClientError>(peer, deadline, &mut || {
                self.attempt()
            })?;
        let response = retain(
            Response::decode(packet.as_slice(), self.budget)?,
            self.budget,
        )?;
        if response.policy_identity() != policy.identity()
            || response.request_identity() != request.identity()
        {
            return Err(ClientError::Mismatch(
                "native response policy or request identity mismatch",
            ));
        }
        Ok(response)
    }

    fn acquire(&mut self, policy: &Policy, subject: Subject) -> Result<Carriage> {
        let recovered = self.exchange(policy, Payload::Recover(&subject))?;
        match recovered.kind() {
            Kind::Recovered => {
                let carriage = retain(recovered.decode_carriage(self.budget)?, self.budget)?;
                if carriage.policy().canonical_bytes() != policy.canonical_bytes()
                    || carriage.request().subject().canonical_bytes() != subject.canonical_bytes()
                {
                    return Err(ClientError::Mismatch(
                        "recovered native carriage differs from expected source",
                    ));
                }
                return Ok(carriage);
            }
            Kind::ReceiptAbsent => (),
            _ => {
                return Err(ClientError::Mismatch(
                    "expected native recovered carriage or absence",
                ));
            }
        }
        let position = (recovered.sequence(), recovered.rollback_anchor());
        let inspected = self.exchange(policy, Payload::Inspect)?;
        if (inspected.sequence(), inspected.rollback_anchor()) != position {
            return Err(ClientError::Mismatch(
                "native durable position changed after absence",
            ));
        }
        let (request, publication) = match inspected.kind() {
            Kind::Ready | Kind::Prepared => {
                let prepared = if inspected.kind() == Kind::Ready {
                    self.exchange(
                        policy,
                        Payload::Prepare {
                            sequence: position.0,
                            prior_rollback_anchor: position.1,
                        },
                    )?
                } else {
                    inspected
                };
                require_kind(&prepared, Kind::Prepared)?;
                if (prepared.sequence(), prepared.rollback_anchor()) != position {
                    return Err(ClientError::Mismatch("native prepare position changed"));
                }
                let challenge = retain(prepared.decode_challenge(self.budget)?, self.budget)?;
                if challenge.policy_identity() != policy.identity()
                    || !challenge.subject().matches_subject(&subject, self.budget)?
                {
                    return Err(ClientError::Mismatch(
                        "native challenge names another source or policy",
                    ));
                }
                let request = retain(Request::new(challenge, subject, self.budget)?, self.budget)?;
                let issued = self.exchange(policy, Payload::Issue(&request))?;
                require_kind(&issued, Kind::Issued)?;
                let publication = retain(issued.decode_publication(self.budget)?, self.budget)?;
                verify_publication(policy, &request, &publication, self.budget)?;
                (request, publication)
            }
            Kind::Issued => {
                let publication = retain(inspected.decode_publication(self.budget)?, self.budget)?;
                let receipt = publication.receipt();
                let challenge = retain(
                    Challenge::new(
                        policy,
                        &subject,
                        receipt.challenge_nonce(),
                        receipt.sequence(),
                        receipt.prior_rollback_anchor(),
                        self.budget,
                    )?,
                    self.budget,
                )?;
                if challenge.identity() != receipt.challenge_identity() {
                    return Err(ClientError::Mismatch(
                        "native issued challenge cannot be reconstructed",
                    ));
                }
                let request = retain(Request::new(challenge, subject, self.budget)?, self.budget)?;
                verify_publication(policy, &request, &publication, self.budget)?;
                (request, publication)
            }
            _ => {
                return Err(ClientError::Mismatch(
                    "unexpected native issuer recovery stage",
                ));
            }
        };
        let published = self.exchange(
            policy,
            Payload::Publish {
                request: &request,
                publication: &publication,
            },
        )?;
        require_kind(&published, Kind::Published)?;
        let ack = retain(published.decode_acknowledgment(self.budget)?, self.budget)?;
        ack.matches_publication(&publication, self.budget)?;
        let policy = retain(
            Policy::decode(policy.canonical_bytes(), self.budget)?,
            self.budget,
        )?;
        let (carriage, delta) = Carriage::new(policy, request, publication, ack, self.budget)?;
        self.budget.reserve_storage(delta.additional_storage())?;
        Ok(carriage)
    }
}

fn retain<T>((value, charge): (T, Storage), budget: &mut Budget<'_>) -> Result<T> {
    budget.reserve_storage(charge.additional_storage())?;
    Ok(value)
}
fn input_floor(owner: usize, policy: usize, input: usize) -> Result<usize> {
    owner
        .checked_add(policy)
        .and_then(|n| n.checked_add(input))
        .ok_or_else(|| Resource::Arithmetic.into())
}
fn require_kind(response: &Response, expected: Kind) -> Result<()> {
    if response.kind() != expected {
        return Err(ClientError::Mismatch("unexpected native response kind"));
    }
    Ok(())
}
fn verify_publication(
    policy: &Policy,
    request: &Request,
    publication: &Publication,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let receipt = retain(
        Receipt::decode(publication.receipt().canonical_bytes(), budget)?,
        budget,
    )?;
    let (_verified, growth) = receipt.verify(
        policy,
        request,
        request.challenge().prior_rollback_anchor(),
        budget,
    )?;
    budget.reserve_storage(growth.additional_storage())?;
    Ok(())
}
