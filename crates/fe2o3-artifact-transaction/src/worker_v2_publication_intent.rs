//! Durable restart input for one admitted Worker V2 publication.
//!
//! The compiler handoff is one-shot. Once a caller has independently admitted the worker output
//! and prepared a complete durable publication plan, this protocol retains the exact output bytes
//! and every identity needed to retry publication in another process. Fresh intent creation is
//! accepted only while the build attempt remains in its unclaimed `Building` phase. An already
//! committed intent may be recovered after the backend claim has been consumed.
//!
//! Intent records are inert coordination evidence. This module does not authenticate the compiler,
//! worker, Verus, or caller-supplied evidence and grants no publication, loading, or launch authority.

use crate::attempt::{AttemptPhase, BackendReceiptV1};
use crate::attempt_scoped_hsaco_publication::publication_receipt;
use crate::{
    AtomicPublicationIdentityV1, BuildAttempt, BuildSession, CanonicalLinkRequestIdentityV1,
    DurableLinkPublicationPlanV1, EmitError, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, PackageIdentityV1, PinnedOutput, PinnedWorkerIdentityV1,
    ProducerIdentity, TargetIdentityV1, UpstreamCodeObjectEvidenceIdentityV1,
    ValidatedResponseIdentityV1, read_attempt_registry,
};
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fsync, openat, renameat, renameat_with,
    statat, unlinkat,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const RECORD_MAGIC: &[u8] = b"FE2O3-WORKER-V2-PUBLICATION-INTENT-V1\0";
const RECORD_VERSION: u16 = 1;
const PRODUCER_DOMAIN: &[u8] = b"fe2o3.worker-v2-publication-intent.producer.v1\0";
const SLOT_DOMAIN: &[u8] = b"fe2o3.worker-v2-publication-intent.slot.v1\0";
const RECORD_CHECKSUM_DOMAIN: &[u8] = b"fe2o3.worker-v2-publication-intent.record-checksum.v1\0";
const RECORD_IDENTITY_DOMAIN: &[u8] = b"fe2o3.worker-v2-publication-intent.record-identity.v1\0";
const FILE_PREFIX: &str = ".fe2o3-worker-v2-publication-intent-v1-";
const OUTPUT_SUFFIX: &str = ".output";
const RECORD_SUFFIX: &str = ".record";
const REDO_SUFFIX: &str = ".record.redo";
const TEMP_SUFFIX: &str = ".tmp-";
const MAX_TEMP_ATTEMPTS: u64 = 64;

// magic, version, slot, attempt, producer, upstream, plan commitment, scope, seven plan
// identities, measured output identity, output length, checksum.
const RECORD_BYTES: usize =
    RECORD_MAGIC.len() + 2 + 32 + 8 + 16 + 32 + 32 + 32 + 32 + (3 * 32) + (7 * 32) + 32 + 8 + 32;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// Exact canonical size of a V1 publication-intent record.
pub const MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES: usize = RECORD_BYTES;

/// Maximum exact Worker V2 output retained by one publication intent.
pub const MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES: usize =
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES;

/// SHA-256 identity of one complete canonical publication-intent record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV2PublicationIntentIdentityV1([u8; 32]);

impl WorkerV2PublicationIntentIdentityV1 {
    /// Constructs an identity from its exact 256-bit representation.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact 256-bit representation.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Canonical identities required to retry one exact Worker V2 publication after restart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentRecordV1 {
    slot: [u8; 32],
    attempt: BuildAttempt,
    producer_identity: [u8; 32],
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    plan: DurableLinkPublicationPlanV1,
    output_identity: FinalizedOutputIdentityV1,
    output_length: usize,
    identity: WorkerV2PublicationIntentIdentityV1,
}

impl WorkerV2PublicationIntentRecordV1 {
    /// Exact build attempt to which this intent is bound.
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    /// Domain-separated identity of the exact producer source and crate name.
    pub const fn producer_identity(self) -> [u8; 32] {
        self.producer_identity
    }

    /// Caller-supplied identity of the admitted upstream Worker V2 evidence.
    pub const fn upstream_evidence(self) -> UpstreamCodeObjectEvidenceIdentityV1 {
        self.upstream_evidence
    }

    /// Complete durable publication plan reconstructed from the canonical record.
    pub const fn plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    /// SHA-256 identity of the exact retained output bytes.
    pub const fn output_identity(self) -> FinalizedOutputIdentityV1 {
        self.output_identity
    }

    /// Exact retained output length.
    pub const fn output_length(self) -> usize {
        self.output_length
    }

    /// Identity of the complete checksummed canonical record.
    pub const fn identity(self) -> WorkerV2PublicationIntentIdentityV1 {
        self.identity
    }

    /// An intent remains inert until the existing attempt-scoped publication API authorizes it.
    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    /// A producer digest and attempt token do not authenticate compiler authorship.
    pub const fn grants_compiler_authority(self) -> bool {
        false
    }

    /// A persisted publication intent does not authorize HSA loading.
    pub const fn grants_load_authority(self) -> bool {
        false
    }

