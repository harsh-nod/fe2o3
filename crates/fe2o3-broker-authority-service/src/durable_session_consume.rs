//! Service-owned durable consume of one committed broker session.
//!
//! The transaction durably stages exact W0 bytes, generates an unpredictable service-owned anchor
//! attempt nonce, forms the nonce-bound challenge internally, and commits a prepared record before
//! challenge bytes can leave the service boundary. A valid proposed-position observation is then
//! recorded, the staged artifact is renamed to the exact plan-bound final component, and a
//! published record is committed. A valid prior-position observation records an abort and never
//! publishes. Recovery replays only one exact legal redo transition and can resume prepared,
//! anchor-committed, published, aborted, or invalid state from a supervisor-retained directory
//! descriptor.
//!
//! There is intentionally no path API. Storage is reached only through the service-owned root
//! retained by `ProtectedBrokerServiceAdmissionV1`, or through a fresh supervisor-supplied root
//! descriptor during restart. No advisory-lock inode, environment variable, `PATH` lookup, client
//! bearer token, linker, loader, or runtime operation participates.
//!
//! `AUTHORITY=none`: same-host storage is not anti-rollback storage. A valid signed observation
//! authenticates only under the caller-pinned key. Linux `getrandom` closes deterministic
//! challenge reconstruction only within the stated process boundary; this crate supplies no
//! durable nonce-freshness service, key provenance, anti-rollback state, external-anchor
//! availability, cross-system atomicity, multi-writer exclusion, publication authority,
//! runtime/GPU authority, or parity claim.
//!
//! The record's domain-separated SHA-256 checksum detects accidental truncation or corruption
//! only. It is unkeyed, authenticates nothing, and does not resist hostile same-UID replacement.
//! Every authority-relevant relation is independently recomputed, including the plan-pinned key,
//! V4 transcript, process identity, W0 reservation/nonce/output, challenge, and signature.
//!
//! A crash after the external service commits but before the local anchor-committed record leaves
//! the local state `Prepared`; recovery must obtain the exact signed observation again. Once that
//! observation is durably recorded, recovery may publish the already-durable staged bytes. This
//! is an explicit reconciliation protocol, not an atomic transaction across anchor and storage.

use crate::session::{
    BrokerAnchorPreparedSessionV1, BrokerCommittedPublicationPartsV1, BrokerDurableBindingV1,
    BrokerSessionMachineV1, BrokerSessionStageV1, CommittedBrokerPublicationV1,
};
use crate::{BrokerSessionMachineErrorV1, DurablePublicationPlanIdentityV1};
use fe2o3_artifact_transaction::{
    NoRetainedDurableDirectoryHooksV1, RetainedDurableArtifactBoundaryV1,
    RetainedDurableDirectoryErrorV1, RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1,
    RetainedDurableFaultTimingV1, RetainedDurableRecordBoundaryV1,
};
use fe2o3_build_authority::{CompletedBrokerTranscriptV4, HOST_LINK_OUTPUT_MODE_V4};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorChallengeV1,
    AnchorDecisionV1, CallerNonceV1, ChallengeKindV1, PinnedAnchorKeyV1, PreparedAnchorAdvanceV1,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;

/// Fixed semantic marker for this entire bounded foundation.
pub const BROKER_DURABLE_SESSION_AUTHORITY_V1: &str = "none";
/// Maximum exact admitted output durably staged by V1.
pub const MAX_BROKER_DURABLE_OUTPUT_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum canonical V1 session record.
pub const MAX_BROKER_DURABLE_RECORD_BYTES_V1: usize = 2_048;

const RECORD_MAGIC: &[u8; 8] = b"F2DBS1\0\0";
const RECORD_VERSION: u16 = 1;
const PLAN_DOMAIN: &[u8] = b"FE2O3/BROKER-DURABLE/PUBLICATION-PLAN/V1\0";
// Corruption/truncation detection only. This unkeyed digest authenticates nothing; all semantic
// fields and signed anchor relations are independently revalidated below.
const RECORD_CHECKSUM_DOMAIN: &[u8] = b"FE2O3/BROKER-DURABLE/RECORD-CHECKSUM/V1\0";
const MAX_DESTINATION_BYTES: usize = 128;

/// Exact service-relative final destination and its canonical broker plan identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DurableBrokerPublicationPlanV1 {
    destination: String,
    anchor_key_identity: [u8; 32],
    identity: [u8; 32],
}

impl DurableBrokerPublicationPlanV1 {
    /// Derives a domain-separated identity for one private-root-relative file component.
    pub fn new(
        destination: impl Into<String>,
        anchor_key: &PinnedAnchorKeyV1,
    ) -> Result<Self, BrokerDurableSessionErrorV1> {
        let destination = destination.into();
        validate_destination(&destination)?;
        let anchor_key_identity = anchor_key.identity().to_bytes();
        let length = u16::try_from(destination.len()).map_err(|_| {
            BrokerDurableSessionErrorV1::InvalidPlan("destination length does not fit V1".into())
        })?;
        let identity = sha256_parts(&[
            PLAN_DOMAIN,
            &RECORD_VERSION.to_le_bytes(),
            &length.to_le_bytes(),
            destination.as_bytes(),
            &anchor_key_identity,
        ]);
        if identity.iter().all(|byte| *byte == 0) {
            return Err(BrokerDurableSessionErrorV1::InvalidPlan(
                "derived plan identity is zero".into(),
            ));
        }
        Ok(Self {
            destination,
            anchor_key_identity,
            identity,
        })
    }

    /// Returns the exact final component, not an authority path.
    pub fn destination(&self) -> &str {
        &self.destination
    }

    /// Returns canonical identity bytes for the pre-link broker reservation and V4 transcript.
    pub const fn identity_bytes(&self) -> [u8; 32] {
        self.identity
    }

    /// Returns the exact caller-pinned external-anchor key identity bound by this plan.
    pub const fn anchor_key_identity(&self) -> [u8; 32] {
        self.anchor_key_identity
    }

    /// Constructs the matching broker identity wrapper.
    pub fn broker_identity(
        &self,
    ) -> Result<DurablePublicationPlanIdentityV1, BrokerSessionMachineErrorV1> {
        DurablePublicationPlanIdentityV1::from_bytes(self.identity)
    }
}

/// Semantic journal state associated with an injected record fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerDurableRecordStageV1 {
    Prepared,
    AnchorCommitted,
    Published,
    Aborted,
    Invalid,
}

/// Exact deterministic crash boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerDurableFaultPointV1 {
    Record {
        stage: BrokerDurableRecordStageV1,
        boundary: RetainedDurableRecordBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    },
    Artifact {
        boundary: RetainedDurableArtifactBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    },
}

/// Bounded deterministic storage fault-injection options.
///
/// Entropy selection is not part of the public API:
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerDurableOptionsV1;
/// let _ = BrokerDurableOptionsV1::with_test_service_nonce([7; 32]);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BrokerDurableOptionsV1 {
    fault: Option<BrokerDurableFaultPointV1>,
    #[cfg(test)]
    entropy: TestServiceEntropyV1,
}

impl BrokerDurableOptionsV1 {
    pub const fn inject_crash(point: BrokerDurableFaultPointV1) -> Self {
        Self {
            fault: Some(point),
            #[cfg(test)]
            entropy: TestServiceEntropyV1::Os,
        }
    }

    #[cfg(test)]
    const fn with_test_service_nonce(nonce: [u8; 32]) -> Self {
        Self {
            fault: None,
            entropy: TestServiceEntropyV1::Fixed(nonce),
        }
    }

