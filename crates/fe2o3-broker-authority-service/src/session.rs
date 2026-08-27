//! Inert broker-owned lifecycle model for one bounded host-link session.
//!
//! The machine has exactly one lifetime slot. It retains the admitted protected-service token and
//! the exact W0 [`AdmittedHostOutputV1`] inside the service. A reservation binds one opaque
//! session ID and nonce, the complete Broker V4 session claim, and one durable publication-plan
//! identity, then issues one move-only link permit. The broker-owned path retains the prepared V4
//! transcript, validates its grant and static-LLD artifact identity, consumes the permit, and owns
//! the authenticated process and admitted output through terminal commit. Completion admits only
//! a terminal V4 transcript that matches the reservation and exact W0 output binding.
//! Anchor preparation consumes the machine into a move-only pre-challenge capability. Only
//! durable preparation can consume that capability, generate a service-owned random attempt nonce,
//! form the nonce-bound transaction and challenge, and release challenge bytes after exact W0
//! staging and canonical `Prepared` record fsync. The durable transaction then owns signature
//! verification and one logical consume decision.
//!
//! `AUTHORITY=none`: this type-state model does not itself generate the service attempt nonce. It
//! does not make reservation, anti-rollback state, durable nonce freshness, or publication
//! durable. Linker invocation still requires a move-only approval minted by an external trusted
//! tool-evidence authority. This model does not authenticate that evidence itself, publish an
//! artifact, authenticate anchor-key provenance, reconcile multiple writers, or grant replay,
//! publication, runtime, or GPU authority.
//! Session ID and nonce uniqueness are caller preconditions; this model only rejects zero values,
//! an ID/nonce collision, and a second reservation in one machine lifetime. The retained Linux
//! admission is revalidated once at public reservation; later pure transitions do not provide
//! continuous process-liveness enforcement. The durable-plan identity is compared bit-for-bit
//! against reservation and terminal transcript, but this crate does not authenticate how that
//! identity was derived from a concrete durable plan.
//!
//! The machine and reservation are deliberately move-only and provide no serialization API:
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::BrokerSessionMachineV1;
//! fn require_clone<T: Clone>() {}
//! require_clone::<BrokerSessionMachineV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::BrokerSessionMachineV1;
//! fn require_copy<T: Copy>() {}
//! require_copy::<BrokerSessionMachineV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::BrokerSessionMachineV1;
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<BrokerSessionMachineV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::BrokerSessionReservationV1;
//! fn require_clone<T: Clone>() {}
//! require_clone::<BrokerSessionReservationV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::BrokerHostLinkPermitV1;
//! fn require_clone<T: Clone>() {}
//! require_clone::<BrokerHostLinkPermitV1>();
//! ```
//!
//! ```compile_fail
//! use fe2o3_broker_authority_service::BrokerHostLinkPermitV1;
//! fn require_serialize<T: serde::Serialize>() {}
//! require_serialize::<BrokerHostLinkPermitV1>();
//! ```

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::os::fd::OwnedFd;

use fe2o3_build_authority::{
    BrokerSessionClaimV4, CompletedBrokerTranscriptV4, GrantedHostLinkTranscriptV4,
    HOST_LINK_OUTPUT_MODE_V4, HostLinkCommitV4, HostLinkGrantV4, PreparedHostLinkTranscriptV4,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorDecisionV1,
    AnchoredStateV1, CallerNonceV1, HashChainHeadV1, PendingAnchorTransitionV1, PinnedAnchorKeyV1,
    TransactionDigestV1,
};
use fe2o3_host_link_closure::{
    AdmittedHostOutputV1, ApprovedStaticHostLldV1, AuthenticatedHostLinkExecutionV1,
    BrokerReservedHostLinkV1, HostLinkBrokerReservationV1, HostLinkClosureV1, HostLinkErrorCodeV1,
    Sha256Digest,
};
use sha2::{Digest, Sha256};

use crate::ProtectedServiceAdmissionV1;

/// Fixed semantic authority marker for every session-machine value.
pub const BROKER_SESSION_MACHINE_AUTHORITY_V1: &str = "none";

/// Exact number of live session slots represented by one machine value.
pub const BROKER_SESSION_CAPACITY_V1: usize = 1;

/// Domain for the canonical digest of every field in a terminal Broker V4 transcript.
pub const BROKER_V4_COMPLETED_TRANSCRIPT_DIGEST_DOMAIN_V1: &[u8] =
    b"FE2O3/BROKER-V4/COMPLETED-TRANSCRIPT-DIGEST/V1\0";

const BROKER_SESSION_CLAIM_DIGEST_DOMAIN_V1: &[u8] = b"FE2O3/BROKER-V4/SESSION-CLAIM-DIGEST/V1\0";
const BROKER_SESSION_ANCHOR_TRANSACTION_DOMAIN_V1: &[u8] =
    b"FE2O3/BROKER-SESSION/ANCHOR-TRANSACTION/SERVICE-ATTEMPT/V1\0";
/// Domain for the exact reservation digest carried by one move-only link permit.
pub const BROKER_LINK_RESERVATION_DIGEST_DOMAIN_V1: &[u8] =
    b"FE2O3/BROKER-SESSION/LINK-RESERVATION/V1\0";

/// Opaque nonzero broker session identifier.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerSessionIdV1([u8; 32]);

impl BrokerSessionIdV1 {
    /// Validates one caller-generated fixed-width identifier.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, BrokerSessionMachineErrorV1> {
        require_nonzero(bytes, BrokerSessionErrorKindV1::ZeroSessionId)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for BrokerSessionIdV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BrokerSessionIdV1(<opaque>)")
    }
}

/// Opaque nonzero nonce retained for the exact external-anchor challenge.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BrokerSessionNonceV1([u8; 32]);

impl BrokerSessionNonceV1 {
    /// Validates caller-generated nonce bytes.
    ///
    /// This check rejects zero only. Cryptographic uniqueness and durable replay detection remain
    /// responsibilities of the future protected service.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, BrokerSessionMachineErrorV1> {
        require_nonzero(bytes, BrokerSessionErrorKindV1::ZeroSessionNonce)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for BrokerSessionNonceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BrokerSessionNonceV1(<opaque>)")
    }
}

/// Exact nonzero identity of one durable publication plan.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct DurablePublicationPlanIdentityV1([u8; 32]);

impl DurablePublicationPlanIdentityV1 {
    /// Validates one fixed-width publication-plan identity.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, BrokerSessionMachineErrorV1> {
        require_nonzero(bytes, BrokerSessionErrorKindV1::ZeroDurablePublicationPlan)?;
        Ok(Self(bytes))
    }

    /// Returns the exact publication-plan identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for DurablePublicationPlanIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DurablePublicationPlanIdentityV1(<opaque>)")
    }
}

/// One move-only request for the machine's single lifetime reservation.
pub struct BrokerSessionReservationV1 {
    session_id: BrokerSessionIdV1,
    nonce: BrokerSessionNonceV1,
    claim_digest: [u8; 32],
    client_pid: u32,
    client_start_time_ticks: u64,
    host_link_plan: [u8; 32],
    host_link_closure: [u8; 32],
    durable_plan: DurablePublicationPlanIdentityV1,
}