    /// A persisted publication intent does not authorize kernel launch.
    pub const fn grants_launch_authority(self) -> bool {
        false
    }

    fn new(
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        output_length: usize,
    ) -> Self {
        let producer_identity = producer_identity(producer);
        let output_identity = plan.finalized_output();
        let mut record = Self {
            slot: slot_identity(producer_identity, attempt),
            attempt,
            producer_identity,
            upstream_evidence,
            plan,
            output_identity,
            output_length,
            identity: WorkerV2PublicationIntentIdentityV1([0; 32]),
        };
        record.identity = record.encoded_identity();
        record
    }

    fn encode(self) -> Vec<u8> {
        let mut bytes = self.encode_body();
        let checksum = sha256_parts(&[RECORD_CHECKSUM_DOMAIN, &bytes]);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), RECORD_BYTES);
        bytes
    }

    fn encode_body(self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(RECORD_BYTES - 32);
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&RECORD_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.slot);
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        bytes.extend_from_slice(&self.producer_identity);
        bytes.extend_from_slice(&self.upstream_evidence.as_bytes());
        bytes.extend_from_slice(&self.plan.identity());
        push_scope(&mut bytes, self.plan.scope());
        bytes.extend_from_slice(self.plan.request().as_bytes());
        bytes.extend_from_slice(self.plan.worker().as_bytes());
        bytes.extend_from_slice(self.plan.response().as_bytes());
        bytes.extend_from_slice(self.plan.linked_output().as_bytes());
        bytes.extend_from_slice(self.plan.finalization().as_bytes());
        bytes.extend_from_slice(self.plan.finalized_output().as_bytes());
        bytes.extend_from_slice(self.plan.publication().as_bytes());
        bytes.extend_from_slice(self.output_identity.as_bytes());
        bytes.extend_from_slice(&(self.output_length as u64).to_le_bytes());
        bytes
    }

    fn encoded_identity(self) -> WorkerV2PublicationIntentIdentityV1 {
        let encoded = self.encode();
        WorkerV2PublicationIntentIdentityV1(sha256_parts(&[RECORD_IDENTITY_DOMAIN, &encoded]))
    }

    fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != RECORD_BYTES {
            return Err("record has a noncanonical length");
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if sha256_parts(&[RECORD_CHECKSUM_DOMAIN, body]).as_slice() != checksum {
            return Err("record checksum mismatch");
        }
        let mut decoder = Decoder::new(body);
        if decoder.take(RECORD_MAGIC.len())? != RECORD_MAGIC {
            return Err("record magic mismatch");
        }
        if decoder.u16()? != RECORD_VERSION {
            return Err("unsupported record version");
        }
        let slot = decoder.array()?;
        let generation = decoder.u64()?;
        let session = crate::BuildSession::from_bytes(decoder.array()?);
        let invocation = crate::BuildInvocation::from_bytes(decoder.array()?);
        let attempt = BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            session.to_hex(),
            invocation.to_hex()
        ))
        .map_err(|_| "record contains an invalid attempt")?;
        let producer_identity = decoder.array()?;
        let upstream_evidence = UpstreamCodeObjectEvidenceIdentityV1::from_bytes(decoder.array()?);
        let committed_plan_identity: [u8; 32] = decoder.array()?;
        let scope = LinkPublicationScopeV1::new(
            PackageIdentityV1::from_bytes(decoder.array()?),
            KernelSetIdentityV1::from_bytes(decoder.array()?),
            TargetIdentityV1::from_bytes(decoder.array()?),
        );
        let plan = DurableLinkPublicationPlanV1::new(
            attempt,
            scope,
            CanonicalLinkRequestIdentityV1::from_bytes(decoder.array()?),
            PinnedWorkerIdentityV1::from_bytes(decoder.array()?),
            ValidatedResponseIdentityV1::from_bytes(decoder.array()?),
            LinkedOutputIdentityV1::from_bytes(decoder.array()?),
            FinalizationIdentityV1::from_bytes(decoder.array()?),
            FinalizedOutputIdentityV1::from_bytes(decoder.array()?),
            AtomicPublicationIdentityV1::from_bytes(decoder.array()?),
        );
        let output_identity = FinalizedOutputIdentityV1::from_bytes(decoder.array()?);
        let output_length =
            usize::try_from(decoder.u64()?).map_err(|_| "record output length is invalid")?;
        if !decoder.finished() {
            return Err("record has trailing body bytes");
        }
        if output_length == 0 || output_length > MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES {
            return Err("record output length is outside the supported bound");
        }
        if committed_plan_identity != plan.identity() {
            return Err("record plan commitment does not match its plan fields");
        }
        if output_identity != plan.finalized_output() {
            return Err("record output identity does not match its publication plan");
        }
        let mut record = Self {
            slot,
            attempt,
            producer_identity,
            upstream_evidence,
            plan,
            output_identity,
            output_length,
            identity: WorkerV2PublicationIntentIdentityV1([0; 32]),
        };
        record.identity =
            WorkerV2PublicationIntentIdentityV1(sha256_parts(&[RECORD_IDENTITY_DOMAIN, bytes]));
        Ok(record)
    }
}

