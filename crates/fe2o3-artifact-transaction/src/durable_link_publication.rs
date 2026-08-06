//! Durable, inert publication of one finalized direct-link artifact.
//!
//! V1 requires the canonical output directory and all of its parents to exist before publication.
//! The caller is responsible for durably creating that topology, including syncing each newly
//! created directory into its parent. The adapter pins the existing output directory, performs
//! descriptor-relative operations beneath it, and syncs it after durable name changes.
//!
//! This adapter assumes Linux `renameat2(RENAME_NOREPLACE)` and a local filesystem that implements
//! the usual `fsync` and atomic-rename contract. It cannot make guarantees for storage that lies
//! about cache flushes, network filesystems with weaker semantics, failing hardware, or a process
//! with permission to mutate the managed directory outside this protocol. V1 does not scan or
//! delete abandoned staging, artifact, redo, or quarantine entries. Suspect entries remain inert
//! and may block the affected scope until an operator repairs them through a separately trusted
//! maintenance path.
//!
//! Each active attempt durably commits a domain-separated digest of its complete publication plan
//! before callback work starts. Redo bytes are written and synced under a unique ignored temp name
//! and become replayable only after an atomic rename. The first generation observed for a scope
//! may be any nonzero external build generation; every later scope generation must be contiguous.

use super::{
    AtomicPublicationIdentityV1, CanonicalLinkRequestIdentityV1, EmitError, FinalizationIdentityV1,
    FinalizedArtifactSnapshot, FinalizedOutputIdentityV1, InvalidationReasonV1,
    LinkPublicationCatalogV1, LinkPublicationCodecError, LinkPublicationPhaseV1,
    LinkPublicationRecordV1, LinkPublicationScopeV1, LinkPublicationStateV1,
    LinkedOutputIdentityV1, NoFaults, PinnedOutput, PinnedWorkerIdentityV1, PublicationOutcomeV1,
    StagingDirectory, ValidatedResponseIdentityV1, read_control_file,
};
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fsync, openat, renameat, renameat_with,
    statat,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const ENVELOPE_MAGIC: &[u8] = b"FE2O3-DURABLE-LINK-V1\0";
const ENVELOPE_VERSION: u16 = 1;
const SCOPE_TAG_PACKAGE: u8 = 0x10;
const SCOPE_TAG_KERNEL_SET: u8 = 0x11;
const SCOPE_TAG_TARGET: u8 = 0x12;
const PLAN_IDENTITY_TAG: u8 = 0x20;
const RECORD_PREFIX: &str = ".fe2o3-link-publication-v1-";
const RECORD_SUFFIX: &str = ".record";
const REDO_SUFFIX: &str = ".redo";
const ARTIFACT_PREFIX: &str = ".fe2o3-link-artifact-v1-";
const ARTIFACT_SUFFIX: &str = ".bin";
const STAGED_ARTIFACT: &str = "finalized-link-artifact";
const SCOPE_IDENTITY_DOMAIN: &[u8] = b"fe2o3.durable-link.scope.v1\0";
const PLAN_IDENTITY_DOMAIN: &[u8] = b"fe2o3.durable-link.complete-plan.v1\0";
const ENVELOPE_CHECKSUM_DOMAIN: &[u8] = b"fe2o3.durable-link.envelope-checksum.v1\0";
const MAX_REDO_TEMP_ATTEMPTS: u64 = 128;

static NEXT_REDO_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// Maximum canonical size of one durable scope envelope.
pub const MAX_DURABLE_LINK_PUBLICATION_RECORD_BYTES: usize = 1_280;

/// Maximum finalized payload accepted by this production adapter.
pub const MAX_DURABLE_FINALIZED_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;

/// Complete expected chain obtained from a validated G5/G6 publication bridge.
///
/// The constructor does not authenticate caller-supplied identities. The normative caller is
/// `fe2o3_artifacts::DirectLinkPublicationBridgeV1`, prepared from a
/// `ValidatedDirectLinkBundleEvidenceV1`. The durable adapter additionally measures finalized
/// bytes and requires their SHA-256 digest to equal `finalized_output`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableLinkPublicationPlanV1 {
    attempt: super::BuildAttempt,
    scope: LinkPublicationScopeV1,
    request: CanonicalLinkRequestIdentityV1,
    worker: PinnedWorkerIdentityV1,
    response: ValidatedResponseIdentityV1,
    linked_output: LinkedOutputIdentityV1,
    finalization: FinalizationIdentityV1,
    finalized_output: FinalizedOutputIdentityV1,
    publication: AtomicPublicationIdentityV1,
}

impl DurableLinkPublicationPlanV1 {
    /// Constructs a plan from one complete validated bridge identity chain.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        attempt: super::BuildAttempt,
        scope: LinkPublicationScopeV1,
        request: CanonicalLinkRequestIdentityV1,
        worker: PinnedWorkerIdentityV1,
        response: ValidatedResponseIdentityV1,
        linked_output: LinkedOutputIdentityV1,
        finalization: FinalizationIdentityV1,
        finalized_output: FinalizedOutputIdentityV1,
        publication: AtomicPublicationIdentityV1,
    ) -> Self {
        Self {
            attempt,
            scope,
            request,
            worker,
            response,
            linked_output,
            finalization,
            finalized_output,
            publication,
        }
    }

    /// Returns the exact build attempt.
    pub const fn attempt(self) -> super::BuildAttempt {
        self.attempt
    }

    /// Returns the trusted publication scope committed by the bridge.
    pub const fn scope(self) -> LinkPublicationScopeV1 {
        self.scope
    }

    /// Returns the canonical request identity.
    pub const fn request(self) -> CanonicalLinkRequestIdentityV1 {
        self.request
    }

    /// Returns the pinned worker and toolchain closure identity.
    pub const fn worker(self) -> PinnedWorkerIdentityV1 {
        self.worker
    }

    /// Returns the validated worker response identity.
    pub const fn response(self) -> ValidatedResponseIdentityV1 {
        self.response
    }

    /// Returns the linked output identity.
    pub const fn linked_output(self) -> LinkedOutputIdentityV1 {
        self.linked_output
    }

    /// Returns the finalization evidence identity.
    pub const fn finalization(self) -> FinalizationIdentityV1 {
        self.finalization
    }

    /// Returns the expected SHA-256 finalized payload identity.
    pub const fn finalized_output(self) -> FinalizedOutputIdentityV1 {
        self.finalized_output
    }

    /// Returns the validated bridge publication identity.
    pub const fn publication(self) -> AtomicPublicationIdentityV1 {
        self.publication
    }

    fn identity(self) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(PLAN_IDENTITY_DOMAIN);
        digest.update(self.attempt.generation().to_le_bytes());
        digest.update(self.attempt.session().as_bytes());
        digest.update(self.attempt.invocation().as_bytes());
        digest.update(self.scope.package().as_bytes());
        digest.update(self.scope.kernel_set().as_bytes());
        digest.update(self.scope.target().as_bytes());
        digest.update(self.request.as_bytes());
        digest.update(self.worker.as_bytes());
        digest.update(self.response.as_bytes());
        digest.update(self.linked_output.as_bytes());
        digest.update(self.finalization.as_bytes());
        digest.update(self.finalized_output.as_bytes());
        digest.update(self.publication.as_bytes());
        digest.finalize().into()
    }
}

/// Durable journal milestone associated with an injected boundary failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableJournalStageV1 {
    /// Initial request-bound plan, committed before callback work begins.
    Planned,
    /// Worker and toolchain closure pinned.
    WorkerPinned,
    /// Worker response and linked bytes validated.
    ResponseValidated,
    /// Finalized bytes inspected and measured.
    Finalized,
    /// Canonical publication pointer commit.
    Published,
    /// Terminal invalidation after a normal callback failure.
    Invalidated,
    /// Restart repair of a valid but incomplete journal.
    Recovered,
}

/// Atomic control-file operation at which a deterministic test may interrupt a commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableJournalBoundaryV1 {
    CreateRedoTemp,
    WriteRedoTemp,
    SyncRedoTemp,
    RenameTempToRedo,
    SyncRedoName,
    RenameRedoToCanonical,
    SyncCanonicalName,
}

