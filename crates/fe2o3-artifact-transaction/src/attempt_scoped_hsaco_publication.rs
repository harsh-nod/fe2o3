use crate::attempt::{AttemptPhase, BackendReceiptV1};
use crate::durable_link_publication::{
    DurablePlanRecoveryStateV1, publish_durable_link_v1_locked, recover_durable_link_plan_locked,
};
use crate::{
    BackendPublicationReceiptV1, BuildAttempt, BuildSession, DurableCurrentLinkPublicationLeaseV1,
    DurableLinkPublicationError, DurableLinkPublicationOptionsV1, DurableLinkPublicationOutcomeV1,
    DurableLinkPublicationPlanV1, DurableLinkPublicationResultV1, DurableLinkPublicationSnapshotV1,
    EmitError, NoFaults, PackageIdentityV1, PinnedOutput, ProducerIdentity, build_attempt_error,
    commit_attempt_registry_direct, fail_build_attempt_locked, read_attempt_registry,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const ATTEMPT_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.attempt.v1\0";
const PRODUCER_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.producer.v1\0";
const PRODUCER_PACKAGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/COORDINATION-PRODUCER-PACKAGE/V1\0";
const SCOPE_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.scope.v1\0";

/// Derives a non-authoritative package namespace from one validated producer identity.
///
/// This helper deliberately does not expose the producer's private source or crate-name fields.
/// The result coordinates cooperating artifact writers; it does not authenticate a package,
/// compiler, source tree, artifact, or launch decision.
pub fn producer_package_identity_v1(producer: &ProducerIdentity) -> PackageIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(PRODUCER_PACKAGE_IDENTITY_DOMAIN_V1);
    update_length_prefixed(&mut digest, producer.stable_source.as_bytes());
    update_length_prefixed(&mut digest, producer.crate_name.as_bytes());
    PackageIdentityV1::from_bytes(digest.finalize().into())
}

fn update_length_prefixed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

/// Identity of upstream code-object evidence committed into a backend receipt.
///
/// Construction is deliberately structural. Possession of this value does not prove that any
/// admission, verification, ABI, memory-safety, race-freedom, or launch-safety check ran.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UpstreamCodeObjectEvidenceIdentityV1([u8; 32]);

impl UpstreamCodeObjectEvidenceIdentityV1 {
    /// Constructs an identity from its exact 256-bit representation.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// How an exact-byte publication completed relative to its attempt receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptScopedHsacoPublicationOutcomeV1 {
    /// This invocation consumed fresh attempt authority and committed a new publication.
    Published,
    /// An interrupted exact plan was recovered and then completed publication.
    RecoveredAndPublished,
    /// Publication had committed before interruption; this invocation completed its receipt.
    RecoveredCommittedPublication,
}

/// Durable publication evidence plus the provenance-bound backend receipt.
#[derive(Debug)]
pub struct AttemptScopedHsacoPublicationResultV1 {
    outcome: AttemptScopedHsacoPublicationOutcomeV1,
    publication: DurableLinkPublicationResultV1,
    receipt: BackendPublicationReceiptV1,
}

/// Failure while matching a backend publication receipt to its typed producer and exact plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BackendPublicationReceiptValidationErrorV1 {
    PlanAttemptMismatch,
    AttemptIdentityMismatch,
    ProducerIdentityMismatch,
    ScopeIdentityMismatch,
    PlanCommitmentMismatch,
    UpstreamEvidenceIdentityMismatch,
    FinalizedOutputIdentityMismatch,
    PublicationIdentityMismatch,
}

impl fmt::Display for BackendPublicationReceiptValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = match self {
            Self::PlanAttemptMismatch => "publication plan attempt",
            Self::AttemptIdentityMismatch => "receipt attempt identity",
            Self::ProducerIdentityMismatch => "receipt producer identity",
            Self::ScopeIdentityMismatch => "receipt scope identity",
            Self::PlanCommitmentMismatch => "receipt plan commitment",
            Self::UpstreamEvidenceIdentityMismatch => "receipt upstream evidence identity",
            Self::FinalizedOutputIdentityMismatch => "receipt finalized output identity",
            Self::PublicationIdentityMismatch => "receipt publication identity",
        };
        write!(formatter, "backend publication {field} does not match")
    }
}

impl std::error::Error for BackendPublicationReceiptValidationErrorV1 {}

impl AttemptScopedHsacoPublicationResultV1 {
    pub const fn outcome(&self) -> AttemptScopedHsacoPublicationOutcomeV1 {
        self.outcome
    }

