use crate::attempt::{AttemptPhase, AttemptRegistry, BackendReceiptV1};
use crate::durable_link_publication::{
    DurablePlanRecoveryStateV1, publish_durable_link_v1_locked, recover_durable_link_plan_locked,
    recover_durable_published_file_binding_locked,
};
use crate::durable_published_claim::{DurablePublishedHsacoClaimV1, DurablePublishedHsacoClaimV2};
use crate::{
    BackendPublicationReceiptV1, BackendPublicationReceiptV2, BuildAttempt, BuildSession,
    DurableCurrentLinkPublicationLeaseV1, DurableLinkPublicationError,
    DurableLinkPublicationOptionsV1, DurableLinkPublicationOutcomeV1, DurableLinkPublicationPlanV1,
    DurableLinkPublicationResultV1, DurableLinkPublicationSnapshotV1, EmitError, NoFaults,
    PackageIdentityV1, PinnedOutput, ProducerIdentity, build_attempt_error,
    commit_attempt_registry_direct, fail_build_attempt_locked, read_attempt_registry,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const ATTEMPT_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.attempt.v1\0";
const PRODUCER_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.producer.v1\0";
const PRODUCER_PACKAGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/COORDINATION-PRODUCER-PACKAGE/V1\0";
const SCOPE_IDENTITY_DOMAIN: &[u8] = b"fe2o3.backend-receipt.scope.v1\0";
const ATTEMPT_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.backend-receipt.attempt.v2\0";
const PRODUCER_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.backend-receipt.producer.v2\0";
const SCOPE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.backend-receipt.scope.v2\0";

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
    claim: DurablePublishedHsacoClaimV1,
}

/// How a protected exact-byte publication completed relative to its V2 receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptScopedHsacoPublicationOutcomeV2 {
    Published,
    RecoveredAndPublished,
    RecoveredCommittedPublication,
}

/// Protected publication evidence carrying the exact compiler closure end to end.
#[derive(Debug)]
pub struct AttemptScopedHsacoPublicationResultV2 {
    outcome: AttemptScopedHsacoPublicationOutcomeV2,
    publication: DurableLinkPublicationResultV1,
    receipt: BackendPublicationReceiptV2,
    claim: DurablePublishedHsacoClaimV2,
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

/// Failure while matching a protected receipt to exact V2 publication inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BackendPublicationReceiptValidationErrorV2 {
    PlanAttemptMismatch,
    AttemptIdentityMismatch,
    ProducerIdentityMismatch,
    ScopeIdentityMismatch,
    PlanCommitmentMismatch,
    UpstreamEvidenceIdentityMismatch,
    FinalizedOutputIdentityMismatch,
    PublicationIdentityMismatch,
    CompilerClosureMismatch,
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

impl fmt::Display for BackendPublicationReceiptValidationErrorV2 {
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
            Self::CompilerClosureMismatch => "receipt compiler closure",
        };
        write!(
            formatter,
            "protected backend publication {field} does not match"
        )
    }
}

impl std::error::Error for BackendPublicationReceiptValidationErrorV2 {}

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

    /// Returns the inert canonical claim that can be persisted across processes.
    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV1 {
        &self.claim
    }

    pub fn into_current_lease(self) -> DurableCurrentLinkPublicationLeaseV1 {
        self.publication.into_current_lease()
    }
}

impl AttemptScopedHsacoPublicationResultV2 {
    pub const fn outcome(&self) -> AttemptScopedHsacoPublicationOutcomeV2 {
        self.outcome
    }

    pub const fn durable_outcome(&self) -> DurableLinkPublicationOutcomeV1 {
        self.publication.outcome()
    }

    pub fn snapshot(&self) -> &DurableLinkPublicationSnapshotV1 {
        self.publication.snapshot()
    }

    pub const fn receipt(&self) -> BackendPublicationReceiptV2 {
        self.receipt
    }

    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.receipt.compiler_closure()
    }

    /// Returns the inert canonical V2 claim that can be persisted across processes.
    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV2 {
        &self.claim
    }

    pub fn into_current_lease(self) -> DurableCurrentLinkPublicationLeaseV1 {
        self.publication.into_current_lease()
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
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
    validate_receipt_fields(receipt, expected, false).map_err(|field| match field {
        ReceiptField::Attempt => {
            BackendPublicationReceiptValidationErrorV1::AttemptIdentityMismatch
        }
        ReceiptField::Producer => {
            BackendPublicationReceiptValidationErrorV1::ProducerIdentityMismatch
        }
        ReceiptField::Scope => BackendPublicationReceiptValidationErrorV1::ScopeIdentityMismatch,
        ReceiptField::Plan => BackendPublicationReceiptValidationErrorV1::PlanCommitmentMismatch,
        ReceiptField::UpstreamEvidence => {
            BackendPublicationReceiptValidationErrorV1::UpstreamEvidenceIdentityMismatch
        }
        ReceiptField::FinalizedOutput => {
            BackendPublicationReceiptValidationErrorV1::FinalizedOutputIdentityMismatch
        }
        ReceiptField::Publication => {
            BackendPublicationReceiptValidationErrorV1::PublicationIdentityMismatch
        }
        ReceiptField::CompilerClosure => unreachable!("V1 receipts have no compiler closure"),
    })
}