impl BrokerSessionReservationV1 {
    /// Binds an opaque session identity to one exact V4 claim and publication plan.
    pub fn new(
        session_id: BrokerSessionIdV1,
        nonce: BrokerSessionNonceV1,
        claim: BrokerSessionClaimV4,
        durable_plan: DurablePublicationPlanIdentityV1,
    ) -> Result<Self, BrokerSessionMachineErrorV1> {
        if session_id.0 == nonce.0 {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::SessionIdNonceCollision,
            ));
        }
        Ok(Self {
            session_id,
            nonce,
            claim_digest: broker_session_claim_digest_v1(claim),
            client_pid: claim.process().pid(),
            client_start_time_ticks: claim.process().start_time_ticks(),
            host_link_plan: claim.plan_identity(),
            host_link_closure: claim.closure_identity(),
            durable_plan,
        })
    }
}

/// Unique move-only permit issued by one successful in-memory reservation.
///
/// The fields are private, the type is neither `Clone` nor serializable, and one successful
/// [`BrokerSessionMachineV1::begin_link`] call irreversibly marks it consumed. Global uniqueness
/// still depends on the caller-supplied session ID and nonce because no durable store exists.
pub struct BrokerHostLinkPermitV1 {
    reservation_digest: [u8; 32],
    consumed: bool,
}

impl BrokerHostLinkPermitV1 {
    /// Returns only whether this permit has already formed its one bound W0 request.
    pub const fn is_consumed(&self) -> bool {
        self.consumed
    }

    /// Returns the fixed non-authority marker.
    pub const fn authority(&self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }

    fn validate_for(
        &self,
        expected_reservation_digest: [u8; 32],
    ) -> Result<(), BrokerSessionMachineErrorV1> {
        if self.consumed {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::LinkPermitAlreadyConsumed,
            ));
        }
        if self.reservation_digest != expected_reservation_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::LinkPermitSubstitution,
            ));
        }
        Ok(())
    }

    fn consume_for(
        &mut self,
        expected_reservation_digest: [u8; 32],
    ) -> Result<(), BrokerSessionMachineErrorV1> {
        self.validate_for(expected_reservation_digest)?;
        self.consumed = true;
        Ok(())
    }

    fn restore_after_failed_request_binding(&mut self) {
        self.consumed = false;
    }
}

impl fmt::Debug for BrokerHostLinkPermitV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerHostLinkPermitV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("consumed", &self.consumed)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for BrokerSessionReservationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerSessionReservationV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("session_id", &self.session_id)
            .field("nonce", &self.nonce)
            .field("durable_plan", &self.durable_plan)
            .finish_non_exhaustive()
    }
}

/// Externally visible lifecycle stage. A stage observation is not a capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerSessionStageV1 {
    /// The machine's one lifetime slot has never been reserved.
    Vacant,
    /// Client, session, V4 claim, and publication plan are bound before linking.
    Reserved,
    /// The unique permit was consumed and one exact reservation-bound W0 request was formed.
    Linking,
    /// One exact matching V4/W0 completion is retained.
    Completed,
    /// The matching anchor transaction and positions are prepared.
    AnchorPrepared,
    /// One exact signed anchor observation is pending.
    AnchorPending,
    /// A valid proposed-position observation committed the anchor transition.
    AnchorCommitted,
    /// A valid prior-position observation deterministically aborted the transaction.
    Aborted,
    /// The single logical consume/publication decision was recorded.
    Consumed,
    /// A one-shot anchor operation failed and the machine failed closed.
    Invalidated,
}

/// Whether the external anchor is being advanced normally or queried during recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerAnchorModeV1 {
    /// Request an ordinary anchor advance.
    Advance,
    /// Query the exact prior/proposed positions for deterministic recovery.
    Recovery,
}

/// Opaque caller-visible observation of a successful lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerSessionObservationV1 {
    stage: BrokerSessionStageV1,
}

impl BrokerSessionObservationV1 {
    /// Returns only the resulting lifecycle stage.
    pub const fn stage(self) -> BrokerSessionStageV1 {
        self.stage
    }

    /// Returns the fixed non-authority marker.
    pub const fn authority(self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }
}

/// Move-only pre-challenge session whose service attempt nonce does not exist yet.
///
/// Construction consumes the only [`BrokerSessionMachineV1`] after it records the caller-visible
/// stable anchor inputs. There is deliberately no nonce, challenge, observation, serialization,
/// or inner-machine accessor. Only durable preparation inside this crate can consume the
/// capability, generate the service attempt nonce, and release challenge bytes after staging the
/// exact W0 output and fsyncing the canonical `Prepared` record.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerAnchorPreparedSessionV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<BrokerAnchorPreparedSessionV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerAnchorPreparedSessionV1;
/// fn leak_challenge(prepared: BrokerAnchorPreparedSessionV1) {
///     let _ = prepared.challenge_bytes();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerAnchorPreparedSessionV1;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<BrokerAnchorPreparedSessionV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerAnchorPreparedSessionV1;
/// fn leak_nonce(prepared: BrokerAnchorPreparedSessionV1) {
///     let _ = prepared.service_attempt_nonce();
/// }
/// ```
pub struct BrokerAnchorPreparedSessionV1 {
    machine: BrokerSessionMachineV1,
}

/// Move-only proof that one exact broker session reached a verified external-anchor commit.
///
/// Fields, output bytes, and retained service-root descriptor are inaccessible to callers. The
/// durable session transaction is the only in-tree consumer. This value is not serializable and
/// carries the fixed `AUTHORITY=none` marker.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::CommittedBrokerPublicationV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CommittedBrokerPublicationV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::CommittedBrokerPublicationV1;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<CommittedBrokerPublicationV1>();
/// ```
pub struct CommittedBrokerPublicationV1 {
    parts: BrokerCommittedPublicationPartsV1,
}

impl fmt::Debug for CommittedBrokerPublicationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CommittedBrokerPublicationV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("output_length", &self.parts.binding.output_length)
            .finish_non_exhaustive()
    }
}

impl CommittedBrokerPublicationV1 {
    /// Returns the fixed non-authority marker.
    pub const fn authority(&self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }

    /// This inert handoff never grants publication authority by itself.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub(crate) fn into_parts(self) -> BrokerCommittedPublicationPartsV1 {
        self.parts
    }
}

impl BrokerAnchorPreparedSessionV1 {
    /// Returns the fixed non-authority marker.
    pub const fn authority(&self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }

    /// Returns only the fixed `AnchorPrepared` lifecycle stage.
    pub const fn stage(&self) -> BrokerSessionStageV1 {
        BrokerSessionStageV1::AnchorPrepared
    }

    pub(crate) fn into_machine(self) -> BrokerSessionMachineV1 {
        self.machine
    }
}

impl fmt::Debug for BrokerAnchorPreparedSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerAnchorPreparedSessionV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("stage", &BrokerSessionStageV1::AnchorPrepared)
            .finish_non_exhaustive()
    }
}