    pub const fn durable_outcome(&self) -> DurableLinkPublicationOutcomeV1 {
        self.publication.outcome()
    }

    pub fn snapshot(&self) -> &DurableLinkPublicationSnapshotV1 {
        self.publication.snapshot()
    }

    pub const fn receipt(&self) -> BackendPublicationReceiptV1 {
        self.receipt
    }

    pub fn into_current_lease(self) -> DurableCurrentLinkPublicationLeaseV1 {
        self.publication.into_current_lease()
    }
}

/// Validates every receipt field against a typed producer and complete publication plan.
///
/// This establishes coordination lineage only. It does not authenticate the producer, upstream
/// evidence, artifact semantics, or current publication state, and grants no load or launch
/// authority.
pub fn validate_backend_publication_receipt_v1(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: BackendPublicationReceiptV1,
) -> Result<(), BackendPublicationReceiptValidationErrorV1> {
    if plan.attempt() != attempt {
        return Err(BackendPublicationReceiptValidationErrorV1::PlanAttemptMismatch);
    }
    let expected = publication_receipt(producer, attempt, plan, upstream_evidence);
    for (matches, error) in [
        (
            receipt.attempt_identity() == expected.attempt_identity(),
            BackendPublicationReceiptValidationErrorV1::AttemptIdentityMismatch,
        ),
        (
            receipt.producer_identity() == expected.producer_identity(),
            BackendPublicationReceiptValidationErrorV1::ProducerIdentityMismatch,
        ),
        (
            receipt.scope_identity() == expected.scope_identity(),
            BackendPublicationReceiptValidationErrorV1::ScopeIdentityMismatch,
        ),
        (
            receipt.plan_commitment() == expected.plan_commitment(),
            BackendPublicationReceiptValidationErrorV1::PlanCommitmentMismatch,
        ),
        (
            receipt.upstream_evidence_identity() == expected.upstream_evidence_identity(),
            BackendPublicationReceiptValidationErrorV1::UpstreamEvidenceIdentityMismatch,
        ),
        (
            receipt.finalized_output_identity() == expected.finalized_output_identity(),
            BackendPublicationReceiptValidationErrorV1::FinalizedOutputIdentityMismatch,
        ),
        (
            receipt.publication_identity() == expected.publication_identity(),
            BackendPublicationReceiptValidationErrorV1::PublicationIdentityMismatch,
        ),
    ] {
        if !matches {
            return Err(error);
        }
    }
    Ok(())
}

/// Durable receipt state retained for an exact build attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedBackendReceiptV1 {
    None,
    /// Compatibility state produced by the pre-receipt artifact backend.
    LegacyCoordination,
    PendingProvenance(BackendPublicationReceiptV1),
    Provenance(BackendPublicationReceiptV1),
}

/// Failure while consuming one build attempt to publish exact HSACO bytes and inert evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum AttemptScopedHsacoPublicationErrorV1 {
    PlanAttemptMismatch,
    Attempt(EmitError),
    /// A consumed attempt had no exact durable plan commitment to recover.
    UnrecoverableClaimedAttempt,
    /// Crash recovery supplied inputs that differ from the pending provenance receipt.
    PendingReceiptMismatch,
    /// An injected crash-like interruption consumed the claim. The caller must retry the exact
    /// same plan and evidence rather than treating this as an ordinary failed publication.
    PublicationInterrupted(DurableLinkPublicationError),
    Durable(DurableLinkPublicationError),
    DurableAndAttemptState {
        publication: Box<DurableLinkPublicationError>,
        attempt_state: Box<EmitError>,
    },
    /// A durable publication predated fresh attempt consumption. It was not adopted as backend
    /// provenance and the attempt was failed closed.
    UnexpectedPreexistingPublication {
        publication: DurableLinkPublicationResultV1,
    },
    /// Exact bytes and publication evidence committed, but the provenance receipt did not.
    /// Retrying the exact inputs reconciles this state.
    PublicationCommittedWithoutReceipt {
        publication: DurableLinkPublicationResultV1,
        expected_receipt: Box<BackendPublicationReceiptV1>,
        attempt_state: Box<EmitError>,
    },
    /// A persisted receipt did not have a matching complete durable publication.
    ReceiptPublicationMismatch,
    /// The exact provenance receipt and publication are already durable. Callers may proceed to
    /// attempt completion without republishing.
    ReceiptAlreadyPersisted {
        receipt: Box<BackendPublicationReceiptV1>,
    },
}