/// Validates a protected receipt against all publication inputs and the exact compiler closure.
pub fn validate_backend_publication_receipt_v2(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    compiler_closure: CompilerClosureV2,
    receipt: BackendPublicationReceiptV2,
) -> Result<(), BackendPublicationReceiptValidationErrorV2> {
    if plan.attempt() != attempt {
        return Err(BackendPublicationReceiptValidationErrorV2::PlanAttemptMismatch);
    }
    let expected =
        publication_receipt_v2(producer, attempt, plan, upstream_evidence, compiler_closure);
    validate_receipt_fields(receipt, expected, true).map_err(|field| match field {
        ReceiptField::Attempt => {
            BackendPublicationReceiptValidationErrorV2::AttemptIdentityMismatch
        }
        ReceiptField::Producer => {
            BackendPublicationReceiptValidationErrorV2::ProducerIdentityMismatch
        }
        ReceiptField::Scope => BackendPublicationReceiptValidationErrorV2::ScopeIdentityMismatch,
        ReceiptField::Plan => BackendPublicationReceiptValidationErrorV2::PlanCommitmentMismatch,
        ReceiptField::UpstreamEvidence => {
            BackendPublicationReceiptValidationErrorV2::UpstreamEvidenceIdentityMismatch
        }
        ReceiptField::FinalizedOutput => {
            BackendPublicationReceiptValidationErrorV2::FinalizedOutputIdentityMismatch
        }
        ReceiptField::Publication => {
            BackendPublicationReceiptValidationErrorV2::PublicationIdentityMismatch
        }
        ReceiptField::CompilerClosure => {
            BackendPublicationReceiptValidationErrorV2::CompilerClosureMismatch
        }
    })
}

#[derive(Clone, Copy)]
pub(crate) enum ReceiptField {
    Attempt,
    Producer,
    Scope,
    Plan,
    UpstreamEvidence,
    FinalizedOutput,
    Publication,
    CompilerClosure,
}

pub(crate) trait ReceiptEvidence: Copy {
    fn attempt_identity(self) -> [u8; 32];
    fn producer_identity(self) -> [u8; 32];
    fn scope_identity(self) -> [u8; 32];
    fn plan_commitment(self) -> [u8; 32];
    fn upstream_evidence_identity(self) -> [u8; 32];
    fn finalized_output_identity(self) -> [u8; 32];
    fn publication_identity(self) -> [u8; 32];
    fn compiler_closure(self) -> Option<CompilerClosureV2>;
}

impl ReceiptEvidence for BackendPublicationReceiptV1 {
    fn attempt_identity(self) -> [u8; 32] {
        self.attempt_identity()
    }

    fn producer_identity(self) -> [u8; 32] {
        self.producer_identity()
    }

    fn scope_identity(self) -> [u8; 32] {
        self.scope_identity()
    }

    fn plan_commitment(self) -> [u8; 32] {
        self.plan_commitment()
    }

    fn upstream_evidence_identity(self) -> [u8; 32] {
        self.upstream_evidence_identity()
    }

    fn finalized_output_identity(self) -> [u8; 32] {
        self.finalized_output_identity()
    }

    fn publication_identity(self) -> [u8; 32] {
        self.publication_identity()
    }

    fn compiler_closure(self) -> Option<CompilerClosureV2> {
        None
    }
}

impl ReceiptEvidence for BackendPublicationReceiptV2 {
    fn attempt_identity(self) -> [u8; 32] {
        self.attempt_identity()
    }

    fn producer_identity(self) -> [u8; 32] {
        self.producer_identity()
    }

    fn scope_identity(self) -> [u8; 32] {
        self.scope_identity()
    }

    fn plan_commitment(self) -> [u8; 32] {
        self.plan_commitment()
    }

    fn upstream_evidence_identity(self) -> [u8; 32] {
        self.upstream_evidence_identity()
    }

    fn finalized_output_identity(self) -> [u8; 32] {
        self.finalized_output_identity()
    }

    fn publication_identity(self) -> [u8; 32] {
        self.publication_identity()
    }

    fn compiler_closure(self) -> Option<CompilerClosureV2> {
        Some(self.compiler_closure())
    }
}

pub(crate) fn validate_receipt_fields<R: ReceiptEvidence>(
    receipt: R,
    expected: R,
    require_compiler_closure: bool,
) -> Result<(), ReceiptField> {
    for (matches, field) in [
        (
            receipt.attempt_identity() == expected.attempt_identity(),
            ReceiptField::Attempt,
        ),
        (
            receipt.producer_identity() == expected.producer_identity(),
            ReceiptField::Producer,
        ),
        (
            receipt.scope_identity() == expected.scope_identity(),
            ReceiptField::Scope,
        ),
        (
            receipt.plan_commitment() == expected.plan_commitment(),
            ReceiptField::Plan,
        ),
        (
            receipt.upstream_evidence_identity() == expected.upstream_evidence_identity(),
            ReceiptField::UpstreamEvidence,
        ),
        (
            receipt.finalized_output_identity() == expected.finalized_output_identity(),
            ReceiptField::FinalizedOutput,
        ),
        (
            receipt.publication_identity() == expected.publication_identity(),
            ReceiptField::Publication,
        ),
    ] {
        if !matches {
            return Err(field);
        }
    }
    if require_compiler_closure && receipt.compiler_closure() != expected.compiler_closure() {
        return Err(ReceiptField::CompilerClosure);
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

/// Durable protected receipt state retained for an exact build attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedBackendReceiptV2 {
    None,
    PendingProvenance(BackendPublicationReceiptV2),
    Provenance(BackendPublicationReceiptV2),
}

/// Attempt-registry durability operation at which a protected test may interrupt publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptScopedHsacoPublicationBoundaryV2 {
    CommitPendingReceipt,
    CommitFinalReceipt,
}

/// Side of an attempt-registry commit at which a protected test interruption occurs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptScopedHsacoPublicationFaultTimingV2 {
    Before,
    After,
}

/// Deterministic protected attempt-scoped receipt-commit crash point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptScopedHsacoPublicationFaultPointV2 {
    pub boundary: AttemptScopedHsacoPublicationBoundaryV2,
    pub timing: AttemptScopedHsacoPublicationFaultTimingV2,
}

/// Protected publication fault options. Production callers use [`Default::default`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AttemptScopedHsacoPublicationOptionsV2 {
    durable: DurableLinkPublicationOptionsV1,
    receipt_crash: Option<AttemptScopedHsacoPublicationFaultPointV2>,
}

