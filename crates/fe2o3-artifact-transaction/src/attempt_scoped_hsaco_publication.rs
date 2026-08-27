use crate::attempt::{AttemptPhase, AttemptRegistry, BackendReceiptV1};
use crate::durable_link_publication::{
    DurablePlanRecoveryStateV1, publish_durable_link_v1_locked, recover_durable_link_plan_locked,
    recover_durable_published_file_binding_locked,
};
use crate::durable_published_claim::{
    DurablePublishedClaimCodecErrorV3, DurablePublishedHsacoClaimV3,
};
use crate::worker_v3_publication_intent::recover_worker_v3_publication_intent_locked_v1;
use crate::{
    BackendPublicationReceiptV3, BuildAttempt, BuildSession, DurableCurrentLinkPublicationLeaseV1,
    DurableLinkPublicationError, DurableLinkPublicationOptionsV1, DurableLinkPublicationOutcomeV1,
    DurableLinkPublicationPlanV1, DurableLinkPublicationResultV1, DurableLinkPublicationSnapshotV1,
    EmitError, NoFaults, PackageIdentityV1, PinnedOutput, ProducerIdentity,
    WorkerV3PublicationBindingV1, WorkerV3PublicationIntentErrorV1, build_attempt_error,
    commit_attempt_registry_direct, fail_build_attempt_locked, read_attempt_registry,
};
use fe2o3_build_authority::CompilerClosureV2;
use sha2::{Digest, Sha256};
use std::fmt;
use std::path::Path;

const PRODUCER_PACKAGE_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/COORDINATION-PRODUCER-PACKAGE/V1\0";
const ATTEMPT_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.backend-receipt.attempt.v3\0";
const PRODUCER_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.backend-receipt.producer.v3\0";
const SCOPE_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.backend-receipt.scope.v3\0";

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

/// How a strict Worker V3 exact-byte publication completed relative to its receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptScopedHsacoPublicationOutcomeV3 {
    Published,
    RecoveredAndPublished,
    RecoveredCommittedPublication,
}

/// Completed strict Worker V3 publication with exact finalizer and restart lineage.
#[derive(Debug)]
pub struct AttemptScopedHsacoPublicationResultV3 {
    outcome: AttemptScopedHsacoPublicationOutcomeV3,
    publication: DurableLinkPublicationResultV1,
    receipt: BackendPublicationReceiptV3,
    claim: DurablePublishedHsacoClaimV3,
}