impl AttemptScopedHsacoPublicationErrorV1 {
    pub const fn committed_publication(&self) -> Option<&DurableLinkPublicationResultV1> {
        match self {
            Self::UnexpectedPreexistingPublication { publication }
            | Self::PublicationCommittedWithoutReceipt { publication, .. } => Some(publication),
            _ => None,
        }
    }

    pub const fn expected_receipt(&self) -> Option<BackendPublicationReceiptV1> {
        match self {
            Self::PublicationCommittedWithoutReceipt {
                expected_receipt, ..
            } => Some(**expected_receipt),
            _ => None,
        }
    }
}

impl fmt::Display for AttemptScopedHsacoPublicationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanAttemptMismatch => formatter
                .write_str("durable publication plan does not match the supplied build attempt"),
            Self::Attempt(error) => write!(formatter, "build attempt rejected: {error}"),
            Self::UnrecoverableClaimedAttempt => formatter.write_str(
                "claimed build attempt has no exact durable publication plan to recover",
            ),
            Self::PendingReceiptMismatch => formatter
                .write_str("crash-recovery inputs do not match the pending backend receipt"),
            Self::PublicationInterrupted(error) => write!(
                formatter,
                "exact-byte publication was interrupted and requires exact-input reconciliation: {error}"
            ),
            Self::Durable(error) => write!(formatter, "exact HSACO publication failed: {error}"),
            Self::DurableAndAttemptState {
                publication,
                attempt_state,
            } => write!(
                formatter,
                "exact HSACO publication failed ({publication}); terminal attempt update also failed ({attempt_state})"
            ),
            Self::UnexpectedPreexistingPublication { .. } => formatter.write_str(
                "fresh attempt found a preexisting durable publication and refused to adopt it",
            ),
            Self::PublicationCommittedWithoutReceipt { attempt_state, .. } => write!(
                formatter,
                "exact HSACO publication committed, but its backend receipt did not: {attempt_state}"
            ),
            Self::ReceiptPublicationMismatch => formatter.write_str(
                "persisted backend receipt has no matching complete durable publication",
            ),
            Self::ReceiptAlreadyPersisted { .. } => formatter.write_str(
                "the exact backend receipt and durable publication are already persisted",
            ),
        }
    }
}

impl std::error::Error for AttemptScopedHsacoPublicationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Attempt(error) => Some(error),
            Self::PublicationInterrupted(error) | Self::Durable(error) => Some(error),
            Self::DurableAndAttemptState { publication, .. } => Some(publication.as_ref()),
            Self::PublicationCommittedWithoutReceipt { attempt_state, .. } => {
                Some(attempt_state.as_ref())
            }
            _ => None,
        }
    }
}

/// Publishes exact HSACO bytes and caller-supplied upstream evidence identity for one attempt.
///
/// This function does not admit, authenticate, or semantically validate `exact_hsaco`, `plan`, or
/// `upstream_evidence`. It verifies only the existing durable protocol invariants, including the
/// finalized-byte digest committed by `plan`. Successful completion persists a receipt binding the
/// attempt, producer, scope, complete plan, caller-supplied upstream evidence identity, measured
/// final digest, and publication identity. The returned evidence grants no loading or launch
/// authority. Callers must still invoke [`crate::finish_build_attempt`] after managed work ends.
pub fn publish_exact_hsaco_evidence_for_attempt_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    exact_hsaco: &[u8],
) -> Result<AttemptScopedHsacoPublicationResultV1, AttemptScopedHsacoPublicationErrorV1> {
    publish_exact_hsaco_evidence_for_attempt_v1_with_options(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        exact_hsaco,
        DurableLinkPublicationOptionsV1::default(),
    )
}