impl AttemptScopedHsacoPublicationOptionsV2 {
    pub const fn inject_durable_crash(point: crate::DurableLinkPublicationFaultPointV1) -> Self {
        Self {
            durable: DurableLinkPublicationOptionsV1::inject_crash(point),
            receipt_crash: None,
        }
    }

    pub fn inject_receipt_crash(point: AttemptScopedHsacoPublicationFaultPointV2) -> Self {
        Self {
            durable: DurableLinkPublicationOptionsV1::default(),
            receipt_crash: Some(point),
        }
    }
}

impl From<DurableLinkPublicationOptionsV1> for AttemptScopedHsacoPublicationOptionsV2 {
    fn from(durable: DurableLinkPublicationOptionsV1) -> Self {
        Self {
            durable,
            receipt_crash: None,
        }
    }
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

/// Failure while publishing or recovering protected V2 HSACO evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum AttemptScopedHsacoPublicationErrorV2 {
    PlanAttemptMismatch,
    Attempt(EmitError),
    /// The attempt contains legacy or otherwise foreign receipt state. It is left untouched.
    IncompatibleReceiptVersion,
    UnrecoverableClaimedAttempt,
    PendingReceiptMismatch,
    ReceiptCommitInterrupted {
        point: AttemptScopedHsacoPublicationFaultPointV2,
    },
    PublicationInterrupted(DurableLinkPublicationError),
    Durable(DurableLinkPublicationError),
    DurableAndAttemptState {
        publication: Box<DurableLinkPublicationError>,
        attempt_state: Box<EmitError>,
    },
    UnexpectedPreexistingPublication {
        publication: DurableLinkPublicationResultV1,
    },
    PublicationCommittedWithoutReceipt {
        publication: DurableLinkPublicationResultV1,
        expected_receipt: Box<BackendPublicationReceiptV2>,
        attempt_state: Box<EmitError>,
    },
    ReceiptPublicationMismatch,
    ReceiptAlreadyPersisted {
        receipt: Box<BackendPublicationReceiptV2>,
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

impl AttemptScopedHsacoPublicationErrorV2 {
    pub const fn committed_publication(&self) -> Option<&DurableLinkPublicationResultV1> {
        match self {
            Self::UnexpectedPreexistingPublication { publication }
            | Self::PublicationCommittedWithoutReceipt { publication, .. } => Some(publication),
            _ => None,
        }
    }

    pub const fn expected_receipt(&self) -> Option<BackendPublicationReceiptV2> {
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

impl fmt::Display for AttemptScopedHsacoPublicationErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanAttemptMismatch => formatter.write_str(
                "durable publication plan does not match the supplied protected build attempt",
            ),
            Self::Attempt(error) => write!(formatter, "protected build attempt rejected: {error}"),
            Self::IncompatibleReceiptVersion => formatter.write_str(
                "protected publication found an incompatible backend receipt version",
            ),
            Self::UnrecoverableClaimedAttempt => formatter.write_str(
                "claimed protected build attempt has no exact durable publication plan to recover",
            ),
            Self::PendingReceiptMismatch => formatter.write_str(
                "crash-recovery inputs do not match the pending protected backend receipt",
            ),
            Self::ReceiptCommitInterrupted { point } => write!(
                formatter,
                "protected receipt commit was interrupted at {point:?}"
            ),
            Self::PublicationInterrupted(error) => write!(
                formatter,
                "protected exact-byte publication was interrupted and requires exact-input reconciliation: {error}"
            ),
            Self::Durable(error) => write!(formatter, "protected exact HSACO publication failed: {error}"),
            Self::DurableAndAttemptState {
                publication,
                attempt_state,
            } => write!(
                formatter,
                "protected exact HSACO publication failed ({publication}); terminal attempt update also failed ({attempt_state})"
            ),
            Self::UnexpectedPreexistingPublication { .. } => formatter.write_str(
                "fresh protected attempt found a preexisting durable publication and refused to adopt it",
            ),
            Self::PublicationCommittedWithoutReceipt { attempt_state, .. } => write!(
                formatter,
                "protected exact HSACO publication committed, but its V2 backend receipt did not: {attempt_state}"
            ),
            Self::ReceiptPublicationMismatch => formatter.write_str(
                "persisted protected backend receipt has no matching complete durable publication",
            ),
            Self::ReceiptAlreadyPersisted { .. } => formatter.write_str(
                "the exact protected backend receipt and durable publication are already persisted",
            ),
        }
    }
}

impl std::error::Error for AttemptScopedHsacoPublicationErrorV2 {
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
    let result = publish_exact_hsaco_evidence_for_attempt::<PublicationSchemaV1>(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        (),
        exact_hsaco,
        PublicationOptions {
            durable: options,
            receipt_crash: None,
        },
    )
    .map_err(publication_error_v1)?;
    Ok(AttemptScopedHsacoPublicationResultV1 {
        outcome: outcome_v1(result.outcome),
        publication: result.publication,
        receipt: result.receipt,
        claim: result.claim,
    })
}

/// Publishes protected exact HSACO while retaining the complete compiler closure in V2 evidence.
#[allow(clippy::too_many_arguments)]
pub fn publish_exact_hsaco_evidence_for_attempt_v2(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    compiler_closure: CompilerClosureV2,
    exact_hsaco: &[u8],
) -> Result<AttemptScopedHsacoPublicationResultV2, AttemptScopedHsacoPublicationErrorV2> {
    publish_exact_hsaco_evidence_for_attempt_v2_with_options(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        compiler_closure,
        exact_hsaco,
        DurableLinkPublicationOptionsV1::default(),
    )
}

/// Fault-injectable form of [`publish_exact_hsaco_evidence_for_attempt_v2`].
#[allow(clippy::too_many_arguments)]
pub fn publish_exact_hsaco_evidence_for_attempt_v2_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    compiler_closure: CompilerClosureV2,
    exact_hsaco: &[u8],
    options: impl Into<AttemptScopedHsacoPublicationOptionsV2>,
) -> Result<AttemptScopedHsacoPublicationResultV2, AttemptScopedHsacoPublicationErrorV2> {
    let options = options.into();
    let result = publish_exact_hsaco_evidence_for_attempt::<PublicationSchemaV2>(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        compiler_closure,
        exact_hsaco,
        PublicationOptions {
            durable: options.durable,
            receipt_crash: options.receipt_crash,
        },
    )
    .map_err(publication_error_v2)?;
    Ok(AttemptScopedHsacoPublicationResultV2 {
        outcome: outcome_v2(result.outcome),
        publication: result.publication,
        receipt: result.receipt,
        claim: result.claim,
    })
}