/// Whether persistence created a new intent or reconciled one left by an earlier process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2PublicationIntentOutcomeV1 {
    /// This call committed a new canonical intent.
    Persisted,
    /// This call recovered an exact intent committed by an earlier process.
    Recovered,
}

/// Immutable restart input returned only after record and output validation under the store lock.
#[derive(Clone, Debug)]
pub struct RecoveredWorkerV2PublicationIntentV1 {
    outcome: WorkerV2PublicationIntentOutcomeV1,
    record: WorkerV2PublicationIntentRecordV1,
    exact_output: Arc<[u8]>,
}

impl RecoveredWorkerV2PublicationIntentV1 {
    /// Reports whether this call committed or recovered the intent.
    pub const fn outcome(&self) -> WorkerV2PublicationIntentOutcomeV1 {
        self.outcome
    }

    /// Returns the validated canonical identity record.
    pub const fn record(&self) -> WorkerV2PublicationIntentRecordV1 {
        self.record
    }

    /// Borrows the exact retained Worker V2 output bytes.
    pub fn exact_output(&self) -> &[u8] {
        &self.exact_output
    }

    /// Consumes this result and returns its immutable output snapshot.
    pub fn into_exact_output(self) -> Arc<[u8]> {
        self.exact_output
    }

    /// Recovery still requires the attempt-scoped publication API to authorize these inputs.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Persisted intent bytes do not authorize HSA loading.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Persisted intent bytes do not authorize kernel launch.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Durable operation at which a test may simulate abrupt process termination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2PublicationIntentBoundaryV1 {
    /// Reserve the private temporary output entry.
    CreateOutputTemp,
    /// Write exact output bytes to the temporary entry.
    WriteOutputTemp,
    /// Synchronize the temporary output file.
    SyncOutputTemp,
    /// Atomically expose the attempt-scoped output name.
    RenameOutput,
    /// Synchronize the exposed output name in the output directory.
    SyncOutputName,
    /// Reserve the private temporary record entry.
    CreateRecordTemp,
    /// Write the canonical checksummed record.
    WriteRecordTemp,
    /// Synchronize the temporary record file.
    SyncRecordTemp,
    /// Atomically expose a replayable redo record.
    RenameRecordToRedo,
    /// Synchronize the replayable redo name.
    SyncRedoName,
    /// Atomically promote the redo record to the canonical name.
    RenameRedoToCanonical,
    /// Synchronize the canonical record name.
    SyncCanonicalName,
}

/// Whether fault injection interrupts immediately before or after one durable operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV2PublicationIntentFaultTimingV1 {
    /// Interrupt before the selected operation.
    Before,
    /// Interrupt after the selected operation.
    After,
}

/// Exact deterministic crash point used by persistence tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentFaultPointV1 {
    /// Durable operation to interrupt.
    pub boundary: WorkerV2PublicationIntentBoundaryV1,
    /// Side of the operation on which to interrupt.
    pub timing: WorkerV2PublicationIntentFaultTimingV1,
}

/// Fault-injection options. Production callers use [`Default::default`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerV2PublicationIntentOptionsV1 {
    injected_crash: Option<WorkerV2PublicationIntentFaultPointV1>,
}

impl WorkerV2PublicationIntentOptionsV1 {
    /// Simulates abrupt process termination at one exact persistence boundary.
    pub const fn inject_crash(point: WorkerV2PublicationIntentFaultPointV1) -> Self {
        Self {
            injected_crash: Some(point),
        }
    }
}

/// Failure to persist or recover exact Worker V2 restart inputs.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2PublicationIntentErrorV1 {
    /// The shared artifact store rejected an operation or changed identity.
    Store(EmitError),
    /// A descriptor-relative filesystem operation failed.
    Io(std::io::Error),
    /// The publication plan names a different build attempt.
    PlanAttemptMismatch,
    /// The exact output is empty or exceeds the protocol bound.
    InvalidOutputSize { actual: usize, maximum: usize },
    /// Exact output bytes do not match the plan's finalized-output identity.
    OutputDigestMismatch,
    /// The build attempt cannot create, recover, or remove this intent.
    Attempt { reason: String },
    /// No committed canonical or replayable intent exists.
    NotFound,
    /// Different exact inputs are already committed for the same producer and attempt.
    ConflictingIntent,
    /// Cleanup named a different canonical record identity.
    IntentIdentityMismatch,
    /// Persisted state is noncanonical, corrupt, substituted, or unsafe to follow.
    InvalidIntent { path: PathBuf, reason: String },
    /// Deterministic crash-like interruption requested by test options.
    InjectedCrash {
        point: WorkerV2PublicationIntentFaultPointV1,
    },
}