/// Fault-injectable form of [`publish_exact_hsaco_evidence_for_attempt_v1`].
pub fn publish_exact_hsaco_evidence_for_attempt_v1_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    exact_hsaco: &[u8],
    options: DurableLinkPublicationOptionsV1,
) -> Result<AttemptScopedHsacoPublicationResultV1, AttemptScopedHsacoPublicationErrorV1> {
    if plan.attempt() != attempt {
        return Err(AttemptScopedHsacoPublicationErrorV1::PlanAttemptMismatch);
    }
    if attempt.session() == BuildSession::DIRECT {
        return Err(AttemptScopedHsacoPublicationErrorV1::Attempt(
            build_attempt_error("the direct compiler token cannot authorize managed publication"),
        ));
    }

    let expected_receipt = publication_receipt(producer, attempt, plan, upstream_evidence);
    let output = PinnedOutput::open_existing(output_dir)
        .map_err(AttemptScopedHsacoPublicationErrorV1::Attempt)?;
    let _lock = output
        .lock()
        .map_err(AttemptScopedHsacoPublicationErrorV1::Attempt)?;
    output
        .verify_path_identity()
        .map_err(AttemptScopedHsacoPublicationErrorV1::Attempt)?;

    let mut attempts =
        read_attempt_registry(&output).map_err(AttemptScopedHsacoPublicationErrorV1::Attempt)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)
        .map_err(AttemptScopedHsacoPublicationErrorV1::Attempt)?;
    if record.crate_name != producer.crate_name {
        return Err(AttemptScopedHsacoPublicationErrorV1::Attempt(
            build_attempt_error("build attempt crate name does not match the producer"),
        ));
    }

    enum Authorization {
        Fresh,
        Recovering(DurablePlanRecoveryStateV1),
    }

    let authorization = match (record.phase, record.backend_receipt) {
        (AttemptPhase::Building, None) => {
            attempts
                .claim_backend_with_pending_receipt(
                    &producer.stable_source,
                    attempt,
                    expected_receipt,
                )
                .map_err(build_attempt_error)
                .map_err(AttemptScopedHsacoPublicationErrorV1::Attempt)?;
            commit_attempt_registry_direct(&output, &attempts)
                .map_err(AttemptScopedHsacoPublicationErrorV1::Attempt)?;
            Authorization::Fresh
        }
        (AttemptPhase::BackendClaimed, Some(BackendReceiptV1::PendingProvenance(pending)))
            if pending == expected_receipt =>
        {
            match recover_durable_link_plan_locked(&output, plan) {
                Ok(Some(state)) => Authorization::Recovering(state),
                recovery => {
                    let failure =
                        fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
                    return match (recovery, failure) {
                        (Err(publication), Ok(())) => {
                            Err(AttemptScopedHsacoPublicationErrorV1::Durable(publication))
                        }
                        (Err(publication), Err(attempt_state)) => Err(
                            AttemptScopedHsacoPublicationErrorV1::DurableAndAttemptState {
                                publication: Box::new(publication),
                                attempt_state: Box::new(attempt_state),
                            },
                        ),
                        (Ok(None), Ok(())) => {
                            Err(AttemptScopedHsacoPublicationErrorV1::UnrecoverableClaimedAttempt)
                        }
                        (Ok(None), Err(attempt_state)) => {
                            Err(AttemptScopedHsacoPublicationErrorV1::Attempt(attempt_state))
                        }
                        (Ok(Some(_)), _) => unreachable!("matched recoverable attempt"),
                    };
                }
            }
        }
        (AttemptPhase::BackendClaimed, Some(BackendReceiptV1::PendingProvenance(_))) => {
            let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
            return Err(AttemptScopedHsacoPublicationErrorV1::PendingReceiptMismatch);
        }
        (AttemptPhase::BackendClaimed, None) => {
            let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
            return Err(AttemptScopedHsacoPublicationErrorV1::UnrecoverableClaimedAttempt);
        }
        (AttemptPhase::BackendClaimed, Some(BackendReceiptV1::Provenance(receipt)))
            if receipt == expected_receipt =>
        {
            match recover_durable_link_plan_locked(&output, plan) {
                Ok(Some(DurablePlanRecoveryStateV1::Published)) => {
                    return Err(
                        AttemptScopedHsacoPublicationErrorV1::ReceiptAlreadyPersisted {
                            receipt: Box::new(receipt),
                        },
                    );
                }
                _ => {
                    let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
                    return Err(AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch);
                }
            }
        }
        _ => {
            return Err(AttemptScopedHsacoPublicationErrorV1::Attempt(
                build_attempt_error(
                    "build attempt cannot authorize exact HSACO publication in its current phase",
                ),
            ));
        }
    };

    let publication = publish_durable_link_v1_locked(&output, plan, options, |transaction| {
        transaction.record_worker_pinned()?;
        transaction.record_response_validated()?;
        transaction.record_finalized(exact_hsaco)
    });
    let publication = match publication {
        Ok(publication) => publication,
        Err(error @ DurableLinkPublicationError::InjectedCrash { .. }) => {
            return Err(AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(error));
        }
        Err(publication) => {
            return match fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults) {
                Ok(()) => Err(AttemptScopedHsacoPublicationErrorV1::Durable(publication)),
                Err(attempt_state) => Err(
                    AttemptScopedHsacoPublicationErrorV1::DurableAndAttemptState {
                        publication: Box::new(publication),
                        attempt_state: Box::new(attempt_state),
                    },
                ),
            };
        }
    };

    if matches!(authorization, Authorization::Fresh)
        && publication.outcome() == DurableLinkPublicationOutcomeV1::AlreadyPublished
    {
        let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
        return Err(
            AttemptScopedHsacoPublicationErrorV1::UnexpectedPreexistingPublication { publication },
        );
    }

    let attempt_state = (|| {
        let mut attempts = read_attempt_registry(&output)?;
        let record = attempts
            .record_exact(&producer.stable_source, attempt)
            .map_err(build_attempt_error)?;
        if record.crate_name != producer.crate_name {
            return Err(build_attempt_error(
                "build attempt crate name changed before publication completion",
            ));
        }
        attempts
            .record_backend_publication_receipt(&producer.stable_source, attempt, expected_receipt)
            .map_err(build_attempt_error)?;
        commit_attempt_registry_direct(&output, &attempts)
    })();
    if let Err(attempt_state) = attempt_state {
        return Err(
            AttemptScopedHsacoPublicationErrorV1::PublicationCommittedWithoutReceipt {
                publication,
                expected_receipt: Box::new(expected_receipt),
                attempt_state: Box::new(attempt_state),
            },
        );
    }

    let outcome = match (authorization, publication.outcome()) {
        (Authorization::Fresh, DurableLinkPublicationOutcomeV1::Published) => {
            AttemptScopedHsacoPublicationOutcomeV1::Published
        }
        (
            Authorization::Recovering(DurablePlanRecoveryStateV1::Incomplete),
            DurableLinkPublicationOutcomeV1::Published,
        ) => AttemptScopedHsacoPublicationOutcomeV1::RecoveredAndPublished,
        (
            Authorization::Recovering(DurablePlanRecoveryStateV1::Published),
            DurableLinkPublicationOutcomeV1::AlreadyPublished,
        ) => AttemptScopedHsacoPublicationOutcomeV1::RecoveredCommittedPublication,
        _ => AttemptScopedHsacoPublicationOutcomeV1::RecoveredCommittedPublication,
    };
    Ok(AttemptScopedHsacoPublicationResultV1 {
        outcome,
        publication,
        receipt: expected_receipt,
    })
}