#[derive(Clone, Copy)]
enum PublicationOutcome {
    Published,
    RecoveredAndPublished,
    RecoveredCommittedPublication,
}

#[derive(Clone, Copy)]
struct PublicationOptions {
    durable: DurableLinkPublicationOptionsV1,
    receipt_crash: Option<AttemptScopedHsacoPublicationFaultPointV2>,
}

enum Authorization {
    Fresh,
    Recovering(DurablePlanRecoveryStateV1),
}

enum SchemaReceiptState<R> {
    None,
    Legacy,
    Pending(R),
    Provenance(R),
    Foreign,
}

struct PublicationResult<R, C> {
    outcome: PublicationOutcome,
    publication: DurableLinkPublicationResultV1,
    receipt: R,
    claim: C,
}

type SchemaPublicationResult<S> = Result<
    PublicationResult<<S as PublicationSchema>::Receipt, <S as PublicationSchema>::Claim>,
    PublicationError<<S as PublicationSchema>::Receipt>,
>;

enum PublicationError<R> {
    PlanAttemptMismatch,
    Attempt(EmitError),
    ForeignReceipt,
    UnrecoverableClaimedAttempt,
    PendingReceiptMismatch,
    ReceiptCommitInterrupted {
        point: AttemptScopedHsacoPublicationFaultPointV2,
    },
    PublicationInterrupted(DurableLinkPublicationError),
    Durable(DurableLinkPublicationError),
    DurableAndAttemptState {
        publication: Box<DurableLinkPublicationError>,
        attempt_state: Box<EmitError>,
    },
    UnexpectedPreexistingPublication {
        publication: DurableLinkPublicationResultV1,
    },
    PublicationCommittedWithoutReceipt {
        publication: DurableLinkPublicationResultV1,
        expected_receipt: Box<R>,
        attempt_state: Box<EmitError>,
    },
    ReceiptPublicationMismatch,
    ReceiptAlreadyPersisted {
        receipt: Box<R>,
    },
}

trait PublicationSchema {
    type Receipt: ReceiptEvidence + Eq;
    type Binding: Copy;
    type Claim;