/// Whether a simulated interruption occurs immediately before or after one journal operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableFaultTimingV1 {
    Before,
    After,
}

/// Finalized-artifact operation at which a deterministic test may interrupt publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableArtifactBoundaryV1 {
    CreateTemp,
    WriteTemp,
    SyncTemp,
    RenameToContentAddress,
    SyncDirectory,
}

/// Deterministic crash boundary exposed for integration testing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableLinkPublicationFaultPointV1 {
    Journal {
        stage: DurableJournalStageV1,
        boundary: DurableJournalBoundaryV1,
        timing: DurableFaultTimingV1,
    },
    Artifact {
        boundary: DurableArtifactBoundaryV1,
        timing: DurableFaultTimingV1,
    },
    /// Transient failure while opening a committed artifact snapshot.
    SnapshotRead,
}

/// Options for deterministic durability testing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DurableLinkPublicationOptionsV1 {
    fault: Option<DurableLinkPublicationFaultPointV1>,
}

impl DurableLinkPublicationOptionsV1 {
    /// Injects one crash-like interruption and suppresses normal failure invalidation.
    pub const fn inject_crash(point: DurableLinkPublicationFaultPointV1) -> Self {
        Self { fault: Some(point) }
    }

    /// Injects one deterministic filesystem fault for integration testing.
    pub const fn inject_fault(point: DurableLinkPublicationFaultPointV1) -> Self {
        Self { fault: Some(point) }
    }
}

/// Result classification for one durable publication request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableLinkPublicationOutcomeV1 {
    Published,
    AlreadyPublished,
}

/// Immutable descriptor-derived snapshot of a durable publication.
#[derive(Clone, Debug)]
pub struct DurableLinkPublicationSnapshotV1 {
    record: LinkPublicationRecordV1,
    artifact: FinalizedArtifactSnapshot,
}

impl DurableLinkPublicationSnapshotV1 {
    /// Returns the complete durable identity chain.
    pub const fn record(&self) -> &LinkPublicationRecordV1 {
        &self.record
    }

    /// Returns immutable bytes captured from the exact published descriptor.
    pub const fn artifact(&self) -> &FinalizedArtifactSnapshot {
        &self.artifact
    }

    /// Durable evidence never grants module-loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Durable evidence never grants kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Snapshot and idempotency classification returned after publication.
#[derive(Clone, Debug)]
pub struct DurableLinkPublicationResultV1 {
    outcome: DurableLinkPublicationOutcomeV1,
    snapshot: DurableLinkPublicationSnapshotV1,
}

impl DurableLinkPublicationResultV1 {
    pub const fn outcome(&self) -> DurableLinkPublicationOutcomeV1 {
        self.outcome
    }

    pub const fn snapshot(&self) -> &DurableLinkPublicationSnapshotV1 {
        &self.snapshot
    }
}

/// Durable publication or recovery failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum DurableLinkPublicationError {
    Filesystem(EmitError),
    Protocol(LinkPublicationCodecError),
    InvalidDurableRecord {
        reason: String,
    },
    ConflictingRedo {
        reason: String,
    },
    UnsafeManagedEntry {
        entry: String,
        reason: String,
    },
    FinalizedArtifactSize {
        actual: usize,
        maximum: usize,
    },
    FinalizedArtifactDigestMismatch,
    IncompleteCallback {
        phase: LinkPublicationPhaseV1,
    },
    Work {
        reason: String,
    },
    InjectedCrash {
        point: DurableLinkPublicationFaultPointV1,
    },
    Cleanup {
        reason: String,
    },
}

impl DurableLinkPublicationError {
    /// Creates a caller-work failure that will durably invalidate the active attempt.
    pub fn work(reason: impl Into<String>) -> Self {
        Self::Work {
            reason: reason.into(),
        }
    }

    fn is_injected_crash_from(&self, faults: &FaultInjector) -> bool {
        matches!(self, Self::InjectedCrash { point } if faults.fired == Some(*point))
    }
}

impl fmt::Display for DurableLinkPublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filesystem(error) => {
                write!(formatter, "durable publication filesystem failure: {error}")
            }
            Self::Protocol(error) => {
                write!(formatter, "durable publication protocol failure: {error}")
            }
            Self::InvalidDurableRecord { reason } => {
                write!(formatter, "invalid durable publication record: {reason}")
            }
            Self::ConflictingRedo { reason } => {
                write!(formatter, "conflicting durable publication redo: {reason}")
            }
            Self::UnsafeManagedEntry { entry, reason } => {
                write!(formatter, "unsafe managed entry {entry}: {reason}")
            }
            Self::FinalizedArtifactSize { actual, maximum } => write!(
                formatter,
                "finalized artifact size {actual} is outside 1..={maximum}"
            ),
            Self::FinalizedArtifactDigestMismatch => formatter.write_str(
                "finalized artifact SHA-256 does not match validated publication evidence",
            ),
            Self::IncompleteCallback { phase } => {
                write!(formatter, "publication callback stopped at {phase:?}")
            }
            Self::Work { reason } => write!(formatter, "direct-link work failed: {reason}"),
            Self::InjectedCrash { point } => {
                write!(formatter, "injected durable publication crash at {point:?}")
            }
            Self::Cleanup { reason } => {
                write!(formatter, "durable publication cleanup failed: {reason}")
            }
        }
    }
}