/// Move-only authority to commit one semantically authenticated Worker V3 publication.
///
/// Safe code cannot construct this capability from free-standing identities. The strict finalizer
/// replay boundary creates it only after independently reconstructing every lineage axis. It grants
/// durable V3 publication authority, but no compiler, proof, load, or launch authority.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::{
///     VerifiedWorkerV3PublicationAuthorityV1, WorkerV3PublicationBindingV1,
/// };
///
/// fn safe_code_cannot_assert_replay(binding: WorkerV3PublicationBindingV1) {
///     let _ = VerifiedWorkerV3PublicationAuthorityV1::
///         from_authenticated_finalizer_replay_unchecked(binding);
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::{
///     VerifiedWorkerV3PublicationAuthorityV1, WorkerV3PublicationBindingV1,
/// };
///
/// fn fields_are_private(binding: WorkerV3PublicationBindingV1) {
///     let _ = VerifiedWorkerV3PublicationAuthorityV1 { binding };
/// }
/// ```
#[derive(Debug)]
pub struct VerifiedWorkerV3PublicationAuthorityV1 {
    binding: WorkerV3PublicationBindingV1,
}

impl VerifiedWorkerV3PublicationAuthorityV1 {
    /// Bridges an independently authenticated strict-finalizer result into the transaction layer.
    ///
    /// # Safety
    ///
    /// The caller must have independently replayed and authenticated the exact finalizer
    /// transcript named by `binding`, including its compiler closure, publication-intent record,
    /// source evidence, handoff, raw inspection, raw output, and finalized output. The caller must
    /// retain that replayed owner alongside the resulting durable publication.
    #[doc(hidden)]
    pub unsafe fn from_authenticated_finalizer_replay_unchecked(
        binding: WorkerV3PublicationBindingV1,
    ) -> Self {
        Self { binding }
    }

    pub const fn publication_binding(&self) -> WorkerV3PublicationBindingV1 {
        self.binding
    }

    pub const fn grants_publication_authority(&self) -> bool {
        true
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Failure while matching a strict Worker V3 receipt to exact publication inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BackendPublicationReceiptValidationErrorV3 {
    PlanAttemptMismatch,
    AttemptIdentityMismatch,
    ProducerIdentityMismatch,
    ScopeIdentityMismatch,
    PlanCommitmentMismatch,
    UpstreamEvidenceIdentityMismatch,
    FinalizedOutputIdentityMismatch,
    PublicationIdentityMismatch,
    CompilerClosureMismatch,
    PublicationBindingMismatch,
}

impl fmt::Display for BackendPublicationReceiptValidationErrorV3 {
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
            Self::PublicationBindingMismatch => "receipt Worker V3 publication binding",
        };
        write!(
            formatter,
            "strict Worker V3 backend publication {field} does not match"
        )
    }
}

impl std::error::Error for BackendPublicationReceiptValidationErrorV3 {}

impl AttemptScopedHsacoPublicationResultV3 {
    pub const fn outcome(&self) -> AttemptScopedHsacoPublicationOutcomeV3 {
        self.outcome
    }

    pub const fn durable_outcome(&self) -> DurableLinkPublicationOutcomeV1 {
        self.publication.outcome()
    }

    pub fn snapshot(&self) -> &DurableLinkPublicationSnapshotV1 {
        self.publication.snapshot()
    }

    pub const fn receipt(&self) -> BackendPublicationReceiptV3 {
        self.receipt
    }

    pub const fn publication_binding(&self) -> WorkerV3PublicationBindingV1 {
        self.receipt.publication_binding()
    }

    pub const fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
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

/// Validates a strict Worker V3 receipt against its exact publication binding and plan.
pub fn validate_backend_publication_receipt_v3(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    publication_binding: WorkerV3PublicationBindingV1,
    receipt: BackendPublicationReceiptV3,
) -> Result<(), BackendPublicationReceiptValidationErrorV3> {
    if plan.attempt() != attempt {
        return Err(BackendPublicationReceiptValidationErrorV3::PlanAttemptMismatch);
    }
    let expected = publication_receipt_v3(
        producer,
        attempt,
        plan,
        upstream_evidence,
        publication_binding,
    );
    validate_receipt_fields(receipt, expected, true).map_err(|field| match field {
        ReceiptField::Attempt => {
            BackendPublicationReceiptValidationErrorV3::AttemptIdentityMismatch
        }
        ReceiptField::Producer => {
            BackendPublicationReceiptValidationErrorV3::ProducerIdentityMismatch
        }
        ReceiptField::Scope => BackendPublicationReceiptValidationErrorV3::ScopeIdentityMismatch,
        ReceiptField::Plan => BackendPublicationReceiptValidationErrorV3::PlanCommitmentMismatch,
        ReceiptField::UpstreamEvidence => {
            BackendPublicationReceiptValidationErrorV3::UpstreamEvidenceIdentityMismatch
        }
        ReceiptField::FinalizedOutput => {
            BackendPublicationReceiptValidationErrorV3::FinalizedOutputIdentityMismatch
        }
        ReceiptField::Publication => {
            BackendPublicationReceiptValidationErrorV3::PublicationIdentityMismatch
        }
        ReceiptField::CompilerClosure => {
            BackendPublicationReceiptValidationErrorV3::CompilerClosureMismatch
        }
    })?;
    if receipt.publication_binding() != publication_binding {
        return Err(BackendPublicationReceiptValidationErrorV3::PublicationBindingMismatch);
    }
    Ok(())
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

impl ReceiptEvidence for BackendPublicationReceiptV3 {
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

/// Durable strict Worker V3 receipt state retained for one exact build attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PersistedBackendReceiptV3 {
    None,
    PendingProvenance(BackendPublicationReceiptV3),
    Provenance(BackendPublicationReceiptV3),
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

/// Failure while publishing or recovering strict Worker V3 HSACO evidence.
#[derive(Debug)]
#[non_exhaustive]
pub enum AttemptScopedHsacoPublicationErrorV3 {
    PlanAttemptMismatch,
    PublicationBindingMismatch,
    PublicationIntent(WorkerV3PublicationIntentErrorV1),
    Attempt(EmitError),
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
        expected_receipt: Box<BackendPublicationReceiptV3>,
        attempt_state: Box<EmitError>,
    },
    ReceiptPublicationMismatch,
    ReceiptAlreadyPersisted {
        receipt: Box<BackendPublicationReceiptV3>,
    },
    PublicationCommittedWithoutClaim {
        publication: DurableLinkPublicationResultV1,
        error: DurablePublishedClaimCodecErrorV3,
    },
    RecoveredClaimRejected {
        error: DurablePublishedClaimCodecErrorV3,
    },
}

impl AttemptScopedHsacoPublicationErrorV3 {
    pub const fn committed_publication(&self) -> Option<&DurableLinkPublicationResultV1> {
        match self {
            Self::UnexpectedPreexistingPublication { publication }
            | Self::PublicationCommittedWithoutReceipt { publication, .. }
            | Self::PublicationCommittedWithoutClaim { publication, .. } => Some(publication),
            _ => None,
        }
    }

    pub const fn expected_receipt(&self) -> Option<BackendPublicationReceiptV3> {
        match self {
            Self::PublicationCommittedWithoutReceipt {
                expected_receipt, ..
            } => Some(**expected_receipt),
            _ => None,
        }
    }
}

impl fmt::Display for AttemptScopedHsacoPublicationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlanAttemptMismatch => formatter.write_str(
                "durable publication plan does not match the supplied strict Worker V3 attempt",
            ),
            Self::PublicationBindingMismatch => formatter.write_str(
                "strict Worker V3 binding does not match the publication plan or exact HSACO",
            ),
            Self::PublicationIntent(error) => write!(
                formatter,
                "strict Worker V3 durable publication intent is not usable: {error}"
            ),
            Self::Attempt(error) => write!(formatter, "strict Worker V3 attempt rejected: {error}"),
            Self::IncompatibleReceiptVersion => formatter
                .write_str("strict Worker V3 publication found an incompatible receipt version"),
            Self::UnrecoverableClaimedAttempt => formatter.write_str(
                "claimed strict Worker V3 attempt has no exact durable publication to recover",
            ),
            Self::PendingReceiptMismatch => formatter.write_str(
                "crash-recovery inputs do not match the pending strict Worker V3 receipt",
            ),
            Self::ReceiptCommitInterrupted { point } => write!(
                formatter,
                "strict Worker V3 receipt commit was interrupted at {point:?}"
            ),
            Self::PublicationInterrupted(error) => write!(
                formatter,
                "strict Worker V3 publication was interrupted and requires exact reconciliation: {error}"
            ),
            Self::Durable(error) => {
                write!(
                    formatter,
                    "strict Worker V3 HSACO publication failed: {error}"
                )
            }
            Self::DurableAndAttemptState {
                publication,
                attempt_state,
            } => write!(
                formatter,
                "strict Worker V3 publication failed ({publication}); terminal attempt update also failed ({attempt_state})"
            ),
            Self::UnexpectedPreexistingPublication { .. } => formatter.write_str(
                "fresh strict Worker V3 attempt found a preexisting durable publication",
            ),
            Self::PublicationCommittedWithoutReceipt { attempt_state, .. } => write!(
                formatter,
                "strict Worker V3 publication committed, but its receipt did not: {attempt_state}"
            ),
            Self::ReceiptPublicationMismatch => formatter
                .write_str("strict Worker V3 receipt has no matching complete durable publication"),
            Self::ReceiptAlreadyPersisted { .. } => formatter.write_str(
                "the exact strict Worker V3 receipt and publication are already persisted",
            ),
            Self::PublicationCommittedWithoutClaim { error, .. } => write!(
                formatter,
                "strict Worker V3 publication committed, but its inert claim was rejected: {error}"
            ),
            Self::RecoveredClaimRejected { error } => write!(
                formatter,
                "strict Worker V3 durable publication could not reconstruct its inert claim: {error}"
            ),
        }
    }
}