    fn receipt(
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        binding: Self::Binding,
    ) -> Self::Receipt;
    fn receipt_state(receipt: Option<BackendReceiptV1>) -> SchemaReceiptState<Self::Receipt>;
    fn persist_pending(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError>;
    fn persist_completed(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError>;
    fn claim(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        receipt: Self::Receipt,
        files: crate::durable_link_publication::DurablePublishedFileBindingV1,
    ) -> Self::Claim;
}

struct PublicationSchemaV1;

impl PublicationSchema for PublicationSchemaV1 {
    type Receipt = BackendPublicationReceiptV1;
    type Binding = ();
    type Claim = DurablePublishedHsacoClaimV1;

    fn receipt(
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        (): (),
    ) -> Self::Receipt {
        publication_receipt(producer, attempt, plan, upstream_evidence)
    }

    fn receipt_state(receipt: Option<BackendReceiptV1>) -> SchemaReceiptState<Self::Receipt> {
        match receipt {
            None => SchemaReceiptState::None,
            Some(BackendReceiptV1::LegacyCoordination) => SchemaReceiptState::Legacy,
            Some(BackendReceiptV1::PendingProvenance(receipt)) => {
                SchemaReceiptState::Pending(receipt)
            }
            Some(BackendReceiptV1::Provenance(receipt)) => SchemaReceiptState::Provenance(receipt),
            Some(BackendReceiptV1::PendingProvenanceV2(_) | BackendReceiptV1::ProvenanceV2(_)) => {
                SchemaReceiptState::Foreign
            }
        }
    }

    fn persist_pending(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError> {
        attempts.claim_backend_with_pending_receipt(stable_source, attempt, receipt)
    }

    fn persist_completed(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError> {
        attempts.record_backend_publication_receipt(stable_source, attempt, receipt)
    }

    fn claim(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        receipt: Self::Receipt,
        files: crate::durable_link_publication::DurablePublishedFileBindingV1,
    ) -> Self::Claim {
        DurablePublishedHsacoClaimV1::new(plan, upstream_evidence, receipt, files)
    }
}

struct PublicationSchemaV2;

impl PublicationSchema for PublicationSchemaV2 {
    type Receipt = BackendPublicationReceiptV2;
    type Binding = CompilerClosureV2;
    type Claim = DurablePublishedHsacoClaimV2;

    fn receipt(
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        compiler_closure: Self::Binding,
    ) -> Self::Receipt {
        publication_receipt_v2(producer, attempt, plan, upstream_evidence, compiler_closure)
    }

    fn receipt_state(receipt: Option<BackendReceiptV1>) -> SchemaReceiptState<Self::Receipt> {
        match receipt {
            None => SchemaReceiptState::None,
            Some(BackendReceiptV1::PendingProvenanceV2(receipt)) => {
                SchemaReceiptState::Pending(receipt)
            }
            Some(BackendReceiptV1::ProvenanceV2(receipt)) => {
                SchemaReceiptState::Provenance(receipt)
            }
            Some(
                BackendReceiptV1::LegacyCoordination
                | BackendReceiptV1::PendingProvenance(_)
                | BackendReceiptV1::Provenance(_),
            ) => SchemaReceiptState::Foreign,
        }
    }

    fn persist_pending(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError> {
        attempts.claim_backend_with_pending_receipt_v2(stable_source, attempt, receipt)
    }

    fn persist_completed(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError> {
        attempts.record_backend_publication_receipt_v2(stable_source, attempt, receipt)
    }

    fn claim(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        receipt: Self::Receipt,
        files: crate::durable_link_publication::DurablePublishedFileBindingV1,
    ) -> Self::Claim {
        DurablePublishedHsacoClaimV2::new(plan, upstream_evidence, receipt, files)
    }
}

#[allow(clippy::too_many_arguments)]
fn publish_exact_hsaco_evidence_for_attempt<S: PublicationSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    binding: S::Binding,
    exact_hsaco: &[u8],
    options: PublicationOptions,
) -> SchemaPublicationResult<S> {
    if plan.attempt() != attempt {
        return Err(PublicationError::PlanAttemptMismatch);
    }
    if attempt.session() == BuildSession::DIRECT {
        return Err(PublicationError::Attempt(build_attempt_error(
            "the direct compiler token cannot authorize managed publication",
        )));
    }

    let expected_receipt = S::receipt(producer, attempt, plan, upstream_evidence, binding);
    let output = PinnedOutput::open_existing(output_dir).map_err(PublicationError::Attempt)?;
    let _lock = output.lock().map_err(PublicationError::Attempt)?;
    output
        .verify_path_identity()
        .map_err(PublicationError::Attempt)?;

    let mut attempts = read_attempt_registry(&output).map_err(PublicationError::Attempt)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)
        .map_err(PublicationError::Attempt)?;
    if record.crate_name != producer.crate_name {
        return Err(PublicationError::Attempt(build_attempt_error(
            "build attempt crate name does not match the producer",
        )));
    }
    let phase = record.phase;
    let receipt_state = S::receipt_state(record.backend_receipt);

    let authorization = match (phase, receipt_state) {
        (AttemptPhase::Building, SchemaReceiptState::None) => {
            hit_receipt_fault::<S::Receipt>(
                options.receipt_crash,
                AttemptScopedHsacoPublicationBoundaryV2::CommitPendingReceipt,
                AttemptScopedHsacoPublicationFaultTimingV2::Before,
            )?;
            S::persist_pending(
                &mut attempts,
                &producer.stable_source,
                attempt,
                expected_receipt,
            )
            .map_err(build_attempt_error)
            .map_err(PublicationError::Attempt)?;
            commit_attempt_registry_direct(&output, &attempts)
                .map_err(PublicationError::Attempt)?;
            hit_receipt_fault::<S::Receipt>(
                options.receipt_crash,
                AttemptScopedHsacoPublicationBoundaryV2::CommitPendingReceipt,
                AttemptScopedHsacoPublicationFaultTimingV2::After,
            )?;
            Authorization::Fresh
        }
        (AttemptPhase::BackendClaimed, SchemaReceiptState::Pending(pending))
            if pending == expected_receipt =>
        {
            match recover_durable_link_plan_locked(&output, plan) {
                Ok(Some(state)) => Authorization::Recovering(state),
                Ok(None) => Authorization::Fresh,
                recovery => {
                    let failure =
                        fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
                    return match (recovery, failure) {
                        (Err(publication), Ok(())) => Err(PublicationError::Durable(publication)),
                        (Err(publication), Err(attempt_state)) => {
                            Err(PublicationError::DurableAndAttemptState {
                                publication: Box::new(publication),
                                attempt_state: Box::new(attempt_state),
                            })
                        }
                        (Ok(None), _) => unreachable!("absent plans retry as fresh publication"),
                        (Ok(Some(_)), _) => unreachable!("matched recoverable attempt"),
                    };
                }
            }
        }
        (AttemptPhase::BackendClaimed, SchemaReceiptState::Pending(_)) => {
            let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
            return Err(PublicationError::PendingReceiptMismatch);
        }
        (AttemptPhase::BackendClaimed, SchemaReceiptState::None) => {
            let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
            return Err(PublicationError::UnrecoverableClaimedAttempt);
        }
        (AttemptPhase::BackendClaimed, SchemaReceiptState::Provenance(receipt))
            if receipt == expected_receipt =>
        {
            match recover_durable_link_plan_locked(&output, plan) {
                Ok(Some(DurablePlanRecoveryStateV1::Published)) => {
                    return Err(PublicationError::ReceiptAlreadyPersisted {
                        receipt: Box::new(receipt),
                    });
                }
                _ => {
                    let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
                    return Err(PublicationError::ReceiptPublicationMismatch);
                }
            }
        }
        (AttemptPhase::BackendClaimed, SchemaReceiptState::Foreign) => {
            return Err(PublicationError::ForeignReceipt);
        }
        _ => {
            return Err(PublicationError::Attempt(build_attempt_error(
                "build attempt cannot authorize exact HSACO publication in its current phase",
            )));
        }
    };

    let publication =
        publish_durable_link_v1_locked(&output, plan, options.durable, |transaction| {
            transaction.record_worker_pinned()?;
            transaction.record_response_validated()?;
            transaction.record_finalized(exact_hsaco)
        });
    let publication = match publication {
        Ok(publication) => publication,
        Err(error @ DurableLinkPublicationError::InjectedCrash { .. }) => {
            return Err(PublicationError::PublicationInterrupted(error));
        }
        Err(publication) => {
            return match fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults) {
                Ok(()) => Err(PublicationError::Durable(publication)),
                Err(attempt_state) => Err(PublicationError::DurableAndAttemptState {
                    publication: Box::new(publication),
                    attempt_state: Box::new(attempt_state),
                }),
            };
        }
    };

    if matches!(authorization, Authorization::Fresh)
        && publication.outcome() == DurableLinkPublicationOutcomeV1::AlreadyPublished
    {
        let _ = fail_build_attempt_locked(&output, producer, attempt, &mut NoFaults);
        return Err(PublicationError::UnexpectedPreexistingPublication { publication });
    }

    hit_receipt_fault::<S::Receipt>(
        options.receipt_crash,
        AttemptScopedHsacoPublicationBoundaryV2::CommitFinalReceipt,
        AttemptScopedHsacoPublicationFaultTimingV2::Before,
    )?;
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
        S::persist_completed(
            &mut attempts,
            &producer.stable_source,
            attempt,
            expected_receipt,
        )
        .map_err(build_attempt_error)?;
        commit_attempt_registry_direct(&output, &attempts)
    })();
    if let Err(attempt_state) = attempt_state {
        return Err(PublicationError::PublicationCommittedWithoutReceipt {
            publication,
            expected_receipt: Box::new(expected_receipt),
            attempt_state: Box::new(attempt_state),
        });
    }
    hit_receipt_fault::<S::Receipt>(
        options.receipt_crash,
        AttemptScopedHsacoPublicationBoundaryV2::CommitFinalReceipt,
        AttemptScopedHsacoPublicationFaultTimingV2::After,
    )?;

    let outcome = match (authorization, publication.outcome()) {
        (Authorization::Fresh, DurableLinkPublicationOutcomeV1::Published) => {
            PublicationOutcome::Published
        }
        (
            Authorization::Recovering(DurablePlanRecoveryStateV1::Incomplete),
            DurableLinkPublicationOutcomeV1::Published,
        ) => PublicationOutcome::RecoveredAndPublished,
        (
            Authorization::Recovering(DurablePlanRecoveryStateV1::Published),
            DurableLinkPublicationOutcomeV1::AlreadyPublished,
        ) => PublicationOutcome::RecoveredCommittedPublication,
        _ => PublicationOutcome::RecoveredCommittedPublication,
    };
    let claim = S::claim(
        plan,
        upstream_evidence,
        expected_receipt,
        publication.published_file_binding(),
    );
    Ok(PublicationResult {
        outcome,
        publication,
        receipt: expected_receipt,
        claim,
    })
}

fn hit_receipt_fault<R>(
    configured: Option<AttemptScopedHsacoPublicationFaultPointV2>,
    boundary: AttemptScopedHsacoPublicationBoundaryV2,
    timing: AttemptScopedHsacoPublicationFaultTimingV2,
) -> Result<(), PublicationError<R>> {
    let point = AttemptScopedHsacoPublicationFaultPointV2 { boundary, timing };
    if configured == Some(point) {
        Err(PublicationError::ReceiptCommitInterrupted { point })
    } else {
        Ok(())
    }
}

fn outcome_v1(outcome: PublicationOutcome) -> AttemptScopedHsacoPublicationOutcomeV1 {
    match outcome {
        PublicationOutcome::Published => AttemptScopedHsacoPublicationOutcomeV1::Published,
        PublicationOutcome::RecoveredAndPublished => {
            AttemptScopedHsacoPublicationOutcomeV1::RecoveredAndPublished
        }
        PublicationOutcome::RecoveredCommittedPublication => {
            AttemptScopedHsacoPublicationOutcomeV1::RecoveredCommittedPublication
        }
    }
}

fn outcome_v2(outcome: PublicationOutcome) -> AttemptScopedHsacoPublicationOutcomeV2 {
    match outcome {
        PublicationOutcome::Published => AttemptScopedHsacoPublicationOutcomeV2::Published,
        PublicationOutcome::RecoveredAndPublished => {
            AttemptScopedHsacoPublicationOutcomeV2::RecoveredAndPublished
        }
        PublicationOutcome::RecoveredCommittedPublication => {
            AttemptScopedHsacoPublicationOutcomeV2::RecoveredCommittedPublication
        }
    }
}

fn publication_error_v1(
    error: PublicationError<BackendPublicationReceiptV1>,
) -> AttemptScopedHsacoPublicationErrorV1 {
    match error {
        PublicationError::PlanAttemptMismatch => {
            AttemptScopedHsacoPublicationErrorV1::PlanAttemptMismatch
        }
        PublicationError::Attempt(error) => AttemptScopedHsacoPublicationErrorV1::Attempt(error),
        PublicationError::ForeignReceipt => AttemptScopedHsacoPublicationErrorV1::Attempt(
            build_attempt_error("build attempt contains an incompatible protected backend receipt"),
        ),
        PublicationError::UnrecoverableClaimedAttempt => {
            AttemptScopedHsacoPublicationErrorV1::UnrecoverableClaimedAttempt
        }
        PublicationError::PendingReceiptMismatch => {
            AttemptScopedHsacoPublicationErrorV1::PendingReceiptMismatch
        }
        PublicationError::ReceiptCommitInterrupted { .. } => {
            unreachable!("V1 publication does not configure receipt fault injection")
        }
        PublicationError::PublicationInterrupted(error) => {
            AttemptScopedHsacoPublicationErrorV1::PublicationInterrupted(error)
        }
        PublicationError::Durable(error) => AttemptScopedHsacoPublicationErrorV1::Durable(error),
        PublicationError::DurableAndAttemptState {
            publication,
            attempt_state,
        } => AttemptScopedHsacoPublicationErrorV1::DurableAndAttemptState {
            publication,
            attempt_state,
        },
        PublicationError::UnexpectedPreexistingPublication { publication } => {
            AttemptScopedHsacoPublicationErrorV1::UnexpectedPreexistingPublication { publication }
        }
        PublicationError::PublicationCommittedWithoutReceipt {
            publication,
            expected_receipt,
            attempt_state,
        } => AttemptScopedHsacoPublicationErrorV1::PublicationCommittedWithoutReceipt {
            publication,
            expected_receipt,
            attempt_state,
        },
        PublicationError::ReceiptPublicationMismatch => {
            AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch
        }
        PublicationError::ReceiptAlreadyPersisted { receipt } => {
            AttemptScopedHsacoPublicationErrorV1::ReceiptAlreadyPersisted { receipt }
        }
    }
}

fn publication_error_v2(
    error: PublicationError<BackendPublicationReceiptV2>,
) -> AttemptScopedHsacoPublicationErrorV2 {
    match error {
        PublicationError::PlanAttemptMismatch => {
            AttemptScopedHsacoPublicationErrorV2::PlanAttemptMismatch
        }
        PublicationError::Attempt(error) => AttemptScopedHsacoPublicationErrorV2::Attempt(error),
        PublicationError::ForeignReceipt => {
            AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion
        }
        PublicationError::UnrecoverableClaimedAttempt => {
            AttemptScopedHsacoPublicationErrorV2::UnrecoverableClaimedAttempt
        }
        PublicationError::PendingReceiptMismatch => {
            AttemptScopedHsacoPublicationErrorV2::PendingReceiptMismatch
        }
        PublicationError::ReceiptCommitInterrupted { point } => {
            AttemptScopedHsacoPublicationErrorV2::ReceiptCommitInterrupted { point }
        }
        PublicationError::PublicationInterrupted(error) => {
            AttemptScopedHsacoPublicationErrorV2::PublicationInterrupted(error)
        }
        PublicationError::Durable(error) => AttemptScopedHsacoPublicationErrorV2::Durable(error),
        PublicationError::DurableAndAttemptState {
            publication,
            attempt_state,
        } => AttemptScopedHsacoPublicationErrorV2::DurableAndAttemptState {
            publication,
            attempt_state,
        },
        PublicationError::UnexpectedPreexistingPublication { publication } => {
            AttemptScopedHsacoPublicationErrorV2::UnexpectedPreexistingPublication { publication }
        }
        PublicationError::PublicationCommittedWithoutReceipt {
            publication,
            expected_receipt,
            attempt_state,
        } => AttemptScopedHsacoPublicationErrorV2::PublicationCommittedWithoutReceipt {
            publication,
            expected_receipt,
            attempt_state,
        },
        PublicationError::ReceiptPublicationMismatch => {
            AttemptScopedHsacoPublicationErrorV2::ReceiptPublicationMismatch
        }
        PublicationError::ReceiptAlreadyPersisted { receipt } => {
            AttemptScopedHsacoPublicationErrorV2::ReceiptAlreadyPersisted { receipt }
        }
    }
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
        Some(BackendReceiptV1::PendingProvenanceV2(_) | BackendReceiptV1::ProvenanceV2(_)) => {
            return Err(build_attempt_error(
                "build attempt contains an incompatible protected backend receipt",
            ));
        }
    })
}