impl std::error::Error for DurableLinkPublicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem(error) => Some(error),
            Self::Protocol(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EmitError> for DurableLinkPublicationError {
    fn from(error: EmitError) -> Self {
        Self::Filesystem(error)
    }
}

impl From<LinkPublicationCodecError> for DurableLinkPublicationError {
    fn from(error: LinkPublicationCodecError) -> Self {
        Self::Protocol(error)
    }
}

impl From<std::io::Error> for DurableLinkPublicationError {
    fn from(error: std::io::Error) -> Self {
        Self::Filesystem(EmitError::Io(error))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DurableEnvelopeV1 {
    scope: LinkPublicationScopeV1,
    generation_floor: u64,
    poisoned: bool,
    published: Option<LinkPublicationRecordV1>,
    active_plan: Option<[u8; 32]>,
    active: Option<LinkPublicationRecordV1>,
}

impl DurableEnvelopeV1 {
    fn empty(scope: LinkPublicationScopeV1) -> Self {
        Self {
            scope,
            generation_floor: 0,
            poisoned: false,
            published: None,
            active_plan: None,
            active: None,
        }
    }

    fn encode(&self) -> Result<Vec<u8>, DurableLinkPublicationError> {
        self.validate()?;
        let published = self
            .published
            .as_ref()
            .map(LinkPublicationRecordV1::encode_canonical)
            .transpose()?;
        let active = self
            .active
            .as_ref()
            .map(LinkPublicationRecordV1::encode_canonical)
            .transpose()?;
        let mut bytes = Vec::with_capacity(MAX_DURABLE_LINK_PUBLICATION_RECORD_BYTES);
        bytes.extend_from_slice(ENVELOPE_MAGIC);
        bytes.extend_from_slice(&ENVELOPE_VERSION.to_le_bytes());
        bytes.push(u8::from(self.poisoned));
        bytes.extend_from_slice(&self.generation_floor.to_le_bytes());
        push_scope(&mut bytes, self.scope);
        push_record(&mut bytes, published.as_deref())?;
        push_optional_identity(&mut bytes, self.active_plan);
        push_record(&mut bytes, active.as_deref())?;
        let checksum = domain_sha256(ENVELOPE_CHECKSUM_DOMAIN, &bytes);
        bytes.extend_from_slice(&checksum);
        if bytes.len() > MAX_DURABLE_LINK_PUBLICATION_RECORD_BYTES {
            return Err(invalid_record("encoded envelope exceeds its byte bound"));
        }
        Ok(bytes)
    }

    fn decode(
        bytes: &[u8],
        expected_scope: LinkPublicationScopeV1,
    ) -> Result<Self, DurableLinkPublicationError> {
        let envelope = Self::decode_any(bytes)?;
        if envelope.scope != expected_scope {
            return Err(invalid_record(
                "scope does not match its canonical record name",
            ));
        }
        Ok(envelope)
    }

    fn decode_any(bytes: &[u8]) -> Result<Self, DurableLinkPublicationError> {
        if bytes.len() > MAX_DURABLE_LINK_PUBLICATION_RECORD_BYTES {
            return Err(invalid_record("envelope exceeds its byte bound"));
        }
        let body_length = bytes
            .len()
            .checked_sub(32)
            .ok_or_else(|| invalid_record("truncated durable envelope checksum"))?;
        let (body, checksum) = bytes.split_at(body_length);
        if domain_sha256(ENVELOPE_CHECKSUM_DOMAIN, body) != checksum {
            return Err(invalid_record("durable envelope checksum mismatch"));
        }
        let mut decoder = EnvelopeDecoder::new(body);
        if decoder.take(ENVELOPE_MAGIC.len())? != ENVELOPE_MAGIC {
            return Err(invalid_record("bad envelope magic"));
        }
        let version = decoder.u16()?;
        if version != ENVELOPE_VERSION {
            return Err(invalid_record(format!(
                "unsupported envelope version {version}"
            )));
        }
        let poisoned = match decoder.byte()? {
            0 => false,
            1 => true,
            _ => return Err(invalid_record("noncanonical poison flag")),
        };
        let generation_floor = decoder.u64()?;
        let scope = decode_scope(&mut decoder)?;
        let published = decoder.record()?;
        let active_plan = decoder.optional_identity(PLAN_IDENTITY_TAG)?;
        let active = decoder.record()?;
        if !decoder.finished() {
            return Err(invalid_record("trailing durable envelope bytes"));
        }
        let envelope = Self {
            scope,
            generation_floor,
            poisoned,
            published,
            active_plan,
            active,
        };
        envelope.validate()?;
        if envelope.encode()? != bytes {
            return Err(invalid_record("noncanonical durable envelope"));
        }
        Ok(envelope)
    }

    fn validate(&self) -> Result<(), DurableLinkPublicationError> {
        if self.active.is_some() != self.active_plan.is_some() {
            return Err(invalid_record(
                "active record and complete-plan commitment must appear together",
            ));
        }
        if let Some(record) = &self.published {
            if record.scope() != self.scope
                || record.state()
                    != LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published)
            {
                return Err(invalid_record(
                    "published slot does not contain a complete record",
                ));
            }
            if record.attempt().generation() > self.generation_floor {
                return Err(invalid_record("published generation exceeds durable floor"));
            }
        }
        if let Some(record) = &self.active {
            if record.scope() != self.scope
                || record.state()
                    == LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published)
            {
                return Err(invalid_record("active slot contains an invalid record"));
            }
            if record.attempt().generation() > self.generation_floor {
                return Err(invalid_record("active generation exceeds durable floor"));
            }
        }
        Ok(())
    }
}

/// Ordered callback handle. Every successful method durably commits its stage before returning.
pub struct DurableLinkPublicationTransactionV1<'a> {
    output: &'a PinnedOutput,
    names: &'a DurableNames,
    envelope: &'a mut DurableEnvelopeV1,
    catalog: LinkPublicationCatalogV1,
    record: LinkPublicationRecordV1,
    plan: DurableLinkPublicationPlanV1,
    finalized_bytes: Option<Arc<[u8]>>,
    faults: &'a mut FaultInjector,
}

impl DurableLinkPublicationTransactionV1<'_> {
    /// Durably records that the expected worker and toolchain closure was pinned.
    pub fn record_worker_pinned(&mut self) -> Result<(), DurableLinkPublicationError> {
        if phase_at_least(self.phase(), LinkPublicationPhaseV1::WorkerPinned) {
            if self.record.worker() != Some(self.plan.worker) {
                return Err(invalid_record(
                    "recovered worker does not match the complete publication plan",
                ));
            }
            return Ok(());
        }
        self.record.record_pinned_worker(
            &self.catalog,
            self.plan.attempt,
            self.plan.request,
            self.plan.worker,
        )?;
        self.persist(DurableJournalStageV1::WorkerPinned)
    }

    /// Durably records validation of the expected response and linked output.
    pub fn record_response_validated(&mut self) -> Result<(), DurableLinkPublicationError> {
        if phase_at_least(self.phase(), LinkPublicationPhaseV1::ResponseValidated) {
            if self.record.response() != Some(self.plan.response)
                || self.record.linked_output() != Some(self.plan.linked_output)
            {
                return Err(invalid_record(
                    "recovered response does not match the complete publication plan",
                ));
            }
            return Ok(());
        }
        self.record.record_validated_response(
            &self.catalog,
            self.plan.attempt,
            self.plan.request,
            self.plan.worker,
            self.plan.response,
            self.plan.linked_output,
        )?;
        self.persist(DurableJournalStageV1::ResponseValidated)
    }

    /// Measures finalized bytes and durably records finalization before publication.
    pub fn record_finalized(&mut self, bytes: &[u8]) -> Result<(), DurableLinkPublicationError> {
        validate_artifact_size(bytes.len())?;
        if sha256(bytes) != *self.plan.finalized_output.as_bytes() {
            return Err(DurableLinkPublicationError::FinalizedArtifactDigestMismatch);
        }
        if phase_at_least(self.phase(), LinkPublicationPhaseV1::Finalized) {
            if self.record.finalization() != Some(self.plan.finalization)
                || self.record.finalized_output() != Some(self.plan.finalized_output)
            {
                return Err(invalid_record(
                    "recovered finalization does not match the complete publication plan",
                ));
            }
            self.finalized_bytes = Some(Arc::from(bytes));
            return Ok(());
        }
        self.record.record_finalization(
            &self.catalog,
            self.plan.attempt,
            self.plan.response,
            self.plan.linked_output,
            self.plan.finalization,
            self.plan.finalized_output,
        )?;
        self.finalized_bytes = Some(Arc::from(bytes));
        self.persist(DurableJournalStageV1::Finalized)
    }

    /// Returns the current journal phase for callback diagnostics.
    pub const fn phase(&self) -> LinkPublicationPhaseV1 {
        match self.record.state() {
            LinkPublicationStateV1::Active(phase)
            | LinkPublicationStateV1::Invalidated {
                prior_phase: phase, ..
            } => phase,
        }
    }

    fn persist(&mut self, stage: DurableJournalStageV1) -> Result<(), DurableLinkPublicationError> {
        self.envelope.generation_floor = self
            .envelope
            .generation_floor
            .max(self.record.attempt().generation());
        self.envelope.active = Some(self.record.clone());
        persist_envelope(self.output, self.names, self.envelope, stage, self.faults)
    }
}

/// Publishes with production defaults.
///
/// The lock is held from recovery through callback completion and canonical record commit. This
/// relies on Linux descriptor-relative operations and a local filesystem honoring `fsync` and
/// atomic rename. It does not cover lying storage caches, weaker network filesystems, hardware
/// loss, or mutation by another process that ignores the lock. The record checksum detects
/// accidental corruption; it is not a keyed authenticator against a same-user attacker.
///
/// `output_dir` must already exist. V1 never creates its directory or parents because publication
/// cannot prove that newly created topology was synced into every parent. The caller must durably
/// provision that topology before calling this function.
///
/// An ordinary callback, validation, or artifact failure is returned only after its terminal
/// `ExplicitFailure` record and canonical directory entry are durable. If that terminal commit
/// cannot be completed, this function returns the journal or injected-crash error instead; restart
/// may then classify the attempt as `CrashRecovery` and permit only an exact complete-plan retry.
pub fn publish_durable_link_v1<F>(
    output_dir: &Path,
    plan: DurableLinkPublicationPlanV1,
    work: F,
) -> Result<DurableLinkPublicationResultV1, DurableLinkPublicationError>
where
    F: FnOnce(
        &mut DurableLinkPublicationTransactionV1<'_>,
    ) -> Result<(), DurableLinkPublicationError>,
{
    publish_durable_link_v1_with_options(
        output_dir,
        plan,
        DurableLinkPublicationOptionsV1::default(),
        work,
    )
}