impl std::error::Error for AttemptScopedHsacoPublicationErrorV3 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PublicationIntent(error) => Some(error),
            Self::Attempt(error) => Some(error),
            Self::PublicationInterrupted(error) | Self::Durable(error) => Some(error),
            Self::DurableAndAttemptState { publication, .. } => Some(publication.as_ref()),
            Self::PublicationCommittedWithoutReceipt { attempt_state, .. } => {
                Some(attempt_state.as_ref())
            }
            Self::PublicationCommittedWithoutClaim { error, .. } => Some(error),
            Self::RecoveredClaimRejected { error } => Some(error),
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
pub fn publish_exact_hsaco_evidence_for_attempt_v3(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    authority: VerifiedWorkerV3PublicationAuthorityV1,
    exact_hsaco: &[u8],
) -> Result<AttemptScopedHsacoPublicationResultV3, AttemptScopedHsacoPublicationErrorV3> {
    publish_exact_hsaco_evidence_for_attempt_v3_with_options(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        authority,
        exact_hsaco,
        DurableLinkPublicationOptionsV1::default(),
    )
}

/// Fault-injectable form of [`publish_exact_hsaco_evidence_for_attempt_v3`].
#[allow(clippy::too_many_arguments)]
pub fn publish_exact_hsaco_evidence_for_attempt_v3_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    authority: VerifiedWorkerV3PublicationAuthorityV1,
    exact_hsaco: &[u8],
    options: impl Into<AttemptScopedHsacoPublicationOptionsV2>,
) -> Result<AttemptScopedHsacoPublicationResultV3, AttemptScopedHsacoPublicationErrorV3> {
    let publication_binding = authority.publication_binding();
    validate_worker_v3_publication_inputs(plan, publication_binding, exact_hsaco)?;
    let options = options.into();
    let result = publish_exact_hsaco_evidence_for_attempt::<PublicationSchemaV3>(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        publication_binding,
        exact_hsaco,
        PublicationOptions {
            durable: options.durable,
            receipt_crash: options.receipt_crash,
        },
    )
    .map_err(publication_error_v3)?;
    Ok(AttemptScopedHsacoPublicationResultV3 {
        outcome: outcome_v3(result.outcome),
        publication: result.publication,
        receipt: result.receipt,
        claim: result.claim,
    })
}