/// Stable classification for a rejected lifecycle transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerSessionErrorKindV1 {
    /// The session identifier was all zero.
    ZeroSessionId,
    /// The anchor nonce was all zero.
    ZeroSessionNonce,
    /// The durable publication-plan identity was all zero.
    ZeroDurablePublicationPlan,
    /// Session ID and nonce used identical bytes.
    SessionIdNonceCollision,
    /// The machine's one fixed slot was already occupied.
    CapacityOccupied,
    /// A permit from another reservation was substituted.
    LinkPermitSubstitution,
    /// The move-only permit already formed its single W0 request.
    LinkPermitAlreadyConsumed,
    /// The transition was attempted from the wrong stage.
    TransitionOrder,
    /// The V4 process does not match the retained admitted client token.
    ClientIdentityMismatch,
    /// The retained admitted client token failed continuity revalidation at reservation.
    ClientIdentityRevalidation,
    /// The completed V4 session claim differs from the reservation.
    TranscriptClaimMismatch,
    /// The completed transcript digest differs from retained completion.
    TranscriptDigestMismatch,
    /// The V4 output digest differs from the W0 admitted output.
    OutputDigestMismatch,
    /// The V4 output length differs from the W0 admitted output.
    OutputLengthMismatch,
    /// The V4 output mode differs from the W0 admitted output.
    OutputModeMismatch,
    /// The V4 host-link plan differs from the W0 admitted output plan.
    HostLinkPlanMismatch,
    /// The V4 closure differs from the W0 admitted output closure.
    HostLinkClosureMismatch,
    /// W0 could not form the exact broker-reservation-bound request.
    HostLinkRequestBinding,
    /// The V4 binding names another static host-LLD artifact identity.
    HostLinkToolIdentityMismatch,
    /// The V4 grant does not continue the broker-owned prepared transcript.
    HostLinkGrantMismatch,
    /// The exact approved static host LLD could not be launched.
    HostLinkLaunch,
    /// The authenticated static host LLD failed output admission.
    HostLinkOutputAdmission,
    /// Completion was requested before an authenticated output was admitted.
    HostLinkOutputPending,
    /// The V4 commit does not continue the broker-owned granted transcript.
    HostLinkCommitMismatch,
    /// The W0 output carries another or no broker reservation.
    HostLinkReservationMismatch,
    /// The W0 output carries another authenticated request nonce.
    HostLinkRequestNonceMismatch,
    /// The durable publication plan differs from the reservation or completion.
    DurablePublicationPlanMismatch,
    /// The pinned anchor key differs from the prepared key identity.
    AnchorKeyMismatch,
    /// The anchor protocol rejected preparation, challenge creation, or observation verification.
    AnchorProtocol,
    /// A commit or abort decision carried another transaction.
    AnchorTransactionMismatch,
    /// A commit or abort decision carried a stale or unexpected sequence.
    AnchorSequenceMismatch,
    /// A decision carried another prior position.
    AnchorPriorMismatch,
    /// A decision carried another proposed position.
    AnchorProposedMismatch,
    /// A decision carried another nonce.
    AnchorNonceMismatch,
    /// Fixed internal state was inconsistent; the machine failed closed.
    InternalState,
    /// Exact service-root or admitted-output handoff could not be retained.
    DurableHandoff,
}

/// Panic-free error from the inert session lifecycle model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerSessionMachineErrorV1 {
    kind: BrokerSessionErrorKindV1,
}

impl BrokerSessionMachineErrorV1 {
    const fn new(kind: BrokerSessionErrorKindV1) -> Self {
        Self { kind }
    }

    /// Returns the stable error classification.
    pub const fn kind(self) -> BrokerSessionErrorKindV1 {
        self.kind
    }
}

impl fmt::Display for BrokerSessionMachineErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "broker session transition rejected: {:?}",
            self.kind
        )
    }
}

impl Error for BrokerSessionMachineErrorV1 {}

/// Non-authoritative identity observation for one broker-retained host-link output.
///
/// This value contains no descriptor and grants no access to the admitted bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerHostOutputObservationV1 {
    sha256: [u8; 32],
    length: u64,
    mode: u32,
}

impl BrokerHostOutputObservationV1 {
    /// Returns the admitted output content identity.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the admitted output length.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Returns the admitted output mode.
    pub const fn mode(self) -> u32 {
        self.mode
    }

    /// Returns the fixed non-authority marker.
    pub const fn authority(self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }

    fn from_output(output: &AdmittedHostOutputV1) -> Self {
        Self {
            sha256: *output.sha256().as_bytes(),
            length: output.size(),
            mode: output.mode(),
        }
    }
}

/// Result of polling one broker-owned authenticated host-link execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerHostLinkPollV1 {
    /// The authenticated worker has not yet produced a terminal admitted output.
    Pending,
    /// The broker retained the admitted output and exposes only its inert identity fields.
    Admitted(BrokerHostOutputObservationV1),
}

/// Move-only reservation retaining the prepared Broker V4 transcript and unique W0 permit.
///
/// The only launch transition validates a matching grant before invoking the linker.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerReservedHostLinkSessionV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<BrokerReservedHostLinkSessionV1>();
/// ```
pub struct BrokerReservedHostLinkSessionV1 {
    machine: BrokerSessionMachineV1,
    permit: BrokerHostLinkPermitV1,
    prepared: PreparedHostLinkTranscriptV4,
}

/// Move-only broker custody over one authenticated static-host-LLD process and its output.
///
/// The execution and admitted descriptor never leave this value. A caller can observe only the
/// bounded output identity needed to construct the terminal Broker V4 commit.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerOwnedHostLinkExecutionV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<BrokerOwnedHostLinkExecutionV1>();
/// ```
pub struct BrokerOwnedHostLinkExecutionV1 {
    machine: BrokerSessionMachineV1,
    granted: GrantedHostLinkTranscriptV4,
    execution: Option<AuthenticatedHostLinkExecutionV1>,
    output: Option<AdmittedHostOutputV1>,
}

/// Move-only completed host-link session retaining the exact terminal V4 transcript.
pub struct BrokerCompletedHostLinkV1 {
    machine: BrokerSessionMachineV1,
    transcript: CompletedBrokerTranscriptV4,
}

impl BrokerCompletedHostLinkV1 {
    /// Returns the completed broker lifecycle stage.
    pub const fn stage(&self) -> BrokerSessionStageV1 {
        self.machine.stage()
    }

    /// Borrows the exact terminal transcript bound to the retained output.
    pub const fn transcript(&self) -> &CompletedBrokerTranscriptV4 {
        &self.transcript
    }

    /// Consumes the wrapper while preserving output custody inside the session machine.
    pub fn into_parts(self) -> (BrokerSessionMachineV1, CompletedBrokerTranscriptV4) {
        (self.machine, self.transcript)
    }
}

impl fmt::Debug for BrokerReservedHostLinkSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerReservedHostLinkSessionV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("stage", &self.machine.stage())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for BrokerOwnedHostLinkExecutionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerOwnedHostLinkExecutionV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("stage", &self.machine.stage())
            .field("output_admitted", &self.output.is_some())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for BrokerCompletedHostLinkV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerCompletedHostLinkV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("stage", &self.machine.stage())
            .finish_non_exhaustive()
    }
}

/// Broker-owned, move-only model of one complete session lifecycle.
///
/// Retained client and output capabilities have no accessors and never leave this value. Anchor
/// preparation consumes the machine into [`BrokerAnchorPreparedSessionV1`] before any service
/// attempt nonce, transaction, or challenge exists. External code cannot begin or observe the
/// anchor until durable preparation returns a [`crate::BrokerDurableSessionTransactionV1`].
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerSessionMachineV1;
/// fn bypass_durability(mut machine: BrokerSessionMachineV1, observation: &[u8]) {
///     let _ = machine.observe_anchor(observation);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::{BrokerAnchorModeV1, BrokerSessionMachineV1};
/// fn choose_attempt(machine: BrokerSessionMachineV1) {
///     let _ = machine.begin_anchor(BrokerAnchorModeV1::Advance, [7; 32]);
/// }
/// ```
pub struct BrokerSessionMachineV1 {
    core: SessionCoreV1<ProtectedServiceAdmissionV1, AdmittedHostOutputV1>,
}