    #[cfg(test)]
    const fn with_test_entropy_failure() -> Self {
        Self {
            fault: None,
            entropy: TestServiceEntropyV1::Fail,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TestServiceEntropyV1 {
    #[default]
    Os,
    Fixed([u8; 32]),
    Fail,
}

/// Terminal result of one live durable session transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerDurableOutcomeV1 {
    Published,
    Aborted,
}

/// Deterministic state observed or completed during restart recovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerDurableRecoveryV1 {
    Prepared,
    AnchorCommitted,
    Published,
    Aborted,
    Invalid,
}

/// Inspects and validates the exact durable state after replaying only a legal complete redo.
///
/// This does not query the external anchor or advance a prepared/anchor-committed transaction.
pub fn inspect_durable_broker_session_v1(
    service_root: OwnedFd,
    plan: DurableBrokerPublicationPlanV1,
) -> Result<BrokerDurableRecoveryV1, BrokerDurableSessionErrorV1> {
    let store = RetainedDurableDirectoryV1::admit_service_owned(service_root)?;
    let names = DurableNames::new(plan.identity);
    recover_redo(&store, &names, &plan, None)?;
    let bytes = store
        .read_private(&names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)?
        .ok_or_else(|| {
            BrokerDurableSessionErrorV1::InvalidRecord("canonical session record is absent".into())
        })?;
    let record = DurableRecordV1::decode(&bytes)?;
    validate_record(&record, &plan)?;
    match record.state {
        RecordStateV1::Prepared => {
            verify_staged(&store, &names, &record)?;
            require_no_public_destination(&store, &plan, &record)?;
            Ok(BrokerDurableRecoveryV1::Prepared)
        }
        RecordStateV1::AnchorCommitted => {
            read_staged_or_final(&store, &names, &plan, &record)?;
            Ok(BrokerDurableRecoveryV1::AnchorCommitted)
        }
        RecordStateV1::Published => {
            verify_final(&store, &plan, &record)?;
            Ok(BrokerDurableRecoveryV1::Published)
        }
        RecordStateV1::Aborted => {
            if store
                .read_published(
                    &plan.destination,
                    MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
                    record.binding.output_mode,
                )?
                .is_some()
            {
                return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                    "aborted transaction has a public destination".into(),
                ));
            }
            Ok(BrokerDurableRecoveryV1::Aborted)
        }
        RecordStateV1::Invalid => {
            require_no_public_destination(&store, &plan, &record)?;
            Ok(BrokerDurableRecoveryV1::Invalid)
        }
    }
}

/// Durable broker-session failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum BrokerDurableSessionErrorV1 {
    Broker(BrokerSessionMachineErrorV1),
    Storage(RetainedDurableDirectoryErrorV1),
    InvalidPlan(String),
    InvalidRecord(String),
    BindingMismatch(&'static str),
    DuplicateConsume,
    OutputSize { actual: u64, maximum: usize },
    OutputRead(io::Error),
    Entropy(io::Error),
    AnchorObservation,
    InjectedCrash { point: BrokerDurableFaultPointV1 },
}

impl fmt::Display for BrokerDurableSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Broker(error) => write!(
                formatter,
                "broker session rejected durable consume: {error}"
            ),
            Self::Storage(error) => write!(formatter, "durable broker storage failed: {error}"),
            Self::InvalidPlan(reason) => write!(formatter, "invalid durable broker plan: {reason}"),
            Self::InvalidRecord(reason) => {
                write!(formatter, "invalid durable broker record: {reason}")
            }
            Self::BindingMismatch(field) => {
                write!(formatter, "durable broker binding mismatch: {field}")
            }
            Self::DuplicateConsume => {
                formatter.write_str("durable broker plan was already consumed or occupied")
            }
            Self::OutputSize { actual, maximum } => write!(
                formatter,
                "broker output size {actual} is outside 1..={maximum}"
            ),
            Self::OutputRead(error) => write!(
                formatter,
                "cannot read exact admitted broker output: {error}"
            ),
            Self::Entropy(error) => {
                write!(
                    formatter,
                    "cannot generate broker service attempt nonce: {error}"
                )
            }
            Self::AnchorObservation => formatter
                .write_str("external-anchor observation is not valid for this prepared session"),
            Self::InjectedCrash { point } => {
                write!(formatter, "injected durable broker crash at {point:?}")
            }
        }
    }
}

impl std::error::Error for BrokerDurableSessionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Broker(error) => Some(error),
            Self::Storage(error) => Some(error),
            Self::OutputRead(error) => Some(error),
            Self::Entropy(error) => Some(error),
            _ => None,
        }
    }
}

impl From<BrokerSessionMachineErrorV1> for BrokerDurableSessionErrorV1 {
    fn from(error: BrokerSessionMachineErrorV1) -> Self {
        Self::Broker(error)
    }
}

impl From<RetainedDurableDirectoryErrorV1> for BrokerDurableSessionErrorV1 {
    fn from(error: RetainedDurableDirectoryErrorV1) -> Self {
        Self::Storage(error)
    }
}

/// Move-only live transaction. Its machine cannot observe the anchor before preparation returns.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerDurableSessionTransactionV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<BrokerDurableSessionTransactionV1>();
/// ```
pub struct BrokerDurableSessionTransactionV1 {
    machine: Option<BrokerSessionMachineV1>,
    store: RetainedDurableDirectoryV1,
    plan: DurableBrokerPublicationPlanV1,
    names: DurableNames,
    prepared: DurableRecordV1,
    output_bytes: Vec<u8>,
}

impl fmt::Debug for BrokerDurableSessionTransactionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerDurableSessionTransactionV1")
            .field("authority", &BROKER_DURABLE_SESSION_AUTHORITY_V1)
            .field("destination", &self.plan.destination)
            .finish_non_exhaustive()
    }
}