/// Reads only a protected V2 receipt and rejects legacy state without changing it.
pub fn read_backend_publication_receipt_v2(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<PersistedBackendReceiptV2, AttemptScopedHsacoPublicationErrorV2> {
    let output = PinnedOutput::open_existing(output_dir)
        .map_err(AttemptScopedHsacoPublicationErrorV2::Attempt)?;
    let _lock = output
        .lock()
        .map_err(AttemptScopedHsacoPublicationErrorV2::Attempt)?;
    output
        .verify_path_identity()
        .map_err(AttemptScopedHsacoPublicationErrorV2::Attempt)?;
    let attempts =
        read_attempt_registry(&output).map_err(AttemptScopedHsacoPublicationErrorV2::Attempt)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)
        .map_err(AttemptScopedHsacoPublicationErrorV2::Attempt)?;
    if record.crate_name != producer.crate_name {
        return Err(AttemptScopedHsacoPublicationErrorV2::Attempt(
            build_attempt_error("build attempt crate name does not match the producer"),
        ));
    }
    match record.backend_receipt {
        None => Ok(PersistedBackendReceiptV2::None),
        Some(BackendReceiptV1::PendingProvenanceV2(receipt)) => {
            Ok(PersistedBackendReceiptV2::PendingProvenance(receipt))
        }
        Some(BackendReceiptV1::ProvenanceV2(receipt)) => {
            Ok(PersistedBackendReceiptV2::Provenance(receipt))
        }
        Some(
            BackendReceiptV1::LegacyCoordination
            | BackendReceiptV1::PendingProvenance(_)
            | BackendReceiptV1::Provenance(_),
        ) => Err(AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion),
    }
}