impl fmt::Debug for BrokerSessionMachineV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerSessionMachineV1")
            .field("authority", &BROKER_SESSION_MACHINE_AUTHORITY_V1)
            .field("capacity", &BROKER_SESSION_CAPACITY_V1)
            .field("stage", &self.core.stage)
            .finish_non_exhaustive()
    }
}

impl Default for BrokerSessionMachineV1 {
    fn default() -> Self {
        Self::new()
    }
}

impl BrokerSessionMachineV1 {
    /// Creates one empty, fixed-capacity, non-authoritative machine.
    pub const fn new() -> Self {
        Self {
            core: SessionCoreV1::new(),
        }
    }

    /// Returns only the current lifecycle stage.
    pub const fn stage(&self) -> BrokerSessionStageV1 {
        self.core.stage
    }

    /// Returns the fixed non-authority marker.
    pub const fn authority(&self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }

    /// Occupies the machine's only lifetime slot before any host-link completion is accepted.
    ///
    /// The protected admission is consumed and retained. Its descriptor, pidfd, PID, start time,
    /// and liveness continuity are revalidated before its process identity is compared to the V4
    /// claim, without exposing that identity through this API.
    pub fn reserve(
        &mut self,
        admission: ProtectedServiceAdmissionV1,
        reservation: BrokerSessionReservationV1,
    ) -> Result<BrokerHostLinkPermitV1, BrokerSessionMachineErrorV1> {
        self.core.require_reservation_capacity()?;
        admission.validate_session_continuity().map_err(|_| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::ClientIdentityRevalidation)
        })?;
        let client_matches = admission
            .matches_client_process(reservation.client_pid, reservation.client_start_time_ticks);
        self.core.reserve(admission, reservation, client_matches)
    }

    /// Consumes an empty machine into the broker-owned V4 host-link path.
    ///
    /// The prepared transcript is retained with the unique W0 permit. It cannot be substituted
    /// after reservation or reused to validate more than one grant.
    pub fn reserve_prepared_link(
        mut self,
        admission: ProtectedServiceAdmissionV1,
        session_id: BrokerSessionIdV1,
        nonce: BrokerSessionNonceV1,
        prepared: PreparedHostLinkTranscriptV4,
        durable_plan: DurablePublicationPlanIdentityV1,
    ) -> Result<BrokerReservedHostLinkSessionV1, BrokerSessionMachineErrorV1> {
        let reservation = BrokerSessionReservationV1::new(
            session_id,
            nonce,
            prepared.session_claim(),
            durable_plan,
        )?;
        let permit = self.reserve(admission, reservation)?;
        Ok(BrokerReservedHostLinkSessionV1 {
            machine: self,
            permit,
            prepared,
        })
    }

    /// Consumes the unique permit before forming one exact reservation-bound W0 request.
    ///
    /// Permit, plan, and closure substitutions fail before the permit is consumed. W0 then
    /// domain-separates the reservation digest into its authenticated request nonce. No linker is
    /// invoked by this operation.
    pub fn begin_link(
        &mut self,
        permit: &mut BrokerHostLinkPermitV1,
        closure: HostLinkClosureV1,
    ) -> Result<BrokerReservedHostLinkV1, BrokerSessionMachineErrorV1> {
        let plan_digest = *closure.plan_digest().as_bytes();
        let closure_digest = *closure.closure_digest().as_bytes();
        self.core
            .validate_link_start(permit, plan_digest, closure_digest)?;
        let reservation_digest = self.core.reservation_digest()?;
        permit.consume_for(reservation_digest)?;
        let reservation = HostLinkBrokerReservationV1::from_sha256(Sha256Digest::from_bytes(
            permit.reservation_digest,
        ))
        .map_err(|_| {
            permit.restore_after_failed_request_binding();
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::HostLinkRequestBinding)
        })?;
        let bound = closure.bind_broker_reservation(reservation).map_err(|_| {
            permit.restore_after_failed_request_binding();
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::HostLinkRequestBinding)
        })?;
        self.core.commit_link_start(
            permit.reservation_digest,
            *bound.request_nonce_sha256().as_bytes(),
        );
        Ok(bound)
    }

    /// Retains one exact W0 output after checking every corresponding V4 terminal field.
    ///
    /// No linker is invoked. The operation only records an in-memory completion after reservation.
    pub fn complete(
        &mut self,
        transcript: &CompletedBrokerTranscriptV4,
        output: AdmittedHostOutputV1,
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.core.require_stage(BrokerSessionStageV1::Linking)?;
        let binding = CompletionBindingV1::from_exact(transcript, &output)?;
        self.core.complete(output, binding)
    }

    /// Consumes this machine into one opaque pre-challenge anchor preparation.
    ///
    /// This retains the caller-visible stable position, mode, and pinned key. It does not form an
    /// anchor transaction or challenge. The service attempt nonce is generated later inside
    /// durable preparation and is bound into both.
    pub fn prepare_anchor(
        mut self,
        mode: BrokerAnchorModeV1,
        stable: AnchoredStateV1,
        key: &PinnedAnchorKeyV1,
    ) -> Result<BrokerAnchorPreparedSessionV1, BrokerSessionMachineErrorV1> {
        self.core.prepare_anchor(mode, stable, key)?;
        Ok(BrokerAnchorPreparedSessionV1 { machine: self })
    }

    pub(crate) fn observe_anchor(
        &mut self,
        observation: &[u8],
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.core.observe_anchor(observation)
    }

    /// Records one logical consume/publication decision after a valid anchor commit.
    ///
    /// The transcript is re-derived and compared to retained completion. No filesystem or
    /// publication operation occurs, and no output capability is returned.
    pub fn consume_publication(
        &mut self,
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.core.consume_publication(transcript)
    }

    /// Consumes this machine into the only move-only committed durable-publication handoff.
    ///
    /// The exact terminal transcript is revalidated before the retained service root and W0
    /// output leave the machine. The result remains inert and grants no filesystem authority on
    /// its own; it is intended for `BrokerDurableSessionTransactionV1`.
    pub fn into_committed_publication(
        mut self,
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<CommittedBrokerPublicationV1, BrokerSessionMachineErrorV1> {
        self.core.validate_committed_publication(transcript)?;
        let binding = self.core.durable_binding()?;
        let challenge = self.core.anchor_challenge()?;
        let anchor_key_bytes = self
            .core
            .anchor_expected
            .ok_or_else(internal_state_error)?
            .anchor_key_bytes;
        let anchor_observation = self
            .core
            .anchor_commit
            .ok_or_else(internal_state_error)?
            .observation;
        let client = self.core.client.take().ok_or_else(internal_state_error)?;
        client.validate_session_continuity().map_err(|_| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::DurableHandoff)
        })?;
        let service_root = client.into_service_root();
        let output = self.core.output.take().ok_or_else(internal_state_error)?;
        Ok(CommittedBrokerPublicationV1 {
            parts: BrokerCommittedPublicationPartsV1 {
                service_root,
                output,
                binding,
                challenge,
                anchor_key_bytes,
                anchor_observation,
            },
        })
    }

    pub(crate) fn durable_preparation_parts(
        &self,
    ) -> Result<BrokerDurablePreparationPartsV1, BrokerSessionMachineErrorV1> {
        self.core
            .require_stage(BrokerSessionStageV1::AnchorPrepared)?;
        let client = self.core.client.as_ref().ok_or_else(internal_state_error)?;
        let output = self.core.output.as_ref().ok_or_else(internal_state_error)?;
        let service_root = client.try_clone_service_root().map_err(|_| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::DurableHandoff)
        })?;
        let output_file = output.try_clone_file().map_err(|_| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::DurableHandoff)
        })?;
        let preparation = self
            .core
            .anchor_preparation
            .ok_or_else(internal_state_error)?;
        Ok(BrokerDurablePreparationPartsV1 {
            service_root,
            output_file,
            binding: self.core.durable_binding()?,
            anchor_key_bytes: preparation.anchor_key_bytes,
        })
    }

    pub(crate) fn begin_anchor_with_service_nonce(
        &mut self,
        service_attempt_nonce: [u8; 32],
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.core.begin_anchor(service_attempt_nonce)
    }

    pub(crate) fn anchor_challenge(
        &self,
    ) -> Result<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1], BrokerSessionMachineErrorV1> {
        self.core.anchor_challenge()
    }
}