impl BrokerDurableSessionTransactionV1 {
    pub const fn authority(&self) -> &'static str {
        BROKER_DURABLE_SESSION_AUTHORITY_V1
    }

    /// Returns exact challenge bytes only; no session or publication capability leaves the value.
    pub fn challenge_bytes(&self) -> &[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1] {
        &self.prepared.challenge
    }

    /// Consumes the live transaction through one signed observation and terminal publication or abort.
    pub fn observe_and_consume(
        self,
        observation: &[u8],
        transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<BrokerDurableOutcomeV1, BrokerDurableSessionErrorV1> {
        self.observe_and_consume_with_options(
            observation,
            transcript,
            BrokerDurableOptionsV1::default(),
        )
    }

    pub fn observe_and_consume_with_options(
        mut self,
        observation: &[u8],
        transcript: &CompletedBrokerTranscriptV4,
        options: BrokerDurableOptionsV1,
    ) -> Result<BrokerDurableOutcomeV1, BrokerDurableSessionErrorV1> {
        let observation_result = self
            .machine
            .as_mut()
            .ok_or_else(|| {
                BrokerDurableSessionErrorV1::InvalidRecord(
                    "session machine was already consumed".into(),
                )
            })?
            .observe_anchor(observation);
        let stage = match observation_result {
            Ok(observation) => observation.stage(),
            Err(error) => {
                let invalid = self.prepared.successor(RecordStateV1::Invalid, None)?;
                let mut faults =
                    FaultInjector::new(options.fault, BrokerDurableRecordStageV1::Invalid);
                let _ = commit_record(&self.store, &self.names, &invalid, &mut faults);
                return Err(BrokerDurableSessionErrorV1::Broker(error));
            }
        };
        match stage {
            BrokerSessionStageV1::Aborted => {
                let observation = exact_observation(observation)?;
                let aborted = self
                    .prepared
                    .successor(RecordStateV1::Aborted, Some(observation))?;
                validate_record(&aborted, &self.plan)?;
                let mut faults =
                    FaultInjector::new(options.fault, BrokerDurableRecordStageV1::Aborted);
                commit_record(&self.store, &self.names, &aborted, &mut faults)?;
                Ok(BrokerDurableOutcomeV1::Aborted)
            }
            BrokerSessionStageV1::AnchorCommitted => {
                let committed_capability = self
                    .machine
                    .take()
                    .ok_or_else(|| {
                        BrokerDurableSessionErrorV1::InvalidRecord(
                            "session machine was already consumed".into(),
                        )
                    })?
                    .into_committed_publication(transcript)?;
                self.consume_committed(committed_capability, observation, options)
            }
            _ => Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "anchor observation produced a nonterminal session stage".into(),
            )),
        }
    }

    fn consume_committed(
        self,
        capability: CommittedBrokerPublicationV1,
        observation: &[u8],
        options: BrokerDurableOptionsV1,
    ) -> Result<BrokerDurableOutcomeV1, BrokerDurableSessionErrorV1> {
        let parts = capability.into_parts();
        validate_committed_parts(&self.store, &self.prepared, &self.output_bytes, &parts)?;
        if parts.anchor_observation.as_slice() != observation {
            return Err(BrokerDurableSessionErrorV1::BindingMismatch(
                "anchor observation",
            ));
        }
        let committed = self.prepared.successor(
            RecordStateV1::AnchorCommitted,
            Some(parts.anchor_observation),
        )?;
        validate_record(&committed, &self.plan)?;
        let mut faults =
            FaultInjector::new(options.fault, BrokerDurableRecordStageV1::AnchorCommitted);
        commit_record(&self.store, &self.names, &committed, &mut faults)?;
        faults.stage = BrokerDurableRecordStageV1::Published;
        let publish_result = self.store.publish_staged(
            &self.names.staged,
            &self.plan.destination,
            &self.output_bytes,
            self.prepared.binding.output_mode,
            &mut faults,
        );
        map_storage_fault(&faults, publish_result)?;
        let published = committed.successor(RecordStateV1::Published, committed.observation)?;
        commit_record(&self.store, &self.names, &published, &mut faults)?;
        Ok(BrokerDurableOutcomeV1::Published)
    }
}

/// Move-only recovery transaction admitted only from one validated canonical `Prepared` record.
///
/// Redo promotion, record validation, and staged-W0 validation complete before construction. This
/// is the only restart API that re-emits challenge bytes; it accepts no path and retains the fresh
/// supervisor-supplied service-root descriptor.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerRecoveredPreparedSessionV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<BrokerRecoveredPreparedSessionV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::BrokerRecoveredPreparedSessionV1;
/// fn require_serialize<T: serde::Serialize>() {}
/// require_serialize::<BrokerRecoveredPreparedSessionV1>();
/// ```
pub struct BrokerRecoveredPreparedSessionV1 {
    store: RetainedDurableDirectoryV1,
    plan: DurableBrokerPublicationPlanV1,
    names: DurableNames,
    prepared: DurableRecordV1,
}

impl fmt::Debug for BrokerRecoveredPreparedSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrokerRecoveredPreparedSessionV1")
            .field("authority", &BROKER_DURABLE_SESSION_AUTHORITY_V1)
            .field("destination", &self.plan.destination)
            .finish_non_exhaustive()
    }
}

impl BrokerRecoveredPreparedSessionV1 {
    pub const fn authority(&self) -> &'static str {
        BROKER_DURABLE_SESSION_AUTHORITY_V1
    }

    /// Returns the exact challenge recovered from validated durable `Prepared` state.
    pub fn challenge_bytes(&self) -> &[u8; ANCHOR_CHALLENGE_WIRE_LEN_V1] {
        &self.prepared.challenge
    }

    /// Reconciles one exact signed observation and consumes this recovered transaction.
    pub fn observe_and_recover(
        self,
        observation: &[u8],
    ) -> Result<BrokerDurableRecoveryV1, BrokerDurableSessionErrorV1> {
        self.observe_and_recover_with_options(observation, BrokerDurableOptionsV1::default())
    }

    pub fn observe_and_recover_with_options(
        self,
        observation: &[u8],
        options: BrokerDurableOptionsV1,
    ) -> Result<BrokerDurableRecoveryV1, BrokerDurableSessionErrorV1> {
        let decision = verify_record_observation(&self.prepared, observation)?;
        match decision {
            AnchorDecisionV1::Abort(_) => {
                let aborted = self.prepared.successor(
                    RecordStateV1::Aborted,
                    Some(exact_observation(observation)?),
                )?;
                let mut faults =
                    FaultInjector::new(options.fault, BrokerDurableRecordStageV1::Aborted);
                commit_record(&self.store, &self.names, &aborted, &mut faults)?;
                Ok(BrokerDurableRecoveryV1::Aborted)
            }
            AnchorDecisionV1::Commit(_) => {
                let committed = self.prepared.successor(
                    RecordStateV1::AnchorCommitted,
                    Some(exact_observation(observation)?),
                )?;
                finish_recovered_commit(&self.store, &self.names, &self.plan, committed, options)
            }
        }
    }
}

/// Consumes one pre-challenge session and durably enables its anchor challenge.
///
/// Exact W0 bytes are staged and directory-synced before Linux `getrandom` supplies a service-owned
/// attempt nonce. The nonce is bound into the transaction and challenge before the canonical
/// `Prepared` record is committed and directory-synced. The returned transaction is the first
/// public value with a challenge-byte accessor; all errors consume the capability without
/// releasing a usable challenge.
pub fn prepare_durable_broker_session_v1(
    prepared_session: BrokerAnchorPreparedSessionV1,
    plan: DurableBrokerPublicationPlanV1,
) -> Result<BrokerDurableSessionTransactionV1, BrokerDurableSessionErrorV1> {
    prepare_durable_broker_session_v1_with_options(
        prepared_session,
        plan,
        BrokerDurableOptionsV1::default(),
    )
}