/// Reconstructs the inert canonical claim for one exact already-published attempt.
///
/// This reopens and validates the durable publication under its lock. The returned claim remains
/// inert and establishes neither compiler/proof authenticity nor load or launch authority.
pub fn recover_published_hsaco_claim_for_attempt_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: BackendPublicationReceiptV1,
) -> Result<DurablePublishedHsacoClaimV1, AttemptScopedHsacoPublicationErrorV1> {
    validate_backend_publication_receipt_v1(producer, attempt, plan, upstream_evidence, receipt)
        .map_err(|_| AttemptScopedHsacoPublicationErrorV1::ReceiptPublicationMismatch)?;
    recover_published_hsaco_claim_for_attempt::<PublicationSchemaV1>(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        receipt,
    )
    .map_err(publication_error_v1)
}

/// Reconstructs a protected claim only from an exact completed V2 receipt and closure.
#[allow(clippy::too_many_arguments)]
pub fn recover_published_hsaco_claim_for_attempt_v2(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    compiler_closure: CompilerClosureV2,
    receipt: BackendPublicationReceiptV2,
) -> Result<DurablePublishedHsacoClaimV2, AttemptScopedHsacoPublicationErrorV2> {
    validate_backend_publication_receipt_v2(
        producer,
        attempt,
        plan,
        upstream_evidence,
        compiler_closure,
        receipt,
    )
    .map_err(|_| AttemptScopedHsacoPublicationErrorV2::ReceiptPublicationMismatch)?;
    recover_published_hsaco_claim_for_attempt::<PublicationSchemaV2>(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        receipt,
    )
    .map_err(publication_error_v2)
}