impl BrokerReservedHostLinkSessionV1 {
    /// Returns the reserved broker lifecycle stage.
    pub const fn stage(&self) -> BrokerSessionStageV1 {
        self.machine.stage()
    }

    /// Returns the fixed non-authority marker.
    pub const fn authority(&self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }

    /// Validates the exact V4 grant and enters broker-owned authenticated linker execution.
    ///
    /// `approval` must come from the external trusted tool-evidence authority. This transition
    /// additionally requires V4 to name the closure's canonical static-LLD artifact identity,
    /// consumes the one-shot W0 permit, and retains the process inside the broker service.
    pub fn grant_and_launch(
        self,
        closure: HostLinkClosureV1,
        grant: HostLinkGrantV4,
        approval: ApprovedStaticHostLldV1,
    ) -> Result<BrokerOwnedHostLinkExecutionV1, BrokerSessionMachineErrorV1> {
        let BrokerReservedHostLinkSessionV1 {
            mut machine,
            mut permit,
            prepared,
        } = self;
        let expected_tool = prepared.expected_binding().static_host_lld_identity();
        let actual_tool = closure
            .static_host_lld_artifact_id()
            .map_err(|_| {
                BrokerSessionMachineErrorV1::new(
                    BrokerSessionErrorKindV1::HostLinkToolIdentityMismatch,
                )
            })?
            .sha256();
        if expected_tool != *actual_tool.as_bytes() {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkToolIdentityMismatch,
            ));
        }
        let granted = prepared.validate_grant(grant).map_err(|_| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::HostLinkGrantMismatch)
        })?;
        let bound = machine.begin_link(&mut permit, closure)?;
        let execution = bound.launch(approval).map_err(|_| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::HostLinkLaunch)
        })?;
        Ok(BrokerOwnedHostLinkExecutionV1 {
            machine,
            granted,
            execution: Some(execution),
            output: None,
        })
    }
}

impl BrokerOwnedHostLinkExecutionV1 {
    /// Returns the linking lifecycle stage.
    pub const fn stage(&self) -> BrokerSessionStageV1 {
        self.machine.stage()
    }

    /// Returns the fixed non-authority marker.
    pub const fn authority(&self) -> &'static str {
        BROKER_SESSION_MACHINE_AUTHORITY_V1
    }

    /// Polls the authenticated child while retaining both execution and output custody.
    pub fn poll_output(&mut self) -> Result<BrokerHostLinkPollV1, BrokerSessionMachineErrorV1> {
        if let Some(output) = &self.output {
            return Ok(BrokerHostLinkPollV1::Admitted(
                BrokerHostOutputObservationV1::from_output(output),
            ));
        }
        let execution = self.execution.as_mut().ok_or_else(|| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::InternalState)
        })?;
        let observation = match execution.try_admit_output() {
            Ok(output) => BrokerHostOutputObservationV1::from_output(output),
            Err(error) if error.code() == HostLinkErrorCodeV1::ResultPending => {
                return Ok(BrokerHostLinkPollV1::Pending);
            }
            Err(_) => {
                return Err(BrokerSessionMachineErrorV1::new(
                    BrokerSessionErrorKindV1::HostLinkOutputAdmission,
                ));
            }
        };
        let output = self
            .execution
            .take()
            .ok_or_else(|| {
                BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::InternalState)
            })?
            .into_admitted_output()
            .map_err(|_| {
                BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::HostLinkOutputAdmission)
            })?;
        self.output = Some(output);
        Ok(BrokerHostLinkPollV1::Admitted(observation))
    }

    /// Returns the admitted output identity without releasing its descriptor.
    pub fn output_observation(&self) -> Option<BrokerHostOutputObservationV1> {
        self.output
            .as_ref()
            .map(BrokerHostOutputObservationV1::from_output)
    }

    /// Consumes the execution after admission and validates one exact terminal V4 commit.
    pub fn complete(
        self,
        commit: HostLinkCommitV4,
    ) -> Result<BrokerCompletedHostLinkV1, BrokerSessionMachineErrorV1> {
        let BrokerOwnedHostLinkExecutionV1 {
            mut machine,
            granted,
            execution,
            output,
        } = self;
        if execution.is_some() {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkOutputPending,
            ));
        }
        let output = output.ok_or_else(|| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::HostLinkOutputPending)
        })?;
        let transcript = granted.validate_commit(commit).map_err(|_| {
            BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::HostLinkCommitMismatch)
        })?;
        machine.complete(&transcript, output)?;
        Ok(BrokerCompletedHostLinkV1 {
            machine,
            transcript,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct BrokerDurableBindingV1 {
    pub(crate) session_id: [u8; 32],
    pub(crate) session_nonce: [u8; 32],
    pub(crate) reservation_digest: [u8; 32],
    pub(crate) request_nonce_sha256: [u8; 32],
    pub(crate) client_pid: u32,
    pub(crate) client_start_time_ticks: u64,
    pub(crate) claim_digest: [u8; 32],
    pub(crate) transcript_digest: [u8; 32],
    pub(crate) transcript_binding_identity: [u8; 32],
    pub(crate) transcript_request_identity: [u8; 32],
    pub(crate) transcript_plan_identity: [u8; 32],
    pub(crate) transcript_closure_identity: [u8; 32],
    pub(crate) transcript_grant_identity: [u8; 32],
    pub(crate) output_digest: [u8; 32],
    pub(crate) output_length: u64,
    pub(crate) output_mode: u32,
    pub(crate) durable_plan: [u8; 32],
}

pub(crate) struct BrokerDurablePreparationPartsV1 {
    pub(crate) service_root: OwnedFd,
    pub(crate) output_file: File,
    pub(crate) binding: BrokerDurableBindingV1,
    pub(crate) anchor_key_bytes: [u8; 32],
}

pub(crate) struct BrokerCommittedPublicationPartsV1 {
    pub(crate) service_root: OwnedFd,
    pub(crate) output: AdmittedHostOutputV1,
    pub(crate) binding: BrokerDurableBindingV1,
    pub(crate) challenge: [u8; ANCHOR_CHALLENGE_WIRE_LEN_V1],
    pub(crate) anchor_key_bytes: [u8; 32],
    pub(crate) anchor_observation: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
}

#[derive(Clone, Copy)]
struct ReservationBindingV1 {
    session_id: BrokerSessionIdV1,
    nonce: BrokerSessionNonceV1,
    claim_digest: [u8; 32],
    client_pid: u32,
    client_start_time_ticks: u64,
    host_link_plan: [u8; 32],
    host_link_closure: [u8; 32],
    durable_plan: DurablePublicationPlanIdentityV1,
}

impl From<BrokerSessionReservationV1> for ReservationBindingV1 {
    fn from(value: BrokerSessionReservationV1) -> Self {
        Self {
            session_id: value.session_id,
            nonce: value.nonce,
            claim_digest: value.claim_digest,
            client_pid: value.client_pid,
            client_start_time_ticks: value.client_start_time_ticks,
            host_link_plan: value.host_link_plan,
            host_link_closure: value.host_link_closure,
            durable_plan: value.durable_plan,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompletionBindingV1 {
    claim_digest: [u8; 32],
    transcript_digest: [u8; 32],
    output_digest: [u8; 32],
    durable_plan: DurablePublicationPlanIdentityV1,
    broker_reservation: Option<[u8; 32]>,
    request_nonce_sha256: [u8; 32],
    output_length: u64,
    output_mode: u32,
    transcript_binding_identity: [u8; 32],
    transcript_request_identity: [u8; 32],
    transcript_plan_identity: [u8; 32],
    transcript_closure_identity: [u8; 32],
    transcript_grant_identity: [u8; 32],
}

impl CompletionBindingV1 {
    fn from_exact(
        transcript: &CompletedBrokerTranscriptV4,
        output: &AdmittedHostOutputV1,
    ) -> Result<Self, BrokerSessionMachineErrorV1> {
        let output_digest = *output.sha256().as_bytes();
        if transcript.output_sha256() != output_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::OutputDigestMismatch,
            ));
        }
        if transcript.output_length() != output.size() {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::OutputLengthMismatch,
            ));
        }
        if transcript.output_mode() != output.mode()
            || transcript.output_mode() != HOST_LINK_OUTPUT_MODE_V4
        {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::OutputModeMismatch,
            ));
        }
        if transcript.plan_identity() != *output.plan_digest().as_bytes() {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkPlanMismatch,
            ));
        }
        if transcript.closure_identity() != *output.closure_digest().as_bytes() {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkClosureMismatch,
            ));
        }
        let durable_plan =
            DurablePublicationPlanIdentityV1::from_bytes(transcript.durable_plan_identity())?;
        Ok(Self {
            claim_digest: broker_session_claim_digest_v1(transcript.session_claim()),
            transcript_digest: completed_broker_transcript_digest_v1(transcript),
            output_digest,
            durable_plan,
            broker_reservation: output
                .broker_reservation()
                .map(|reservation| *reservation.sha256().as_bytes()),
            request_nonce_sha256: *output.request_nonce_sha256().as_bytes(),
            output_length: output.size(),
            output_mode: output.mode(),
            transcript_binding_identity: transcript.binding_identity(),
            transcript_request_identity: transcript.request_identity(),
            transcript_plan_identity: transcript.plan_identity(),
            transcript_closure_identity: transcript.closure_identity(),
            transcript_grant_identity: transcript.grant_identity(),
        })
    }

    fn from_transcript(
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<Self, BrokerSessionMachineErrorV1> {
        Ok(Self {
            claim_digest: broker_session_claim_digest_v1(transcript.session_claim()),
            transcript_digest: completed_broker_transcript_digest_v1(transcript),
            output_digest: transcript.output_sha256(),
            durable_plan: DurablePublicationPlanIdentityV1::from_bytes(
                transcript.durable_plan_identity(),
            )?,
            broker_reservation: None,
            request_nonce_sha256: [0; 32],
            output_length: transcript.output_length(),
            output_mode: transcript.output_mode(),
            transcript_binding_identity: transcript.binding_identity(),
            transcript_request_identity: transcript.request_identity(),
            transcript_plan_identity: transcript.plan_identity(),
            transcript_closure_identity: transcript.closure_identity(),
            transcript_grant_identity: transcript.grant_identity(),
        })
    }
}