pub fn prepare_durable_broker_session_v1_with_options(
    prepared_session: BrokerAnchorPreparedSessionV1,
    plan: DurableBrokerPublicationPlanV1,
    options: BrokerDurableOptionsV1,
) -> Result<BrokerDurableSessionTransactionV1, BrokerDurableSessionErrorV1> {
    let mut machine = prepared_session.into_machine();
    let parts = machine.durable_preparation_parts()?;
    let store = RetainedDurableDirectoryV1::admit_service_owned(parts.service_root)?;
    let names = DurableNames::new(plan.identity);
    if parts.binding.durable_plan != plan.identity {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch("durable plan"));
    }
    let prepared_key = PinnedAnchorKeyV1::from_bytes(parts.anchor_key_bytes)
        .map_err(|_| BrokerDurableSessionErrorV1::BindingMismatch("anchor key"))?;
    if prepared_key.identity().to_bytes() != plan.anchor_key_identity {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch("anchor key"));
    }
    if store
        .read_private(&names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)?
        .is_some()
        || store
            .read_private(&names.redo, MAX_BROKER_DURABLE_RECORD_BYTES_V1)?
            .is_some()
        || store
            .read_private(&names.staged, MAX_BROKER_DURABLE_OUTPUT_BYTES_V1)?
            .is_some()
        || store
            .read_published(
                &plan.destination,
                MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
                HOST_LINK_OUTPUT_MODE_V4,
            )?
            .is_some()
    {
        return Err(BrokerDurableSessionErrorV1::DuplicateConsume);
    }
    let output_bytes = read_exact_output(&parts.output_file, parts.binding.output_length)?;
    require_output_identity(&output_bytes, &parts.binding)?;
    let mut faults = FaultInjector::new(options.fault, BrokerDurableRecordStageV1::Prepared);
    let stage_result = store.stage_artifact(
        &names.staged,
        &output_bytes,
        MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
        &mut faults,
    );
    map_storage_fault(&faults, stage_result)?;
    let service_attempt_nonce = fresh_service_attempt_nonce(options, parts.binding.session_nonce)?;
    machine.begin_anchor_with_service_nonce(service_attempt_nonce)?;
    let challenge = machine.anchor_challenge()?;
    let prepared = DurableRecordV1 {
        state: RecordStateV1::Prepared,
        plan_identity: plan.identity,
        destination: plan.destination.clone(),
        binding: parts.binding,
        challenge,
        anchor_key_bytes: parts.anchor_key_bytes,
        observation: None,
    };
    validate_record(&prepared, &plan)?;
    commit_record(&store, &names, &prepared, &mut faults)?;
    Ok(BrokerDurableSessionTransactionV1 {
        machine: Some(machine),
        store,
        plan,
        names,
        prepared,
        output_bytes,
    })
}

/// Admits one exact canonical `Prepared` record as an opaque restart transaction.
///
/// The challenge is unavailable unless redo recovery, a directory-sync/revalidation barrier for
/// the canonical record, and exact staged-W0 and destination validation all succeed through the
/// retained service-root descriptor.
pub fn recover_prepared_durable_broker_session_v1(
    service_root: OwnedFd,
    plan: DurableBrokerPublicationPlanV1,
) -> Result<BrokerRecoveredPreparedSessionV1, BrokerDurableSessionErrorV1> {
    recover_prepared_durable_broker_session_v1_with_hooks(
        service_root,
        plan,
        &mut NoRetainedDurableDirectoryHooksV1,
    )
}

fn recover_prepared_durable_broker_session_v1_with_hooks(
    service_root: OwnedFd,
    plan: DurableBrokerPublicationPlanV1,
    hooks: &mut impl RetainedDurableDirectoryHooksV1,
) -> Result<BrokerRecoveredPreparedSessionV1, BrokerDurableSessionErrorV1> {
    let store = RetainedDurableDirectoryV1::admit_service_owned(service_root)?;
    let names = DurableNames::new(plan.identity);
    recover_redo(&store, &names, &plan, None)?;
    let bytes = store
        .read_private(&names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)?
        .ok_or_else(|| {
            BrokerDurableSessionErrorV1::InvalidRecord("canonical session record is absent".into())
        })?;
    let visible = DurableRecordV1::decode(&bytes)?;
    validate_record(&visible, &plan)?;
    if visible.state != RecordStateV1::Prepared {
        return Err(BrokerDurableSessionErrorV1::InvalidRecord(
            "canonical session is not prepared".into(),
        ));
    }
    let durable_bytes = store.establish_recovered_record_durability(
        &names.record,
        &names.redo,
        &bytes,
        MAX_BROKER_DURABLE_RECORD_BYTES_V1,
        hooks,
    )?;
    let prepared = DurableRecordV1::decode(&durable_bytes)?;
    validate_record(&prepared, &plan)?;
    if prepared.state != RecordStateV1::Prepared {
        return Err(BrokerDurableSessionErrorV1::InvalidRecord(
            "canonical session changed across the recovery durability barrier".into(),
        ));
    }
    verify_staged(&store, &names, &prepared)?;
    require_no_public_destination(&store, &plan, &prepared)?;
    Ok(BrokerRecoveredPreparedSessionV1 {
        store,
        plan,
        names,
        prepared,
    })
}

/// Recovers one exact plan from a fresh supervisor-supplied service-root descriptor.
pub fn recover_durable_broker_session_v1(
    service_root: OwnedFd,
    plan: DurableBrokerPublicationPlanV1,
    external_observation: Option<&[u8]>,
) -> Result<BrokerDurableRecoveryV1, BrokerDurableSessionErrorV1> {
    recover_durable_broker_session_v1_with_options(
        service_root,
        plan,
        external_observation,
        BrokerDurableOptionsV1::default(),
    )
}

pub fn recover_durable_broker_session_v1_with_options(
    service_root: OwnedFd,
    plan: DurableBrokerPublicationPlanV1,
    external_observation: Option<&[u8]>,
    options: BrokerDurableOptionsV1,
) -> Result<BrokerDurableRecoveryV1, BrokerDurableSessionErrorV1> {
    let store = RetainedDurableDirectoryV1::admit_service_owned(service_root)?;
    let names = DurableNames::new(plan.identity);
    recover_redo(&store, &names, &plan, options.fault)?;
    let Some(bytes) = store.read_private(&names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)? else {
        if store
            .read_private(&names.staged, MAX_BROKER_DURABLE_OUTPUT_BYTES_V1)?
            .is_some()
            || store
                .read_published(
                    &plan.destination,
                    MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
                    HOST_LINK_OUTPUT_MODE_V4,
                )?
                .is_some()
        {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "orphan artifact exists without a canonical session record".into(),
            ));
        }
        return Err(BrokerDurableSessionErrorV1::InvalidRecord(
            "canonical session record is absent".into(),
        ));
    };
    let record = DurableRecordV1::decode(&bytes)?;
    validate_record(&record, &plan)?;
    match record.state {
        RecordStateV1::Prepared => {
            verify_staged(&store, &names, &record)?;
            require_no_public_destination(&store, &plan, &record)?;
            let Some(observation) = external_observation else {
                return Ok(BrokerDurableRecoveryV1::Prepared);
            };
            let decision = verify_record_observation(&record, observation)?;
            match decision {
                AnchorDecisionV1::Abort(_) => {
                    let aborted = record.successor(
                        RecordStateV1::Aborted,
                        Some(exact_observation(observation)?),
                    )?;
                    let mut faults =
                        FaultInjector::new(options.fault, BrokerDurableRecordStageV1::Aborted);
                    commit_record(&store, &names, &aborted, &mut faults)?;
                    Ok(BrokerDurableRecoveryV1::Aborted)
                }
                AnchorDecisionV1::Commit(_) => {
                    let committed = record.successor(
                        RecordStateV1::AnchorCommitted,
                        Some(exact_observation(observation)?),
                    )?;
                    finish_recovered_commit(&store, &names, &plan, committed, options)
                }
            }
        }
        RecordStateV1::AnchorCommitted => {
            if let Some(observation) = external_observation
                && record
                    .observation
                    .as_ref()
                    .map(<[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1]>::as_slice)
                    != Some(observation)
            {
                return Err(BrokerDurableSessionErrorV1::BindingMismatch(
                    "recovery observation",
                ));
            }
            finish_recovered_commit(&store, &names, &plan, record, options)
        }
        RecordStateV1::Published => {
            verify_final(&store, &plan, &record)?;
            Ok(BrokerDurableRecoveryV1::Published)
        }
        RecordStateV1::Aborted => {
            require_no_public_destination(&store, &plan, &record)?;
            Ok(BrokerDurableRecoveryV1::Aborted)
        }
        RecordStateV1::Invalid => {
            require_no_public_destination(&store, &plan, &record)?;
            Ok(BrokerDurableRecoveryV1::Invalid)
        }
    }
}

