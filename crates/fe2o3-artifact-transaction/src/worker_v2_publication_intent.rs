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
use crate::attempt_scoped_hsaco_publication::{
    backend_publication_receipt_attempt_identity_v2, publication_receipt, publication_receipt_v2,
};
use crate::{
    AtomicPublicationIdentityV1, BuildAttempt, BuildSession, CanonicalLinkRequestIdentityV1,
    DurableLinkPublicationPlanV1, EmitError, FinalizationIdentityV1, FinalizedOutputIdentityV1,
    KernelSetIdentityV1, LinkPublicationScopeV1, LinkedOutputIdentityV1,
    MAX_DURABLE_FINALIZED_ARTIFACT_BYTES, OutputLock, PackageIdentityV1, PinnedOutput,
    PinnedWorkerIdentityV1, ProducerIdentity, TargetIdentityV1,
    UpstreamCodeObjectEvidenceIdentityV1, ValidatedResponseIdentityV1, read_attempt_registry,
};
use fe2o3_build_authority::{CompilerClosureErrorV2, CompilerClosureV2};
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
    let expected = WorkerV2PublicationIntentRecordV1::new(
        producer,
        attempt,
        plan,
        upstream_evidence,
        exact_output.len(),
    );
    let recovered = persist_intent::<PublicationIntentSchemaV1>(
        output_dir,
        producer,
        attempt,
        expected,
        exact_output,
        options.injected_crash.map(engine_fault_point_v1),
    )?;
    Ok(RecoveredWorkerV2PublicationIntentV1 {
        outcome: if recovered.persisted {
            WorkerV2PublicationIntentOutcomeV1::Persisted
        } else {
            WorkerV2PublicationIntentOutcomeV1::Recovered
        },
        record: recovered.record,
        exact_output: recovered.exact_output,
    })
}

/// Recovers exact persisted inputs after a process restart.
pub fn recover_worker_v2_publication_intent_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<RecoveredWorkerV2PublicationIntentV1, WorkerV2PublicationIntentErrorV1> {
    let recovered = recover_intent::<PublicationIntentSchemaV1>(output_dir, producer, attempt, ())?;
    Ok(RecoveredWorkerV2PublicationIntentV1 {
        outcome: WorkerV2PublicationIntentOutcomeV1::Recovered,
        record: recovered.record,
        exact_output: recovered.exact_output,
    })
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
    clear_intent::<PublicationIntentSchemaV1>(
        output_dir,
        producer,
        attempt,
        (),
        identity.as_bytes(),
    )
}

trait PublicationIntentRecord: Copy + Eq {
    type Binding: Copy + Eq;

    fn slot(self) -> [u8; 32];
    fn attempt(self) -> BuildAttempt;
    fn producer_identity(self) -> [u8; 32];
    fn plan(self) -> DurableLinkPublicationPlanV1;
    fn output_identity(self) -> [u8; 32];
    fn output_length(self) -> usize;
    fn identity(self) -> [u8; 32];
    fn binding(self) -> Self::Binding;
}

trait PublicationIntentEngineError: Sized + From<EmitError> + From<std::io::Error> {
    fn plan_attempt_mismatch() -> Self;
    fn invalid_output_size(actual: usize, maximum: usize) -> Self;
    fn output_digest_mismatch() -> Self;
    fn attempt(reason: impl Into<String>) -> Self;
    fn not_found() -> Self;
    fn conflicting_intent() -> Self;
    fn binding_mismatch() -> Self;
    fn identity_mismatch() -> Self;
    fn invalid_intent(path: PathBuf, reason: impl Into<String>) -> Self;
    fn injected_crash(point: EngineFaultPoint) -> Self;
}

trait PublicationIntentSchema {
    type Record: PublicationIntentRecord;
    type Error: PublicationIntentEngineError;

    const PRODUCER_DOMAIN: &'static [u8];
    const SLOT_DOMAIN: &'static [u8];
    const FILE_PREFIX: &'static str;
    const RECORD_BYTES: usize;

    fn encode(record: Self::Record) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self::Record, &'static str>;
    fn has_exact_durable_receipt(
        receipt: Option<BackendReceiptV1>,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        intent: Self::Record,
    ) -> bool;
}

struct PublicationIntentSchemaV1;

impl PublicationIntentRecord for WorkerV2PublicationIntentRecordV1 {
    type Binding = ();

    fn slot(self) -> [u8; 32] {
        self.slot
    }

    fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    fn producer_identity(self) -> [u8; 32] {
        self.producer_identity
    }

    fn plan(self) -> DurableLinkPublicationPlanV1 {
        self.plan
    }

    fn output_identity(self) -> [u8; 32] {
        *self.output_identity.as_bytes()
    }

    fn output_length(self) -> usize {
        self.output_length
    }

    fn identity(self) -> [u8; 32] {
        self.identity.as_bytes()
    }

    fn binding(self) -> Self::Binding {}
}

impl PublicationIntentSchema for PublicationIntentSchemaV1 {
    type Record = WorkerV2PublicationIntentRecordV1;
    type Error = WorkerV2PublicationIntentErrorV1;

    const PRODUCER_DOMAIN: &'static [u8] = PRODUCER_DOMAIN;
    const SLOT_DOMAIN: &'static [u8] = SLOT_DOMAIN;
    const FILE_PREFIX: &'static str = FILE_PREFIX;
    const RECORD_BYTES: usize = RECORD_BYTES;

    fn encode(record: Self::Record) -> Vec<u8> {
        record.encode()
    }

    fn decode(bytes: &[u8]) -> Result<Self::Record, &'static str> {
        WorkerV2PublicationIntentRecordV1::decode(bytes)
    }

    fn has_exact_durable_receipt(
        receipt: Option<BackendReceiptV1>,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        intent: Self::Record,
    ) -> bool {
        let expected =
            publication_receipt(producer, attempt, intent.plan(), intent.upstream_evidence());
        receipt == Some(BackendReceiptV1::Provenance(expected))
    }
}

impl PublicationIntentEngineError for WorkerV2PublicationIntentErrorV1 {
    fn plan_attempt_mismatch() -> Self {
        Self::PlanAttemptMismatch
    }

    fn invalid_output_size(actual: usize, maximum: usize) -> Self {
        Self::InvalidOutputSize { actual, maximum }
    }

    fn output_digest_mismatch() -> Self {
        Self::OutputDigestMismatch
    }

    fn attempt(reason: impl Into<String>) -> Self {
        Self::Attempt {
            reason: reason.into(),
        }
    }

    fn not_found() -> Self {
        Self::NotFound
    }

    fn conflicting_intent() -> Self {
        Self::ConflictingIntent
    }

    fn binding_mismatch() -> Self {
        unreachable!("V1 has no closure binding")
    }

    fn identity_mismatch() -> Self {
        Self::IntentIdentityMismatch
    }

    fn invalid_intent(path: PathBuf, reason: impl Into<String>) -> Self {
        Self::InvalidIntent {
            path,
            reason: reason.into(),
        }
    }

    fn injected_crash(point: EngineFaultPoint) -> Self {
        Self::InjectedCrash {
            point: public_fault_point_v1(point),
        }
    }
}

struct RecoveredIntent<R> {
    persisted: bool,
    record: R,
    exact_output: Arc<[u8]>,
}

fn persist_intent<S: PublicationIntentSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    expected: S::Record,
    exact_output: &[u8],
    injected_crash: Option<EngineFaultPoint>,
) -> Result<RecoveredIntent<S::Record>, S::Error> {
    validate_inputs::<S>(attempt, expected, exact_output)?;
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    let authorization = authorize::<S>(&output, producer, attempt)?;
    let names = IntentNames::new::<S>(expected.producer_identity(), expected.slot());
    cleanup_temps::<S>(&output, &names)?;

    if let Some(recovered) = recover_locked::<S>(&output, &names, producer, attempt)? {
        if recovered.record != expected || recovered.exact_output.as_ref() != exact_output {
            return Err(S::Error::conflicting_intent());
        }
        return Ok(recovered);
    }
    if authorization != AttemptPhase::Building {
        return Err(S::Error::attempt(
            "a fresh intent may be created only before backend authority is claimed",
        ));
    }

    let mut faults = FaultInjector::new(injected_crash);
    persist_output::<S>(&output, &names, exact_output, &mut faults)?;
    persist_record::<S>(&output, &names, expected, &mut faults)?;
    let mut recovered = recover_locked::<S>(&output, &names, producer, attempt)?
        .ok_or_else(|| invalid::<S>(&output, &names.record, "record disappeared after commit"))?;
    if recovered.record != expected || recovered.exact_output.as_ref() != exact_output {
        return Err(S::Error::conflicting_intent());
    }
    recovered.persisted = true;
    Ok(recovered)
}

fn recover_intent<S: PublicationIntentSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    binding: <S::Record as PublicationIntentRecord>::Binding,
) -> Result<RecoveredIntent<S::Record>, S::Error> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize::<S>(&output, producer, attempt)?;
    let producer_identity = producer_identity_for::<S>(producer);
    let names = IntentNames::new::<S>(
        producer_identity,
        slot_identity_for::<S>(producer_identity, attempt),
    );
    cleanup_temps::<S>(&output, &names)?;
    let recovered =
        recover_locked::<S>(&output, &names, producer, attempt)?.ok_or_else(S::Error::not_found)?;
    if recovered.record.binding() != binding {
        return Err(S::Error::binding_mismatch());
    }
    Ok(recovered)
}

fn clear_intent<S: PublicationIntentSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    binding: <S::Record as PublicationIntentRecord>::Binding,
    identity: [u8; 32],
) -> Result<(), S::Error> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize_any_phase::<S>(&output, producer, attempt)?;
    let producer_identity = producer_identity_for::<S>(producer);
    let names = IntentNames::new::<S>(
        producer_identity,
        slot_identity_for::<S>(producer_identity, attempt),
    );
    let inspected = inspect_committed_locked::<S>(&output, &names, producer, attempt)?
        .ok_or_else(S::Error::not_found)?;
    let recovered = inspected.recovered;
    if recovered.record.binding() != binding {
        return Err(S::Error::binding_mismatch());
    }
    if recovered.record.identity() != identity {
        return Err(S::Error::identity_mismatch());
    }
    authorize_clear::<S>(&output, producer, attempt, recovered.record)?;
    cleanup_temps::<S>(&output, &names)?;
    unlinkat(&output.fd, inspected.entry.name(&names), AtFlags::empty())
        .map_err(std::io::Error::from)?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    unlinkat(&output.fd, &names.output, AtFlags::empty()).map_err(std::io::Error::from)?;
    fsync(&output.fd).map_err(std::io::Error::from)?;
    Ok(())
}