#[derive(Clone, Copy)]
struct LinkBindingV1 {
    broker_reservation: [u8; 32],
    request_nonce_sha256: [u8; 32],
}

#[derive(Clone, Copy)]
struct AnchorPreparationV1 {
    mode: BrokerAnchorModeV1,
    stable_sequence: u64,
    stable_head: HashChainHeadV1,
    anchor_key_bytes: [u8; 32],
}

#[derive(Clone, Copy)]
struct AnchorExpectedV1 {
    transaction: TransactionDigestV1,
    expected_sequence: u64,
    prior_head: HashChainHeadV1,
    proposed_head: HashChainHeadV1,
    nonce: [u8; 32],
    anchor_key_bytes: [u8; 32],
    challenge: [u8; ANCHOR_CHALLENGE_WIRE_LEN_V1],
}

#[derive(Clone, Copy)]
struct AnchorCommitBindingV1 {
    observation: [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1],
}

struct SessionCoreV1<C, O> {
    stage: BrokerSessionStageV1,
    client: Option<C>,
    output: Option<O>,
    reservation: Option<ReservationBindingV1>,
    completion: Option<CompletionBindingV1>,
    link: Option<LinkBindingV1>,
    anchor_preparation: Option<AnchorPreparationV1>,
    pending: Option<PendingAnchorTransitionV1>,
    anchor_expected: Option<AnchorExpectedV1>,
    anchor_commit: Option<AnchorCommitBindingV1>,
}

impl<C, O> SessionCoreV1<C, O> {
    const fn new() -> Self {
        Self {
            stage: BrokerSessionStageV1::Vacant,
            client: None,
            output: None,
            reservation: None,
            completion: None,
            link: None,
            anchor_preparation: None,
            pending: None,
            anchor_expected: None,
            anchor_commit: None,
        }
    }