fn finish_recovered_commit(
    store: &RetainedDurableDirectoryV1,
    names: &DurableNames,
    plan: &DurableBrokerPublicationPlanV1,
    committed: DurableRecordV1,
    options: BrokerDurableOptionsV1,
) -> Result<BrokerDurableRecoveryV1, BrokerDurableSessionErrorV1> {
    let mut faults = FaultInjector::new(options.fault, BrokerDurableRecordStageV1::AnchorCommitted);
    let current = store.read_private(&names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)?;
    if current.as_deref() != Some(committed.encode()?.as_slice()) {
        commit_record(store, names, &committed, &mut faults)?;
    }
    let output_bytes = read_staged_or_final(store, names, plan, &committed)?;
    faults.stage = BrokerDurableRecordStageV1::Published;
    let publish_result = store.publish_staged(
        &names.staged,
        &plan.destination,
        &output_bytes,
        committed.binding.output_mode,
        &mut faults,
    );
    map_storage_fault(&faults, publish_result)?;
    let published = committed.successor(RecordStateV1::Published, committed.observation)?;
    commit_record(store, names, &published, &mut faults)?;
    Ok(BrokerDurableRecoveryV1::Published)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum RecordStateV1 {
    Prepared = 1,
    AnchorCommitted = 2,
    Published = 3,
    Aborted = 4,
    Invalid = 5,
}

impl RecordStateV1 {
    fn decode(value: u8) -> Result<Self, BrokerDurableSessionErrorV1> {
        match value {
            1 => Ok(Self::Prepared),
            2 => Ok(Self::AnchorCommitted),
            3 => Ok(Self::Published),
            4 => Ok(Self::Aborted),
            5 => Ok(Self::Invalid),
            _ => Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "unknown journal state".into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DurableRecordV1 {
    state: RecordStateV1,
    plan_identity: [u8; 32],
    destination: String,
    binding: BrokerDurableBindingV1,
    challenge: [u8; ANCHOR_CHALLENGE_WIRE_LEN_V1],
    anchor_key_bytes: [u8; 32],
    observation: Option<[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1]>,
}

impl DurableRecordV1 {
    fn successor(
        &self,
        state: RecordStateV1,
        observation: Option<[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1]>,
    ) -> Result<Self, BrokerDurableSessionErrorV1> {
        let legal = matches!(
            (self.state, state),
            (RecordStateV1::Prepared, RecordStateV1::AnchorCommitted)
                | (RecordStateV1::Prepared, RecordStateV1::Aborted)
                | (RecordStateV1::Prepared, RecordStateV1::Invalid)
                | (RecordStateV1::AnchorCommitted, RecordStateV1::Published)
                | (RecordStateV1::AnchorCommitted, RecordStateV1::Invalid)
        );
        if !legal {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "illegal durable session state transition".into(),
            ));
        }
        Ok(Self {
            state,
            plan_identity: self.plan_identity,
            destination: self.destination.clone(),
            binding: self.binding,
            challenge: self.challenge,
            anchor_key_bytes: self.anchor_key_bytes,
            observation,
        })
    }

    fn encode(&self) -> Result<Vec<u8>, BrokerDurableSessionErrorV1> {
        let destination_length = u16::try_from(self.destination.len()).map_err(|_| {
            BrokerDurableSessionErrorV1::InvalidRecord("destination length overflow".into())
        })?;
        let mut bytes = Vec::with_capacity(MAX_BROKER_DURABLE_RECORD_BYTES_V1);
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&RECORD_VERSION.to_le_bytes());
        bytes.push(self.state as u8);
        bytes.push(u8::from(self.observation.is_some()));
        bytes.extend_from_slice(&destination_length.to_le_bytes());
        bytes.extend_from_slice(&self.plan_identity);
        bytes.extend_from_slice(self.destination.as_bytes());
        encode_binding(&mut bytes, self.binding);
        bytes.extend_from_slice(&self.challenge);
        bytes.extend_from_slice(&self.anchor_key_bytes);
        if let Some(observation) = self.observation {
            bytes.extend_from_slice(&observation);
        }
        let checksum = sha256_parts(&[RECORD_CHECKSUM_DOMAIN, &bytes]);
        bytes.extend_from_slice(&checksum);
        if bytes.len() > MAX_BROKER_DURABLE_RECORD_BYTES_V1 {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "encoded record exceeds V1 bound".into(),
            ));
        }
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, BrokerDurableSessionErrorV1> {
        if bytes.len() > MAX_BROKER_DURABLE_RECORD_BYTES_V1 || bytes.len() < 32 {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "record length".into(),
            ));
        }
        let body_length = bytes.len() - 32;
        let (body, checksum) = bytes.split_at(body_length);
        if sha256_parts(&[RECORD_CHECKSUM_DOMAIN, body]) != checksum {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "record checksum".into(),
            ));
        }
        let mut decoder = Decoder::new(body);
        if decoder.take(RECORD_MAGIC.len())? != RECORD_MAGIC {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "record magic".into(),
            ));
        }
        if decoder.u16()? != RECORD_VERSION {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "record version".into(),
            ));
        }
        let state = RecordStateV1::decode(decoder.u8()?)?;
        let has_observation = match decoder.u8()? {
            0 => false,
            1 => true,
            _ => {
                return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                    "observation flag".into(),
                ));
            }
        };
        let destination_length = usize::from(decoder.u16()?);
        let plan_identity = decoder.array()?;
        let destination =
            String::from_utf8(decoder.take(destination_length)?.to_vec()).map_err(|_| {
                BrokerDurableSessionErrorV1::InvalidRecord("destination encoding".into())
            })?;
        let binding = decode_binding(&mut decoder)?;
        let challenge = decoder.array()?;
        let anchor_key_bytes = decoder.array()?;
        let observation = has_observation.then(|| decoder.array()).transpose()?;
        if !decoder.is_empty() {
            return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                "trailing record bytes".into(),
            ));
        }
        Ok(Self {
            state,
            plan_identity,
            destination,
            binding,
            challenge,
            anchor_key_bytes,
            observation,
        })
    }
}

struct DurableNames {
    record: String,
    redo: String,
    staged: String,
}

impl DurableNames {
    fn new(identity: [u8; 32]) -> Self {
        let suffix = hex(&identity);
        Self {
            record: format!(".fe2o3-broker-session-v1-{suffix}.record"),
            redo: format!(".fe2o3-broker-session-v1-{suffix}.redo"),
            staged: format!(".fe2o3-broker-session-v1-{suffix}.artifact"),
        }
    }
}

struct FaultInjector {
    requested: Option<BrokerDurableFaultPointV1>,
    fired: Option<BrokerDurableFaultPointV1>,
    stage: BrokerDurableRecordStageV1,
}

impl FaultInjector {
    const fn new(
        requested: Option<BrokerDurableFaultPointV1>,
        stage: BrokerDurableRecordStageV1,
    ) -> Self {
        Self {
            requested,
            fired: None,
            stage,
        }
    }

    fn hit(&mut self, point: BrokerDurableFaultPointV1) -> io::Result<()> {
        if self.requested == Some(point) {
            self.fired = Some(point);
            Err(io::Error::other("injected durable broker crash"))
        } else {
            Ok(())
        }
    }
}

impl RetainedDurableDirectoryHooksV1 for FaultInjector {
    fn record(
        &mut self,
        boundary: RetainedDurableRecordBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        self.hit(BrokerDurableFaultPointV1::Record {
            stage: self.stage,
            boundary,
            timing,
        })
    }