/// Publishes with deterministic crash injection for integration tests.
pub fn publish_durable_link_v1_with_options<F>(
    output_dir: &Path,
    plan: DurableLinkPublicationPlanV1,
    options: DurableLinkPublicationOptionsV1,
    work: F,
) -> Result<DurableLinkPublicationResultV1, DurableLinkPublicationError>
where
    F: FnOnce(
        &mut DurableLinkPublicationTransactionV1<'_>,
    ) -> Result<(), DurableLinkPublicationError>,
{
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    let names = DurableNames::new(plan.scope);
    let mut faults = FaultInjector::new(options.fault);
    let mut envelope = recover_envelope(&output, &names, plan.scope)?;
    recover_incomplete(&output, &names, &mut envelope)?;
    verify_or_invalidate_published(&output, &names, &mut envelope, &mut faults)?;

    if let Some(record) = envelope.published.as_ref() {
        if record_matches_plan(record, plan) {
            let snapshot = snapshot_for_record(&output, record, &mut faults)?;
            return Ok(DurableLinkPublicationResultV1 {
                outcome: DurableLinkPublicationOutcomeV1::AlreadyPublished,
                snapshot,
            });
        }
        if record.attempt().generation() >= plan.attempt.generation() {
            return Err(DurableLinkPublicationError::Protocol(
                LinkPublicationCodecError::StaleAttempt,
            ));
        }
    }
    let crash_retry = crash_retry_matches_plan(&envelope, plan);
    if envelope.poisoned && plan.attempt.generation() <= envelope.generation_floor && !crash_retry {
        return Err(invalid_record(
            "corruption tombstone requires a newer build generation",
        ));
    }
    if plan.attempt.generation() < envelope.generation_floor
        || (plan.attempt.generation() == envelope.generation_floor && !crash_retry)
    {
        return Err(DurableLinkPublicationError::Protocol(
            LinkPublicationCodecError::StaleAttempt,
        ));
    }
    if !crash_retry
        && envelope.generation_floor != 0
        && envelope
            .generation_floor
            .checked_add(1)
            .is_none_or(|next| plan.attempt.generation() != next)
    {
        return Err(invalid_record(
            "a subsequent scope generation must advance the durable floor by exactly one",
        ));
    }

    let mut catalog = catalog_from_published(envelope.published.as_ref())?;
    let record = if crash_retry {
        let recovered = envelope
            .active
            .as_ref()
            .expect("crash retry requires an active record");
        let mut resumed = catalog.begin(plan.attempt, plan.scope, plan.request)?;
        advance_record(&mut resumed, &catalog, recovered)?;
        resumed
    } else {
        catalog.begin(plan.attempt, plan.scope, plan.request)?
    };
    envelope.generation_floor = envelope.generation_floor.max(plan.attempt.generation());
    envelope.active_plan = Some(plan.identity());
    envelope.active = Some(record.clone());
    persist_envelope(
        &output,
        &names,
        &envelope,
        DurableJournalStageV1::Planned,
        &mut faults,
    )?;

    let mut transaction = DurableLinkPublicationTransactionV1 {
        output: &output,
        names: &names,
        envelope: &mut envelope,
        catalog,
        record,
        plan,
        finalized_bytes: None,
        faults: &mut faults,
    };
    let work_result = work(&mut transaction);
    if let Err(error) = work_result {
        if !error.is_injected_crash_from(transaction.faults) {
            invalidate_transaction(&mut transaction, InvalidationReasonV1::ExplicitFailure)?;
        }
        return Err(error);
    }
    if transaction.phase() != LinkPublicationPhaseV1::Finalized {
        let phase = transaction.phase();
        invalidate_transaction(&mut transaction, InvalidationReasonV1::ExplicitFailure)?;
        return Err(DurableLinkPublicationError::IncompleteCallback { phase });
    }

    let bytes = transaction
        .finalized_bytes
        .clone()
        .ok_or_else(|| invalid_record("finalized phase omitted immutable bytes"))?;
    let artifact = match publish_artifact(
        transaction.output,
        bytes,
        transaction.plan.finalized_output,
        transaction.faults,
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            if !error.is_injected_crash_from(transaction.faults) {
                invalidate_transaction(&mut transaction, InvalidationReasonV1::ExplicitFailure)?;
            }
            return Err(error);
        }
    };

    let publication_outcome = transaction.record.publish(
        &mut transaction.catalog,
        transaction.plan.attempt,
        transaction.plan.finalization,
        transaction.plan.finalized_output,
        transaction.plan.publication,
    )?;
    transaction.envelope.published = Some(transaction.record.clone());
    transaction.envelope.active_plan = None;
    transaction.envelope.active = None;
    transaction.envelope.poisoned = false;
    persist_envelope(
        transaction.output,
        transaction.names,
        transaction.envelope,
        DurableJournalStageV1::Published,
        transaction.faults,
    )?;
    Ok(DurableLinkPublicationResultV1 {
        outcome: match publication_outcome {
            PublicationOutcomeV1::Published => DurableLinkPublicationOutcomeV1::Published,
            PublicationOutcomeV1::AlreadyPublished => {
                DurableLinkPublicationOutcomeV1::AlreadyPublished
            }
        },
        snapshot: DurableLinkPublicationSnapshotV1 {
            record: transaction.record.clone(),
            artifact,
        },
    })
}

/// Recovers one scope and returns its last complete immutable publication, if any.
///
/// Recovery uses the same existing-directory and storage assumptions documented by
/// [`publish_durable_link_v1`]. Corrupt, conflicting, and unsafe entries are left in place and
/// reported without mutating canonical state. Incomplete protocol records are durably invalidated;
/// their artifact and staging names remain inert for explicit operator repair.
pub fn recover_durable_link_publication_v1(
    output_dir: &Path,
    scope: LinkPublicationScopeV1,
) -> Result<Option<DurableLinkPublicationSnapshotV1>, DurableLinkPublicationError> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    let names = DurableNames::new(scope);
    let mut faults = FaultInjector::new(None);
    let mut envelope = recover_envelope(&output, &names, scope)?;
    recover_incomplete(&output, &names, &mut envelope)?;
    verify_or_invalidate_published(&output, &names, &mut envelope, &mut faults)?;
    envelope
        .published
        .as_ref()
        .map(|record| snapshot_for_record(&output, record, &mut faults))
        .transpose()
}

fn invalidate_transaction(
    transaction: &mut DurableLinkPublicationTransactionV1<'_>,
    reason: InvalidationReasonV1,
) -> Result<(), DurableLinkPublicationError> {
    transaction
        .record
        .invalidate(&mut transaction.catalog, transaction.plan.attempt, reason)?;
    transaction.envelope.active = Some(transaction.record.clone());
    persist_envelope(
        transaction.output,
        transaction.names,
        transaction.envelope,
        DurableJournalStageV1::Invalidated,
        transaction.faults,
    )
}