fn validate_worker_v3_publication_inputs(
    plan: DurableLinkPublicationPlanV1,
    publication_binding: WorkerV3PublicationBindingV1,
    exact_hsaco: &[u8],
) -> Result<(), AttemptScopedHsacoPublicationErrorV3> {
    let exact_length = u64::try_from(exact_hsaco.len())
        .map_err(|_| AttemptScopedHsacoPublicationErrorV3::PublicationBindingMismatch)?;
    let exact_sha256: [u8; 32] = Sha256::digest(exact_hsaco).into();
    if publication_binding.finalized_output_sha256() != *plan.finalized_output().as_bytes()
        || publication_binding.finalized_output_sha256() != exact_sha256
        || publication_binding.finalized_output_length() != exact_length
        || publication_binding.raw_output_sha256() != *plan.linked_output().as_bytes()
    {
        return Err(AttemptScopedHsacoPublicationErrorV3::PublicationBindingMismatch);
    }
    Ok(())
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
    ReconstructingCompleted,
}

enum SchemaReceiptState<R> {
    None,
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
    PublicationBindingMismatch,
    WorkerV3PublicationIntent(WorkerV3PublicationIntentErrorV1),
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
    ClaimConstructionFailed {
        publication: DurableLinkPublicationResultV1,
        error: ClaimConstructionError,
    },
    ClaimRecoveryFailed(ClaimConstructionError),
}

enum ClaimConstructionError {
    WorkerV3(DurablePublishedClaimCodecErrorV3),
}

trait PublicationSchema {
    type Receipt: ReceiptEvidence + Eq;
    type Binding: Copy;
    type Claim;

    const RECONSTRUCT_COMPLETED_PUBLICATION: bool;

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
    ) -> Result<Self::Claim, ClaimConstructionError>;
    fn validate_storage(
        output: &PinnedOutput,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        binding: Self::Binding,
        exact_hsaco: &[u8],
    ) -> Result<(), PublicationError<Self::Receipt>>;
}

struct PublicationSchemaV3;