impl fmt::Display for WorkerV2PublicationIntentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(
                formatter,
                "artifact store rejected publication intent: {error}"
            ),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::PlanAttemptMismatch => formatter
                .write_str("Worker V2 publication plan does not match the supplied build attempt"),
            Self::InvalidOutputSize { actual, maximum } => write!(
                formatter,
                "Worker V2 publication-intent output size {actual} is outside 1..={maximum} bytes"
            ),
            Self::OutputDigestMismatch => formatter.write_str(
                "Worker V2 publication-intent output digest does not match the durable plan",
            ),
            Self::Attempt { reason } => write!(
                formatter,
                "invalid Worker V2 publication-intent attempt: {reason}"
            ),
            Self::NotFound => formatter.write_str("Worker V2 publication intent was not found"),
            Self::ConflictingIntent => formatter.write_str(
                "a different Worker V2 publication intent is already committed for this attempt",
            ),
            Self::IntentIdentityMismatch => formatter.write_str(
                "Worker V2 publication-intent identity does not match the committed record",
            ),
            Self::InvalidIntent { path, reason } => write!(
                formatter,
                "invalid Worker V2 publication intent {}: {reason}",
                path.display()
            ),
            Self::InjectedCrash { point } => {
                write!(
                    formatter,
                    "injected Worker V2 publication-intent crash at {point:?}"
                )
            }
        }
    }
}

impl std::error::Error for WorkerV2PublicationIntentErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EmitError> for WorkerV2PublicationIntentErrorV1 {
    fn from(error: EmitError) -> Self {
        Self::Store(error)
    }
}

impl From<std::io::Error> for WorkerV2PublicationIntentErrorV1 {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Persists exact restart input before the attempt-scoped backend claim is consumed.
pub fn persist_worker_v2_publication_intent_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    exact_output: &[u8],
) -> Result<RecoveredWorkerV2PublicationIntentV1, WorkerV2PublicationIntentErrorV1> {
    persist_worker_v2_publication_intent_v1_with_options(
        output_dir,
        producer,
        attempt,
        plan,
        upstream_evidence,
        exact_output,
        WorkerV2PublicationIntentOptionsV1::default(),
    )
}

/// Fault-injectable form of [`persist_worker_v2_publication_intent_v1`].
pub fn persist_worker_v2_publication_intent_v1_with_options(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
    exact_output: &[u8],
    options: WorkerV2PublicationIntentOptionsV1,
) -> Result<RecoveredWorkerV2PublicationIntentV1, WorkerV2PublicationIntentErrorV1> {
    validate_inputs(attempt, plan, exact_output)?;
    let expected = WorkerV2PublicationIntentRecordV1::new(
        producer,
        attempt,
        plan,
        upstream_evidence,
        exact_output.len(),
    );
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let authorization = authorize(&output, producer, attempt)?;
    let names = IntentNames::new(expected.producer_identity, expected.slot);
    cleanup_temps(&output, &names)?;

    if let Some(recovered) = recover_locked(&output, &names, producer, attempt)? {
        if recovered.record != expected || recovered.exact_output.as_ref() != exact_output {
            return Err(WorkerV2PublicationIntentErrorV1::ConflictingIntent);
        }
        return Ok(recovered);
    }
    if authorization != AttemptPhase::Building {
        return Err(WorkerV2PublicationIntentErrorV1::Attempt {
            reason: "a fresh intent may be created only before backend authority is claimed"
                .to_string(),
        });
    }

    let mut faults = FaultInjector::new(options.injected_crash);
    persist_output(&output, &names, exact_output, &mut faults)?;
    persist_record(&output, &names, expected, &mut faults)?;
    let recovered = recover_locked(&output, &names, producer, attempt)?
        .ok_or_else(|| invalid(&output, &names.record, "record disappeared after commit"))?;
    if recovered.record != expected || recovered.exact_output.as_ref() != exact_output {
        return Err(WorkerV2PublicationIntentErrorV1::ConflictingIntent);
    }
    Ok(RecoveredWorkerV2PublicationIntentV1 {
        outcome: WorkerV2PublicationIntentOutcomeV1::Persisted,
        ..recovered
    })
}

/// Recovers exact persisted inputs after a process restart.
pub fn recover_worker_v2_publication_intent_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredWorkerV2PublicationIntentV1, WorkerV2PublicationIntentErrorV1> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize(&output, producer, attempt)?;
    let producer_identity = producer_identity(producer);
    let names = IntentNames::new(producer_identity, slot_identity(producer_identity, attempt));
    cleanup_temps(&output, &names)?;
    recover_locked(&output, &names, producer, attempt)?
        .ok_or(WorkerV2PublicationIntentErrorV1::NotFound)
}

/// Removes one exact committed intent after its publication receipt is durable.
///
/// The record name is removed and synced before its output bytes are unlinked. A crash may leave an
/// inert output orphan, but cannot leave a committed record naming deleted bytes.
pub fn clear_worker_v2_publication_intent_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    identity: WorkerV2PublicationIntentIdentityV1,
) -> Result<(), WorkerV2PublicationIntentErrorV1> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize_any_phase(&output, producer, attempt)?;
    let producer_identity = producer_identity(producer);
    let names = IntentNames::new(producer_identity, slot_identity(producer_identity, attempt));
    cleanup_temps(&output, &names)?;
    let recovered = recover_locked(&output, &names, producer, attempt)?
        .ok_or(WorkerV2PublicationIntentErrorV1::NotFound)?;
    if recovered.record.identity != identity {
        return Err(WorkerV2PublicationIntentErrorV1::IntentIdentityMismatch);
    }
    authorize_clear(&output, producer, attempt, recovered.record)?;
    unlinkat(&output.fd, &names.record, AtFlags::empty()).map_err(std::io::Error::from)?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    unlinkat(&output.fd, &names.output, AtFlags::empty()).map_err(std::io::Error::from)?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    Ok(())
}