fn persist_envelope(
    output: &PinnedOutput,
    names: &DurableNames,
    envelope: &DurableEnvelopeV1,
    stage: DurableJournalStageV1,
    faults: &mut FaultInjector,
) -> Result<(), DurableLinkPublicationError> {
    let bytes = envelope.encode()?;
    let start = NEXT_REDO_TEMP_ID.fetch_add(MAX_REDO_TEMP_ATTEMPTS, Ordering::Relaxed);
    let mut temporary = None;
    for offset in 0..MAX_REDO_TEMP_ATTEMPTS {
        let candidate = format!(
            "{}.tmp-{}-{}",
            names.redo,
            std::process::id(),
            start.wrapping_add(offset)
        );
        faults.hit_journal(
            stage,
            DurableJournalBoundaryV1::CreateRedoTemp,
            DurableFaultTimingV1::Before,
        )?;
        match openat(
            &output.fd,
            &candidate,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                faults.hit_journal(
                    stage,
                    DurableJournalBoundaryV1::CreateRedoTemp,
                    DurableFaultTimingV1::After,
                )?;
                temporary = Some((candidate, fs::File::from(fd)));
                break;
            }
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    let Some((temporary_name, mut temporary_file)) = temporary else {
        return Err(invalid_record("could not reserve a private redo temp name"));
    };

    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::WriteRedoTemp,
        DurableFaultTimingV1::Before,
    )?;
    temporary_file.write_all(&bytes)?;
    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::WriteRedoTemp,
        DurableFaultTimingV1::After,
    )?;

    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::SyncRedoTemp,
        DurableFaultTimingV1::Before,
    )?;
    temporary_file.sync_all()?;
    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::SyncRedoTemp,
        DurableFaultTimingV1::After,
    )?;

    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::RenameTempToRedo,
        DurableFaultTimingV1::Before,
    )?;
    renameat_with(
        &output.fd,
        &temporary_name,
        &output.fd,
        &names.redo,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::RenameTempToRedo,
        DurableFaultTimingV1::After,
    )?;

    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::SyncRedoName,
        DurableFaultTimingV1::Before,
    )?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::SyncRedoName,
        DurableFaultTimingV1::After,
    )?;

    output.verify_path_identity()?;
    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::RenameRedoToCanonical,
        DurableFaultTimingV1::Before,
    )?;
    renameat(&output.fd, &names.redo, &output.fd, &names.record).map_err(std::io::Error::from)?;
    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::RenameRedoToCanonical,
        DurableFaultTimingV1::After,
    )?;

    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::SyncCanonicalName,
        DurableFaultTimingV1::Before,
    )?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    faults.hit_journal(
        stage,
        DurableJournalBoundaryV1::SyncCanonicalName,
        DurableFaultTimingV1::After,
    )
}

fn recover_envelope(
    output: &PinnedOutput,
    names: &DurableNames,
    scope: LinkPublicationScopeV1,
) -> Result<DurableEnvelopeV1, DurableLinkPublicationError> {
    match read_envelope(output, &names.redo, scope) {
        Ok(Some(redo)) => {
            let canonical = read_envelope(output, &names.record, scope)
                .map_err(DurableReadError::into_public)?
                .unwrap_or_else(|| DurableEnvelopeV1::empty(scope));
            match classify_redo(&canonical, &redo) {
                RedoDisposition::Replay => {}
                RedoDisposition::Stale => {
                    return Err(DurableLinkPublicationError::ConflictingRedo {
                        reason: "stale redo remains beside newer canonical state; V1 refuses destructive cleanup"
                            .to_string(),
                    });
                }
                RedoDisposition::Conflict => {
                    return Err(DurableLinkPublicationError::ConflictingRedo {
                        reason: "redo is not one exact legal transition from canonical state"
                            .to_string(),
                    });
                }
            }
            output.verify_path_identity()?;
            renameat(&output.fd, &names.redo, &output.fd, &names.record)
                .map_err(std::io::Error::from)?;
            fsync(&output.fd).map_err(std::io::Error::from)?;
        }
        Ok(None) => {}
        Err(error) => return Err(error.into_public()),
    }

    Ok(read_envelope(output, &names.record, scope)
        .map_err(DurableReadError::into_public)?
        .unwrap_or_else(|| DurableEnvelopeV1::empty(scope)))
}

enum DurableReadError {
    Transient(DurableLinkPublicationError),
    Corrupt(DurableLinkPublicationError),
}

impl DurableReadError {
    fn into_public(self) -> DurableLinkPublicationError {
        match self {
            Self::Transient(error) | Self::Corrupt(error) => error,
        }
    }
}

fn read_envelope(
    output: &PinnedOutput,
    entry: &str,
    scope: LinkPublicationScopeV1,
) -> Result<Option<DurableEnvelopeV1>, DurableReadError> {
    let bytes = read_control_file(
        output,
        entry,
        "durable link publication record",
        MAX_DURABLE_LINK_PUBLICATION_RECORD_BYTES,
    )
    .map_err(classify_control_read_error)?;
    bytes
        .map(|bytes| DurableEnvelopeV1::decode(&bytes, scope).map_err(DurableReadError::Corrupt))
        .transpose()
}

fn classify_control_read_error(error: EmitError) -> DurableReadError {
    match error {
        EmitError::InvalidArtifactDestination { .. } => {
            DurableReadError::Corrupt(DurableLinkPublicationError::Filesystem(error))
        }
        EmitError::Io(ref io_error)
            if io_error.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) =>
        {
            DurableReadError::Corrupt(DurableLinkPublicationError::Filesystem(error))
        }
        _ => DurableReadError::Transient(DurableLinkPublicationError::Filesystem(error)),
    }
}

fn recover_incomplete(
    output: &PinnedOutput,
    names: &DurableNames,
    envelope: &mut DurableEnvelopeV1,
) -> Result<(), DurableLinkPublicationError> {
    let Some(mut active) = envelope.active.clone() else {
        return Ok(());
    };
    if matches!(active.state(), LinkPublicationStateV1::Invalidated { .. }) {
        return Ok(());
    }
    let mut catalog = catalog_from_published(envelope.published.as_ref())?;
    reconstruct_active(&mut catalog, &active)?;
    active.recover(&mut catalog)?;
    envelope.generation_floor = envelope.generation_floor.max(active.attempt().generation());
    envelope.active = Some(active);
    persist_envelope(
        output,
        names,
        envelope,
        DurableJournalStageV1::Recovered,
        &mut FaultInjector::new(None),
    )
}

fn verify_or_invalidate_published(
    output: &PinnedOutput,
    names: &DurableNames,
    envelope: &mut DurableEnvelopeV1,
    faults: &mut FaultInjector,
) -> Result<(), DurableLinkPublicationError> {
    let Some(record) = envelope.published.as_ref() else {
        return Ok(());
    };
    match snapshot_for_record_checked(output, record, faults) {
        Ok(_) => return Ok(()),
        Err(SnapshotReadError::Transient(error)) => return Err(error),
        Err(SnapshotReadError::Missing | SnapshotReadError::Corrupt(_)) => {}
    }
    envelope.generation_floor = envelope.generation_floor.max(record.attempt().generation());
    envelope.published = None;
    envelope.poisoned = true;
    persist_envelope(
        output,
        names,
        envelope,
        DurableJournalStageV1::Recovered,
        &mut FaultInjector::new(None),
    )
}

fn catalog_from_published(
    published: Option<&LinkPublicationRecordV1>,
) -> Result<LinkPublicationCatalogV1, DurableLinkPublicationError> {
    let mut catalog = LinkPublicationCatalogV1::default();
    if let Some(record) = published {
        let mut rebuilt = catalog.begin(record.attempt(), record.scope(), record.request())?;
        advance_record(&mut rebuilt, &catalog, record)?;
        rebuilt.publish(
            &mut catalog,
            record.attempt(),
            record
                .finalization()
                .ok_or_else(|| invalid_record("missing finalization"))?,
            record
                .finalized_output()
                .ok_or_else(|| invalid_record("missing finalized output"))?,
            record
                .publication()
                .ok_or_else(|| invalid_record("missing publication"))?,
        )?;
    }
    Ok(catalog)
}

fn reconstruct_active(
    catalog: &mut LinkPublicationCatalogV1,
    record: &LinkPublicationRecordV1,
) -> Result<(), DurableLinkPublicationError> {
    let mut rebuilt = catalog.begin(record.attempt(), record.scope(), record.request())?;
    advance_record(&mut rebuilt, catalog, record)?;
    if let LinkPublicationStateV1::Invalidated { reason, .. } = record.state() {
        rebuilt.invalidate(catalog, record.attempt(), reason)?;
    }
    Ok(())
}