impl PublicationSchema for PublicationSchemaV3 {
    type Receipt = BackendPublicationReceiptV3;
    type Binding = WorkerV3PublicationBindingV1;
    type Claim = DurablePublishedHsacoClaimV3;

    const RECONSTRUCT_COMPLETED_PUBLICATION: bool = true;

    fn receipt(
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        publication_binding: Self::Binding,
    ) -> Self::Receipt {
        publication_receipt_v3(
            producer,
            attempt,
            plan,
            upstream_evidence,
            publication_binding,
        )
    }

    fn receipt_state(receipt: Option<BackendReceiptV1>) -> SchemaReceiptState<Self::Receipt> {
        match receipt {
            None => SchemaReceiptState::None,
            Some(BackendReceiptV1::PendingProvenanceV3(receipt)) => {
                SchemaReceiptState::Pending(receipt)
            }
            Some(BackendReceiptV1::ProvenanceV3(receipt)) => {
                SchemaReceiptState::Provenance(receipt)
            }
            Some(BackendReceiptV1::EnvelopeCustodyV3(receipt, _)) => {
                SchemaReceiptState::Provenance(receipt)
            }
            Some(
                BackendReceiptV1::LegacyCoordination
                | BackendReceiptV1::PendingProvenance(_)
                | BackendReceiptV1::Provenance(_)
                | BackendReceiptV1::PendingProvenanceV2(_)
                | BackendReceiptV1::ProvenanceV2(_)
                | BackendReceiptV1::SimulationObservation(_),
            ) => SchemaReceiptState::Foreign,
        }
    }