fn validate_inputs(
    attempt: BuildAttempt,
    plan: DurableLinkPublicationPlanV1,
    exact_output: &[u8],
) -> Result<(), WorkerV2PublicationIntentErrorV1> {
    if plan.attempt() != attempt {
        return Err(WorkerV2PublicationIntentErrorV1::PlanAttemptMismatch);
    }
    if attempt.session() == BuildSession::DIRECT {
        return Err(WorkerV2PublicationIntentErrorV1::Attempt {
            reason: "the direct compiler token cannot own a restart intent".to_string(),
        });
    }
    if exact_output.is_empty() || exact_output.len() > MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES
    {
        return Err(WorkerV2PublicationIntentErrorV1::InvalidOutputSize {
            actual: exact_output.len(),
            maximum: MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES,
        });
    }
    if sha256(exact_output) != *plan.finalized_output().as_bytes() {
        return Err(WorkerV2PublicationIntentErrorV1::OutputDigestMismatch);
    }
    Ok(())
}

fn authorize(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<AttemptPhase, WorkerV2PublicationIntentErrorV1> {
    let phase = authorize_any_phase(output, producer, attempt)?;
    if !matches!(
        phase,
        AttemptPhase::Building | AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) {
        return Err(WorkerV2PublicationIntentErrorV1::Attempt {
            reason: "build attempt cannot recover a publication intent in its current phase"
                .to_string(),
        });
    }
    Ok(phase)
}

fn authorize_any_phase(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<AttemptPhase, WorkerV2PublicationIntentErrorV1> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(WorkerV2PublicationIntentErrorV1::Attempt {
            reason: "the direct compiler token cannot own a restart intent".to_string(),
        });
    }
    let attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|error| WorkerV2PublicationIntentErrorV1::Attempt {
            reason: error.to_string(),
        })?;
    if record.crate_name != producer.crate_name {
        return Err(WorkerV2PublicationIntentErrorV1::Attempt {
            reason: "build attempt crate name does not match the producer".to_string(),
        });
    }
    Ok(record.phase)
}

fn authorize_clear(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    intent: WorkerV2PublicationIntentRecordV1,
) -> Result<(), WorkerV2PublicationIntentErrorV1> {
    let attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|error| WorkerV2PublicationIntentErrorV1::Attempt {
            reason: error.to_string(),
        })?;
    let expected = publication_receipt(producer, attempt, intent.plan, intent.upstream_evidence);
    if !matches!(
        record.phase,
        AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) || record.backend_receipt != Some(BackendReceiptV1::Provenance(expected))
    {
        return Err(WorkerV2PublicationIntentErrorV1::Attempt {
            reason: "the exact backend provenance receipt is not durable".to_string(),
        });
    }
    Ok(())
}