fn advance_record(
    rebuilt: &mut LinkPublicationRecordV1,
    catalog: &LinkPublicationCatalogV1,
    expected: &LinkPublicationRecordV1,
) -> Result<(), DurableLinkPublicationError> {
    let phase = evidence_phase(expected.state());
    if phase_at_least(phase, LinkPublicationPhaseV1::WorkerPinned) {
        rebuilt.record_pinned_worker(
            catalog,
            expected.attempt(),
            expected.request(),
            expected
                .worker()
                .ok_or_else(|| invalid_record("missing worker"))?,
        )?;
    }
    if phase_at_least(phase, LinkPublicationPhaseV1::ResponseValidated) {
        rebuilt.record_validated_response(
            catalog,
            expected.attempt(),
            expected.request(),
            expected
                .worker()
                .ok_or_else(|| invalid_record("missing worker"))?,
            expected
                .response()
                .ok_or_else(|| invalid_record("missing response"))?,
            expected
                .linked_output()
                .ok_or_else(|| invalid_record("missing linked output"))?,
        )?;
    }
    if phase_at_least(phase, LinkPublicationPhaseV1::Finalized) {
        rebuilt.record_finalization(
            catalog,
            expected.attempt(),
            expected
                .response()
                .ok_or_else(|| invalid_record("missing response"))?,
            expected
                .linked_output()
                .ok_or_else(|| invalid_record("missing linked output"))?,
            expected
                .finalization()
                .ok_or_else(|| invalid_record("missing finalization"))?,
            expected
                .finalized_output()
                .ok_or_else(|| invalid_record("missing finalized output"))?,
        )?;
    }
    Ok(())
}

fn publish_artifact(
    output: &PinnedOutput,
    bytes: Arc<[u8]>,
    identity: FinalizedOutputIdentityV1,
    faults: &mut FaultInjector,
) -> Result<FinalizedArtifactSnapshot, DurableLinkPublicationError> {
    let entry = artifact_name(identity);
    match open_artifact(output, &entry, identity, faults) {
        Ok(snapshot) => return Ok(snapshot),
        Err(SnapshotReadError::Missing) => {}
        Err(error) => return Err(error.into_public(&entry)),
    }

    let mut staging = StagingDirectory::create(output, &mut NoFaults).map_err(|failure| {
        DurableLinkPublicationError::Cleanup {
            reason: failure.primary.to_string(),
        }
    })?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::CreateTemp,
        DurableFaultTimingV1::Before,
    )?;
    let fd = openat(
        &staging.fd,
        STAGED_ARTIFACT,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(std::io::Error::from)?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::CreateTemp,
        DurableFaultTimingV1::After,
    )?;
    let mut file = fs::File::from(fd);
    faults.hit_artifact(
        DurableArtifactBoundaryV1::WriteTemp,
        DurableFaultTimingV1::Before,
    )?;
    file.write_all(&bytes)?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::WriteTemp,
        DurableFaultTimingV1::After,
    )?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::SyncTemp,
        DurableFaultTimingV1::Before,
    )?;
    file.sync_all()?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::SyncTemp,
        DurableFaultTimingV1::After,
    )?;
    validate_pinned_file(&file, bytes.len())?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::RenameToContentAddress,
        DurableFaultTimingV1::Before,
    )?;
    renameat_with(
        &staging.fd,
        STAGED_ARTIFACT,
        &output.fd,
        &entry,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::RenameToContentAddress,
        DurableFaultTimingV1::After,
    )?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::SyncDirectory,
        DurableFaultTimingV1::Before,
    )?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    faults.hit_artifact(
        DurableArtifactBoundaryV1::SyncDirectory,
        DurableFaultTimingV1::After,
    )?;
    let published =
        statat(&output.fd, &entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    let pinned = fstat(&file).map_err(std::io::Error::from)?;
    if !same_private_file(&pinned, &published, bytes.len()) {
        return Err(unsafe_entry(
            &entry,
            "published name does not match the pinned artifact",
        ));
    }
    let cleanup = staging.cleanup(&mut NoFaults);
    if !cleanup.is_empty() {
        return Err(DurableLinkPublicationError::Cleanup {
            reason: cleanup
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        });
    }
    fsync(&output.fd).map_err(std::io::Error::from)?;
    Ok(FinalizedArtifactSnapshot::from_bytes(
        output.display_path.join(entry),
        bytes,
    ))
}

fn snapshot_for_record(
    output: &PinnedOutput,
    record: &LinkPublicationRecordV1,
    faults: &mut FaultInjector,
) -> Result<DurableLinkPublicationSnapshotV1, DurableLinkPublicationError> {
    snapshot_for_record_checked(output, record, faults)
        .map_err(|error| error.into_public("published artifact"))
}

enum SnapshotReadError {
    Missing,
    Corrupt(DurableLinkPublicationError),
    Transient(DurableLinkPublicationError),
}

impl SnapshotReadError {
    fn into_public(self, entry: &str) -> DurableLinkPublicationError {
        match self {
            Self::Missing => unsafe_entry(entry, "committed artifact is absent"),
            Self::Corrupt(error) | Self::Transient(error) => error,
        }
    }
}

fn snapshot_for_record_checked(
    output: &PinnedOutput,
    record: &LinkPublicationRecordV1,
    faults: &mut FaultInjector,
) -> Result<DurableLinkPublicationSnapshotV1, SnapshotReadError> {
    if record.state() != LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published) {
        return Err(SnapshotReadError::Corrupt(invalid_record(
            "snapshot requested for an incomplete record",
        )));
    }
    let identity = record.finalized_output().ok_or_else(|| {
        SnapshotReadError::Corrupt(invalid_record("published record omitted finalized output"))
    })?;
    Ok(DurableLinkPublicationSnapshotV1 {
        record: record.clone(),
        artifact: open_artifact(output, &artifact_name(identity), identity, faults)?,
    })
}

fn open_artifact(
    output: &PinnedOutput,
    entry: &str,
    identity: FinalizedOutputIdentityV1,
    faults: &mut FaultInjector,
) -> Result<FinalizedArtifactSnapshot, SnapshotReadError> {
    faults.hit_snapshot_read()?;
    let fd = match openat(
        &output.fd,
        entry,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(error) if error == rustix::io::Errno::NOENT => {
            return Err(SnapshotReadError::Missing);
        }
        Err(error) if error == rustix::io::Errno::LOOP => {
            return Err(SnapshotReadError::Corrupt(unsafe_entry(
                entry,
                "artifact name resolves to a symlink",
            )));
        }
        Err(error) => {
            return Err(SnapshotReadError::Transient(
                std::io::Error::from(error).into(),
            ));
        }
    };
    let mut file = fs::File::from(fd);
    let before = fstat(&file)
        .map_err(std::io::Error::from)
        .map_err(DurableLinkPublicationError::from)
        .map_err(SnapshotReadError::Transient)?;
    let length = usize::try_from(before.st_size).unwrap_or(usize::MAX);
    validate_artifact_size(length).map_err(SnapshotReadError::Corrupt)?;
    if !is_private_regular(&before) {
        return Err(SnapshotReadError::Corrupt(unsafe_entry(
            entry,
            "artifact is not a private single-link regular file",
        )));
    }
    let mut bytes = Vec::with_capacity(length);
    Read::by_ref(&mut file)
        .take((MAX_DURABLE_FINALIZED_ARTIFACT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(DurableLinkPublicationError::from)
        .map_err(SnapshotReadError::Transient)?;
    let after = fstat(&file)
        .map_err(std::io::Error::from)
        .map_err(DurableLinkPublicationError::from)
        .map_err(SnapshotReadError::Transient)?;
    let named = match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(named) => named,
        Err(error) if error == rustix::io::Errno::NOENT => {
            return Err(SnapshotReadError::Corrupt(unsafe_entry(
                entry,
                "artifact name disappeared while its descriptor was read",
            )));
        }
        Err(error) => {
            return Err(SnapshotReadError::Transient(
                std::io::Error::from(error).into(),
            ));
        }
    };
    if bytes.len() != length
        || !same_private_file(&before, &after, length)
        || !same_private_file(&before, &named, length)
    {
        return Err(SnapshotReadError::Corrupt(unsafe_entry(
            entry,
            "artifact changed while its descriptor snapshot was captured",
        )));
    }
    if sha256(&bytes) != *identity.as_bytes() {
        return Err(SnapshotReadError::Corrupt(
            DurableLinkPublicationError::FinalizedArtifactDigestMismatch,
        ));
    }
    Ok(FinalizedArtifactSnapshot::from_bytes(
        output.display_path.join(entry),
        Arc::<[u8]>::from(bytes),
    ))
}

fn record_matches_plan(
    record: &LinkPublicationRecordV1,
    plan: DurableLinkPublicationPlanV1,
) -> bool {
    record.attempt() == plan.attempt
        && record.scope() == plan.scope
        && record.request() == plan.request
        && record.worker() == Some(plan.worker)
        && record.response() == Some(plan.response)
        && record.linked_output() == Some(plan.linked_output)
        && record.finalization() == Some(plan.finalization)
        && record.finalized_output() == Some(plan.finalized_output)
        && record.publication() == Some(plan.publication)
        && record.state() == LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published)
}