    fn artifact(
        &mut self,
        boundary: RetainedDurableArtifactBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        self.hit(BrokerDurableFaultPointV1::Artifact { boundary, timing })
    }
}

fn commit_record(
    store: &RetainedDurableDirectoryV1,
    names: &DurableNames,
    record: &DurableRecordV1,
    faults: &mut FaultInjector,
) -> Result<(), BrokerDurableSessionErrorV1> {
    let bytes = record.encode()?;
    let result = store.commit_record(
        &names.record,
        &names.redo,
        &bytes,
        MAX_BROKER_DURABLE_RECORD_BYTES_V1,
        faults,
    );
    map_storage_fault(faults, result)
}

fn map_storage_fault<T>(
    faults: &FaultInjector,
    result: Result<T, RetainedDurableDirectoryErrorV1>,
) -> Result<T, BrokerDurableSessionErrorV1> {
    match result {
        Ok(value) => Ok(value),
        Err(_) if faults.fired.is_some() => Err(BrokerDurableSessionErrorV1::InjectedCrash {
            point: faults.fired.expect("checked injected fault"),
        }),
        Err(error) => Err(BrokerDurableSessionErrorV1::Storage(error)),
    }
}

fn validate_committed_parts(
    store: &RetainedDurableDirectoryV1,
    prepared: &DurableRecordV1,
    staged_bytes: &[u8],
    parts: &BrokerCommittedPublicationPartsV1,
) -> Result<(), BrokerDurableSessionErrorV1> {
    if !store.matches_descriptor(&parts.service_root)? {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch("service root"));
    }
    if parts.binding != prepared.binding
        || parts.challenge != prepared.challenge
        || parts.anchor_key_bytes != prepared.anchor_key_bytes
    {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "committed session",
        ));
    }
    if *parts.output.sha256().as_bytes() != prepared.binding.output_digest
        || parts.output.size() != prepared.binding.output_length
        || parts.output.mode() != prepared.binding.output_mode
    {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "committed output",
        ));
    }
    let file = parts.output.try_clone_file().map_err(|error| {
        BrokerDurableSessionErrorV1::OutputRead(io::Error::other(error.to_string()))
    })?;
    let current_bytes = read_exact_output(&file, parts.output.size())?;
    if current_bytes != staged_bytes {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "retained output bytes",
        ));
    }
    Ok(())
}

fn validate_record(
    record: &DurableRecordV1,
    plan: &DurableBrokerPublicationPlanV1,
) -> Result<(), BrokerDurableSessionErrorV1> {
    if record.plan_identity != plan.identity
        || record.binding.durable_plan != plan.identity
        || record.destination != plan.destination
    {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "publication plan",
        ));
    }
    validate_destination(&record.destination)?;
    let challenge = AnchorChallengeV1::decode(&record.challenge)
        .map_err(|_| BrokerDurableSessionErrorV1::InvalidRecord("anchor challenge".into()))?;
    let key = PinnedAnchorKeyV1::from_bytes(record.anchor_key_bytes)
        .map_err(|_| BrokerDurableSessionErrorV1::InvalidRecord("anchor key".into()))?;
    let reconstructed = DurableBrokerPublicationPlanV1::new(record.destination.clone(), &key)?;
    if reconstructed.identity != record.plan_identity
        || reconstructed.anchor_key_identity != plan.anchor_key_identity
    {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "plan derivation/key identity",
        ));
    }
    if challenge.anchor_key_identity() != key.identity()
        || challenge.anchor_key_identity().to_bytes() != plan.anchor_key_identity
        || challenge.nonce() == [0; 32]
        || challenge.nonce() == record.binding.session_nonce
        || challenge.transaction().to_bytes()
            != anchor_transaction(record.binding, challenge.nonce())
    {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "anchor challenge",
        ));
    }
    validate_binding_relations(record.binding)?;
    match record.state {
        RecordStateV1::Prepared => {
            if record.observation.is_some() {
                return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                    "prepared record carries an observation".into(),
                ));
            }
        }
        RecordStateV1::AnchorCommitted | RecordStateV1::Published => {
            let observation = record.observation.as_ref().ok_or_else(|| {
                BrokerDurableSessionErrorV1::InvalidRecord(
                    "committed record lacks observation".into(),
                )
            })?;
            if !matches!(
                verify_record_observation(record, observation)?,
                AnchorDecisionV1::Commit(_)
            ) {
                return Err(BrokerDurableSessionErrorV1::AnchorObservation);
            }
        }
        RecordStateV1::Aborted => {
            let observation = record.observation.as_ref().ok_or_else(|| {
                BrokerDurableSessionErrorV1::InvalidRecord(
                    "aborted record lacks observation".into(),
                )
            })?;
            if !matches!(
                verify_record_observation(record, observation)?,
                AnchorDecisionV1::Abort(_)
            ) {
                return Err(BrokerDurableSessionErrorV1::AnchorObservation);
            }
        }
        RecordStateV1::Invalid => {
            if record.observation.is_some() {
                return Err(BrokerDurableSessionErrorV1::InvalidRecord(
                    "invalid record carries an admitted observation".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_binding_relations(
    binding: BrokerDurableBindingV1,
) -> Result<(), BrokerDurableSessionErrorV1> {
    const CLAIM_DOMAIN: &[u8] = b"FE2O3/BROKER-V4/SESSION-CLAIM-DIGEST/V1\0";
    const TRANSCRIPT_DOMAIN: &[u8] = b"FE2O3/BROKER-V4/COMPLETED-TRANSCRIPT-DIGEST/V1\0";
    const RESERVATION_DOMAIN: &[u8] = b"FE2O3/BROKER-SESSION/LINK-RESERVATION/V1\0";
    let claim = sha256_parts(&[
        CLAIM_DOMAIN,
        &binding.transcript_binding_identity,
        &binding.client_pid.to_le_bytes(),
        &binding.client_start_time_ticks.to_le_bytes(),
        &binding.transcript_request_identity,
        &binding.transcript_plan_identity,
        &binding.transcript_closure_identity,
    ]);
    let transcript = sha256_parts(&[
        TRANSCRIPT_DOMAIN,
        &binding.transcript_binding_identity,
        &binding.client_pid.to_le_bytes(),
        &binding.client_start_time_ticks.to_le_bytes(),
        &binding.transcript_request_identity,
        &binding.transcript_plan_identity,
        &binding.transcript_closure_identity,
        &binding.transcript_grant_identity,
        &binding.output_digest,
        &binding.output_length.to_le_bytes(),
        &binding.output_mode.to_le_bytes(),
        &binding.durable_plan,
    ]);
    let reservation = sha256_parts(&[
        RESERVATION_DOMAIN,
        &binding.session_id,
        &binding.session_nonce,
        &binding.claim_digest,
        &binding.client_pid.to_le_bytes(),
        &binding.client_start_time_ticks.to_le_bytes(),
        &binding.transcript_plan_identity,
        &binding.transcript_closure_identity,
        &binding.durable_plan,
    ]);
    if claim != binding.claim_digest {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "session claim digest",
        ));
    }
    if transcript != binding.transcript_digest {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "completed transcript digest",
        ));
    }
    if reservation != binding.reservation_digest {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "link reservation digest",
        ));
    }
    if binding.output_mode != HOST_LINK_OUTPUT_MODE_V4
        || binding.output_length == 0
        || binding.output_length > MAX_BROKER_DURABLE_OUTPUT_BYTES_V1 as u64
        || binding.session_id.iter().all(|byte| *byte == 0)
        || binding.session_nonce.iter().all(|byte| *byte == 0)
        || binding.session_id == binding.session_nonce
    {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "bounded session fields",
        ));
    }
    Ok(())
}