fn validate_inputs<S: PublicationIntentSchema>(
    attempt: BuildAttempt,
    record: S::Record,
    exact_output: &[u8],
) -> Result<(), S::Error> {
    if record.plan().attempt() != attempt {
        return Err(S::Error::plan_attempt_mismatch());
    }
    if attempt.session() == BuildSession::DIRECT {
        return Err(S::Error::attempt(
            "the direct compiler token cannot own a restart intent",
        ));
    }
    if exact_output.is_empty() || exact_output.len() > MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES
    {
        return Err(S::Error::invalid_output_size(
            exact_output.len(),
            MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES,
        ));
    }
    if sha256(exact_output) != record.output_identity() {
        return Err(S::Error::output_digest_mismatch());
    }
    Ok(())
}

fn authorize<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<AttemptPhase, S::Error> {
    let phase = authorize_any_phase::<S>(output, producer, attempt)?;
    if !matches!(
        phase,
        AttemptPhase::Building | AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) {
        return Err(S::Error::attempt(
            "build attempt cannot recover a publication intent in its current phase",
        ));
    }
    Ok(phase)
}

fn authorize_any_phase<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<AttemptPhase, S::Error> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(S::Error::attempt(
            "the direct compiler token cannot own a restart intent",
        ));
    }
    let attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|error| S::Error::attempt(error.to_string()))?;
    if record.crate_name != producer.crate_name {
        return Err(S::Error::attempt(
            "build attempt crate name does not match the producer",
        ));
    }
    Ok(record.phase)
}