fn crash_retry_matches_plan(
    envelope: &DurableEnvelopeV1,
    plan: DurableLinkPublicationPlanV1,
) -> bool {
    let Some(record) = envelope.active.as_ref() else {
        return false;
    };
    matches!(
        record.state(),
        LinkPublicationStateV1::Invalidated {
            reason: InvalidationReasonV1::CrashRecovery,
            ..
        }
    ) && envelope.active_plan == Some(plan.identity())
        && record.attempt() == plan.attempt
        && record.scope() == plan.scope
        && record.request() == plan.request
        && record
            .worker()
            .is_none_or(|identity| identity == plan.worker)
        && record
            .response()
            .is_none_or(|identity| identity == plan.response)
        && record
            .linked_output()
            .is_none_or(|identity| identity == plan.linked_output)
        && record
            .finalization()
            .is_none_or(|identity| identity == plan.finalization)
        && record
            .finalized_output()
            .is_none_or(|identity| identity == plan.finalized_output)
        && record
            .publication()
            .is_none_or(|identity| identity == plan.publication)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RedoDisposition {
    Replay,
    Stale,
    Conflict,
}

fn classify_redo(canonical: &DurableEnvelopeV1, redo: &DurableEnvelopeV1) -> RedoDisposition {
    if redo == canonical {
        return RedoDisposition::Replay;
    }
    if redo.generation_floor < canonical.generation_floor {
        return RedoDisposition::Stale;
    }
    if redo.generation_floor > canonical.generation_floor {
        return if is_legal_next_generation(canonical, redo) {
            RedoDisposition::Replay
        } else {
            RedoDisposition::Conflict
        };
    }
    if is_legal_same_generation_transition(canonical, redo) {
        RedoDisposition::Replay
    } else {
        RedoDisposition::Conflict
    }
}

fn is_legal_next_generation(canonical: &DurableEnvelopeV1, redo: &DurableEnvelopeV1) -> bool {
    let first_observed_generation = canonical.generation_floor == 0
        && canonical.published.is_none()
        && canonical.active.is_none()
        && !canonical.poisoned;
    let contiguous_generation = canonical
        .generation_floor
        .checked_add(1)
        .is_some_and(|next| redo.generation_floor == next);
    let canonical_is_terminal = canonical
        .active
        .as_ref()
        .is_none_or(|record| matches!(record.state(), LinkPublicationStateV1::Invalidated { .. }));
    let Some(candidate) = redo.active.as_ref() else {
        return false;
    };
    (first_observed_generation || contiguous_generation)
        && canonical_is_terminal
        && redo.published == canonical.published
        && redo.poisoned == canonical.poisoned
        && redo.active_plan.is_some()
        && candidate.attempt().generation() == redo.generation_floor
        && candidate.state() == LinkPublicationStateV1::Active(LinkPublicationPhaseV1::RequestBound)
}

fn is_legal_same_generation_transition(
    canonical: &DurableEnvelopeV1,
    redo: &DurableEnvelopeV1,
) -> bool {
    if canonical.published == redo.published
        && canonical.poisoned == redo.poisoned
        && canonical.active_plan == redo.active_plan
    {
        return classify_active_redo(canonical.active.as_ref(), redo.active.as_ref())
            == RedoDisposition::Replay;
    }

    if let (Some(current), Some(candidate)) = (canonical.active.as_ref(), redo.published.as_ref())
        && current.state() == LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Finalized)
        && candidate.state() == LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published)
        && redo.active.is_none()
        && redo.active_plan.is_none()
        && !redo.poisoned
        && records_have_same_prefix(current, candidate)
        && canonical.active_plan == published_plan_identity(candidate)
    {
        return true;
    }

    canonical.published.is_some()
        && redo.published.is_none()
        && redo.poisoned
        && canonical.active == redo.active
        && canonical.active_plan == redo.active_plan
}

fn published_plan_identity(record: &LinkPublicationRecordV1) -> Option<[u8; 32]> {
    if record.state() != LinkPublicationStateV1::Active(LinkPublicationPhaseV1::Published) {
        return None;
    }
    Some(
        DurableLinkPublicationPlanV1::new(
            record.attempt(),
            record.scope(),
            record.request(),
            record.worker()?,
            record.response()?,
            record.linked_output()?,
            record.finalization()?,
            record.finalized_output()?,
            record.publication()?,
        )
        .identity(),
    )
}

fn classify_active_redo(
    canonical: Option<&LinkPublicationRecordV1>,
    redo: Option<&LinkPublicationRecordV1>,
) -> RedoDisposition {
    match (canonical, redo) {
        (Some(current), Some(candidate)) if !records_have_same_prefix(current, candidate) => {
            RedoDisposition::Conflict
        }
        (Some(current), Some(candidate)) => match (current.state(), candidate.state()) {
            (
                LinkPublicationStateV1::Active(current_phase),
                LinkPublicationStateV1::Active(next_phase),
            ) if phase_number(next_phase) == phase_number(current_phase).saturating_add(1) => {
                RedoDisposition::Replay
            }
            (
                LinkPublicationStateV1::Active(current_phase),
                LinkPublicationStateV1::Invalidated {
                    prior_phase: next_phase,
                    ..
                },
            ) if next_phase == current_phase && records_have_exact_evidence(current, candidate) => {
                RedoDisposition::Replay
            }
            (
                LinkPublicationStateV1::Invalidated {
                    prior_phase: current_phase,
                    reason: InvalidationReasonV1::CrashRecovery,
                },
                LinkPublicationStateV1::Active(next_phase),
            ) if next_phase == current_phase && records_have_exact_evidence(current, candidate) => {
                RedoDisposition::Replay
            }
            _ => RedoDisposition::Conflict,
        },
        _ => RedoDisposition::Conflict,
    }
}

fn records_have_exact_evidence(
    left: &LinkPublicationRecordV1,
    right: &LinkPublicationRecordV1,
) -> bool {
    left.attempt() == right.attempt()
        && left.scope() == right.scope()
        && left.request() == right.request()
        && left.worker() == right.worker()
        && left.response() == right.response()
        && left.linked_output() == right.linked_output()
        && left.finalization() == right.finalization()
        && left.finalized_output() == right.finalized_output()
        && left.publication() == right.publication()
}

fn records_have_same_prefix(
    left: &LinkPublicationRecordV1,
    right: &LinkPublicationRecordV1,
) -> bool {
    left.attempt() == right.attempt()
        && left.scope() == right.scope()
        && left.request() == right.request()
        && options_compatible(left.worker(), right.worker())
        && options_compatible(left.response(), right.response())
        && options_compatible(left.linked_output(), right.linked_output())
        && options_compatible(left.finalization(), right.finalization())
        && options_compatible(left.finalized_output(), right.finalized_output())
        && options_compatible(left.publication(), right.publication())
}

fn options_compatible<T: Eq>(left: Option<T>, right: Option<T>) -> bool {
    left.is_none() || right.is_none() || left == right
}

fn validate_pinned_file(
    file: &fs::File,
    expected: usize,
) -> Result<(), DurableLinkPublicationError> {
    let stat = fstat(file).map_err(std::io::Error::from)?;
    if !is_private_regular(&stat) || usize::try_from(stat.st_size).ok() != Some(expected) {
        return Err(invalid_record(
            "staged finalized artifact changed before publication",
        ));
    }
    Ok(())
}

fn is_private_regular(stat: &rustix::fs::Stat) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
        && stat.st_nlink == 1
        && stat.st_mode & 0o077 == 0
}