fn recover_published_hsaco_claim_for_attempt<S: PublicationSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    receipt: S::Receipt,
) -> Result<S::Claim, PublicationError<S::Receipt>> {
    let output = PinnedOutput::open_existing(output_dir).map_err(PublicationError::Attempt)?;
    let _lock = output.lock().map_err(PublicationError::Attempt)?;
    output
        .verify_path_identity()
        .map_err(PublicationError::Attempt)?;
    let attempts = read_attempt_registry(&output).map_err(PublicationError::Attempt)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)
        .map_err(PublicationError::Attempt)?;
    if matches!(
        S::receipt_state(record.backend_receipt),
        SchemaReceiptState::Foreign
    ) {
        return Err(PublicationError::ForeignReceipt);
    }
    if record.crate_name != producer.crate_name
        || !matches!(
            record.phase,
            AttemptPhase::BackendClaimed | AttemptPhase::Completed
        )
        || !matches!(
            S::receipt_state(record.backend_receipt),
            SchemaReceiptState::Provenance(persisted) if persisted == receipt
        )
    {
        return Err(PublicationError::ReceiptPublicationMismatch);
    }
    let files = recover_durable_published_file_binding_locked(&output, plan)
        .map_err(PublicationError::Durable)?
        .ok_or(PublicationError::ReceiptPublicationMismatch)?;
    Ok(S::claim(plan, upstream_evidence, receipt, files))
}

pub(crate) fn publication_receipt(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
) -> BackendPublicationReceiptV1 {
    publication_receipt_for_producer_identity(
        attempt,
        plan,
        upstream_evidence,
        producer_receipt_identity_v1(&producer.stable_source, &producer.crate_name),
    )
}

pub(crate) fn publication_receipt_for_producer_identity(
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    producer_identity: [u8; 32],
) -> BackendPublicationReceiptV1 {
    let (attempt_identity, scope_identity) = receipt_context_identities(
        attempt,
        plan,
        ATTEMPT_IDENTITY_DOMAIN,
        SCOPE_IDENTITY_DOMAIN,
    );

    BackendPublicationReceiptV1::new(
        attempt_identity,
        producer_identity,
        scope_identity,
        plan.identity(),
        upstream_evidence.as_bytes(),
        *plan.finalized_output().as_bytes(),
        *plan.publication().as_bytes(),
    )
}

pub(crate) fn publication_receipt_v2(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    compiler_closure: CompilerClosureV2,
) -> BackendPublicationReceiptV2 {
    publication_receipt_for_producer_identity_v2(
        attempt,
        plan,
        upstream_evidence,
        compiler_closure,
        producer_receipt_identity_v2(&producer.stable_source, &producer.crate_name),
    )
}

/// Returns the canonical V2 receipt identity for one exact build attempt.
pub(crate) fn backend_publication_receipt_attempt_identity_v2(attempt: BuildAttempt) -> [u8; 32] {
    receipt_attempt_identity(attempt, ATTEMPT_IDENTITY_DOMAIN_V2)
}

pub(crate) fn publication_receipt_for_producer_identity_v2(
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    compiler_closure: CompilerClosureV2,
    producer_identity: [u8; 32],
) -> BackendPublicationReceiptV2 {
    let (attempt_identity, scope_identity) = receipt_context_identities(
        attempt,
        plan,
        ATTEMPT_IDENTITY_DOMAIN_V2,
        SCOPE_IDENTITY_DOMAIN_V2,
    );

    BackendPublicationReceiptV2::new(
        attempt_identity,
        producer_identity,
        scope_identity,
        plan.identity(),
        upstream_evidence.as_bytes(),
        *plan.finalized_output().as_bytes(),
        *plan.publication().as_bytes(),
        compiler_closure,
    )
}

fn receipt_context_identities(
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    attempt_domain: &[u8],
    scope_domain: &[u8],
) -> ([u8; 32], [u8; 32]) {
    let attempt_identity = receipt_attempt_identity(attempt, attempt_domain);

    let scope = plan.scope();
    let mut scope_digest = Sha256::new();
    scope_digest.update(scope_domain);
    scope_digest.update(scope.package().as_bytes());
    scope_digest.update(scope.kernel_set().as_bytes());
    scope_digest.update(scope.target().as_bytes());

    (attempt_identity, scope_digest.finalize().into())
}

fn receipt_attempt_identity(attempt: BuildAttempt, attempt_domain: &[u8]) -> [u8; 32] {
    let mut attempt_digest = Sha256::new();
    attempt_digest.update(attempt_domain);
    attempt_digest.update(attempt.generation().to_le_bytes());
    attempt_digest.update(attempt.session().as_bytes());
    attempt_digest.update(attempt.invocation().as_bytes());
    attempt_digest.finalize().into()
}

pub(crate) fn producer_receipt_identity_v1(stable_source: &str, crate_name: &str) -> [u8; 32] {
    producer_receipt_identity(stable_source, crate_name, PRODUCER_IDENTITY_DOMAIN)
}

pub(crate) fn producer_receipt_identity_v2(stable_source: &str, crate_name: &str) -> [u8; 32] {
    producer_receipt_identity(stable_source, crate_name, PRODUCER_IDENTITY_DOMAIN_V2)
}

fn producer_receipt_identity(stable_source: &str, crate_name: &str, domain: &[u8]) -> [u8; 32] {
    let mut producer_digest = Sha256::new();
    producer_digest.update(domain);
    producer_digest.update((stable_source.len() as u64).to_le_bytes());
    producer_digest.update(stable_source.as_bytes());
    producer_digest.update((crate_name.len() as u64).to_le_bytes());
    producer_digest.update(crate_name.as_bytes());
    producer_digest.finalize().into()
}