fn recover_locked(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<Option<RecoveredWorkerV2PublicationIntentV1>, WorkerV2PublicationIntentErrorV1> {
    let canonical = entry_exists(output, &names.record)?;
    let redo = entry_exists(output, &names.redo)?;
    if canonical && redo {
        return Err(invalid(
            output,
            &names.record,
            "canonical and redo records coexist",
        ));
    }
    let entry = if canonical {
        &names.record
    } else if redo {
        &names.redo
    } else {
        return Ok(None);
    };
    let record = read_bound_record(output, names, entry, producer, attempt)?;
    let exact_output = read_output(output, names, &record)?;
    if redo {
        output.verify_path_identity()?;
        renameat(&output.fd, &names.redo, &output.fd, &names.record)
            .map_err(std::io::Error::from)?;
        fsync(&output.fd).map_err(std::io::Error::from)?;
    }
    Ok(Some(RecoveredWorkerV2PublicationIntentV1 {
        outcome: WorkerV2PublicationIntentOutcomeV1::Recovered,
        record,
        exact_output: Arc::from(exact_output),
    }))
}

fn persist_output(
    output: &PinnedOutput,
    names: &IntentNames,
    exact_output: &[u8],
    faults: &mut FaultInjector,
) -> Result<(), WorkerV2PublicationIntentErrorV1> {
    if entry_exists(output, &names.output)? {
        let actual = read_output_unbound(output, names, exact_output.len(), sha256(exact_output))?;
        if actual != exact_output {
            return Err(WorkerV2PublicationIntentErrorV1::ConflictingIntent);
        }
        return Ok(());
    }
    let (temp_name, mut temp) = create_temp(
        output,
        names,
        "output",
        WorkerV2PublicationIntentBoundaryV1::CreateOutputTemp,
        faults,
    )?;
    faults.around(WorkerV2PublicationIntentBoundaryV1::WriteOutputTemp, || {
        temp.write_all(exact_output).map_err(Into::into)
    })?;
    faults.around(WorkerV2PublicationIntentBoundaryV1::SyncOutputTemp, || {
        temp.sync_all().map_err(Into::into)
    })?;
    faults.hit(
        WorkerV2PublicationIntentBoundaryV1::RenameOutput,
        WorkerV2PublicationIntentFaultTimingV1::Before,
    )?;
    renameat_with(
        &output.fd,
        &temp_name,
        &output.fd,
        &names.output,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit(
        WorkerV2PublicationIntentBoundaryV1::RenameOutput,
        WorkerV2PublicationIntentFaultTimingV1::After,
    )?;
    validate_renamed_file(output, &names.output, &temp, exact_output.len())?;
    faults.around(WorkerV2PublicationIntentBoundaryV1::SyncOutputName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    Ok(())
}

fn persist_record(
    output: &PinnedOutput,
    names: &IntentNames,
    record: WorkerV2PublicationIntentRecordV1,
    faults: &mut FaultInjector,
) -> Result<(), WorkerV2PublicationIntentErrorV1> {
    let bytes = record.encode();
    let (temp_name, mut temp) = create_temp(
        output,
        names,
        "record",
        WorkerV2PublicationIntentBoundaryV1::CreateRecordTemp,
        faults,
    )?;
    faults.around(WorkerV2PublicationIntentBoundaryV1::WriteRecordTemp, || {
        temp.write_all(&bytes).map_err(Into::into)
    })?;
    faults.around(WorkerV2PublicationIntentBoundaryV1::SyncRecordTemp, || {
        temp.sync_all().map_err(Into::into)
    })?;
    faults.hit(
        WorkerV2PublicationIntentBoundaryV1::RenameRecordToRedo,
        WorkerV2PublicationIntentFaultTimingV1::Before,
    )?;
    renameat_with(
        &output.fd,
        &temp_name,
        &output.fd,
        &names.redo,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit(
        WorkerV2PublicationIntentBoundaryV1::RenameRecordToRedo,
        WorkerV2PublicationIntentFaultTimingV1::After,
    )?;
    validate_renamed_file(output, &names.redo, &temp, bytes.len())?;
    faults.around(WorkerV2PublicationIntentBoundaryV1::SyncRedoName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    faults.hit(
        WorkerV2PublicationIntentBoundaryV1::RenameRedoToCanonical,
        WorkerV2PublicationIntentFaultTimingV1::Before,
    )?;
    renameat(&output.fd, &names.redo, &output.fd, &names.record).map_err(std::io::Error::from)?;
    faults.hit(
        WorkerV2PublicationIntentBoundaryV1::RenameRedoToCanonical,
        WorkerV2PublicationIntentFaultTimingV1::After,
    )?;
    faults.around(
        WorkerV2PublicationIntentBoundaryV1::SyncCanonicalName,
        || {
            fsync(&output.fd)
                .map_err(std::io::Error::from)
                .map_err(Into::into)
        },
    )?;
    Ok(())
}

fn create_temp(
    output: &PinnedOutput,
    names: &IntentNames,
    purpose: &str,
    boundary: WorkerV2PublicationIntentBoundaryV1,
    faults: &mut FaultInjector,
) -> Result<(String, fs::File), WorkerV2PublicationIntentErrorV1> {
    faults.hit(boundary, WorkerV2PublicationIntentFaultTimingV1::Before)?;
    let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
    for offset in 0..MAX_TEMP_ATTEMPTS {
        let name = format!(
            "{}{purpose}-{}-{}",
            names.temp_prefix,
            std::process::id(),
            start.wrapping_add(offset)
        );
        match openat(
            &output.fd,
            &name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => {
                faults.hit(boundary, WorkerV2PublicationIntentFaultTimingV1::After)?;
                return Ok((name, fs::File::from(fd)));
            }
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Err(invalid(
        output,
        &names.temp_prefix,
        "could not reserve a private temporary entry",
    ))
}

fn cleanup_temps(
    output: &PinnedOutput,
    names: &IntentNames,
) -> Result<(), WorkerV2PublicationIntentErrorV1> {
    let directory = rustix::io::fcntl_dupfd_cloexec(&output.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = rustix::fs::Dir::read_from(&directory).map_err(std::io::Error::from)?;
    let mut temps = Vec::new();
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_string_lossy();
        if !name.starts_with(&names.temp_prefix) {
            continue;
        }
        if temps.len() == MAX_TEMP_ATTEMPTS as usize {
            return Err(invalid(
                output,
                &names.temp_prefix,
                "too many temporary entries",
            ));
        }
        let stat = statat(&output.fd, name.as_ref(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if !is_private_file(&stat) {
            return Err(invalid(
                output,
                name.as_ref(),
                "temporary entry is not private",
            ));
        }
        temps.push(name.into_owned());
    }
    if !temps.is_empty() {
        for name in temps {
            unlinkat(&output.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
    }
    Ok(())
}

fn read_bound_record(
    output: &PinnedOutput,
    names: &IntentNames,
    entry: &str,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<WorkerV2PublicationIntentRecordV1, WorkerV2PublicationIntentErrorV1> {
    let bytes = read_private_file(output, entry, RECORD_BYTES)?;
    let record = WorkerV2PublicationIntentRecordV1::decode(&bytes)
        .map_err(|reason| invalid(output, entry, reason))?;
    let expected_producer = producer_identity(producer);
    if record.producer_identity != expected_producer
        || record.slot != slot_identity(expected_producer, attempt)
        || record.attempt != attempt
        || record.plan.attempt() != attempt
        || names.base != IntentNames::new(expected_producer, record.slot).base
    {
        return Err(invalid(
            output,
            entry,
            "record binding does not match the requested attempt and producer",
        ));
    }
    Ok(record)
}

fn read_output(
    output: &PinnedOutput,
    names: &IntentNames,
    record: &WorkerV2PublicationIntentRecordV1,
) -> Result<Vec<u8>, WorkerV2PublicationIntentErrorV1> {
    read_output_unbound(
        output,
        names,
        record.output_length,
        *record.output_identity.as_bytes(),
    )
}

fn read_output_unbound(
    output: &PinnedOutput,
    names: &IntentNames,
    length: usize,
    identity: [u8; 32],
) -> Result<Vec<u8>, WorkerV2PublicationIntentErrorV1> {
    let bytes = read_private_file(output, &names.output, length)?;
    if sha256(&bytes) != identity {
        return Err(WorkerV2PublicationIntentErrorV1::OutputDigestMismatch);
    }
    Ok(bytes)
}

fn read_private_file(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
) -> Result<Vec<u8>, WorkerV2PublicationIntentErrorV1> {
    let fd = openat(
        &output.fd,
        entry,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| invalid(output, entry, std::io::Error::from(error).to_string()))?;
    let mut file = fs::File::from(fd);
    let before = fstat(&file).map_err(std::io::Error::from)?;
    if !is_private_file(&before) || usize::try_from(before.st_size).ok() != Some(exact_length) {
        return Err(invalid(
            output,
            entry,
            "expected a private single-link regular file with canonical length",
        ));
    }
    let mut bytes = Vec::with_capacity(exact_length);
    Read::by_ref(&mut file)
        .take((exact_length + 1) as u64)
        .read_to_end(&mut bytes)?;
    let after = fstat(&file).map_err(std::io::Error::from)?;
    let named =
        statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if bytes.len() != exact_length
        || !same_private_file(&before, &after, exact_length)
        || !same_private_file(&before, &named, exact_length)
    {
        return Err(invalid(
            output,
            entry,
            "file changed while its pinned descriptor was read",
        ));
    }
    Ok(bytes)
}

fn validate_renamed_file(
    output: &PinnedOutput,
    entry: &str,
    file: &fs::File,
    length: usize,
) -> Result<(), WorkerV2PublicationIntentErrorV1> {
    let pinned = fstat(file).map_err(std::io::Error::from)?;
    let named =
        statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !same_private_file(&pinned, &named, length) {
        return Err(invalid(
            output,
            entry,
            "renamed entry does not match its pinned descriptor",
        ));
    }
    Ok(())
}

fn entry_exists(
    output: &PinnedOutput,
    entry: &str,
) -> Result<bool, WorkerV2PublicationIntentErrorV1> {
    match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if !is_private_file(&stat) {
                return Err(invalid(
                    output,
                    entry,
                    "entry is not a private single-link regular file",
                ));
            }
            Ok(true)
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(false),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

fn producer_identity(producer: &ProducerIdentity) -> [u8; 32] {
    sha256_parts(&[
        PRODUCER_DOMAIN,
        &(producer.stable_source.len() as u64).to_le_bytes(),
        producer.stable_source.as_bytes(),
        &(producer.crate_name.len() as u64).to_le_bytes(),
        producer.crate_name.as_bytes(),
    ])
}

fn slot_identity(producer: [u8; 32], attempt: BuildAttempt) -> [u8; 32] {
    sha256_parts(&[
        SLOT_DOMAIN,
        &producer,
        &attempt.generation().to_le_bytes(),
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
    ])
}

fn push_scope(bytes: &mut Vec<u8>, scope: LinkPublicationScopeV1) {
    bytes.extend_from_slice(scope.package().as_bytes());
    bytes.extend_from_slice(scope.kernel_set().as_bytes());
    bytes.extend_from_slice(scope.target().as_bytes());
}

fn is_private_file(stat: &rustix::fs::Stat) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
        && stat.st_nlink == 1
        && stat.st_mode & 0o777 == 0o600
}

fn same_private_file(left: &rustix::fs::Stat, right: &rustix::fs::Stat, length: usize) -> bool {
    is_private_file(left)
        && is_private_file(right)
        && left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_size == length as i64
        && right.st_size == length as i64
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    encoded
}

fn invalid(
    output: &PinnedOutput,
    entry: &str,
    reason: impl Into<String>,
) -> WorkerV2PublicationIntentErrorV1 {
    WorkerV2PublicationIntentErrorV1::InvalidIntent {
        path: output.display_path.join(entry),
        reason: reason.into(),
    }
}

struct IntentNames {
    base: String,
    output: String,
    record: String,
    redo: String,
    temp_prefix: String,
}

impl IntentNames {
    fn new(producer: [u8; 32], slot: [u8; 32]) -> Self {
        let base = format!("{FILE_PREFIX}{}-{}", hex(&producer), hex(&slot));
        Self {
            output: format!("{base}{OUTPUT_SUFFIX}"),
            record: format!("{base}{RECORD_SUFFIX}"),
            redo: format!("{base}{REDO_SUFFIX}"),
            temp_prefix: format!("{base}{TEMP_SUFFIX}"),
            base,
        }
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or("record length overflow")?;
        let bytes = self.bytes.get(self.offset..end).ok_or("truncated record")?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], &'static str> {
        self.take(N)?.try_into().map_err(|_| "truncated record")
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

struct FaultInjector {
    point: Option<WorkerV2PublicationIntentFaultPointV1>,
    fired: bool,
}

impl FaultInjector {
    const fn new(point: Option<WorkerV2PublicationIntentFaultPointV1>) -> Self {
        Self {
            point,
            fired: false,
        }
    }

    fn hit(
        &mut self,
        boundary: WorkerV2PublicationIntentBoundaryV1,
        timing: WorkerV2PublicationIntentFaultTimingV1,
    ) -> Result<(), WorkerV2PublicationIntentErrorV1> {
        let point = WorkerV2PublicationIntentFaultPointV1 { boundary, timing };
        if !self.fired && self.point == Some(point) {
            self.fired = true;
            Err(WorkerV2PublicationIntentErrorV1::InjectedCrash { point })
        } else {
            Ok(())
        }
    }

    fn around(
        &mut self,
        boundary: WorkerV2PublicationIntentBoundaryV1,
        operation: impl FnOnce() -> Result<(), WorkerV2PublicationIntentErrorV1>,
    ) -> Result<(), WorkerV2PublicationIntentErrorV1> {
        self.hit(boundary, WorkerV2PublicationIntentFaultTimingV1::Before)?;
        operation()?;
        self.hit(boundary, WorkerV2PublicationIntentFaultTimingV1::After)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildInvocation, BuildSession};

    fn attempt() -> BuildAttempt {
        BuildAttempt::from_env_value(&format!(
            "7:{}:{}",
            BuildSession::from_bytes([0x11; 16]).to_hex(),
            BuildInvocation::from_bytes([0x22; 32]).to_hex()
        ))
        .unwrap()
    }

    fn plan(output: &[u8]) -> DurableLinkPublicationPlanV1 {
        DurableLinkPublicationPlanV1::new(
            attempt(),
            LinkPublicationScopeV1::new(
                PackageIdentityV1::from_bytes([1; 32]),
                KernelSetIdentityV1::from_bytes([2; 32]),
                TargetIdentityV1::from_bytes([3; 32]),
            ),
            CanonicalLinkRequestIdentityV1::from_bytes([4; 32]),
            PinnedWorkerIdentityV1::from_bytes([5; 32]),
            ValidatedResponseIdentityV1::from_bytes([6; 32]),
            LinkedOutputIdentityV1::from_bytes([7; 32]),
            FinalizationIdentityV1::from_bytes([8; 32]),
            FinalizedOutputIdentityV1::from_bytes(sha256(output)),
            AtomicPublicationIdentityV1::from_bytes([9; 32]),
        )
    }

    #[test]
    fn codec_is_fixed_size_canonical_checksummed_and_plan_bound() {
        let output = b"exact output";
        let producer =
            ProducerIdentity::from_codegen("kernel", Some(Path::new("/src/lib.rs"))).unwrap();
        let record = WorkerV2PublicationIntentRecordV1::new(
            &producer,
            attempt(),
            plan(output),
            UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0xaa; 32]),
            output.len(),
        );
        let encoded = record.encode();
        assert_eq!(MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES, 616);
        assert_eq!(encoded.len(), MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES);
        assert_eq!(
            WorkerV2PublicationIntentRecordV1::decode(&encoded).unwrap(),
            record
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(WorkerV2PublicationIntentRecordV1::decode(&trailing).is_err());
        assert!(WorkerV2PublicationIntentRecordV1::decode(&encoded[..encoded.len() - 1]).is_err());

        let mut bad_checksum = encoded.clone();
        bad_checksum[RECORD_MAGIC.len() + 4] ^= 1;
        assert_eq!(
            WorkerV2PublicationIntentRecordV1::decode(&bad_checksum),
            Err("record checksum mismatch")
        );

        let mut bad_plan = encoded;
        let plan_commitment_offset = RECORD_MAGIC.len() + 2 + 32 + 8 + 16 + 32 + 32 + 32;
        bad_plan[plan_commitment_offset] ^= 1;
        let body_len = bad_plan.len() - 32;
        let checksum = sha256_parts(&[RECORD_CHECKSUM_DOMAIN, &bad_plan[..body_len]]);
        bad_plan[body_len..].copy_from_slice(&checksum);
        assert_eq!(
            WorkerV2PublicationIntentRecordV1::decode(&bad_plan),
            Err("record plan commitment does not match its plan fields")
        );
    }
}