fn authorize_clear<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    intent: S::Record,
) -> Result<(), S::Error> {
    let attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|error| S::Error::attempt(error.to_string()))?;
    if !matches!(
        record.phase,
        AttemptPhase::BackendClaimed | AttemptPhase::Completed
    ) || !S::has_exact_durable_receipt(record.backend_receipt, producer, attempt, intent)
    {
        return Err(S::Error::attempt(
            "the exact backend provenance receipt is not durable",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum CommittedIntentEntry {
    Canonical,
    Redo,
}

impl CommittedIntentEntry {
    fn name(self, names: &IntentNames) -> &str {
        match self {
            Self::Canonical => &names.record,
            Self::Redo => &names.redo,
        }
    }
}

struct InspectedCommittedIntent<R> {
    recovered: RecoveredIntent<R>,
    entry: CommittedIntentEntry,
}

fn inspect_committed_locked<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<Option<InspectedCommittedIntent<S::Record>>, S::Error> {
    let canonical = entry_exists::<S>(output, &names.record)?;
    let redo = entry_exists::<S>(output, &names.redo)?;
    if canonical && redo {
        return Err(invalid::<S>(
            output,
            &names.record,
            "canonical and redo records coexist",
        ));
    }
    let entry = if canonical {
        CommittedIntentEntry::Canonical
    } else if redo {
        CommittedIntentEntry::Redo
    } else {
        return Ok(None);
    };
    let record = read_bound_record::<S>(output, names, entry.name(names), producer, attempt)?;
    let exact_output = read_output::<S>(output, names, record)?;
    Ok(Some(InspectedCommittedIntent {
        recovered: RecoveredIntent {
            persisted: false,
            record,
            exact_output: Arc::from(exact_output),
        },
        entry,
    }))
}

fn recover_locked<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<Option<RecoveredIntent<S::Record>>, S::Error> {
    let canonical = entry_exists::<S>(output, &names.record)?;
    let redo = entry_exists::<S>(output, &names.redo)?;
    if canonical && redo {
        return Err(invalid::<S>(
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
    let record = read_bound_record::<S>(output, names, entry, producer, attempt)?;
    let exact_output = read_output::<S>(output, names, record)?;
    if redo {
        output.verify_path_identity()?;
        renameat(&output.fd, &names.redo, &output.fd, &names.record)
            .map_err(std::io::Error::from)?;
        fsync(&output.fd).map_err(std::io::Error::from)?;
    }
    Ok(Some(RecoveredIntent {
        persisted: false,
        record,
        exact_output: Arc::from(exact_output),
    }))
}

fn persist_output<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    exact_output: &[u8],
    faults: &mut FaultInjector,
) -> Result<(), S::Error> {
    if entry_exists::<S>(output, &names.output)? {
        let actual =
            read_output_unbound::<S>(output, names, exact_output.len(), sha256(exact_output))?;
        if actual != exact_output {
            return Err(S::Error::conflicting_intent());
        }
        return Ok(());
    }
    let (temp_name, mut temp) = create_temp::<S>(
        output,
        names,
        "output",
        EngineBoundary::CreateOutputTemp,
        faults,
    )?;
    faults.around::<S::Error>(EngineBoundary::WriteOutputTemp, || {
        temp.write_all(exact_output).map_err(Into::into)
    })?;
    faults.around::<S::Error>(EngineBoundary::SyncOutputTemp, || {
        temp.sync_all().map_err(Into::into)
    })?;
    faults.hit::<S::Error>(EngineBoundary::RenameOutput, EngineFaultTiming::Before)?;
    renameat_with(
        &output.fd,
        &temp_name,
        &output.fd,
        &names.output,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit::<S::Error>(EngineBoundary::RenameOutput, EngineFaultTiming::After)?;
    validate_renamed_file::<S>(output, &names.output, &temp, exact_output.len())?;
    faults.around::<S::Error>(EngineBoundary::SyncOutputName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    Ok(())
}

fn persist_record<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    record: S::Record,
    faults: &mut FaultInjector,
) -> Result<(), S::Error> {
    let bytes = S::encode(record);
    let (temp_name, mut temp) = create_temp::<S>(
        output,
        names,
        "record",
        EngineBoundary::CreateRecordTemp,
        faults,
    )?;
    faults.around::<S::Error>(EngineBoundary::WriteRecordTemp, || {
        temp.write_all(&bytes).map_err(Into::into)
    })?;
    faults.around::<S::Error>(EngineBoundary::SyncRecordTemp, || {
        temp.sync_all().map_err(Into::into)
    })?;
    faults.hit::<S::Error>(
        EngineBoundary::RenameRecordToRedo,
        EngineFaultTiming::Before,
    )?;
    renameat_with(
        &output.fd,
        &temp_name,
        &output.fd,
        &names.redo,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    faults.hit::<S::Error>(EngineBoundary::RenameRecordToRedo, EngineFaultTiming::After)?;
    validate_renamed_file::<S>(output, &names.redo, &temp, bytes.len())?;
    faults.around::<S::Error>(EngineBoundary::SyncRedoName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    faults.hit::<S::Error>(
        EngineBoundary::RenameRedoToCanonical,
        EngineFaultTiming::Before,
    )?;
    renameat(&output.fd, &names.redo, &output.fd, &names.record).map_err(std::io::Error::from)?;
    faults.hit::<S::Error>(
        EngineBoundary::RenameRedoToCanonical,
        EngineFaultTiming::After,
    )?;
    faults.around::<S::Error>(EngineBoundary::SyncCanonicalName, || {
        fsync(&output.fd)
            .map_err(std::io::Error::from)
            .map_err(Into::into)
    })?;
    Ok(())
}

fn create_temp<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    purpose: &str,
    boundary: EngineBoundary,
    faults: &mut FaultInjector,
) -> Result<(String, fs::File), S::Error> {
    faults.hit::<S::Error>(boundary, EngineFaultTiming::Before)?;
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
                faults.hit::<S::Error>(boundary, EngineFaultTiming::After)?;
                return Ok((name, fs::File::from(fd)));
            }
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Err(invalid::<S>(
        output,
        &names.temp_prefix,
        "could not reserve a private temporary entry",
    ))
}

fn cleanup_temps<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
) -> Result<(), S::Error> {
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
            return Err(invalid::<S>(
                output,
                &names.temp_prefix,
                "too many temporary entries",
            ));
        }
        let stat = statat(&output.fd, name.as_ref(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if !is_private_file(&stat) {
            return Err(invalid::<S>(
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

fn read_bound_record<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    entry: &str,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<S::Record, S::Error> {
    let bytes = read_private_file::<S>(output, entry, S::RECORD_BYTES)?;
    let record = S::decode(&bytes).map_err(|reason| invalid::<S>(output, entry, reason))?;
    let expected_producer = producer_identity_for::<S>(producer);
    if record.producer_identity() != expected_producer
        || record.slot() != slot_identity_for::<S>(expected_producer, attempt)
        || record.attempt() != attempt
        || record.plan().attempt() != attempt
        || names.base != IntentNames::new::<S>(expected_producer, record.slot()).base
    {
        return Err(invalid::<S>(
            output,
            entry,
            "record binding does not match the requested attempt and producer",
        ));
    }
    Ok(record)
}

fn read_output<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    record: S::Record,
) -> Result<Vec<u8>, S::Error> {
    read_output_unbound::<S>(
        output,
        names,
        record.output_length(),
        record.output_identity(),
    )
}

fn read_output_unbound<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    names: &IntentNames,
    length: usize,
    identity: [u8; 32],
) -> Result<Vec<u8>, S::Error> {
    let bytes = read_private_file::<S>(output, &names.output, length)?;
    if sha256(&bytes) != identity {
        return Err(S::Error::output_digest_mismatch());
    }
    Ok(bytes)
}

fn read_private_file<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    entry: &str,
    exact_length: usize,
) -> Result<Vec<u8>, S::Error> {
    let fd = openat(
        &output.fd,
        entry,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| invalid::<S>(output, entry, std::io::Error::from(error).to_string()))?;
    let mut file = fs::File::from(fd);
    let before = fstat(&file).map_err(std::io::Error::from)?;
    if !is_private_file(&before) || usize::try_from(before.st_size).ok() != Some(exact_length) {
        return Err(invalid::<S>(
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
        return Err(invalid::<S>(
            output,
            entry,
            "file changed while its pinned descriptor was read",
        ));
    }
    Ok(bytes)
}

fn validate_renamed_file<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    entry: &str,
    file: &fs::File,
    length: usize,
) -> Result<(), S::Error> {
    let pinned = fstat(file).map_err(std::io::Error::from)?;
    let named =
        statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !same_private_file(&pinned, &named, length) {
        return Err(invalid::<S>(
            output,
            entry,
            "renamed entry does not match its pinned descriptor",
        ));
    }
    Ok(())
}

fn entry_exists<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    entry: &str,
) -> Result<bool, S::Error> {
    match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if !is_private_file(&stat) {
                return Err(invalid::<S>(
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

fn producer_identity_for<S: PublicationIntentSchema>(producer: &ProducerIdentity) -> [u8; 32] {
    sha256_parts(&[
        S::PRODUCER_DOMAIN,
        &(producer.stable_source.len() as u64).to_le_bytes(),
        producer.stable_source.as_bytes(),
        &(producer.crate_name.len() as u64).to_le_bytes(),
        producer.crate_name.as_bytes(),
    ])
}

fn slot_identity_for<S: PublicationIntentSchema>(
    producer: [u8; 32],
    attempt: BuildAttempt,
) -> [u8; 32] {
    sha256_parts(&[
        S::SLOT_DOMAIN,
        &producer,
        &attempt.generation().to_le_bytes(),
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
    ])
}

fn producer_identity(producer: &ProducerIdentity) -> [u8; 32] {
    producer_identity_for::<PublicationIntentSchemaV1>(producer)
}

fn slot_identity(producer: [u8; 32], attempt: BuildAttempt) -> [u8; 32] {
    slot_identity_for::<PublicationIntentSchemaV1>(producer, attempt)
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

fn invalid<S: PublicationIntentSchema>(
    output: &PinnedOutput,
    entry: &str,
    reason: impl Into<String>,
) -> S::Error {
    S::Error::invalid_intent(output.display_path.join(entry), reason)
}

struct IntentNames {
    base: String,
    output: String,
    record: String,
    redo: String,
    temp_prefix: String,
}

impl IntentNames {
    fn new<S: PublicationIntentSchema>(producer: [u8; 32], slot: [u8; 32]) -> Self {
        let base = format!("{}{}-{}", S::FILE_PREFIX, hex(&producer), hex(&slot));
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EngineBoundary {
    CreateOutputTemp,
    WriteOutputTemp,
    SyncOutputTemp,
    RenameOutput,
    SyncOutputName,
    CreateRecordTemp,
    WriteRecordTemp,
    SyncRecordTemp,
    RenameRecordToRedo,
    SyncRedoName,
    RenameRedoToCanonical,
    SyncCanonicalName,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EngineFaultTiming {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EngineFaultPoint {
    boundary: EngineBoundary,
    timing: EngineFaultTiming,
}

fn engine_fault_point_v1(point: WorkerV2PublicationIntentFaultPointV1) -> EngineFaultPoint {
    EngineFaultPoint {
        boundary: match point.boundary {
            WorkerV2PublicationIntentBoundaryV1::CreateOutputTemp => {
                EngineBoundary::CreateOutputTemp
            }
            WorkerV2PublicationIntentBoundaryV1::WriteOutputTemp => EngineBoundary::WriteOutputTemp,
            WorkerV2PublicationIntentBoundaryV1::SyncOutputTemp => EngineBoundary::SyncOutputTemp,
            WorkerV2PublicationIntentBoundaryV1::RenameOutput => EngineBoundary::RenameOutput,
            WorkerV2PublicationIntentBoundaryV1::SyncOutputName => EngineBoundary::SyncOutputName,
            WorkerV2PublicationIntentBoundaryV1::CreateRecordTemp => {
                EngineBoundary::CreateRecordTemp
            }
            WorkerV2PublicationIntentBoundaryV1::WriteRecordTemp => EngineBoundary::WriteRecordTemp,
            WorkerV2PublicationIntentBoundaryV1::SyncRecordTemp => EngineBoundary::SyncRecordTemp,
            WorkerV2PublicationIntentBoundaryV1::RenameRecordToRedo => {
                EngineBoundary::RenameRecordToRedo
            }
            WorkerV2PublicationIntentBoundaryV1::SyncRedoName => EngineBoundary::SyncRedoName,
            WorkerV2PublicationIntentBoundaryV1::RenameRedoToCanonical => {
                EngineBoundary::RenameRedoToCanonical
            }
            WorkerV2PublicationIntentBoundaryV1::SyncCanonicalName => {
                EngineBoundary::SyncCanonicalName
            }
        },
        timing: match point.timing {
            WorkerV2PublicationIntentFaultTimingV1::Before => EngineFaultTiming::Before,
            WorkerV2PublicationIntentFaultTimingV1::After => EngineFaultTiming::After,
        },
    }
}

fn public_fault_point_v1(point: EngineFaultPoint) -> WorkerV2PublicationIntentFaultPointV1 {
    WorkerV2PublicationIntentFaultPointV1 {
        boundary: match point.boundary {
            EngineBoundary::CreateOutputTemp => {
                WorkerV2PublicationIntentBoundaryV1::CreateOutputTemp
            }
            EngineBoundary::WriteOutputTemp => WorkerV2PublicationIntentBoundaryV1::WriteOutputTemp,
            EngineBoundary::SyncOutputTemp => WorkerV2PublicationIntentBoundaryV1::SyncOutputTemp,
            EngineBoundary::RenameOutput => WorkerV2PublicationIntentBoundaryV1::RenameOutput,
            EngineBoundary::SyncOutputName => WorkerV2PublicationIntentBoundaryV1::SyncOutputName,
            EngineBoundary::CreateRecordTemp => {
                WorkerV2PublicationIntentBoundaryV1::CreateRecordTemp
            }
            EngineBoundary::WriteRecordTemp => WorkerV2PublicationIntentBoundaryV1::WriteRecordTemp,
            EngineBoundary::SyncRecordTemp => WorkerV2PublicationIntentBoundaryV1::SyncRecordTemp,
            EngineBoundary::RenameRecordToRedo => {
                WorkerV2PublicationIntentBoundaryV1::RenameRecordToRedo
            }
            EngineBoundary::SyncRedoName => WorkerV2PublicationIntentBoundaryV1::SyncRedoName,
            EngineBoundary::RenameRedoToCanonical => {
                WorkerV2PublicationIntentBoundaryV1::RenameRedoToCanonical
            }
            EngineBoundary::SyncCanonicalName => {
                WorkerV2PublicationIntentBoundaryV1::SyncCanonicalName
            }
        },
        timing: match point.timing {
            EngineFaultTiming::Before => WorkerV2PublicationIntentFaultTimingV1::Before,
            EngineFaultTiming::After => WorkerV2PublicationIntentFaultTimingV1::After,
        },
    }
}

struct FaultInjector {
    point: Option<EngineFaultPoint>,
    fired: bool,
}

impl FaultInjector {
    const fn new(point: Option<EngineFaultPoint>) -> Self {
        Self {
            point,
            fired: false,
        }
    }

    fn hit<E: PublicationIntentEngineError>(
        &mut self,
        boundary: EngineBoundary,
        timing: EngineFaultTiming,
    ) -> Result<(), E> {
        let point = EngineFaultPoint { boundary, timing };
        if !self.fired && self.point == Some(point) {
            self.fired = true;
            Err(E::injected_crash(point))
        } else {
            Ok(())
        }
    }

    fn around<E: PublicationIntentEngineError>(
        &mut self,
        boundary: EngineBoundary,
        operation: impl FnOnce() -> Result<(), E>,
    ) -> Result<(), E> {
        self.hit::<E>(boundary, EngineFaultTiming::Before)?;
        operation()?;
        self.hit::<E>(boundary, EngineFaultTiming::After)
    }
}

mod publication_intent_v2 {
    use super::*;

    const RECORD_MAGIC_V2: &[u8] = b"FE2O3-WORKER-V2-PUBLICATION-INTENT-V2\0";
    const RECORD_VERSION_V2: u16 = 2;
    const PRODUCER_DOMAIN_V2: &[u8] = b"fe2o3.worker-v2-publication-intent.producer.v2\0";
    const SLOT_DOMAIN_V2: &[u8] = b"fe2o3.worker-v2-publication-intent.slot.v2\0";
    const RECORD_CHECKSUM_DOMAIN_V2: &[u8] =
        b"fe2o3.worker-v2-publication-intent.record-checksum.v2\0";
    const RECORD_IDENTITY_DOMAIN_V2: &[u8] =
        b"fe2o3.worker-v2-publication-intent.record-identity.v2\0";
    const FILE_PREFIX_V2: &str = ".fe2o3-worker-v2-publication-intent-v2-";

    // Six compiler pins, transition protocol, and aggregate identity.
    const COMPILER_CLOSURE_BYTES_V2: usize = (6 * 32) + 2 + 32;

    // V1 fields under independent V2 domains, followed by the complete compiler-closure preimage.
    const RECORD_BYTES_V2: usize = RECORD_MAGIC_V2.len()
        + 2
        + 32
        + 8
        + 16
        + 32
        + 32
        + 32
        + 32
        + (3 * 32)
        + (7 * 32)
        + 32
        + 8
        + COMPILER_CLOSURE_BYTES_V2
        + 32;

    /// Exact canonical size of a V2 publication-intent record.
    pub const MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES_V2: usize = RECORD_BYTES_V2;

    /// Maximum exact Worker V2 output retained by one V2 publication intent.
    pub const MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES_V2: usize =
        MAX_DURABLE_FINALIZED_ARTIFACT_BYTES;

    fn encode_compiler_closure_v2(closure: CompilerClosureV2, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&closure.cargo_executable_sha256());
        bytes.extend_from_slice(&closure.cargo_binding_trampoline_sha256());
        bytes.extend_from_slice(&closure.cargo_fe2o3_binding_wrapper_sha256());
        bytes.extend_from_slice(&closure.rustc_executable_sha256());
        bytes.extend_from_slice(&closure.rustc_runtime_tree_sha256());
        bytes.extend_from_slice(&closure.codegen_backend_sha256());
        bytes.extend_from_slice(
            &closure
                .cargo_binding_transition_protocol_version()
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&closure.identity_sha256());
    }

    fn decode_compiler_closure_v2(
        decoder: &mut Decoder<'_>,
    ) -> Result<CompilerClosureV2, &'static str> {
        CompilerClosureV2::from_pins_and_identity(
            decoder.array()?,
            decoder.array()?,
            decoder.array()?,
            decoder.array()?,
            decoder.array()?,
            decoder.array()?,
            decoder.u16()?,
            decoder.array()?,
        )
        .map_err(|error| match error {
            CompilerClosureErrorV2::ZeroDigest { .. } => {
                "record compiler closure contains a zero digest"
            }
            CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion { .. } => {
                "record compiler closure uses an unsupported transition protocol version"
            }
            CompilerClosureErrorV2::IdentityMismatch => {
                "record compiler closure identity does not match its role-specific pins"
            }
            _ => "record compiler closure is not supported by this protocol",
        })
    }

    /// SHA-256 identity of one complete canonical V2 publication-intent record.
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct WorkerV2PublicationIntentIdentityV2([u8; 32]);

    impl WorkerV2PublicationIntentIdentityV2 {
        /// Constructs an identity from its exact 256-bit representation.
        pub const fn from_bytes(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }

        /// Returns the exact 256-bit representation.
        pub const fn as_bytes(self) -> [u8; 32] {
            self.0
        }
    }

    /// Canonical identities and compiler closure required to retry one exact publication.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct WorkerV2PublicationIntentRecordV2 {
        slot: [u8; 32],
        attempt: BuildAttempt,
        producer_identity: [u8; 32],
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        plan: DurableLinkPublicationPlanV1,
        output_identity: FinalizedOutputIdentityV1,
        output_length: usize,
        compiler_closure: CompilerClosureV2,
        identity: WorkerV2PublicationIntentIdentityV2,
    }

    impl WorkerV2PublicationIntentRecordV2 {
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

        /// Complete canonical compiler-closure preimage bound into this intent.
        pub const fn compiler_closure(self) -> CompilerClosureV2 {
            self.compiler_closure
        }

        /// Identity of the complete V2 checksummed canonical record.
        pub const fn identity(self) -> WorkerV2PublicationIntentIdentityV2 {
            self.identity
        }

        /// An intent remains inert until the attempt-scoped publication API authorizes it.
        pub const fn grants_publication_authority(self) -> bool {
            false
        }

        /// A retained compiler closure is evidence only and does not authenticate authorship.
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
            compiler_closure: CompilerClosureV2,
        ) -> Self {
            let producer_identity = producer_identity_v2(producer);
            let output_identity = plan.finalized_output();
            let mut record = Self {
                slot: slot_identity_v2(producer_identity, attempt),
                attempt,
                producer_identity,
                upstream_evidence,
                plan,
                output_identity,
                output_length,
                compiler_closure,
                identity: WorkerV2PublicationIntentIdentityV2([0; 32]),
            };
            record.identity = record.encoded_identity();
            record
        }

        fn encode(self) -> Vec<u8> {
            let mut bytes = self.encode_body();
            let checksum = sha256_parts(&[RECORD_CHECKSUM_DOMAIN_V2, &bytes]);
            bytes.extend_from_slice(&checksum);
            debug_assert_eq!(bytes.len(), RECORD_BYTES_V2);
            bytes
        }

        fn encode_body(self) -> Vec<u8> {
            let mut bytes = Vec::with_capacity(RECORD_BYTES_V2 - 32);
            bytes.extend_from_slice(RECORD_MAGIC_V2);
            bytes.extend_from_slice(&RECORD_VERSION_V2.to_le_bytes());
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
            encode_compiler_closure_v2(self.compiler_closure, &mut bytes);
            bytes
        }

        fn encoded_identity(self) -> WorkerV2PublicationIntentIdentityV2 {
            WorkerV2PublicationIntentIdentityV2(sha256_parts(&[
                RECORD_IDENTITY_DOMAIN_V2,
                &self.encode(),
            ]))
        }

        fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
            if bytes.len() != RECORD_BYTES_V2 {
                return Err("record has a noncanonical V2 length");
            }
            let (body, checksum) = bytes.split_at(bytes.len() - 32);
            if sha256_parts(&[RECORD_CHECKSUM_DOMAIN_V2, body]).as_slice() != checksum {
                return Err("record checksum mismatch");
            }
            let mut decoder = Decoder::new(body);
            if decoder.take(RECORD_MAGIC_V2.len())? != RECORD_MAGIC_V2 {
                return Err("record magic mismatch");
            }
            if decoder.u16()? != RECORD_VERSION_V2 {
                return Err("unsupported V2 record version");
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
            let upstream_evidence =
                UpstreamCodeObjectEvidenceIdentityV1::from_bytes(decoder.array()?);
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
            let compiler_closure = decode_compiler_closure_v2(&mut decoder)?;
            if !decoder.finished() {
                return Err("record has trailing body bytes");
            }
            if output_length == 0
                || output_length > MAX_WORKER_V2_PUBLICATION_INTENT_OUTPUT_BYTES_V2
            {
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
                compiler_closure,
                identity: WorkerV2PublicationIntentIdentityV2([0; 32]),
            };
            record.identity = WorkerV2PublicationIntentIdentityV2(sha256_parts(&[
                RECORD_IDENTITY_DOMAIN_V2,
                bytes,
            ]));
            Ok(record)
        }
    }

    /// Whether V2 persistence created a new intent or reconciled an existing exact intent.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum WorkerV2PublicationIntentOutcomeV2 {
        /// This call committed a new canonical V2 intent.
        Persisted,
        /// This call recovered an exact V2 intent committed by an earlier process.
        Recovered,
    }

    /// Immutable V2 restart input validated under the shared artifact-store lock.
    #[derive(Clone, Debug)]
    pub struct RecoveredWorkerV2PublicationIntentV2 {
        outcome: WorkerV2PublicationIntentOutcomeV2,
        record: WorkerV2PublicationIntentRecordV2,
        exact_output: Arc<[u8]>,
    }

    impl RecoveredWorkerV2PublicationIntentV2 {
        /// Reports whether this call committed or recovered the intent.
        pub const fn outcome(&self) -> WorkerV2PublicationIntentOutcomeV2 {
            self.outcome
        }

        /// Returns the validated canonical V2 identity record.
        pub const fn record(&self) -> WorkerV2PublicationIntentRecordV2 {
            self.record
        }

        /// Returns the complete compiler closure carried by the validated record.
        pub const fn compiler_closure(&self) -> CompilerClosureV2 {
            self.record.compiler_closure
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

        /// Retained compiler evidence does not grant compiler authority.
        pub const fn grants_compiler_authority(&self) -> bool {
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

    /// Durable V2 operation at which a test may simulate abrupt process termination.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum WorkerV2PublicationIntentBoundaryV2 {
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

    /// Whether V2 fault injection interrupts before or after one durable operation.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum WorkerV2PublicationIntentFaultTimingV2 {
        /// Interrupt before the selected operation.
        Before,
        /// Interrupt after the selected operation.
        After,
    }

    /// Exact deterministic V2 crash point used by persistence tests.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct WorkerV2PublicationIntentFaultPointV2 {
        /// Durable operation to interrupt.
        pub boundary: WorkerV2PublicationIntentBoundaryV2,
        /// Side of the operation on which to interrupt.
        pub timing: WorkerV2PublicationIntentFaultTimingV2,
    }

    /// V2 fault-injection options. Production callers use [`Default::default`].
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
    pub struct WorkerV2PublicationIntentOptionsV2 {
        injected_crash: Option<WorkerV2PublicationIntentFaultPointV2>,
    }

    impl WorkerV2PublicationIntentOptionsV2 {
        /// Simulates abrupt process termination at one exact V2 persistence boundary.
        pub const fn inject_crash(point: WorkerV2PublicationIntentFaultPointV2) -> Self {
            Self {
                injected_crash: Some(point),
            }
        }
    }

    /// Failure to persist, recover, or clear exact V2 restart inputs.
    #[derive(Debug)]
    #[non_exhaustive]
    pub enum WorkerV2PublicationIntentErrorV2 {
        /// The shared artifact store rejected an operation or changed identity.
        Store(EmitError),
        /// A descriptor-relative filesystem operation failed.
        Io(std::io::Error),
        /// The publication plan names a different build attempt.
        PlanAttemptMismatch,
        /// The exact output is empty or exceeds the protocol bound.
        InvalidOutputSize {
            /// Supplied output length.
            actual: usize,
            /// Largest supported output length.
            maximum: usize,
        },
        /// Exact output bytes do not match the plan's finalized-output identity.
        OutputDigestMismatch,
        /// The build attempt cannot create, recover, or remove this intent.
        Attempt {
            /// Stable diagnostic for the rejected attempt state.
            reason: String,
        },
        /// No committed canonical or replayable V2 intent exists.
        NotFound,
        /// Different exact inputs are already committed for the V2 slot.
        ConflictingIntent,
        /// The supplied compiler closure differs from the committed preimage.
        CompilerClosureMismatch,
        /// Cleanup named a different canonical V2 record identity.
        IntentIdentityMismatch,
        /// Persisted V2 state is noncanonical, corrupt, substituted, or unsafe to follow.
        InvalidIntent {
            /// Rejected record or output path.
            path: PathBuf,
            /// Stable diagnostic for the invalid persisted state.
            reason: String,
        },
        /// Deterministic crash-like interruption requested by test options.
        InjectedCrash {
            /// Exact interrupted durability boundary.
            point: WorkerV2PublicationIntentFaultPointV2,
        },
    }

    impl fmt::Display for WorkerV2PublicationIntentErrorV2 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Store(error) => write!(
                    formatter,
                    "artifact store rejected V2 publication intent: {error}"
                ),
                Self::Io(error) => write!(formatter, "{error}"),
                Self::PlanAttemptMismatch => formatter.write_str(
                    "Worker V2 publication plan does not match the supplied build attempt",
                ),
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
                Self::NotFound => formatter.write_str("V2 publication intent was not found"),
                Self::ConflictingIntent => {
                    formatter.write_str("a different V2 publication intent is already committed")
                }
                Self::CompilerClosureMismatch => formatter.write_str(
                    "compiler closure does not match the canonical V2 publication intent",
                ),
                Self::IntentIdentityMismatch => formatter.write_str(
                    "Worker V2 publication-intent identity does not match the committed V2 record",
                ),
                Self::InvalidIntent { path, reason } => write!(
                    formatter,
                    "invalid V2 publication intent {}: {reason}",
                    path.display()
                ),
                Self::InjectedCrash { point } => write!(
                    formatter,
                    "injected V2 publication-intent crash at {point:?}"
                ),
            }
        }
    }

    impl std::error::Error for WorkerV2PublicationIntentErrorV2 {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Store(error) => Some(error),
                Self::Io(error) => Some(error),
                _ => None,
            }
        }
    }

    impl From<EmitError> for WorkerV2PublicationIntentErrorV2 {
        fn from(error: EmitError) -> Self {
            Self::Store(error)
        }
    }

    impl From<std::io::Error> for WorkerV2PublicationIntentErrorV2 {
        fn from(error: std::io::Error) -> Self {
            Self::Io(error)
        }
    }

    struct PublicationIntentSchemaV2;

    impl PublicationIntentRecord for WorkerV2PublicationIntentRecordV2 {
        type Binding = CompilerClosureV2;

        fn slot(self) -> [u8; 32] {
            self.slot
        }

        fn attempt(self) -> BuildAttempt {
            self.attempt
        }

        fn producer_identity(self) -> [u8; 32] {
            self.producer_identity
        }

        fn plan(self) -> DurableLinkPublicationPlanV1 {
            self.plan
        }

        fn output_identity(self) -> [u8; 32] {
            *self.output_identity.as_bytes()
        }

        fn output_length(self) -> usize {
            self.output_length
        }

        fn identity(self) -> [u8; 32] {
            self.identity.as_bytes()
        }

        fn binding(self) -> Self::Binding {
            self.compiler_closure
        }
    }

    impl PublicationIntentSchema for PublicationIntentSchemaV2 {
        type Record = WorkerV2PublicationIntentRecordV2;
        type Error = WorkerV2PublicationIntentErrorV2;

        const PRODUCER_DOMAIN: &'static [u8] = PRODUCER_DOMAIN_V2;
        const SLOT_DOMAIN: &'static [u8] = SLOT_DOMAIN_V2;
        const FILE_PREFIX: &'static str = FILE_PREFIX_V2;
        const RECORD_BYTES: usize = RECORD_BYTES_V2;

        fn encode(record: Self::Record) -> Vec<u8> {
            record.encode()
        }

        fn decode(bytes: &[u8]) -> Result<Self::Record, &'static str> {
            WorkerV2PublicationIntentRecordV2::decode(bytes)
        }

        fn has_exact_durable_receipt(
            receipt: Option<BackendReceiptV1>,
            producer: &ProducerIdentity,
            attempt: BuildAttempt,
            intent: Self::Record,
        ) -> bool {
            let expected = publication_receipt_v2(
                producer,
                attempt,
                intent.plan(),
                intent.upstream_evidence(),
                intent.compiler_closure(),
            );
            receipt == Some(BackendReceiptV1::ProvenanceV2(expected))
        }
    }

    impl PublicationIntentEngineError for WorkerV2PublicationIntentErrorV2 {
        fn plan_attempt_mismatch() -> Self {
            Self::PlanAttemptMismatch
        }

        fn invalid_output_size(actual: usize, maximum: usize) -> Self {
            Self::InvalidOutputSize { actual, maximum }
        }

        fn output_digest_mismatch() -> Self {
            Self::OutputDigestMismatch
        }

        fn attempt(reason: impl Into<String>) -> Self {
            Self::Attempt {
                reason: reason.into(),
            }
        }

        fn not_found() -> Self {
            Self::NotFound
        }

        fn conflicting_intent() -> Self {
            Self::ConflictingIntent
        }

        fn binding_mismatch() -> Self {
            Self::CompilerClosureMismatch
        }

        fn identity_mismatch() -> Self {
            Self::IntentIdentityMismatch
        }

        fn invalid_intent(path: PathBuf, reason: impl Into<String>) -> Self {
            Self::InvalidIntent {
                path,
                reason: reason.into(),
            }
        }

        fn injected_crash(point: EngineFaultPoint) -> Self {
            Self::InjectedCrash {
                point: public_fault_point_v2(point),
            }
        }
    }

    /// Persists exact V2 restart input before the attempt-scoped backend claim is consumed.
    #[allow(clippy::too_many_arguments)]
    pub fn persist_worker_v2_publication_intent_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        compiler_closure: CompilerClosureV2,
        exact_output: &[u8],
    ) -> Result<RecoveredWorkerV2PublicationIntentV2, WorkerV2PublicationIntentErrorV2> {
        persist_worker_v2_publication_intent_v2_with_options(
            output_dir,
            producer,
            attempt,
            plan,
            upstream_evidence,
            compiler_closure,
            exact_output,
            WorkerV2PublicationIntentOptionsV2::default(),
        )
    }

    /// Fault-injectable form of [`persist_worker_v2_publication_intent_v2`].
    #[allow(clippy::too_many_arguments)]
    pub fn persist_worker_v2_publication_intent_v2_with_options(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        plan: DurableLinkPublicationPlanV1,
        upstream_evidence: UpstreamCodeObjectEvidenceIdentityV1,
        compiler_closure: CompilerClosureV2,
        exact_output: &[u8],
        options: WorkerV2PublicationIntentOptionsV2,
    ) -> Result<RecoveredWorkerV2PublicationIntentV2, WorkerV2PublicationIntentErrorV2> {
        let expected = WorkerV2PublicationIntentRecordV2::new(
            producer,
            attempt,
            plan,
            upstream_evidence,
            exact_output.len(),
            compiler_closure,
        );
        let recovered = persist_intent::<PublicationIntentSchemaV2>(
            output_dir,
            producer,
            attempt,
            expected,
            exact_output,
            options.injected_crash.map(engine_fault_point_v2),
        )?;
        Ok(RecoveredWorkerV2PublicationIntentV2 {
            outcome: if recovered.persisted {
                WorkerV2PublicationIntentOutcomeV2::Persisted
            } else {
                WorkerV2PublicationIntentOutcomeV2::Recovered
            },
            record: recovered.record,
            exact_output: recovered.exact_output,
        })
    }

    /// Recovers exact persisted V2 inputs without consulting or upgrading a V1 record.
    pub fn recover_worker_v2_publication_intent_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: CompilerClosureV2,
    ) -> Result<RecoveredWorkerV2PublicationIntentV2, WorkerV2PublicationIntentErrorV2> {
        let recovered = recover_intent::<PublicationIntentSchemaV2>(
            output_dir,
            producer,
            attempt,
            compiler_closure,
        )?;
        Ok(RecoveredWorkerV2PublicationIntentV2 {
            outcome: WorkerV2PublicationIntentOutcomeV2::Recovered,
            record: recovered.record,
            exact_output: recovered.exact_output,
        })
    }

    /// Removes one exact committed V2 intent after its publication receipt is durable.
    pub fn clear_worker_v2_publication_intent_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: CompilerClosureV2,
        identity: WorkerV2PublicationIntentIdentityV2,
    ) -> Result<(), WorkerV2PublicationIntentErrorV2> {
        clear_intent::<PublicationIntentSchemaV2>(
            output_dir,
            producer,
            attempt,
            compiler_closure,
            identity.as_bytes(),
        )
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct WorkerV2PublicationIntentLeaseFileV2 {
        device: u64,
        inode: u64,
    }

    impl WorkerV2PublicationIntentLeaseFileV2 {
        fn capture(
            output: &PinnedOutput,
            entry: &str,
            exact_length: usize,
        ) -> Result<Self, WorkerV2PublicationIntentErrorV2> {
            let stat = statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)?;
            if !is_private_file(&stat) || usize::try_from(stat.st_size).ok() != Some(exact_length) {
                return Err(invalid::<PublicationIntentSchemaV2>(
                    output,
                    entry,
                    "lease entry is not a private file with its canonical length",
                ));
            }
            Ok(Self {
                device: stat.st_dev,
                inode: stat.st_ino,
            })
        }
    }

    /// Exclusive local intent lease retained across a caller's related durable publication.
    ///
    /// The lease serializes cooperating fe2o3 writers. It also detects record, output, and output
    /// directory replacement when [`Self::revalidate`] is called. It does not constrain same-UID
    /// code that ignores the artifact-store lock; see the crate-level filesystem concurrency
    /// contract.
    pub struct WorkerV2PublicationIntentLeaseV2 {
        _lock: OutputLock,
        output: PinnedOutput,
        producer: ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: CompilerClosureV2,
        recovered: RecoveredWorkerV2PublicationIntentV2,
        record_file: WorkerV2PublicationIntentLeaseFileV2,
        output_file: WorkerV2PublicationIntentLeaseFileV2,
    }

    impl WorkerV2PublicationIntentLeaseV2 {
        /// Returns the exact intent snapshot validated while this lease was acquired.
        pub const fn recovered(&self) -> &RecoveredWorkerV2PublicationIntentV2 {
            &self.recovered
        }

        /// Revalidates the exact intent and output while retaining the same artifact lock.
        pub fn revalidate(&self) -> Result<(), WorkerV2PublicationIntentErrorV2> {
            self.output.verify_path_identity()?;
            authorize::<PublicationIntentSchemaV2>(&self.output, &self.producer, self.attempt)?;
            let producer_identity = producer_identity_v2(&self.producer);
            let names = IntentNames::new::<PublicationIntentSchemaV2>(
                producer_identity,
                slot_identity_v2(producer_identity, self.attempt),
            );
            cleanup_temps::<PublicationIntentSchemaV2>(&self.output, &names)?;
            let recovered = recover_locked::<PublicationIntentSchemaV2>(
                &self.output,
                &names,
                &self.producer,
                self.attempt,
            )?
            .ok_or(WorkerV2PublicationIntentErrorV2::NotFound)?;
            if recovered.record.compiler_closure() != self.compiler_closure {
                return Err(WorkerV2PublicationIntentErrorV2::CompilerClosureMismatch);
            }
            if recovered.record != self.recovered.record
                || recovered.exact_output.as_ref() != self.recovered.exact_output.as_ref()
            {
                return Err(WorkerV2PublicationIntentErrorV2::ConflictingIntent);
            }
            let record_file = WorkerV2PublicationIntentLeaseFileV2::capture(
                &self.output,
                &names.record,
                RECORD_BYTES_V2,
            )?;
            let output_file = WorkerV2PublicationIntentLeaseFileV2::capture(
                &self.output,
                &names.output,
                recovered.record.output_length(),
            )?;
            if record_file != self.record_file || output_file != self.output_file {
                return Err(WorkerV2PublicationIntentErrorV2::ConflictingIntent);
            }
            self.output.verify_path_identity()?;
            Ok(())
        }

        /// A local lease coordinates mutation but grants no publication authority.
        pub const fn grants_publication_authority(&self) -> bool {
            false
        }

        /// A local lease grants no load authority.
        pub const fn grants_load_authority(&self) -> bool {
            false
        }
    }

    /// Acquires the exact local V2 intent lease until the returned guard is dropped.
    pub fn acquire_worker_v2_publication_intent_lease_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: CompilerClosureV2,
        identity: WorkerV2PublicationIntentIdentityV2,
    ) -> Result<WorkerV2PublicationIntentLeaseV2, WorkerV2PublicationIntentErrorV2> {
        let output = PinnedOutput::open_existing(output_dir)?;
        let lock = output.lock()?;
        output.verify_path_identity()?;
        authorize::<PublicationIntentSchemaV2>(&output, producer, attempt)?;
        let producer_identity = producer_identity_v2(producer);
        let names = IntentNames::new::<PublicationIntentSchemaV2>(
            producer_identity,
            slot_identity_v2(producer_identity, attempt),
        );
        cleanup_temps::<PublicationIntentSchemaV2>(&output, &names)?;
        let recovered =
            recover_locked::<PublicationIntentSchemaV2>(&output, &names, producer, attempt)?
                .ok_or(WorkerV2PublicationIntentErrorV2::NotFound)?;
        if recovered.record.compiler_closure() != compiler_closure {
            return Err(WorkerV2PublicationIntentErrorV2::CompilerClosureMismatch);
        }
        if recovered.record.identity() != identity {
            return Err(WorkerV2PublicationIntentErrorV2::IntentIdentityMismatch);
        }
        let record_file =
            WorkerV2PublicationIntentLeaseFileV2::capture(&output, &names.record, RECORD_BYTES_V2)?;
        let output_file = WorkerV2PublicationIntentLeaseFileV2::capture(
            &output,
            &names.output,
            recovered.record.output_length(),
        )?;
        output.verify_path_identity()?;
        Ok(WorkerV2PublicationIntentLeaseV2 {
            _lock: lock,
            output,
            producer: producer.clone(),
            attempt,
            compiler_closure,
            recovered: RecoveredWorkerV2PublicationIntentV2 {
                outcome: WorkerV2PublicationIntentOutcomeV2::Recovered,
                record: recovered.record,
                exact_output: recovered.exact_output,
            },
            record_file,
            output_file,
        })
    }

    fn producer_identity_v2(producer: &ProducerIdentity) -> [u8; 32] {
        producer_identity_for::<PublicationIntentSchemaV2>(producer)
    }

    fn slot_identity_v2(producer: [u8; 32], attempt: BuildAttempt) -> [u8; 32] {
        slot_identity_for::<PublicationIntentSchemaV2>(producer, attempt)
    }

    fn engine_fault_point_v2(point: WorkerV2PublicationIntentFaultPointV2) -> EngineFaultPoint {
        EngineFaultPoint {
            boundary: match point.boundary {
                WorkerV2PublicationIntentBoundaryV2::CreateOutputTemp => {
                    EngineBoundary::CreateOutputTemp
                }
                WorkerV2PublicationIntentBoundaryV2::WriteOutputTemp => {
                    EngineBoundary::WriteOutputTemp
                }
                WorkerV2PublicationIntentBoundaryV2::SyncOutputTemp => {
                    EngineBoundary::SyncOutputTemp
                }
                WorkerV2PublicationIntentBoundaryV2::RenameOutput => EngineBoundary::RenameOutput,
                WorkerV2PublicationIntentBoundaryV2::SyncOutputName => {
                    EngineBoundary::SyncOutputName
                }
                WorkerV2PublicationIntentBoundaryV2::CreateRecordTemp => {
                    EngineBoundary::CreateRecordTemp
                }
                WorkerV2PublicationIntentBoundaryV2::WriteRecordTemp => {
                    EngineBoundary::WriteRecordTemp
                }
                WorkerV2PublicationIntentBoundaryV2::SyncRecordTemp => {
                    EngineBoundary::SyncRecordTemp
                }
                WorkerV2PublicationIntentBoundaryV2::RenameRecordToRedo => {
                    EngineBoundary::RenameRecordToRedo
                }
                WorkerV2PublicationIntentBoundaryV2::SyncRedoName => EngineBoundary::SyncRedoName,
                WorkerV2PublicationIntentBoundaryV2::RenameRedoToCanonical => {
                    EngineBoundary::RenameRedoToCanonical
                }
                WorkerV2PublicationIntentBoundaryV2::SyncCanonicalName => {
                    EngineBoundary::SyncCanonicalName
                }
            },
            timing: match point.timing {
                WorkerV2PublicationIntentFaultTimingV2::Before => EngineFaultTiming::Before,
                WorkerV2PublicationIntentFaultTimingV2::After => EngineFaultTiming::After,
            },
        }
    }

    fn public_fault_point_v2(point: EngineFaultPoint) -> WorkerV2PublicationIntentFaultPointV2 {
        WorkerV2PublicationIntentFaultPointV2 {
            boundary: match point.boundary {
                EngineBoundary::CreateOutputTemp => {
                    WorkerV2PublicationIntentBoundaryV2::CreateOutputTemp
                }
                EngineBoundary::WriteOutputTemp => {
                    WorkerV2PublicationIntentBoundaryV2::WriteOutputTemp
                }
                EngineBoundary::SyncOutputTemp => {
                    WorkerV2PublicationIntentBoundaryV2::SyncOutputTemp
                }
                EngineBoundary::RenameOutput => WorkerV2PublicationIntentBoundaryV2::RenameOutput,
                EngineBoundary::SyncOutputName => {
                    WorkerV2PublicationIntentBoundaryV2::SyncOutputName
                }
                EngineBoundary::CreateRecordTemp => {
                    WorkerV2PublicationIntentBoundaryV2::CreateRecordTemp
                }
                EngineBoundary::WriteRecordTemp => {
                    WorkerV2PublicationIntentBoundaryV2::WriteRecordTemp
                }
                EngineBoundary::SyncRecordTemp => {
                    WorkerV2PublicationIntentBoundaryV2::SyncRecordTemp
                }
                EngineBoundary::RenameRecordToRedo => {
                    WorkerV2PublicationIntentBoundaryV2::RenameRecordToRedo
                }
                EngineBoundary::SyncRedoName => WorkerV2PublicationIntentBoundaryV2::SyncRedoName,
                EngineBoundary::RenameRedoToCanonical => {
                    WorkerV2PublicationIntentBoundaryV2::RenameRedoToCanonical
                }
                EngineBoundary::SyncCanonicalName => {
                    WorkerV2PublicationIntentBoundaryV2::SyncCanonicalName
                }
            },
            timing: match point.timing {
                EngineFaultTiming::Before => WorkerV2PublicationIntentFaultTimingV2::Before,
                EngineFaultTiming::After => WorkerV2PublicationIntentFaultTimingV2::After,
            },
        }
    }

    include!("worker_v2_publication_intent_cleanup_escrow.rs");

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::BuildInvocation;
        use std::thread;

        static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

        struct TestDirectory {
            path: PathBuf,
        }

        impl TestDirectory {
            fn new() -> Self {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "fe2o3-worker-v2-publication-intent-v2-{}-{id}",
                    std::process::id()
                ));
                fs::create_dir(&path).unwrap();
                Self { path }
            }

            fn output(&self) -> PathBuf {
                self.path.join("output")
            }
        }

        impl Drop for TestDirectory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.path);
            }
        }

        fn make_producer(source: &str) -> ProducerIdentity {
            ProducerIdentity::from_codegen("kernel", Some(Path::new(source))).unwrap()
        }

        fn make_attempt(seed: u8) -> BuildAttempt {
            BuildAttempt::from_env_value(&format!(
                "{}:{}:{}",
                u64::from(seed) + 1,
                BuildSession::from_bytes([seed; 16]).to_hex(),
                BuildInvocation::from_bytes([seed.wrapping_add(1); 32]).to_hex()
            ))
            .unwrap()
        }

        fn begin(output: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
            crate::begin_build_attempt(
                output,
                producer,
                BuildInvocation::from_bytes([seed; 32]),
                BuildSession::from_bytes([seed.wrapping_add(1); 16]),
            )
            .unwrap()
        }

        fn plan(attempt: BuildAttempt, output: &[u8], seed: u8) -> DurableLinkPublicationPlanV1 {
            DurableLinkPublicationPlanV1::new(
                attempt,
                LinkPublicationScopeV1::new(
                    PackageIdentityV1::from_bytes([seed; 32]),
                    KernelSetIdentityV1::from_bytes([seed.wrapping_add(1); 32]),
                    TargetIdentityV1::from_bytes([seed.wrapping_add(2); 32]),
                ),
                CanonicalLinkRequestIdentityV1::from_bytes([seed.wrapping_add(3); 32]),
                PinnedWorkerIdentityV1::from_bytes([seed.wrapping_add(4); 32]),
                ValidatedResponseIdentityV1::from_bytes([seed.wrapping_add(5); 32]),
                LinkedOutputIdentityV1::from_bytes([seed.wrapping_add(6); 32]),
                FinalizationIdentityV1::from_bytes([seed.wrapping_add(7); 32]),
                FinalizedOutputIdentityV1::from_bytes(sha256(output)),
                AtomicPublicationIdentityV1::from_bytes([seed.wrapping_add(8); 32]),
            )
        }

        fn make_closure(seed: u8) -> CompilerClosureV2 {
            CompilerClosureV2::new(
                [seed; 32],
                [seed.wrapping_add(1); 32],
                [seed.wrapping_add(2); 32],
                [seed.wrapping_add(3); 32],
                [seed.wrapping_add(4); 32],
                [seed.wrapping_add(5); 32],
            )
            .unwrap()
        }

        fn make_record(
            producer: &ProducerIdentity,
            attempt: BuildAttempt,
            output: &[u8],
            seed: u8,
            closure: CompilerClosureV2,
        ) -> WorkerV2PublicationIntentRecordV2 {
            WorkerV2PublicationIntentRecordV2::new(
                producer,
                attempt,
                plan(attempt, output, seed),
                UpstreamCodeObjectEvidenceIdentityV1::from_bytes([seed.wrapping_add(9); 32]),
                output.len(),
                closure,
            )
        }

        fn reseal(bytes: &mut [u8]) {
            let body_len = bytes.len() - 32;
            let checksum = sha256_parts(&[RECORD_CHECKSUM_DOMAIN_V2, &bytes[..body_len]]);
            bytes[body_len..].copy_from_slice(&checksum);
        }

        fn intent_entry_count(output: &Path, prefix: &str, suffix: &str) -> usize {
            fs::read_dir(output)
                .unwrap()
                .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                .filter(|name| name.starts_with(prefix) && name.ends_with(suffix))
                .count()
        }

        #[test]
        fn compiler_closure_matches_the_shared_golden_and_rejects_noncanonical_preimages() {
            let pins = [
                [0x11; 32], [0x22; 32], [0x33; 32], [0x44; 32], [0x55; 32], [0x66; 32],
            ];
            let closure =
                CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5])
                    .unwrap();
            assert_eq!(closure.cargo_executable_sha256(), pins[0]);
            assert_eq!(closure.cargo_binding_trampoline_sha256(), pins[1]);
            assert_eq!(closure.cargo_fe2o3_binding_wrapper_sha256(), pins[2]);
            assert_eq!(closure.rustc_executable_sha256(), pins[3]);
            assert_eq!(closure.rustc_runtime_tree_sha256(), pins[4]);
            assert_eq!(closure.codegen_backend_sha256(), pins[5]);
            assert_eq!(closure.cargo_binding_transition_protocol_version(), 1);
            assert_eq!(
                closure.identity_sha256(),
                [
                    0x9c, 0x28, 0x98, 0x53, 0x25, 0x45, 0xab, 0xbc, 0x57, 0x7c, 0x9d, 0x6f, 0x20,
                    0x2e, 0x7e, 0x31, 0x82, 0xee, 0x79, 0x5e, 0xc8, 0x87, 0xfc, 0xb0, 0x54, 0x0e,
                    0xb4, 0x10, 0x71, 0x96, 0x77, 0xf9,
                ]
            );
            assert_eq!(
                CompilerClosureV2::from_pins_and_identity(
                    pins[0],
                    pins[1],
                    pins[2],
                    pins[3],
                    pins[4],
                    pins[5],
                    2,
                    closure.identity_sha256(),
                ),
                Err(CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion { version: 2 })
            );
            assert_eq!(
                CompilerClosureV2::new([0; 32], pins[1], pins[2], pins[3], pins[4], pins[5],),
                Err(CompilerClosureErrorV2::ZeroDigest {
                    field: fe2o3_build_authority::CompilerClosureDigestFieldV2::CargoExecutable,
                })
            );
            let mut wrong_identity = closure.identity_sha256();
            wrong_identity[0] ^= 1;
            assert_eq!(
                CompilerClosureV2::from_pins_and_identity(
                    pins[0],
                    pins[1],
                    pins[2],
                    pins[3],
                    pins[4],
                    pins[5],
                    1,
                    wrong_identity,
                ),
                Err(CompilerClosureErrorV2::IdentityMismatch)
            );
        }

        #[test]
        fn v2_codec_is_canonical_and_binds_every_publication_input() {
            let producer = make_producer("/src/v2-codec.rs");
            let output = b"exact V2 output";
            let attempt = make_attempt(0x11);
            let closure = make_closure(0x21);
            let record = make_record(&producer, attempt, output, 0x31, closure);
            let encoded = record.encode();
            assert_eq!(MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES, 616);
            assert_eq!(MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES_V2, 842);
            assert_eq!(
                encoded.len(),
                MAX_WORKER_V2_PUBLICATION_INTENT_RECORD_BYTES_V2
            );
            assert_eq!(
                record.identity().as_bytes(),
                [
                    0xfe, 0x84, 0x21, 0x81, 0xa3, 0xe4, 0xaf, 0xe2, 0x80, 0xbf, 0xcb, 0x0d, 0x3f,
                    0x24, 0x56, 0x5c, 0xc9, 0x58, 0xba, 0xc0, 0x87, 0x99, 0x8b, 0x2b, 0x55, 0x09,
                    0x1d, 0xa0, 0x93, 0x28, 0x93, 0x7c,
                ]
            );
            assert_eq!(
                WorkerV2PublicationIntentRecordV2::decode(&encoded),
                Ok(record)
            );
            assert_eq!(record.compiler_closure(), closure);
            assert_ne!(RECORD_MAGIC, RECORD_MAGIC_V2);
            assert_ne!(RECORD_CHECKSUM_DOMAIN, RECORD_CHECKSUM_DOMAIN_V2);
            assert_ne!(RECORD_IDENTITY_DOMAIN, RECORD_IDENTITY_DOMAIN_V2);
            assert_ne!(
                producer_identity(&producer),
                producer_identity_v2(&producer)
            );
            assert_ne!(
                slot_identity(producer_identity(&producer), attempt),
                slot_identity_v2(producer_identity_v2(&producer), attempt)
            );

            let other_producer = make_producer("/src/v2-codec-other.rs");
            assert_ne!(
                make_record(&other_producer, attempt, output, 0x31, closure).identity(),
                record.identity()
            );
            let other_attempt = make_attempt(0x12);
            assert_ne!(
                make_record(&producer, other_attempt, output, 0x31, closure).identity(),
                record.identity()
            );
            let mut other_upstream = record;
            other_upstream.upstream_evidence =
                UpstreamCodeObjectEvidenceIdentityV1::from_bytes([9; 32]);
            other_upstream.identity = other_upstream.encoded_identity();
            assert_ne!(other_upstream.identity(), record.identity());
            assert_ne!(
                make_record(&producer, attempt, output, 0x32, closure).identity(),
                record.identity()
            );
            let other_output = b"different exact output";
            assert_ne!(
                make_record(&producer, attempt, other_output, 0x31, closure).identity(),
                record.identity()
            );
            assert_ne!(
                make_record(&producer, attempt, output, 0x31, make_closure(0x22)).identity(),
                record.identity()
            );
        }

        #[test]
        fn mutation_role_alias_unknown_protocol_version_and_v1_downgrade_fail_closed() {
            let producer = make_producer("/src/v2-mutation.rs");
            let output = b"mutation output";
            let attempt = make_attempt(0x41);
            let closure = make_closure(0x51);
            let encoded = make_record(&producer, attempt, output, 0x61, closure).encode();

            for index in 0..encoded.len() {
                let mut mutated = encoded.clone();
                mutated[index] ^= 1;
                assert_eq!(
                    WorkerV2PublicationIntentRecordV2::decode(&mutated),
                    Err("record checksum mismatch"),
                    "unchecked byte {index}"
                );
            }

            let closure_offset = RECORD_BYTES - 32;
            for role in 0..6 {
                let mut mutated = encoded.clone();
                mutated[closure_offset + (role * 32)] ^= 1;
                reseal(&mut mutated);
                assert_eq!(
                    WorkerV2PublicationIntentRecordV2::decode(&mutated),
                    Err("record compiler closure identity does not match its role-specific pins")
                );
            }

            let mut aliased_roles = encoded.clone();
            let first: [u8; 32] = aliased_roles[closure_offset..closure_offset + 32]
                .try_into()
                .unwrap();
            let second: [u8; 32] = aliased_roles[closure_offset + 32..closure_offset + 64]
                .try_into()
                .unwrap();
            aliased_roles[closure_offset..closure_offset + 32].copy_from_slice(&second);
            aliased_roles[closure_offset + 32..closure_offset + 64].copy_from_slice(&first);
            reseal(&mut aliased_roles);
            assert_eq!(
                WorkerV2PublicationIntentRecordV2::decode(&aliased_roles),
                Err("record compiler closure identity does not match its role-specific pins")
            );

            let mut unknown_protocol = encoded.clone();
            unknown_protocol[closure_offset + (6 * 32)..closure_offset + (6 * 32) + 2]
                .copy_from_slice(&2u16.to_le_bytes());
            reseal(&mut unknown_protocol);
            assert_eq!(
                WorkerV2PublicationIntentRecordV2::decode(&unknown_protocol),
                Err("record compiler closure uses an unsupported transition protocol version")
            );

            let mut unknown_version = encoded.clone();
            unknown_version[RECORD_MAGIC_V2.len()..RECORD_MAGIC_V2.len() + 2]
                .copy_from_slice(&3u16.to_le_bytes());
            reseal(&mut unknown_version);
            assert_eq!(
                WorkerV2PublicationIntentRecordV2::decode(&unknown_version),
                Err("unsupported V2 record version")
            );

            let v1 = WorkerV2PublicationIntentRecordV1::new(
                &producer,
                attempt,
                plan(attempt, output, 0x61),
                UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x6a; 32]),
                output.len(),
            )
            .encode();
            assert_eq!(
                WorkerV2PublicationIntentRecordV2::decode(&v1),
                Err("record has a noncanonical V2 length")
            );
            let mut downgraded = encoded;
            downgraded[..RECORD_MAGIC.len()].copy_from_slice(RECORD_MAGIC);
            downgraded[RECORD_MAGIC.len()..RECORD_MAGIC.len() + 2]
                .copy_from_slice(&RECORD_VERSION.to_le_bytes());
            reseal(&mut downgraded);
            assert_eq!(
                WorkerV2PublicationIntentRecordV2::decode(&downgraded),
                Err("record magic mismatch")
            );
        }

        #[test]
        fn persist_recover_and_clear_require_the_exact_full_closure() {
            let temp = TestDirectory::new();
            let output_dir = temp.output();
            let producer = make_producer("/src/v2-wrong-closure.rs");
            let attempt = begin(&output_dir, &producer, 0x71);
            let output = b"closure-bound persisted output";
            let plan = plan(attempt, output, 0x72);
            let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x73; 32]);
            let closure = make_closure(0x74);
            let wrong_closure = make_closure(0x75);

            let persisted = persist_worker_v2_publication_intent_v2(
                &output_dir,
                &producer,
                attempt,
                plan,
                upstream,
                closure,
                output,
            )
            .unwrap();
            assert_eq!(
                persisted.outcome(),
                WorkerV2PublicationIntentOutcomeV2::Persisted
            );
            assert_eq!(persisted.compiler_closure(), closure);
            assert_eq!(persisted.record().compiler_closure(), closure);
            assert_eq!(persisted.exact_output(), output);
            assert!(!persisted.grants_publication_authority());
            assert!(!persisted.grants_compiler_authority());
            assert!(!persisted.grants_load_authority());
            assert!(!persisted.grants_launch_authority());
            assert!(matches!(
                recover_worker_v2_publication_intent_v2(
                    &output_dir,
                    &producer,
                    attempt,
                    wrong_closure,
                ),
                Err(WorkerV2PublicationIntentErrorV2::CompilerClosureMismatch)
            ));
            assert!(matches!(
                persist_worker_v2_publication_intent_v2(
                    &output_dir,
                    &producer,
                    attempt,
                    plan,
                    upstream,
                    wrong_closure,
                    output,
                ),
                Err(WorkerV2PublicationIntentErrorV2::ConflictingIntent)
            ));
            assert!(matches!(
                clear_worker_v2_publication_intent_v2(
                    &output_dir,
                    &producer,
                    attempt,
                    wrong_closure,
                    persisted.record().identity(),
                ),
                Err(WorkerV2PublicationIntentErrorV2::CompilerClosureMismatch)
            ));

            let recovered =
                recover_worker_v2_publication_intent_v2(&output_dir, &producer, attempt, closure)
                    .unwrap();
            assert_eq!(
                recovered.outcome(),
                WorkerV2PublicationIntentOutcomeV2::Recovered
            );
            assert_eq!(recovered.record(), persisted.record());
            crate::publish_exact_hsaco_evidence_for_attempt_v2(
                &output_dir,
                &producer,
                attempt,
                plan,
                upstream,
                closure,
                output,
            )
            .unwrap();
            clear_worker_v2_publication_intent_v2(
                &output_dir,
                &producer,
                attempt,
                closure,
                persisted.record().identity(),
            )
            .unwrap();
            assert!(matches!(
                recover_worker_v2_publication_intent_v2(&output_dir, &producer, attempt, closure,),
                Err(WorkerV2PublicationIntentErrorV2::NotFound)
            ));
        }

        #[test]
        fn v2_recovery_never_consumes_or_upgrades_v1_records() {
            let temp = TestDirectory::new();
            let output_dir = temp.output();
            let producer = make_producer("/src/v1-compatibility-only.rs");
            let attempt = begin(&output_dir, &producer, 0x81);
            let output = b"side-by-side compatibility output";
            let plan = plan(attempt, output, 0x82);
            let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0x83; 32]);
            let closure = make_closure(0x84);

            persist_worker_v2_publication_intent_v1(
                &output_dir,
                &producer,
                attempt,
                plan,
                upstream,
                output,
            )
            .unwrap();
            assert!(matches!(
                recover_worker_v2_publication_intent_v2(&output_dir, &producer, attempt, closure,),
                Err(WorkerV2PublicationIntentErrorV2::NotFound)
            ));
            assert_eq!(
                intent_entry_count(&output_dir, FILE_PREFIX, RECORD_SUFFIX),
                1
            );
            assert_eq!(
                intent_entry_count(&output_dir, FILE_PREFIX_V2, RECORD_SUFFIX),
                0
            );
            assert_eq!(
                recover_worker_v2_publication_intent_v1(&output_dir, &producer, attempt)
                    .unwrap()
                    .exact_output(),
                output
            );

            persist_worker_v2_publication_intent_v2(
                &output_dir,
                &producer,
                attempt,
                plan,
                upstream,
                closure,
                output,
            )
            .unwrap();
            assert_eq!(
                intent_entry_count(&output_dir, FILE_PREFIX, RECORD_SUFFIX),
                1
            );
            assert_eq!(
                intent_entry_count(&output_dir, FILE_PREFIX_V2, RECORD_SUFFIX),
                1
            );
            assert_eq!(
                recover_worker_v2_publication_intent_v1(&output_dir, &producer, attempt)
                    .unwrap()
                    .exact_output(),
                output
            );
            assert_eq!(
                recover_worker_v2_publication_intent_v2(&output_dir, &producer, attempt, closure,)
                    .unwrap()
                    .exact_output(),
                output
            );
        }

        #[test]
        fn every_v2_persistence_boundary_reconciles_after_a_crash() {
            let boundaries = [
                WorkerV2PublicationIntentBoundaryV2::CreateOutputTemp,
                WorkerV2PublicationIntentBoundaryV2::WriteOutputTemp,
                WorkerV2PublicationIntentBoundaryV2::SyncOutputTemp,
                WorkerV2PublicationIntentBoundaryV2::RenameOutput,
                WorkerV2PublicationIntentBoundaryV2::SyncOutputName,
                WorkerV2PublicationIntentBoundaryV2::CreateRecordTemp,
                WorkerV2PublicationIntentBoundaryV2::WriteRecordTemp,
                WorkerV2PublicationIntentBoundaryV2::SyncRecordTemp,
                WorkerV2PublicationIntentBoundaryV2::RenameRecordToRedo,
                WorkerV2PublicationIntentBoundaryV2::SyncRedoName,
                WorkerV2PublicationIntentBoundaryV2::RenameRedoToCanonical,
                WorkerV2PublicationIntentBoundaryV2::SyncCanonicalName,
            ];
            let timings = [
                WorkerV2PublicationIntentFaultTimingV2::Before,
                WorkerV2PublicationIntentFaultTimingV2::After,
            ];
            for (index, (boundary, timing)) in boundaries
                .into_iter()
                .flat_map(|boundary| timings.into_iter().map(move |timing| (boundary, timing)))
                .enumerate()
            {
                let temp = TestDirectory::new();
                let output_dir = temp.output();
                let producer = make_producer(&format!("/src/v2-crash-{index}.rs"));
                let attempt = begin(&output_dir, &producer, 0x90 + index as u8);
                let output = format!("V2 crash {boundary:?} {timing:?}").into_bytes();
                let plan = plan(attempt, &output, 0xa0 + index as u8);
                let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0xb0; 32]);
                let closure = make_closure(0xc0);
                let point = WorkerV2PublicationIntentFaultPointV2 { boundary, timing };
                assert!(matches!(
                    persist_worker_v2_publication_intent_v2_with_options(
                        &output_dir,
                        &producer,
                        attempt,
                        plan,
                        upstream,
                        closure,
                        &output,
                        WorkerV2PublicationIntentOptionsV2::inject_crash(point),
                    ),
                    Err(WorkerV2PublicationIntentErrorV2::InjectedCrash { point: actual })
                        if actual == point
                ));
                let reconciled = persist_worker_v2_publication_intent_v2(
                    &output_dir,
                    &producer,
                    attempt,
                    plan,
                    upstream,
                    closure,
                    &output,
                )
                .unwrap();
                assert_eq!(reconciled.compiler_closure(), closure);
                assert_eq!(reconciled.exact_output(), output);
                assert_eq!(
                    recover_worker_v2_publication_intent_v2(
                        &output_dir,
                        &producer,
                        attempt,
                        closure,
                    )
                    .unwrap()
                    .record(),
                    reconciled.record()
                );
            }
        }

        #[test]
        fn concurrent_identical_v2_writers_commit_one_canonical_intent() {
            let temp = TestDirectory::new();
            let output_dir = temp.output();
            let producer = make_producer("/src/v2-concurrent.rs");
            let attempt = begin(&output_dir, &producer, 0xd1);
            let output = b"concurrent exact V2 output";
            let plan = plan(attempt, output, 0xd2);
            let upstream = UpstreamCodeObjectEvidenceIdentityV1::from_bytes([0xd3; 32]);
            let closure = make_closure(0xd4);

            let results = thread::scope(|scope| {
                let handles = (0..8)
                    .map(|_| {
                        scope.spawn(|| {
                            persist_worker_v2_publication_intent_v2(
                                &output_dir,
                                &producer,
                                attempt,
                                plan,
                                upstream,
                                closure,
                                output,
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .map(|handle| handle.join().unwrap().unwrap())
                    .collect::<Vec<_>>()
            });
            assert_eq!(
                results
                    .iter()
                    .filter(
                        |result| result.outcome() == WorkerV2PublicationIntentOutcomeV2::Persisted
                    )
                    .count(),
                1
            );
            assert!(
                results
                    .windows(2)
                    .all(|pair| pair[0].record() == pair[1].record())
            );
            assert_eq!(
                intent_entry_count(&output_dir, FILE_PREFIX_V2, RECORD_SUFFIX),
                1
            );
            assert_eq!(
                intent_entry_count(&output_dir, FILE_PREFIX_V2, OUTPUT_SUFFIX),
                1
            );
        }
    }
}

pub use publication_intent_v2::*;

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