/// Reads the durable backend receipt for an exact producer and attempt.
pub fn read_backend_publication_receipt_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<PersistedBackendReceiptV1, EmitError> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let attempts = read_attempt_registry(&output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)?;
    if record.crate_name != producer.crate_name {
        return Err(build_attempt_error(
            "build attempt crate name does not match the producer",
        ));
    }
    Ok(match record.backend_receipt {
        None => PersistedBackendReceiptV1::None,
        Some(BackendReceiptV1::LegacyCoordination) => PersistedBackendReceiptV1::LegacyCoordination,
        Some(BackendReceiptV1::PendingProvenance(receipt)) => {
            PersistedBackendReceiptV1::PendingProvenance(receipt)
        }
        Some(BackendReceiptV1::Provenance(receipt)) => {
            PersistedBackendReceiptV1::Provenance(receipt)
        }
    })
}

pub(crate) fn publication_receipt(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
) -> BackendPublicationReceiptV1 {
    let mut attempt_digest = Sha256::new();
    attempt_digest.update(ATTEMPT_IDENTITY_DOMAIN);
    attempt_digest.update(attempt.generation().to_le_bytes());
    attempt_digest.update(attempt.session().as_bytes());
    attempt_digest.update(attempt.invocation().as_bytes());

    let mut producer_digest = Sha256::new();
    producer_digest.update(PRODUCER_IDENTITY_DOMAIN);
    producer_digest.update((producer.stable_source.len() as u64).to_le_bytes());
    producer_digest.update(producer.stable_source.as_bytes());
    producer_digest.update((producer.crate_name.len() as u64).to_le_bytes());
    producer_digest.update(producer.crate_name.as_bytes());

    let scope = plan.scope();
    let mut scope_digest = Sha256::new();
    scope_digest.update(SCOPE_IDENTITY_DOMAIN);
    scope_digest.update(scope.package().as_bytes());
    scope_digest.update(scope.kernel_set().as_bytes());
    scope_digest.update(scope.target().as_bytes());

    BackendPublicationReceiptV1::new(
        attempt_digest.finalize().into(),
        producer_digest.finalize().into(),
        scope_digest.finalize().into(),
        plan.identity(),
        upstream_evidence.as_bytes(),
        *plan.finalized_output().as_bytes(),
        *plan.publication().as_bytes(),
    )
}