    fn reserve(
        &mut self,
        client: C,
        reservation: BrokerSessionReservationV1,
        client_matches: bool,
    ) -> Result<BrokerHostLinkPermitV1, BrokerSessionMachineErrorV1> {
        self.require_reservation_capacity()?;
        if !client_matches {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::ClientIdentityMismatch,
            ));
        }
        let reservation = ReservationBindingV1::from(reservation);
        let reservation_digest = broker_link_reservation_digest_v1(reservation);
        self.client = Some(client);
        self.reservation = Some(reservation);
        self.stage = BrokerSessionStageV1::Reserved;
        Ok(BrokerHostLinkPermitV1 {
            reservation_digest,
            consumed: false,
        })
    }

    fn require_reservation_capacity(&self) -> Result<(), BrokerSessionMachineErrorV1> {
        if self.stage == BrokerSessionStageV1::Vacant
            && self.client.is_none()
            && self.reservation.is_none()
        {
            Ok(())
        } else {
            Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::CapacityOccupied,
            ))
        }
    }

    fn validate_link_start(
        &self,
        permit: &BrokerHostLinkPermitV1,
        plan_digest: [u8; 32],
        closure_digest: [u8; 32],
    ) -> Result<(), BrokerSessionMachineErrorV1> {
        if permit.consumed {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::LinkPermitAlreadyConsumed,
            ));
        }
        self.require_stage(BrokerSessionStageV1::Reserved)?;
        let reservation = self.reservation.ok_or_else(internal_state_error)?;
        permit.validate_for(broker_link_reservation_digest_v1(reservation))?;
        if plan_digest != reservation.host_link_plan {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkPlanMismatch,
            ));
        }
        if closure_digest != reservation.host_link_closure {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkClosureMismatch,
            ));
        }
        Ok(())
    }

    fn reservation_digest(&self) -> Result<[u8; 32], BrokerSessionMachineErrorV1> {
        self.require_stage(BrokerSessionStageV1::Reserved)?;
        self.reservation
            .map(broker_link_reservation_digest_v1)
            .ok_or_else(internal_state_error)
    }

    fn commit_link_start(&mut self, broker_reservation: [u8; 32], request_nonce_sha256: [u8; 32]) {
        self.link = Some(LinkBindingV1 {
            broker_reservation,
            request_nonce_sha256,
        });
        self.stage = BrokerSessionStageV1::Linking;
    }

    fn complete(
        &mut self,
        output: O,
        completion: CompletionBindingV1,
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.require_stage(BrokerSessionStageV1::Linking)?;
        let reservation = self.reservation.ok_or_else(internal_state_error)?;
        let link = self.link.ok_or_else(internal_state_error)?;
        if completion.claim_digest != reservation.claim_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::TranscriptClaimMismatch,
            ));
        }
        if completion.durable_plan != reservation.durable_plan {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::DurablePublicationPlanMismatch,
            ));
        }
        if completion.broker_reservation != Some(link.broker_reservation) {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkReservationMismatch,
            ));
        }
        if completion.request_nonce_sha256 != link.request_nonce_sha256 {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::HostLinkRequestNonceMismatch,
            ));
        }
        self.output = Some(output);
        self.completion = Some(completion);
        self.stage = BrokerSessionStageV1::Completed;
        Ok(self.observation())
    }

    fn prepare_anchor(
        &mut self,
        mode: BrokerAnchorModeV1,
        stable: AnchoredStateV1,
        key: &PinnedAnchorKeyV1,
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.require_stage(BrokerSessionStageV1::Completed)?;
        stable
            .sequence()
            .checked_add(1)
            .ok_or_else(anchor_protocol_error)?;
        self.anchor_preparation = Some(AnchorPreparationV1 {
            mode,
            stable_sequence: stable.sequence(),
            stable_head: stable.head(),
            anchor_key_bytes: key.to_bytes(),
        });
        self.stage = BrokerSessionStageV1::AnchorPrepared;
        Ok(self.observation())
    }

    fn begin_anchor(
        &mut self,
        service_attempt_nonce: [u8; 32],
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.require_stage(BrokerSessionStageV1::AnchorPrepared)?;
        if service_attempt_nonce == [0; 32] {
            return Err(anchor_protocol_error());
        }
        let preparation = self
            .anchor_preparation
            .take()
            .ok_or_else(internal_state_error)?;
        let key = PinnedAnchorKeyV1::from_bytes(preparation.anchor_key_bytes)
            .map_err(|_| anchor_protocol_error())?;
        let transaction = self.anchor_transaction(service_attempt_nonce)?;
        let prepared =
            AnchoredStateV1::from_local_state(preparation.stable_sequence, preparation.stable_head)
                .prepare(transaction, &key)
                .map_err(|_| anchor_protocol_error())?;
        let mut expected = AnchorExpectedV1 {
            transaction: prepared.transaction(),
            expected_sequence: prepared.expected_sequence(),
            prior_head: prepared.prior_head(),
            proposed_head: prepared.proposed_head(),
            nonce: service_attempt_nonce,
            anchor_key_bytes: preparation.anchor_key_bytes,
            challenge: [0; ANCHOR_CHALLENGE_WIRE_LEN_V1],
        };
        let nonce = CallerNonceV1::from_bytes(expected.nonce);
        let pending_result = match preparation.mode {
            BrokerAnchorModeV1::Advance => prepared.begin_advance(nonce, &key),
            BrokerAnchorModeV1::Recovery => prepared.begin_recovery(nonce, &key),
        };
        let pending = match pending_result {
            Ok(pending) => pending,
            Err(_) => {
                self.stage = BrokerSessionStageV1::Invalidated;
                return Err(anchor_protocol_error());
            }
        };
        let mut bytes = [0_u8; ANCHOR_CHALLENGE_WIRE_LEN_V1];
        bytes.copy_from_slice(pending.challenge().as_bytes());
        expected.challenge = bytes;
        self.anchor_expected = Some(expected);
        self.pending = Some(pending);
        self.stage = BrokerSessionStageV1::AnchorPending;
        Ok(self.observation())
    }

    fn observe_anchor(
        &mut self,
        observation: &[u8],
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.require_stage(BrokerSessionStageV1::AnchorPending)?;
        let expected = self.anchor_expected.ok_or_else(internal_state_error)?;
        let Some(pending) = self.pending.take() else {
            self.stage = BrokerSessionStageV1::Invalidated;
            return Err(internal_state_error());
        };
        let decision = match pending.verify(observation) {
            Ok(decision) => decision,
            Err(_) => {
                self.stage = BrokerSessionStageV1::Invalidated;
                return Err(anchor_protocol_error());
            }
        };
        let result = (|| match decision {
            AnchorDecisionV1::Commit(commit) => {
                require_anchor_field(
                    commit.transaction() == expected.transaction,
                    BrokerSessionErrorKindV1::AnchorTransactionMismatch,
                )?;
                require_anchor_field(
                    commit.sequence() == expected.expected_sequence,
                    BrokerSessionErrorKindV1::AnchorSequenceMismatch,
                )?;
                require_anchor_field(
                    commit.prior_head() == expected.prior_head,
                    BrokerSessionErrorKindV1::AnchorPriorMismatch,
                )?;
                require_anchor_field(
                    commit.head() == expected.proposed_head,
                    BrokerSessionErrorKindV1::AnchorProposedMismatch,
                )?;
                require_anchor_field(
                    commit.observed_nonce() == &expected.nonce,
                    BrokerSessionErrorKindV1::AnchorNonceMismatch,
                )?;
                Ok(BrokerSessionStageV1::AnchorCommitted)
            }
            AnchorDecisionV1::Abort(abort) => {
                let prior_sequence = expected
                    .expected_sequence
                    .checked_sub(1)
                    .ok_or_else(internal_state_error)?;
                require_anchor_field(
                    abort.transaction() == expected.transaction,
                    BrokerSessionErrorKindV1::AnchorTransactionMismatch,
                )?;
                require_anchor_field(
                    abort.sequence() == prior_sequence,
                    BrokerSessionErrorKindV1::AnchorSequenceMismatch,
                )?;
                require_anchor_field(
                    abort.head() == expected.prior_head,
                    BrokerSessionErrorKindV1::AnchorPriorMismatch,
                )?;
                require_anchor_field(
                    abort.proposed_head() == expected.proposed_head,
                    BrokerSessionErrorKindV1::AnchorProposedMismatch,
                )?;
                require_anchor_field(
                    abort.observed_nonce() == &expected.nonce,
                    BrokerSessionErrorKindV1::AnchorNonceMismatch,
                )?;
                Ok(BrokerSessionStageV1::Aborted)
            }
        })();
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                self.stage = BrokerSessionStageV1::Invalidated;
                return Err(error);
            }
        };
        if result == BrokerSessionStageV1::AnchorCommitted {
            let mut canonical = [0_u8; ANCHOR_OBSERVATION_WIRE_LEN_V1];
            canonical.copy_from_slice(observation);
            self.anchor_commit = Some(AnchorCommitBindingV1 {
                observation: canonical,
            });
        }
        self.stage = result;
        Ok(self.observation())
    }

    fn consume_publication(
        &mut self,
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<BrokerSessionObservationV1, BrokerSessionMachineErrorV1> {
        self.require_stage(BrokerSessionStageV1::AnchorCommitted)?;
        let retained = self.completion.ok_or_else(internal_state_error)?;
        let supplied = CompletionBindingV1::from_transcript(transcript)?;
        if supplied.claim_digest != retained.claim_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::TranscriptClaimMismatch,
            ));
        }
        if supplied.output_digest != retained.output_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::OutputDigestMismatch,
            ));
        }
        if supplied.durable_plan != retained.durable_plan {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::DurablePublicationPlanMismatch,
            ));
        }
        if supplied.transcript_digest != retained.transcript_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::TranscriptDigestMismatch,
            ));
        }
        self.stage = BrokerSessionStageV1::Consumed;
        Ok(self.observation())
    }

    fn validate_committed_publication(
        &self,
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<(), BrokerSessionMachineErrorV1> {
        self.require_stage(BrokerSessionStageV1::AnchorCommitted)?;
        self.validate_consumed_transcript(transcript)
    }

    fn validate_consumed_transcript(
        &self,
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<(), BrokerSessionMachineErrorV1> {
        let retained = self.completion.ok_or_else(internal_state_error)?;
        let supplied = CompletionBindingV1::from_transcript(transcript)?;
        if supplied.claim_digest != retained.claim_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::TranscriptClaimMismatch,
            ));
        }
        if supplied.output_digest != retained.output_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::OutputDigestMismatch,
            ));
        }
        if supplied.durable_plan != retained.durable_plan {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::DurablePublicationPlanMismatch,
            ));
        }
        if supplied.transcript_digest != retained.transcript_digest {
            return Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::TranscriptDigestMismatch,
            ));
        }
        Ok(())
    }

    fn anchor_challenge(
        &self,
    ) -> Result<[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1], BrokerSessionMachineErrorV1> {
        self.anchor_expected
            .map(|expected| expected.challenge)
            .ok_or_else(internal_state_error)
    }

    fn durable_binding(&self) -> Result<BrokerDurableBindingV1, BrokerSessionMachineErrorV1> {
        let reservation = self.reservation.ok_or_else(internal_state_error)?;
        let completion = self.completion.ok_or_else(internal_state_error)?;
        let link = self.link.ok_or_else(internal_state_error)?;
        Ok(BrokerDurableBindingV1 {
            session_id: reservation.session_id.0,
            session_nonce: reservation.nonce.0,
            reservation_digest: link.broker_reservation,
            request_nonce_sha256: link.request_nonce_sha256,
            client_pid: reservation.client_pid,
            client_start_time_ticks: reservation.client_start_time_ticks,
            claim_digest: reservation.claim_digest,
            transcript_digest: completion.transcript_digest,
            transcript_binding_identity: completion.transcript_binding_identity,
            transcript_request_identity: completion.transcript_request_identity,
            transcript_plan_identity: completion.transcript_plan_identity,
            transcript_closure_identity: completion.transcript_closure_identity,
            transcript_grant_identity: completion.transcript_grant_identity,
            output_digest: completion.output_digest,
            output_length: completion.output_length,
            output_mode: completion.output_mode,
            durable_plan: completion.durable_plan.0,
        })
    }

    fn anchor_transaction(
        &self,
        service_attempt_nonce: [u8; 32],
    ) -> Result<TransactionDigestV1, BrokerSessionMachineErrorV1> {
        let reservation = self.reservation.ok_or_else(internal_state_error)?;
        let completion = self.completion.ok_or_else(internal_state_error)?;
        let mut digest = Sha256::new();
        digest.update(BROKER_SESSION_ANCHOR_TRANSACTION_DOMAIN_V1);
        digest.update(reservation.session_id.0);
        digest.update(reservation.nonce.0);
        digest.update(reservation.claim_digest);
        digest.update(link_binding_bytes(self.link)?);
        digest.update(completion.transcript_digest);
        digest.update(completion.output_digest);
        digest.update(completion.durable_plan.0);
        digest.update(service_attempt_nonce);
        Ok(TransactionDigestV1::from_bytes(digest.finalize().into()))
    }

    fn require_stage(
        &self,
        expected: BrokerSessionStageV1,
    ) -> Result<(), BrokerSessionMachineErrorV1> {
        if self.stage == expected {
            Ok(())
        } else {
            Err(BrokerSessionMachineErrorV1::new(
                BrokerSessionErrorKindV1::TransitionOrder,
            ))
        }
    }

    const fn observation(&self) -> BrokerSessionObservationV1 {
        BrokerSessionObservationV1 { stage: self.stage }
    }
}