    fn persist_pending(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError> {
        attempts.claim_backend_with_pending_receipt_v3(stable_source, attempt, receipt)
    }

    fn persist_completed(
        attempts: &mut AttemptRegistry,
        stable_source: &str,
        attempt: BuildAttempt,
        receipt: Self::Receipt,
    ) -> Result<(), crate::AttemptCodecError> {
        attempts.record_backend_publication_receipt_v3(stable_source, attempt, receipt)
    }

    fn claim(
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        receipt: Self::Receipt,
        files: crate::durable_link_publication::DurablePublishedFileBindingV1,
    ) -> Result<Self::Claim, ClaimConstructionError> {
        DurablePublishedHsacoClaimV3::new(plan, upstream_evidence, receipt, files)
            .map_err(ClaimConstructionError::WorkerV3)
    }

    fn validate_storage(
        output: &PinnedOutput,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        binding: Self::Binding,
        exact_hsaco: &[u8],
    ) -> Result<(), PublicationError<Self::Receipt>> {
        let recovered = recover_worker_v3_publication_intent_locked_v1(output, producer, attempt)
            .map_err(PublicationError::WorkerV3PublicationIntent)?;
        if recovered.record().plan() != plan
            || recovered.record().identity().as_bytes()
                != binding.publication_intent_record_identity()
            || recovered.exact_output() != exact_hsaco
        {
            return Err(PublicationError::PublicationBindingMismatch);
        }
        Ok(())
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
            S::validate_storage(&output, producer, attempt, plan, binding, exact_hsaco)?;
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
            S::validate_storage(&output, producer, attempt, plan, binding, exact_hsaco)?;
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
        (
            AttemptPhase::BackendClaimed | AttemptPhase::Completed,
            SchemaReceiptState::Provenance(receipt),
        ) if receipt == expected_receipt => {
            S::validate_storage(&output, producer, attempt, plan, binding, exact_hsaco)?;
            match recover_durable_link_plan_locked(&output, plan) {
                Ok(Some(DurablePlanRecoveryStateV1::Published))
                    if S::RECONSTRUCT_COMPLETED_PUBLICATION =>
                {
                    Authorization::ReconstructingCompleted
                }
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

    if !matches!(authorization, Authorization::ReconstructingCompleted) {
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
    }

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
        (
            Authorization::ReconstructingCompleted,
            DurableLinkPublicationOutcomeV1::AlreadyPublished,
        ) => PublicationOutcome::RecoveredCommittedPublication,
        _ => PublicationOutcome::RecoveredCommittedPublication,
    };
    let claim = match S::claim(
        plan,
        upstream_evidence,
        expected_receipt,
        publication.published_file_binding(),
    ) {
        Ok(claim) => claim,
        Err(error) => {
            return Err(PublicationError::ClaimConstructionFailed { publication, error });
        }
    };
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

fn outcome_v3(outcome: PublicationOutcome) -> AttemptScopedHsacoPublicationOutcomeV3 {
    match outcome {
        PublicationOutcome::Published => AttemptScopedHsacoPublicationOutcomeV3::Published,
        PublicationOutcome::RecoveredAndPublished => {
            AttemptScopedHsacoPublicationOutcomeV3::RecoveredAndPublished
        }
        PublicationOutcome::RecoveredCommittedPublication => {
            AttemptScopedHsacoPublicationOutcomeV3::RecoveredCommittedPublication
        }
    }
}

fn publication_error_v3(
    error: PublicationError<BackendPublicationReceiptV3>,
) -> AttemptScopedHsacoPublicationErrorV3 {
    match error {
        PublicationError::PlanAttemptMismatch => {
            AttemptScopedHsacoPublicationErrorV3::PlanAttemptMismatch
        }
        PublicationError::PublicationBindingMismatch => {
            AttemptScopedHsacoPublicationErrorV3::PublicationBindingMismatch
        }
        PublicationError::WorkerV3PublicationIntent(error) => {
            AttemptScopedHsacoPublicationErrorV3::PublicationIntent(error)
        }
        PublicationError::Attempt(error) => AttemptScopedHsacoPublicationErrorV3::Attempt(error),
        PublicationError::ForeignReceipt => {
            AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion
        }
        PublicationError::UnrecoverableClaimedAttempt => {
            AttemptScopedHsacoPublicationErrorV3::UnrecoverableClaimedAttempt
        }
        PublicationError::PendingReceiptMismatch => {
            AttemptScopedHsacoPublicationErrorV3::PendingReceiptMismatch
        }
        PublicationError::ReceiptCommitInterrupted { point } => {
            AttemptScopedHsacoPublicationErrorV3::ReceiptCommitInterrupted { point }
        }
        PublicationError::PublicationInterrupted(error) => {
            AttemptScopedHsacoPublicationErrorV3::PublicationInterrupted(error)
        }
        PublicationError::Durable(error) => AttemptScopedHsacoPublicationErrorV3::Durable(error),
        PublicationError::DurableAndAttemptState {
            publication,
            attempt_state,
        } => AttemptScopedHsacoPublicationErrorV3::DurableAndAttemptState {
            publication,
            attempt_state,
        },
        PublicationError::UnexpectedPreexistingPublication { publication } => {
            AttemptScopedHsacoPublicationErrorV3::UnexpectedPreexistingPublication { publication }
        }
        PublicationError::PublicationCommittedWithoutReceipt {
            publication,
            expected_receipt,
            attempt_state,
        } => AttemptScopedHsacoPublicationErrorV3::PublicationCommittedWithoutReceipt {
            publication,
            expected_receipt,
            attempt_state,
        },
        PublicationError::ReceiptPublicationMismatch => {
            AttemptScopedHsacoPublicationErrorV3::ReceiptPublicationMismatch
        }
        PublicationError::ReceiptAlreadyPersisted { receipt } => {
            AttemptScopedHsacoPublicationErrorV3::ReceiptAlreadyPersisted { receipt }
        }
        PublicationError::ClaimConstructionFailed {
            publication,
            error: ClaimConstructionError::WorkerV3(error),
        } => AttemptScopedHsacoPublicationErrorV3::PublicationCommittedWithoutClaim {
            publication,
            error,
        },
        PublicationError::ClaimRecoveryFailed(ClaimConstructionError::WorkerV3(error)) => {
            AttemptScopedHsacoPublicationErrorV3::RecoveredClaimRejected { error }
        }
    }
}

/// Reads only a strict Worker V3 receipt and rejects all older receipt schemas.
pub fn read_backend_publication_receipt_v3(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<PersistedBackendReceiptV3, AttemptScopedHsacoPublicationErrorV3> {
    let output = PinnedOutput::open_existing(output_dir)
        .map_err(AttemptScopedHsacoPublicationErrorV3::Attempt)?;
    let _lock = output
        .lock()
        .map_err(AttemptScopedHsacoPublicationErrorV3::Attempt)?;
    output
        .verify_path_identity()
        .map_err(AttemptScopedHsacoPublicationErrorV3::Attempt)?;
    let attempts =
        read_attempt_registry(&output).map_err(AttemptScopedHsacoPublicationErrorV3::Attempt)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(build_attempt_error)
        .map_err(AttemptScopedHsacoPublicationErrorV3::Attempt)?;
    if record.crate_name != producer.crate_name {
        return Err(AttemptScopedHsacoPublicationErrorV3::Attempt(
            build_attempt_error("build attempt crate name does not match the producer"),
        ));
    }
    match record.backend_receipt {
        None => Ok(PersistedBackendReceiptV3::None),
        Some(BackendReceiptV1::PendingProvenanceV3(receipt)) => {
            Ok(PersistedBackendReceiptV3::PendingProvenance(receipt))
        }
        Some(BackendReceiptV1::ProvenanceV3(receipt)) => {
            Ok(PersistedBackendReceiptV3::Provenance(receipt))
        }
        Some(BackendReceiptV1::EnvelopeCustodyV3(receipt, _)) => {
            Ok(PersistedBackendReceiptV3::Provenance(receipt))
        }
        Some(
            BackendReceiptV1::LegacyCoordination
            | BackendReceiptV1::PendingProvenance(_)
            | BackendReceiptV1::Provenance(_)
            | BackendReceiptV1::PendingProvenanceV2(_)
            | BackendReceiptV1::ProvenanceV2(_)
            | BackendReceiptV1::SimulationObservation(_),
        ) => Err(AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion),
    }
}

/// Reconstructs a strict claim only from an exact completed V3 receipt and binding.
#[allow(clippy::too_many_arguments)]
pub fn recover_published_hsaco_claim_for_attempt_v3(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    publication_binding: WorkerV3PublicationBindingV1,
    receipt: BackendPublicationReceiptV3,
) -> Result<DurablePublishedHsacoClaimV3, AttemptScopedHsacoPublicationErrorV3> {
    validate_backend_publication_receipt_v3(
        producer,
        attempt,
        plan,
        upstream_evidence,
        publication_binding,
        receipt,
    )
    .map_err(|error| match error {
        BackendPublicationReceiptValidationErrorV3::PlanAttemptMismatch => {
            AttemptScopedHsacoPublicationErrorV3::PlanAttemptMismatch
        }
        BackendPublicationReceiptValidationErrorV3::PublicationBindingMismatch => {
            AttemptScopedHsacoPublicationErrorV3::PublicationBindingMismatch
        }
        _ => AttemptScopedHsacoPublicationErrorV3::ReceiptPublicationMismatch,
    })?;
    recover_published_hsaco_claim_for_attempt::<PublicationSchemaV3>(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        receipt,
    )
    .map_err(publication_error_v3)
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
    S::claim(plan, upstream_evidence, receipt, files).map_err(PublicationError::ClaimRecoveryFailed)
}

pub(crate) fn publication_receipt_v3(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    publication_binding: WorkerV3PublicationBindingV1,
) -> BackendPublicationReceiptV3 {
    publication_receipt_for_producer_identity_v3(
        attempt,
        plan,
        upstream_evidence,
        publication_binding,
        producer_receipt_identity_v3(&producer.stable_source, &producer.crate_name),
    )
}

pub(crate) fn publication_receipt_for_producer_identity_v3(
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    publication_binding: WorkerV3PublicationBindingV1,
    producer_identity: [u8; 32],
) -> BackendPublicationReceiptV3 {
    let (attempt_identity, scope_identity) = receipt_context_identities(
        attempt,
        plan,
        ATTEMPT_IDENTITY_DOMAIN_V3,
        SCOPE_IDENTITY_DOMAIN_V3,
    );
    BackendPublicationReceiptV3::new(
        attempt_identity,
        producer_identity,
        scope_identity,
        plan.identity(),
        upstream_evidence.as_bytes(),
        *plan.finalized_output().as_bytes(),
        *plan.publication().as_bytes(),
        publication_binding,
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

pub(crate) fn producer_receipt_identity_v3(stable_source: &str, crate_name: &str) -> [u8; 32] {
    producer_receipt_identity(stable_source, crate_name, PRODUCER_IDENTITY_DOMAIN_V3)
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