fn verify_record_observation(
    record: &DurableRecordV1,
    observation: &[u8],
) -> Result<AnchorDecisionV1, BrokerDurableSessionErrorV1> {
    let challenge = AnchorChallengeV1::decode(&record.challenge)
        .map_err(|_| BrokerDurableSessionErrorV1::AnchorObservation)?;
    let key = PinnedAnchorKeyV1::from_bytes(record.anchor_key_bytes)
        .map_err(|_| BrokerDurableSessionErrorV1::AnchorObservation)?;
    let prepared = PreparedAnchorAdvanceV1::recover_from_local_state(
        challenge.expected_sequence(),
        challenge.prior_head(),
        challenge.transaction(),
        challenge.proposed_head(),
        &key,
    )
    .map_err(|_| BrokerDurableSessionErrorV1::AnchorObservation)?;
    let pending = match challenge.kind() {
        ChallengeKindV1::Advance => {
            prepared.begin_advance(CallerNonceV1::from_bytes(challenge.nonce()), &key)
        }
        ChallengeKindV1::Recover => {
            prepared.begin_recovery(CallerNonceV1::from_bytes(challenge.nonce()), &key)
        }
    }
    .map_err(|_| BrokerDurableSessionErrorV1::AnchorObservation)?;
    pending
        .verify(observation)
        .map_err(|_| BrokerDurableSessionErrorV1::AnchorObservation)
}

fn anchor_transaction(
    binding: BrokerDurableBindingV1,
    service_attempt_nonce: [u8; 32],
) -> [u8; 32] {
    const DOMAIN: &[u8] = b"FE2O3/BROKER-SESSION/ANCHOR-TRANSACTION/SERVICE-ATTEMPT/V1\0";
    sha256_parts(&[
        DOMAIN,
        &binding.session_id,
        &binding.session_nonce,
        &binding.claim_digest,
        &binding.reservation_digest,
        &binding.request_nonce_sha256,
        &binding.transcript_digest,
        &binding.output_digest,
        &binding.durable_plan,
        &service_attempt_nonce,
    ])
}

fn recover_redo(
    store: &RetainedDurableDirectoryV1,
    names: &DurableNames,
    plan: &DurableBrokerPublicationPlanV1,
    fault: Option<BrokerDurableFaultPointV1>,
) -> Result<(), BrokerDurableSessionErrorV1> {
    let Some(redo_bytes) = store.read_private(&names.redo, MAX_BROKER_DURABLE_RECORD_BYTES_V1)?
    else {
        return Ok(());
    };
    let redo = DurableRecordV1::decode(&redo_bytes)?;
    validate_record(&redo, plan)?;
    let canonical_bytes = store.read_private(&names.record, MAX_BROKER_DURABLE_RECORD_BYTES_V1)?;
    let canonical = canonical_bytes
        .as_deref()
        .map(DurableRecordV1::decode)
        .transpose()?;
    if let Some(canonical) = &canonical {
        validate_record(canonical, plan)?;
    }
    let legal = match canonical.as_ref().map(|record| record.state) {
        None => redo.state == RecordStateV1::Prepared,
        Some(RecordStateV1::Prepared) => {
            matches!(
                redo.state,
                RecordStateV1::AnchorCommitted | RecordStateV1::Aborted | RecordStateV1::Invalid
            ) && same_transaction(canonical.as_ref().expect("present"), &redo)
        }
        Some(RecordStateV1::AnchorCommitted) => {
            matches!(
                redo.state,
                RecordStateV1::Published | RecordStateV1::Invalid
            ) && same_transaction(canonical.as_ref().expect("present"), &redo)
        }
        Some(_) => false,
    };
    if !legal {
        return Err(BrokerDurableSessionErrorV1::InvalidRecord(
            "redo is not one exact legal successor".into(),
        ));
    }
    let mut faults = FaultInjector::new(fault, record_stage(redo.state));
    let result = store.promote_validated_redo(
        &names.record,
        &names.redo,
        canonical_bytes.as_deref(),
        &redo_bytes,
        MAX_BROKER_DURABLE_RECORD_BYTES_V1,
        &mut faults,
    );
    map_storage_fault(&faults, result)
}

const fn record_stage(state: RecordStateV1) -> BrokerDurableRecordStageV1 {
    match state {
        RecordStateV1::Prepared => BrokerDurableRecordStageV1::Prepared,
        RecordStateV1::AnchorCommitted => BrokerDurableRecordStageV1::AnchorCommitted,
        RecordStateV1::Published => BrokerDurableRecordStageV1::Published,
        RecordStateV1::Aborted => BrokerDurableRecordStageV1::Aborted,
        RecordStateV1::Invalid => BrokerDurableRecordStageV1::Invalid,
    }
}

fn same_transaction(left: &DurableRecordV1, right: &DurableRecordV1) -> bool {
    left.plan_identity == right.plan_identity
        && left.destination == right.destination
        && left.binding == right.binding
        && left.challenge == right.challenge
        && left.anchor_key_bytes == right.anchor_key_bytes
}

fn verify_staged(
    store: &RetainedDurableDirectoryV1,
    names: &DurableNames,
    record: &DurableRecordV1,
) -> Result<(), BrokerDurableSessionErrorV1> {
    let Some((digest, length)) = store.staged_file_sha256(
        &names.staged,
        MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
        record.binding.output_mode,
    )?
    else {
        return Err(BrokerDurableSessionErrorV1::InvalidRecord(
            "prepared artifact is absent".into(),
        ));
    };
    if digest != record.binding.output_digest || length as u64 != record.binding.output_length {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "prepared artifact",
        ));
    }
    Ok(())
}

fn verify_final(
    store: &RetainedDurableDirectoryV1,
    plan: &DurableBrokerPublicationPlanV1,
    record: &DurableRecordV1,
) -> Result<(), BrokerDurableSessionErrorV1> {
    let Some((digest, length)) = store.published_file_sha256(
        &plan.destination,
        MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
        record.binding.output_mode,
    )?
    else {
        return Err(BrokerDurableSessionErrorV1::InvalidRecord(
            "published artifact is absent".into(),
        ));
    };
    if digest != record.binding.output_digest || length as u64 != record.binding.output_length {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "published artifact",
        ));
    }
    Ok(())
}

fn require_no_public_destination(
    store: &RetainedDurableDirectoryV1,
    plan: &DurableBrokerPublicationPlanV1,
    record: &DurableRecordV1,
) -> Result<(), BrokerDurableSessionErrorV1> {
    if store
        .read_published(
            &plan.destination,
            MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
            record.binding.output_mode,
        )?
        .is_some()
    {
        return Err(BrokerDurableSessionErrorV1::InvalidRecord(
            "non-publishing transaction has a public destination".into(),
        ));
    }
    Ok(())
}

fn read_staged_or_final(
    store: &RetainedDurableDirectoryV1,
    names: &DurableNames,
    plan: &DurableBrokerPublicationPlanV1,
    record: &DurableRecordV1,
) -> Result<Vec<u8>, BrokerDurableSessionErrorV1> {
    let bytes = match store.read_staged(
        &names.staged,
        MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
        record.binding.output_mode,
    )? {
        Some(bytes) => bytes,
        None => store
            .read_published(
                &plan.destination,
                MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
                record.binding.output_mode,
            )?
            .ok_or_else(|| {
                BrokerDurableSessionErrorV1::InvalidRecord(
                    "artifact missing after anchor commit".into(),
                )
            })?,
    };
    require_output_identity(&bytes, &record.binding)?;
    Ok(bytes)
}