fn same_private_file(left: &rustix::fs::Stat, right: &rustix::fs::Stat, length: usize) -> bool {
    is_private_regular(left)
        && is_private_regular(right)
        && left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_size == right.st_size
        && usize::try_from(left.st_size).ok() == Some(length)
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

fn validate_artifact_size(actual: usize) -> Result<(), DurableLinkPublicationError> {
    if actual == 0 || actual > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES {
        Err(DurableLinkPublicationError::FinalizedArtifactSize {
            actual,
            maximum: MAX_DURABLE_FINALIZED_ARTIFACT_BYTES,
        })
    } else {
        Ok(())
    }
}

fn evidence_phase(state: LinkPublicationStateV1) -> LinkPublicationPhaseV1 {
    match state {
        LinkPublicationStateV1::Active(phase)
        | LinkPublicationStateV1::Invalidated {
            prior_phase: phase, ..
        } => phase,
    }
}

fn phase_at_least(actual: LinkPublicationPhaseV1, expected: LinkPublicationPhaseV1) -> bool {
    phase_number(actual) >= phase_number(expected)
}

const fn phase_number(phase: LinkPublicationPhaseV1) -> u8 {
    match phase {
        LinkPublicationPhaseV1::RequestBound => 1,
        LinkPublicationPhaseV1::WorkerPinned => 2,
        LinkPublicationPhaseV1::ResponseValidated => 3,
        LinkPublicationPhaseV1::Finalized => 4,
        LinkPublicationPhaseV1::Published => 5,
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn domain_sha256(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn artifact_name(identity: FinalizedOutputIdentityV1) -> String {
    format!(
        "{ARTIFACT_PREFIX}{}{ARTIFACT_SUFFIX}",
        hex(identity.as_bytes())
    )
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(DIGITS[usize::from(byte >> 4)]));
        encoded.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    encoded
}

struct DurableNames {
    record: String,
    redo: String,
}

impl DurableNames {
    fn new(scope: LinkPublicationScopeV1) -> Self {
        let mut digest = Sha256::new();
        digest.update(SCOPE_IDENTITY_DOMAIN);
        digest.update(scope.package().as_bytes());
        digest.update(scope.kernel_set().as_bytes());
        digest.update(scope.target().as_bytes());
        let key = hex(&digest.finalize());
        let record = format!("{RECORD_PREFIX}{key}{RECORD_SUFFIX}");
        let redo = format!("{record}{REDO_SUFFIX}");
        Self { record, redo }
    }
}

struct FaultInjector {
    point: Option<DurableLinkPublicationFaultPointV1>,
    fired: Option<DurableLinkPublicationFaultPointV1>,
}

impl FaultInjector {
    const fn new(point: Option<DurableLinkPublicationFaultPointV1>) -> Self {
        Self { point, fired: None }
    }

    fn hit(
        &mut self,
        point: DurableLinkPublicationFaultPointV1,
    ) -> Result<(), DurableLinkPublicationError> {
        if self.fired.is_none() && self.point == Some(point) {
            self.fired = Some(point);
            Err(DurableLinkPublicationError::InjectedCrash { point })
        } else {
            Ok(())
        }
    }

    fn hit_snapshot_read(&mut self) -> Result<(), SnapshotReadError> {
        let point = DurableLinkPublicationFaultPointV1::SnapshotRead;
        if self.fired.is_none() && self.point == Some(point) {
            self.fired = Some(point);
            Err(SnapshotReadError::Transient(
                std::io::Error::other("injected transient snapshot read failure").into(),
            ))
        } else {
            Ok(())
        }
    }

    fn hit_journal(
        &mut self,
        stage: DurableJournalStageV1,
        boundary: DurableJournalBoundaryV1,
        timing: DurableFaultTimingV1,
    ) -> Result<(), DurableLinkPublicationError> {
        self.hit(DurableLinkPublicationFaultPointV1::Journal {
            stage,
            boundary,
            timing,
        })
    }

    fn hit_artifact(
        &mut self,
        boundary: DurableArtifactBoundaryV1,
        timing: DurableFaultTimingV1,
    ) -> Result<(), DurableLinkPublicationError> {
        self.hit(DurableLinkPublicationFaultPointV1::Artifact { boundary, timing })
    }
}

fn push_scope(bytes: &mut Vec<u8>, scope: LinkPublicationScopeV1) {
    bytes.push(SCOPE_TAG_PACKAGE);
    bytes.extend_from_slice(scope.package().as_bytes());
    bytes.push(SCOPE_TAG_KERNEL_SET);
    bytes.extend_from_slice(scope.kernel_set().as_bytes());
    bytes.push(SCOPE_TAG_TARGET);
    bytes.extend_from_slice(scope.target().as_bytes());
}

fn push_optional_identity(bytes: &mut Vec<u8>, identity: Option<[u8; 32]>) {
    match identity {
        Some(identity) => {
            bytes.push(PLAN_IDENTITY_TAG);
            bytes.extend_from_slice(&identity);
        }
        None => bytes.push(0),
    }
}

fn decode_scope(
    decoder: &mut EnvelopeDecoder<'_>,
) -> Result<LinkPublicationScopeV1, DurableLinkPublicationError> {
    Ok(LinkPublicationScopeV1::new(
        super::PackageIdentityV1::from_bytes(decoder.identity(SCOPE_TAG_PACKAGE)?),
        super::KernelSetIdentityV1::from_bytes(decoder.identity(SCOPE_TAG_KERNEL_SET)?),
        super::TargetIdentityV1::from_bytes(decoder.identity(SCOPE_TAG_TARGET)?),
    ))
}

fn push_record(
    bytes: &mut Vec<u8>,
    record: Option<&[u8]>,
) -> Result<(), DurableLinkPublicationError> {
    let length = record.map_or(0, <[u8]>::len);
    let length =
        u16::try_from(length).map_err(|_| invalid_record("nested record length overflow"))?;
    bytes.extend_from_slice(&length.to_le_bytes());
    if let Some(record) = record {
        bytes.extend_from_slice(record);
    }
    Ok(())
}

struct EnvelopeDecoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> EnvelopeDecoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], DurableLinkPublicationError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| invalid_record("record offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid_record("truncated durable envelope"))?;
        self.offset = end;
        Ok(bytes)
    }

    fn byte(&mut self) -> Result<u8, DurableLinkPublicationError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, DurableLinkPublicationError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two bytes"),
        ))
    }

    fn u64(&mut self) -> Result<u64, DurableLinkPublicationError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }

    fn identity(&mut self, expected_tag: u8) -> Result<[u8; 32], DurableLinkPublicationError> {
        let actual = self.byte()?;
        if actual != expected_tag {
            return Err(invalid_record(format!(
                "identity tag {actual:#x} does not match {expected_tag:#x}"
            )));
        }
        Ok(self.take(32)?.try_into().expect("32-byte identity"))
    }

    fn optional_identity(
        &mut self,
        expected_tag: u8,
    ) -> Result<Option<[u8; 32]>, DurableLinkPublicationError> {
        let actual = self.byte()?;
        if actual == 0 {
            return Ok(None);
        }
        if actual != expected_tag {
            return Err(invalid_record(format!(
                "optional identity tag {actual:#x} does not match {expected_tag:#x}",
            )));
        }
        Ok(Some(self.take(32)?.try_into().expect("32-byte identity")))
    }

    fn record(&mut self) -> Result<Option<LinkPublicationRecordV1>, DurableLinkPublicationError> {
        let length = usize::from(self.u16()?);
        if length == 0 {
            return Ok(None);
        }
        if length > super::MAX_LINK_PUBLICATION_RECORD_BYTES {
            return Err(invalid_record(
                "nested link publication record exceeds its bound",
            ));
        }
        Ok(Some(LinkPublicationRecordV1::decode_canonical(
            self.take(length)?,
        )?))
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn invalid_record(reason: impl Into<String>) -> DurableLinkPublicationError {
    DurableLinkPublicationError::InvalidDurableRecord {
        reason: reason.into(),
    }
}

fn unsafe_entry(
    entry: impl Into<String>,
    reason: impl Into<String>,
) -> DurableLinkPublicationError {
    DurableLinkPublicationError::UnsafeManagedEntry {
        entry: entry.into(),
        reason: reason.into(),
    }
}