fn link_binding_bytes(
    link: Option<LinkBindingV1>,
) -> Result<[u8; 64], BrokerSessionMachineErrorV1> {
    let link = link.ok_or_else(internal_state_error)?;
    let mut bytes = [0_u8; 64];
    bytes[..32].copy_from_slice(&link.broker_reservation);
    bytes[32..].copy_from_slice(&link.request_nonce_sha256);
    Ok(bytes)
}

/// Derives a canonical digest over every field of one completed Broker V4 transcript.
pub fn completed_broker_transcript_digest_v1(transcript: &CompletedBrokerTranscriptV4) -> [u8; 32] {
    let process = transcript.process();
    let mut digest = Sha256::new();
    digest.update(BROKER_V4_COMPLETED_TRANSCRIPT_DIGEST_DOMAIN_V1);
    digest.update(transcript.binding_identity());
    digest.update(process.pid().to_le_bytes());
    digest.update(process.start_time_ticks().to_le_bytes());
    digest.update(transcript.request_identity());
    digest.update(transcript.plan_identity());
    digest.update(transcript.closure_identity());
    digest.update(transcript.grant_identity());
    digest.update(transcript.output_sha256());
    digest.update(transcript.output_length().to_le_bytes());
    digest.update(transcript.output_mode().to_le_bytes());
    digest.update(transcript.durable_plan_identity());
    digest.finalize().into()
}

fn broker_session_claim_digest_v1(claim: BrokerSessionClaimV4) -> [u8; 32] {
    let process = claim.process();
    let mut digest = Sha256::new();
    digest.update(BROKER_SESSION_CLAIM_DIGEST_DOMAIN_V1);
    digest.update(claim.binding_identity());
    digest.update(process.pid().to_le_bytes());
    digest.update(process.start_time_ticks().to_le_bytes());
    digest.update(claim.request_identity());
    digest.update(claim.plan_identity());
    digest.update(claim.closure_identity());
    digest.finalize().into()
}

fn broker_link_reservation_digest_v1(reservation: ReservationBindingV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(BROKER_LINK_RESERVATION_DIGEST_DOMAIN_V1);
    digest.update(reservation.session_id.0);
    digest.update(reservation.nonce.0);
    digest.update(reservation.claim_digest);
    digest.update(reservation.client_pid.to_le_bytes());
    digest.update(reservation.client_start_time_ticks.to_le_bytes());
    digest.update(reservation.host_link_plan);
    digest.update(reservation.host_link_closure);
    digest.update(reservation.durable_plan.0);
    digest.finalize().into()
}

fn require_nonzero(
    bytes: [u8; 32],
    kind: BrokerSessionErrorKindV1,
) -> Result<(), BrokerSessionMachineErrorV1> {
    if bytes.iter().all(|byte| *byte == 0) {
        Err(BrokerSessionMachineErrorV1::new(kind))
    } else {
        Ok(())
    }
}

fn require_anchor_field(
    condition: bool,
    kind: BrokerSessionErrorKindV1,
) -> Result<(), BrokerSessionMachineErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(BrokerSessionMachineErrorV1::new(kind))
    }
}

const fn internal_state_error() -> BrokerSessionMachineErrorV1 {
    BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::InternalState)
}

const fn anchor_protocol_error() -> BrokerSessionMachineErrorV1 {
    BrokerSessionMachineErrorV1::new(BrokerSessionErrorKindV1::AnchorProtocol)
}

#[cfg(test)]
mod tests;