fn read_exact_output(file: &File, length: u64) -> Result<Vec<u8>, BrokerDurableSessionErrorV1> {
    if length == 0 || length > MAX_BROKER_DURABLE_OUTPUT_BYTES_V1 as u64 {
        return Err(BrokerDurableSessionErrorV1::OutputSize {
            actual: length,
            maximum: MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
        });
    }
    let length = usize::try_from(length).map_err(|_| BrokerDurableSessionErrorV1::OutputSize {
        actual: length,
        maximum: MAX_BROKER_DURABLE_OUTPUT_BYTES_V1,
    })?;
    let mut bytes = vec![0_u8; length];
    file.read_exact_at(&mut bytes, 0)
        .map_err(BrokerDurableSessionErrorV1::OutputRead)?;
    let metadata = file
        .metadata()
        .map_err(BrokerDurableSessionErrorV1::OutputRead)?;
    if metadata.len() != length as u64 {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "output length",
        ));
    }
    Ok(bytes)
}

fn require_output_identity(
    bytes: &[u8],
    binding: &BrokerDurableBindingV1,
) -> Result<(), BrokerDurableSessionErrorV1> {
    if bytes.len() as u64 != binding.output_length
        || Sha256::digest(bytes).as_slice() != binding.output_digest
    {
        return Err(BrokerDurableSessionErrorV1::BindingMismatch(
            "output digest/length",
        ));
    }
    Ok(())
}

fn fresh_service_attempt_nonce(
    options: BrokerDurableOptionsV1,
    caller_session_nonce: [u8; 32],
) -> Result<[u8; 32], BrokerDurableSessionErrorV1> {
    #[cfg(not(test))]
    let _ = options;
    #[cfg(test)]
    match options.entropy {
        TestServiceEntropyV1::Fixed(nonce) => {
            return validate_service_attempt_nonce(nonce, caller_session_nonce);
        }
        TestServiceEntropyV1::Fail => {
            return Err(BrokerDurableSessionErrorV1::Entropy(io::Error::other(
                "injected service entropy failure",
            )));
        }
        TestServiceEntropyV1::Os => {}
    }

    const MAX_NONCE_ATTEMPTS: usize = 4;
    for _ in 0..MAX_NONCE_ATTEMPTS {
        let mut nonce = [0_u8; 32];
        let mut filled = 0;
        while filled < nonce.len() {
            let count = rustix::rand::getrandom(
                &mut nonce[filled..],
                rustix::rand::GetRandomFlags::empty(),
            )
            .map_err(|error| {
                BrokerDurableSessionErrorV1::Entropy(io::Error::from_raw_os_error(
                    error.raw_os_error(),
                ))
            })?;
            if count == 0 {
                return Err(BrokerDurableSessionErrorV1::Entropy(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Linux getrandom returned zero bytes",
                )));
            }
            filled += count;
        }
        if let Ok(nonce) = validate_service_attempt_nonce(nonce, caller_session_nonce) {
            return Ok(nonce);
        }
    }
    Err(BrokerDurableSessionErrorV1::Entropy(io::Error::other(
        "Linux getrandom did not produce an admissible service attempt nonce",
    )))
}

fn validate_service_attempt_nonce(
    nonce: [u8; 32],
    caller_session_nonce: [u8; 32],
) -> Result<[u8; 32], BrokerDurableSessionErrorV1> {
    if nonce == [0; 32] || nonce == caller_session_nonce {
        Err(BrokerDurableSessionErrorV1::Entropy(io::Error::other(
            "service attempt nonce failed bounded admission",
        )))
    } else {
        Ok(nonce)
    }
}

fn exact_observation(
    observation: &[u8],
) -> Result<[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1], BrokerDurableSessionErrorV1> {
    observation
        .try_into()
        .map_err(|_| BrokerDurableSessionErrorV1::AnchorObservation)
}

fn validate_destination(destination: &str) -> Result<(), BrokerDurableSessionErrorV1> {
    let valid = !destination.is_empty()
        && destination.len() <= MAX_DESTINATION_BYTES
        && destination != "."
        && destination != ".."
        && !destination.starts_with(".fe2o3-")
        && destination
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(BrokerDurableSessionErrorV1::InvalidPlan(
            "destination must be one non-reserved ASCII file component".into(),
        ))
    }
}

fn encode_binding(bytes: &mut Vec<u8>, binding: BrokerDurableBindingV1) {
    bytes.extend_from_slice(&binding.session_id);
    bytes.extend_from_slice(&binding.session_nonce);
    bytes.extend_from_slice(&binding.reservation_digest);
    bytes.extend_from_slice(&binding.request_nonce_sha256);
    bytes.extend_from_slice(&binding.client_pid.to_le_bytes());
    bytes.extend_from_slice(&binding.client_start_time_ticks.to_le_bytes());
    bytes.extend_from_slice(&binding.claim_digest);
    bytes.extend_from_slice(&binding.transcript_digest);
    bytes.extend_from_slice(&binding.transcript_binding_identity);
    bytes.extend_from_slice(&binding.transcript_request_identity);
    bytes.extend_from_slice(&binding.transcript_plan_identity);
    bytes.extend_from_slice(&binding.transcript_closure_identity);
    bytes.extend_from_slice(&binding.transcript_grant_identity);
    bytes.extend_from_slice(&binding.output_digest);
    bytes.extend_from_slice(&binding.output_length.to_le_bytes());
    bytes.extend_from_slice(&binding.output_mode.to_le_bytes());
    bytes.extend_from_slice(&binding.durable_plan);
}

fn decode_binding(
    decoder: &mut Decoder<'_>,
) -> Result<BrokerDurableBindingV1, BrokerDurableSessionErrorV1> {
    Ok(BrokerDurableBindingV1 {
        session_id: decoder.array()?,
        session_nonce: decoder.array()?,
        reservation_digest: decoder.array()?,
        request_nonce_sha256: decoder.array()?,
        client_pid: decoder.u32()?,
        client_start_time_ticks: decoder.u64()?,
        claim_digest: decoder.array()?,
        transcript_digest: decoder.array()?,
        transcript_binding_identity: decoder.array()?,
        transcript_request_identity: decoder.array()?,
        transcript_plan_identity: decoder.array()?,
        transcript_closure_identity: decoder.array()?,
        transcript_grant_identity: decoder.array()?,
        output_digest: decoder.array()?,
        output_length: decoder.u64()?,
        output_mode: decoder.u32()?,
        durable_plan: decoder.array()?,
    })
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], BrokerDurableSessionErrorV1> {
        let end = self.offset.checked_add(length).ok_or_else(|| {
            BrokerDurableSessionErrorV1::InvalidRecord("record offset overflow".into())
        })?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| BrokerDurableSessionErrorV1::InvalidRecord("truncated record".into()))?;
        self.offset = end;
        Ok(result)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], BrokerDurableSessionErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| BrokerDurableSessionErrorV1::InvalidRecord("fixed field length".into()))
    }

    fn u8(&mut self) -> Result<u8, BrokerDurableSessionErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, BrokerDurableSessionErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, BrokerDurableSessionErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, BrokerDurableSessionErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn sha256_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part);
    }
    digest.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(test)]
mod tests;
