//! One-shot, build-attempt-scoped transfer of exact module-handoff bytes.
//!
//! A handoff is coordination state, not artifact publication or loading authority. Both producer
//! and consumer must present the exact [`BuildAttempt`] and [`ProducerIdentity`], and the attempt
//! must still be claimable in the durable attempt registry. Success proves only that a cooperating
//! process possessing that current attempt committed the measured bytes through this protocol. It
//! does not authenticate that rustc, the fe2o3 backend, or any other particular program authored
//! them. There is deliberately no lookup for a "current" handoff.

use super::{
    AttemptPhase, BuildAttempt, BuildSession, EmitError, PinnedOutput, ProducerIdentity,
    SimulationObservationReceiptV1, commit_attempt_registry_direct, read_attempt_registry,
};
use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, fstat, fsync, mkdirat, openat,
    renameat_with, statat, unlinkat,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const PARENT_PREFIX: &str = ".fe2o3-compiler-module-handoff-v1-";
const SLOT_PREFIX: &str = "attempt-";
const PAYLOAD_ENTRY: &str = "module";
const READY_ENTRY: &str = "ready";
const CONSUMED_ENTRY: &str = "consumed";
const TEMP_PREFIX: &str = ".tmp-";
const RECORD_MAGIC: &[u8] = b"FE2O3-COMPILER-MODULE-HANDOFF-V1\0";
const RECORD_VERSION: u16 = 1;
const PRODUCER_DOMAIN: &[u8] = b"fe2o3.compiler-module-handoff.producer.v1\0";
const SLOT_DOMAIN: &[u8] = b"fe2o3.compiler-module-handoff.slot.v1\0";
const NAMED_SLOT_DOMAIN: &[u8] = b"fe2o3.compiler-module-handoff.named-slot.v1\0";
const RECORD_DOMAIN: &[u8] = b"fe2o3.compiler-module-handoff.record.v1\0";
const MAX_SLOT_ENTRIES: usize = 16;
const MAX_STALE_SLOTS: usize = 1024;
const MAX_TEMP_ATTEMPTS: u64 = 64;
const RECORD_BYTES: usize = RECORD_MAGIC.len() + 2 + 32 + 8 + 16 + 32 + 32 + 32 + 8 + (7 * 8) + 32;
const SIMULATION_PARENT_PREFIX: &str = ".fe2o3-simulation-kir-handoff-v1-";
const SIMULATION_SLOT_PREFIX: &str = "attempt-";
const SIMULATION_RECORD_MAGIC: &[u8] = b"FE2O3-SIMULATION-KIR-HANDOFF-V1\0";
const SIMULATION_RECORD_VERSION: u16 = 1;
const SIMULATION_PRODUCER_DOMAIN: &[u8] = b"fe2o3.simulation-kir-handoff.producer.v1\0";
const SIMULATION_SLOT_DOMAIN: &[u8] = b"fe2o3.simulation-kir-handoff.slot.v1\0";
const SIMULATION_NAMED_SLOT_DOMAIN: &[u8] = b"fe2o3.simulation-kir-handoff.named-slot.v1\0";
const SIMULATION_RECORD_DOMAIN: &[u8] = b"fe2o3.simulation-kir-handoff.record.v1\0";
const SIMULATION_RECORD_BYTES: usize =
    SIMULATION_RECORD_MAGIC.len() + 2 + 32 + 8 + 16 + 32 + 32 + 32 + 8 + (7 * 8) + 32;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// Maximum complete canonical compiler-FFI V1 handoff accepted by this transport.
///
/// This mirrors `fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V1`: a 64 MiB module,
/// 512 KiB envelope, 128-byte target, and 83 bytes of canonical framing. This lower-level
/// filesystem crate deliberately does not depend on `fe2o3-compiler-ffi`; integration must keep
/// the two V1 constants equal.
pub const MAX_COMPILER_MODULE_HANDOFF_BYTES: usize = (64 * 1024 * 1024) + (512 * 1024) + 128 + 83;

/// Closed attempt-local transport slot for one compiler module handoff.
///
/// The default value preserves the original single-module protocol. The two
/// general-GEMM values let one rustc process transfer the independently built
/// reference and vectorized schedules while retaining one build attempt and
/// one non-Clone frontend correspondence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerModuleHandoffSlotV1 {
    /// Original one-module transport slot.
    Default = 0,
    /// Issue #138 reference wave64 XOR4 schedule.
    GeneralGemmReference = 1,
    /// Issue #138 A-only BF16 vector-transfer schedule.
    GeneralGemmVectorizedAOnly = 2,
}

/// SHA-256 identity of exact bytes committed by a holder of the named cooperative attempt.
///
/// The identity authenticates byte equality only. It does not establish who authored the bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerModuleHandoffIdentityV1([u8; 32]);

impl CompilerModuleHandoffIdentityV1 {
    /// Constructs an identity from its exact SHA-256 representation.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact SHA-256 representation.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Durable receipt for bytes committed by a holder of one exact cooperative attempt.
///
/// The receipt records attempt possession and byte integrity, not rustc or backend authorship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerModuleHandoffReceiptV1 {
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
    identity: CompilerModuleHandoffIdentityV1,
    length: usize,
}

impl CompilerModuleHandoffReceiptV1 {
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the exact attempt-local transport slot.
    pub const fn slot(self) -> CompilerModuleHandoffSlotV1 {
        self.slot
    }

    pub const fn identity(self) -> CompilerModuleHandoffIdentityV1 {
        self.identity
    }

    pub const fn length(self) -> usize {
        self.length
    }

    /// A handoff receipt is inert coordination evidence.
    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    /// Attempt possession does not authenticate compiler authorship.
    pub const fn grants_compiler_authority(self) -> bool {
        false
    }
}

/// Immutable bytes returned by the one successful consumption of a cooperative-attempt slot.
///
/// This value proves only that the measured bytes passed through the slot while its exact attempt
/// remained claimable. It is inert and carries no compiler-authorship or executable authority.
#[derive(Clone, Debug)]
pub struct ConsumedCompilerModuleHandoffV1 {
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
    identity: CompilerModuleHandoffIdentityV1,
    bytes: Arc<[u8]>,
}

/// Closed attempt-local slot for exact canonical KIR captured for CPU simulation.
///
/// This is deliberately a different protocol domain from compiler-module handoffs used by
/// protected production pipelines.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum SimulationKernelIrHandoffSlotV1 {
    CanonicalKirV6 = 0,
}

/// SHA-256 identity of canonical KIR bytes in the simulation-only custody slot.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationKernelIrHandoffIdentityV1([u8; 32]);

impl SimulationKernelIrHandoffIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert receipt for a simulation-only canonical KIR capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationKernelIrHandoffReceiptV1 {
    attempt: BuildAttempt,
    identity: SimulationKernelIrHandoffIdentityV1,
    length: usize,
}

impl SimulationKernelIrHandoffReceiptV1 {
    pub const fn attempt(self) -> BuildAttempt {
        self.attempt
    }

    pub const fn identity(self) -> SimulationKernelIrHandoffIdentityV1 {
        self.identity
    }

    pub const fn length(self) -> usize {
        self.length
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_hardware_authority(self) -> bool {
        false
    }
}

/// Non-forgeable result of consuming one exact simulation KIR capture.
#[derive(Clone, Debug)]
pub struct ConsumedSimulationKernelIrHandoffV1 {
    attempt: BuildAttempt,
    identity: SimulationKernelIrHandoffIdentityV1,
    bytes: Arc<[u8]>,
}

impl ConsumedSimulationKernelIrHandoffV1 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    pub const fn identity(&self) -> SimulationKernelIrHandoffIdentityV1 {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_hardware_authority(&self) -> bool {
        false
    }
}

impl ConsumedCompilerModuleHandoffV1 {
    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the exact attempt-local transport slot.
    pub const fn slot(&self) -> CompilerModuleHandoffSlotV1 {
        self.slot
    }

    pub const fn identity(&self) -> CompilerModuleHandoffIdentityV1 {
        self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumed bytes still require the finalizer's independent validation chain.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Attempt possession does not authenticate compiler authorship.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Consumed handoff bytes do not authorize linking.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Consumed handoff bytes do not authorize module loading.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Consumed handoff bytes do not authorize kernel launch.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Failure to publish, recover, or consume a compiler module handoff.
#[derive(Debug)]
pub enum CompilerModuleHandoffErrorV1 {
    Io(std::io::Error),
    Attempt { reason: String },
    AttemptNotClaimable,
    InvalidSlot { path: PathBuf, reason: String },
    InvalidHandoffSize { actual: usize, maximum: usize },
    AlreadyPublished,
    ConflictingPublication,
    AlreadyConsumed,
    NotPublished,
    DigestMismatch,
}

impl fmt::Display for CompilerModuleHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Attempt { reason } => {
                write!(formatter, "invalid build-attempt handoff: {reason}")
            }
            Self::AttemptNotClaimable => formatter.write_str(
                "invalid build-attempt handoff: build attempt is not in the required claimable phase",
            ),
            Self::InvalidSlot { path, reason } => {
                write!(
                    formatter,
                    "invalid compiler module handoff {}: {reason}",
                    path.display()
                )
            }
            Self::InvalidHandoffSize { actual, maximum } => write!(
                formatter,
                "canonical compiler module handoff size {actual} is outside 1..={maximum} bytes"
            ),
            Self::AlreadyPublished => {
                formatter.write_str("compiler module handoff is already published")
            }
            Self::ConflictingPublication => {
                formatter.write_str("compiler module handoff conflicts with the committed module")
            }
            Self::AlreadyConsumed => {
                formatter.write_str("compiler module handoff was already consumed")
            }
            Self::NotPublished => formatter.write_str("compiler module handoff is not published"),
            Self::DigestMismatch => {
                formatter.write_str("compiler module handoff SHA-256 identity mismatch")
            }
        }
    }
}

impl std::error::Error for CompilerModuleHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for CompilerModuleHandoffErrorV1 {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<EmitError> for CompilerModuleHandoffErrorV1 {
    fn from(error: EmitError) -> Self {
        match error {
            EmitError::BuildAttempt { reason } => Self::Attempt { reason },
            error => Self::InvalidSlot {
                path: PathBuf::new(),
                reason: error.to_string(),
            },
        }
    }
}

/// Atomically publishes caller-supplied bytes under one explicit cooperative attempt.
///
/// Success establishes attempt possession and a durable SHA-256 binding. The caller remains
/// responsible for establishing compiler authorship through a separate trusted process boundary.
///
/// This function takes the artifact-store lock and must not be called from an artifact transaction
/// callback, because those callbacks run under the same non-reentrant lock.
pub fn publish_compiler_module_handoff_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_bytes: &[u8],
) -> Result<CompilerModuleHandoffReceiptV1, CompilerModuleHandoffErrorV1> {
    publish_with_hooks(output_dir, producer, attempt, handoff_bytes, &mut NoFaults)
}

/// Atomically publishes bytes in one closed attempt-local named slot.
///
/// Named slots remain inert coordination state. They neither combine the two
/// modules nor authorize a compiler, finalizer, publication, load, or launch.
pub fn publish_compiler_module_handoff_in_slot_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
    handoff_bytes: &[u8],
) -> Result<CompilerModuleHandoffReceiptV1, CompilerModuleHandoffErrorV1> {
    publish_in_slot_with_hooks(
        output_dir,
        producer,
        attempt,
        slot,
        handoff_bytes,
        &mut NoFaults,
    )
}

/// Consumes one explicit attempt's complete canonical handoff exactly once.
///
/// The durable `consumed` tombstone is committed before bytes are returned. Cleanup of the now
/// inert payload is best-effort; a later replay rejects the tombstone and retries cleanup.
pub fn consume_compiler_module_handoff_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<ConsumedCompilerModuleHandoffV1, CompilerModuleHandoffErrorV1> {
    consume_with_hooks(output_dir, producer, attempt, &mut NoFaults)
}

/// Consumes one closed attempt-local named slot exactly once.
pub fn consume_compiler_module_handoff_in_slot_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
) -> Result<ConsumedCompilerModuleHandoffV1, CompilerModuleHandoffErrorV1> {
    consume_in_slot_with_hooks(output_dir, producer, attempt, slot, &mut NoFaults)
}

/// Publishes exact canonical KIR into the simulation-only backend custody slot.
///
/// On first publication this atomically claims a current managed Building attempt for the
/// backend. The domain-separated slot then authorizes only an exact BackendClaimed attempt with
/// no backend receipt. It grants no artifact, hardware, load, or launch authority.
pub fn publish_simulation_kernel_ir_handoff_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    canonical_kir: &[u8],
) -> Result<SimulationKernelIrHandoffReceiptV1, CompilerModuleHandoffErrorV1> {
    publish_in_slot_engine::<SimulationKernelIrHandoffSchemaV1>(
        output_dir,
        producer,
        attempt,
        SimulationKernelIrHandoffSlotV1::CanonicalKirV6,
        (),
        canonical_kir,
        &mut NoFaults,
    )
    .map(|published| SimulationKernelIrHandoffReceiptV1 {
        attempt: published.attempt,
        identity: SimulationKernelIrHandoffIdentityV1(published.identity),
        length: published.length,
    })
    .map_err(HandoffEngineError::into_v1)
}

/// Consumes one exact canonical KIR capture from the simulation-only custody slot.
pub fn consume_simulation_kernel_ir_handoff_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<ConsumedSimulationKernelIrHandoffV1, CompilerModuleHandoffErrorV1> {
    consume_in_slot_engine::<SimulationKernelIrHandoffSchemaV1>(
        output_dir,
        producer,
        attempt,
        SimulationKernelIrHandoffSlotV1::CanonicalKirV6,
        (),
        &mut NoFaults,
    )
    .map(|consumed| ConsumedSimulationKernelIrHandoffV1 {
        attempt: consumed.attempt,
        identity: SimulationKernelIrHandoffIdentityV1(consumed.identity),
        bytes: consumed.payload,
    })
    .map_err(HandoffEngineError::into_v1)
}

/// Completes a simulation attempt after its exact canonical KIR was consumed.
///
/// Records a distinct, authority-free simulation observation receipt solely to
/// retire the managed attempt. Artifact and publication schemas reject it.
/// It is not hardware observation or artifact authority. The private fields of the consumed value
/// prevent completion without first consuming the domain-separated simulation slot.
pub fn complete_simulation_kernel_ir_attempt_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    consumed: &ConsumedSimulationKernelIrHandoffV1,
) -> Result<SimulationObservationReceiptV1, CompilerModuleHandoffErrorV1> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize_for_custody(
        &output,
        producer,
        consumed.attempt,
        HandoffAttemptCustodyV1::SimulationBackendClaimed,
    )?;
    let mut attempts = read_attempt_registry(&output)?;
    let receipt = SimulationObservationReceiptV1::new(*consumed.identity.as_bytes());
    attempts
        .record_simulation_observation_receipt(&producer.stable_source, consumed.attempt, receipt)
        .map_err(|error| attempt_error(error.to_string()))?;
    attempts
        .mark_completed(&producer.stable_source, consumed.attempt)
        .map_err(|error| attempt_error(error.to_string()))?;
    commit_attempt_registry_direct(&output, &attempts)?;
    Ok(receipt)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileIdentity {
    device: u64,
    inode: u64,
    length: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl FileIdentity {
    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            length: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }

    fn matches(self, stat: &rustix::fs::Stat) -> bool {
        is_private_file(stat)
            && self.device == stat.st_dev
            && self.inode == stat.st_ino
            && self.length == stat.st_size
            && self.modified_seconds == stat.st_mtime
            && self.modified_nanoseconds == stat.st_mtime_nsec
            && self.changed_seconds == stat.st_ctime
            && self.changed_nanoseconds == stat.st_ctime_nsec
    }
}

trait HandoffSchema: Sized {
    type Slot: Copy + Eq + 'static;
    type Binding: Copy + Eq;
    type Payload;

    const PARENT_PREFIX: &'static str;
    const SLOT_PREFIX: &'static str;
    const RECORD_MAGIC: &'static [u8];
    const RECORD_VERSION: u16;
    const PRODUCER_DOMAIN: &'static [u8];
    const SLOT_DOMAIN: &'static [u8];
    const NAMED_SLOT_DOMAIN: &'static [u8];
    const RECORD_DOMAIN: &'static [u8];
    const RECORD_BYTES: usize;
    const MAX_HANDOFF_BYTES: usize;
    const DECODE_WORKING_SET_MULTIPLIER: usize;
    const DECODE_WORKING_SET_FIXED_BYTES: usize;
    const MAX_DECODE_WORKING_SET_BYTES: usize;
    const VALIDATE_RECORD_DURING_RECOVERY: bool;
    const ALL_SLOTS: &'static [Self::Slot];
    const ATTEMPT_CUSTODY: HandoffAttemptCustodyV1 = HandoffAttemptCustodyV1::FrontendBuilding;

    fn default_slot() -> Self::Slot;
    fn slot_tag(slot: Self::Slot) -> u8;
    fn encode_binding(binding: Self::Binding, bytes: &mut Vec<u8>);
    fn decode_binding(decoder: &mut Decoder<'_>) -> Result<Self::Binding, &'static str>;
    fn binding_matches_length(binding: Self::Binding, length: usize) -> bool;
    fn decode_payload(
        binding: Self::Binding,
        bytes: Vec<u8>,
    ) -> Result<Self::Payload, HandoffEngineError>;
    fn derive_identity(
        producer: [u8; 32],
        slot: [u8; 32],
        attempt: BuildAttempt,
        binding: Self::Binding,
        handoff_bytes: &[u8],
    ) -> [u8; 32];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HandoffAttemptCustodyV1 {
    FrontendBuilding,
    SimulationBackendClaimed,
}

struct HandoffV1Schema;

impl HandoffSchema for HandoffV1Schema {
    type Slot = CompilerModuleHandoffSlotV1;
    type Binding = ();
    type Payload = Arc<[u8]>;

    const PARENT_PREFIX: &'static str = PARENT_PREFIX;
    const SLOT_PREFIX: &'static str = SLOT_PREFIX;
    const RECORD_MAGIC: &'static [u8] = RECORD_MAGIC;
    const RECORD_VERSION: u16 = RECORD_VERSION;
    const PRODUCER_DOMAIN: &'static [u8] = PRODUCER_DOMAIN;
    const SLOT_DOMAIN: &'static [u8] = SLOT_DOMAIN;
    const NAMED_SLOT_DOMAIN: &'static [u8] = NAMED_SLOT_DOMAIN;
    const RECORD_DOMAIN: &'static [u8] = RECORD_DOMAIN;
    const RECORD_BYTES: usize = RECORD_BYTES;
    const MAX_HANDOFF_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES;
    const DECODE_WORKING_SET_MULTIPLIER: usize = 1;
    const DECODE_WORKING_SET_FIXED_BYTES: usize = 0;
    const MAX_DECODE_WORKING_SET_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES;
    const VALIDATE_RECORD_DURING_RECOVERY: bool = false;
    const ALL_SLOTS: &'static [Self::Slot] = &[
        CompilerModuleHandoffSlotV1::Default,
        CompilerModuleHandoffSlotV1::GeneralGemmReference,
        CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly,
    ];

    fn default_slot() -> Self::Slot {
        CompilerModuleHandoffSlotV1::Default
    }

    fn slot_tag(slot: Self::Slot) -> u8 {
        slot as u8
    }

    fn encode_binding(_binding: Self::Binding, _bytes: &mut Vec<u8>) {}

    fn decode_binding(_decoder: &mut Decoder<'_>) -> Result<Self::Binding, &'static str> {
        Ok(())
    }

    fn binding_matches_length(_binding: Self::Binding, _length: usize) -> bool {
        true
    }

    fn decode_payload(
        _binding: Self::Binding,
        bytes: Vec<u8>,
    ) -> Result<Self::Payload, HandoffEngineError> {
        Ok(Arc::from(bytes))
    }

    fn derive_identity(
        _producer: [u8; 32],
        _slot: [u8; 32],
        _attempt: BuildAttempt,
        _binding: Self::Binding,
        handoff_bytes: &[u8],
    ) -> [u8; 32] {
        sha256(handoff_bytes)
    }
}

struct SimulationKernelIrHandoffSchemaV1;

impl HandoffSchema for SimulationKernelIrHandoffSchemaV1 {
    type Slot = SimulationKernelIrHandoffSlotV1;
    type Binding = ();
    type Payload = Arc<[u8]>;

    const PARENT_PREFIX: &'static str = SIMULATION_PARENT_PREFIX;
    const SLOT_PREFIX: &'static str = SIMULATION_SLOT_PREFIX;
    const RECORD_MAGIC: &'static [u8] = SIMULATION_RECORD_MAGIC;
    const RECORD_VERSION: u16 = SIMULATION_RECORD_VERSION;
    const PRODUCER_DOMAIN: &'static [u8] = SIMULATION_PRODUCER_DOMAIN;
    const SLOT_DOMAIN: &'static [u8] = SIMULATION_SLOT_DOMAIN;
    const NAMED_SLOT_DOMAIN: &'static [u8] = SIMULATION_NAMED_SLOT_DOMAIN;
    const RECORD_DOMAIN: &'static [u8] = SIMULATION_RECORD_DOMAIN;
    const RECORD_BYTES: usize = SIMULATION_RECORD_BYTES;
    const MAX_HANDOFF_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES;
    const DECODE_WORKING_SET_MULTIPLIER: usize = 1;
    const DECODE_WORKING_SET_FIXED_BYTES: usize = 0;
    const MAX_DECODE_WORKING_SET_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES;
    const VALIDATE_RECORD_DURING_RECOVERY: bool = false;
    const ALL_SLOTS: &'static [Self::Slot] = &[SimulationKernelIrHandoffSlotV1::CanonicalKirV6];
    const ATTEMPT_CUSTODY: HandoffAttemptCustodyV1 =
        HandoffAttemptCustodyV1::SimulationBackendClaimed;

    fn default_slot() -> Self::Slot {
        SimulationKernelIrHandoffSlotV1::CanonicalKirV6
    }

    fn slot_tag(slot: Self::Slot) -> u8 {
        slot as u8
    }

    fn encode_binding(_binding: Self::Binding, _bytes: &mut Vec<u8>) {}

    fn decode_binding(_decoder: &mut Decoder<'_>) -> Result<Self::Binding, &'static str> {
        Ok(())
    }

    fn binding_matches_length(_binding: Self::Binding, _length: usize) -> bool {
        true
    }

    fn decode_payload(
        _binding: Self::Binding,
        bytes: Vec<u8>,
    ) -> Result<Self::Payload, HandoffEngineError> {
        Ok(Arc::from(bytes))
    }

    fn derive_identity(
        _producer: [u8; 32],
        _slot: [u8; 32],
        _attempt: BuildAttempt,
        _binding: Self::Binding,
        canonical_kir: &[u8],
    ) -> [u8; 32] {
        sha256(canonical_kir)
    }
}

enum HandoffEngineError {
    Common(CompilerModuleHandoffErrorV1),
    WrongBinding,
    PayloadBindingMismatch,
    WorkingSetBudgetExceeded { required: usize, maximum: usize },
    PayloadAllocationFailed { requested: usize },
    InvalidCanonicalV3(fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3),
}

impl From<CompilerModuleHandoffErrorV1> for HandoffEngineError {
    fn from(error: CompilerModuleHandoffErrorV1) -> Self {
        Self::Common(error)
    }
}

impl From<std::io::Error> for HandoffEngineError {
    fn from(error: std::io::Error) -> Self {
        Self::Common(error.into())
    }
}

impl From<EmitError> for HandoffEngineError {
    fn from(error: EmitError) -> Self {
        Self::Common(error.into())
    }
}

impl HandoffEngineError {
    fn into_v1(self) -> CompilerModuleHandoffErrorV1 {
        match self {
            Self::Common(error) => error,
            Self::WrongBinding => invalid_slot(
                Path::new(""),
                "V1 handoff unexpectedly carried a protocol binding",
            ),
            Self::PayloadBindingMismatch => invalid_slot(
                Path::new(""),
                "V1 handoff unexpectedly failed a protocol payload binding",
            ),
            Self::WorkingSetBudgetExceeded { required, maximum } => invalid_slot(
                Path::new(""),
                format!(
                    "V1 handoff unexpectedly required {required} bytes of decode working set with a {maximum}-byte limit"
                ),
            ),
            Self::PayloadAllocationFailed { requested } => invalid_slot(
                Path::new(""),
                format!("could not reserve {requested} bytes for the V1 handoff payload"),
            ),
            Self::InvalidCanonicalV3(error) => invalid_slot(
                Path::new(""),
                format!("V1 handoff unexpectedly required V3 decoding: {error}"),
            ),
        }
    }
}

struct PublishedHandoff<S: HandoffSchema> {
    attempt: BuildAttempt,
    slot: S::Slot,
    binding: S::Binding,
    identity: [u8; 32],
    length: usize,
}

struct ConsumedHandoff<S: HandoffSchema> {
    attempt: BuildAttempt,
    slot: S::Slot,
    binding: S::Binding,
    identity: [u8; 32],
    payload: S::Payload,
}

struct HandoffRecord<S: HandoffSchema> {
    slot: [u8; 32],
    attempt: BuildAttempt,
    producer: [u8; 32],
    binding: S::Binding,
    identity: [u8; 32],
    length: usize,
    file: FileIdentity,
}

impl<S: HandoffSchema> HandoffRecord<S> {
    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(S::RECORD_BYTES);
        bytes.extend_from_slice(S::RECORD_MAGIC);
        bytes.extend_from_slice(&S::RECORD_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.slot);
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        bytes.extend_from_slice(&self.producer);
        S::encode_binding(self.binding, &mut bytes);
        bytes.extend_from_slice(&self.identity);
        bytes.extend_from_slice(&(self.length as u64).to_le_bytes());
        bytes.extend_from_slice(&self.file.device.to_le_bytes());
        bytes.extend_from_slice(&self.file.inode.to_le_bytes());
        bytes.extend_from_slice(&self.file.modified_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.file.modified_nanoseconds.to_le_bytes());
        bytes.extend_from_slice(&self.file.changed_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.file.changed_nanoseconds.to_le_bytes());
        bytes.extend_from_slice(&(self.file.length as u64).to_le_bytes());
        let checksum = sha256_parts(&[S::RECORD_DOMAIN, &bytes]);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), S::RECORD_BYTES);
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != S::RECORD_BYTES {
            return Err("record has a noncanonical length");
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if sha256_parts(&[S::RECORD_DOMAIN, body]).as_slice() != checksum {
            return Err("record checksum mismatch");
        }
        let mut decoder = Decoder::new(body);
        if decoder.take(S::RECORD_MAGIC.len())? != S::RECORD_MAGIC {
            return Err("record magic mismatch");
        }
        if decoder.u16()? != S::RECORD_VERSION {
            return Err("unsupported record version");
        }
        let slot = decoder.array()?;
        let generation = decoder.u64()?;
        let session = super::BuildSession::from_bytes(decoder.array()?);
        let invocation = super::BuildInvocation::from_bytes(decoder.array()?);
        let attempt = BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            session.to_hex(),
            invocation.to_hex()
        ))
        .map_err(|_| "record contains an invalid attempt")?;
        let producer = decoder.array()?;
        let binding = S::decode_binding(&mut decoder)?;
        let identity = decoder.array()?;
        let length = usize::try_from(decoder.u64()?).map_err(|_| "record length is invalid")?;
        let file = FileIdentity {
            device: decoder.u64()?,
            inode: decoder.u64()?,
            modified_seconds: decoder.u64()? as i64,
            modified_nanoseconds: decoder.u64()?,
            changed_seconds: decoder.u64()? as i64,
            changed_nanoseconds: decoder.u64()?,
            length: decoder.u64()? as i64,
        };
        if !decoder.finished()
            || length == 0
            || length > S::MAX_HANDOFF_BYTES
            || !S::binding_matches_length(binding, length)
        {
            return Err("record contains an invalid module length");
        }
        Ok(Self {
            slot,
            attempt,
            producer,
            binding,
            identity,
            length,
            file,
        })
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
        let result = self.bytes.get(self.offset..end).ok_or("truncated record")?;
        self.offset = end;
        Ok(result)
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

struct PinnedDirectory {
    fd: OwnedFd,
    parent_fd: OwnedFd,
    name: PathBuf,
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl PinnedDirectory {
    fn verify(&self) -> Result<(), CompilerModuleHandoffErrorV1> {
        let opened = fstat(&self.fd).map_err(std::io::Error::from)?;
        let named = statat(&self.parent_fd, &self.name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if !is_private_directory(&opened)
            || !is_private_directory(&named)
            || opened.st_dev != self.device
            || opened.st_ino != self.inode
            || named.st_dev != self.device
            || named.st_ino != self.inode
        {
            return Err(invalid_slot(
                &self.path,
                "private directory identity changed",
            ));
        }
        Ok(())
    }
}

fn publish_with_hooks(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_bytes: &[u8],
    hooks: &mut impl HandoffHooks,
) -> Result<CompilerModuleHandoffReceiptV1, CompilerModuleHandoffErrorV1> {
    publish_in_slot_with_hooks(
        output_dir,
        producer,
        attempt,
        CompilerModuleHandoffSlotV1::Default,
        handoff_bytes,
        hooks,
    )
}

fn publish_in_slot_with_hooks(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_slot: CompilerModuleHandoffSlotV1,
    handoff_bytes: &[u8],
    hooks: &mut impl HandoffHooks,
) -> Result<CompilerModuleHandoffReceiptV1, CompilerModuleHandoffErrorV1> {
    publish_in_slot_engine::<HandoffV1Schema>(
        output_dir,
        producer,
        attempt,
        handoff_slot,
        (),
        handoff_bytes,
        hooks,
    )
    .map(|published| CompilerModuleHandoffReceiptV1 {
        attempt: published.attempt,
        slot: published.slot,
        identity: CompilerModuleHandoffIdentityV1(published.identity),
        length: published.length,
    })
    .map_err(HandoffEngineError::into_v1)
}

fn publish_in_slot_engine<S: HandoffSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_slot: S::Slot,
    binding: S::Binding,
    handoff_bytes: &[u8],
    hooks: &mut impl HandoffHooks,
) -> Result<PublishedHandoff<S>, HandoffEngineError> {
    validate_handoff_size::<S>(handoff_bytes.len())?;
    let output = PinnedOutput::open(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    prepare_publication_attempt::<S>(&output, producer, attempt)?;
    authorize_for_custody(&output, producer, attempt, S::ATTEMPT_CUSTODY)?;
    let producer_id = producer_identity_for::<S>(producer);
    let slot_id = slot_identity_for::<S>(producer_id, attempt, handoff_slot);
    let parent = open_or_create_private_directory(
        &output.fd,
        &output.display_path,
        &format!("{}{}", S::PARENT_PREFIX, hex(&producer_id)),
        hooks,
    )?;
    cleanup_stale_slots::<S>(&parent, producer_id, attempt)?;
    let slot = open_or_create_private_directory(
        &parent.fd,
        &parent.path,
        &format!("{}{}", S::SLOT_PREFIX, hex(&slot_id)),
        hooks,
    )?;
    recover_slot::<S>(&slot)?;
    if entry_exists(&slot, CONSUMED_ENTRY)? {
        read_bound_record::<S>(
            &slot,
            CONSUMED_ENTRY,
            producer_id,
            slot_id,
            attempt,
            binding,
        )?;
        cleanup_consumed_payload(&slot);
        return Err(CompilerModuleHandoffErrorV1::AlreadyConsumed.into());
    }
    if entry_exists(&slot, READY_ENTRY)? {
        let committed =
            read_bound_record::<S>(&slot, READY_ENTRY, producer_id, slot_id, attempt, binding)?;
        let committed_bytes =
            read_payload::<S>(&slot, &committed, S::MAX_DECODE_WORKING_SET_BYTES)?;
        return if committed_bytes == handoff_bytes {
            Err(CompilerModuleHandoffErrorV1::AlreadyPublished.into())
        } else {
            Err(CompilerModuleHandoffErrorV1::ConflictingPublication.into())
        };
    }

    let identity = S::derive_identity(producer_id, slot_id, attempt, binding, handoff_bytes);
    let (payload_temp, mut payload) = create_temp(&slot, "module")?;
    hooks.hit(FaultPoint::PayloadCreated)?;
    payload.write_all(handoff_bytes)?;
    hooks.hit(FaultPoint::PayloadWritten)?;
    payload.sync_all()?;
    hooks.hit(FaultPoint::PayloadSynced)?;
    renameat_with(
        &slot.fd,
        &payload_temp,
        &slot.fd,
        PAYLOAD_ENTRY,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::PayloadRenamed)?;
    fsync(&slot.fd).map_err(std::io::Error::from)?;
    slot.verify()?;
    let payload_stat = fstat(&payload).map_err(std::io::Error::from)?;
    let named =
        statat(&slot.fd, PAYLOAD_ENTRY, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !same_private_file(&payload_stat, &named, handoff_bytes.len()) {
        return Err(invalid_slot(
            &slot.path,
            "published payload does not match its pinned descriptor",
        )
        .into());
    }
    let record = HandoffRecord::<S> {
        slot: slot_id,
        attempt,
        producer: producer_id,
        binding,
        identity,
        length: handoff_bytes.len(),
        file: FileIdentity::from_stat(&named),
    };
    let record_bytes = record.encode();
    let (record_temp, mut record_file) = create_temp(&slot, "record")?;
    record_file.write_all(&record_bytes)?;
    hooks.hit(FaultPoint::RecordWritten)?;
    record_file.sync_all()?;
    hooks.hit(FaultPoint::RecordSynced)?;
    renameat_with(
        &slot.fd,
        &record_temp,
        &slot.fd,
        READY_ENTRY,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::RecordRenamed)?;
    fsync(&slot.fd).map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::PublishedSynced)?;
    slot.verify()?;
    validate_named_record::<S>(&slot, READY_ENTRY, &record_bytes)?;
    Ok(PublishedHandoff::<S> {
        attempt,
        slot: handoff_slot,
        binding,
        identity,
        length: handoff_bytes.len(),
    })
}

fn consume_with_hooks(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    hooks: &mut impl HandoffHooks,
) -> Result<ConsumedCompilerModuleHandoffV1, CompilerModuleHandoffErrorV1> {
    consume_in_slot_with_hooks(
        output_dir,
        producer,
        attempt,
        CompilerModuleHandoffSlotV1::Default,
        hooks,
    )
}

fn consume_in_slot_with_hooks(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_slot: CompilerModuleHandoffSlotV1,
    hooks: &mut impl HandoffHooks,
) -> Result<ConsumedCompilerModuleHandoffV1, CompilerModuleHandoffErrorV1> {
    consume_in_slot_engine::<HandoffV1Schema>(
        output_dir,
        producer,
        attempt,
        handoff_slot,
        (),
        hooks,
    )
    .map(|consumed| ConsumedCompilerModuleHandoffV1 {
        attempt: consumed.attempt,
        slot: consumed.slot,
        identity: CompilerModuleHandoffIdentityV1(consumed.identity),
        bytes: consumed.payload,
    })
    .map_err(HandoffEngineError::into_v1)
}

fn consume_in_slot_engine<S: HandoffSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_slot: S::Slot,
    binding: S::Binding,
    hooks: &mut impl HandoffHooks,
) -> Result<ConsumedHandoff<S>, HandoffEngineError> {
    consume_in_slot_engine_with_working_set_limit::<S>(
        output_dir,
        producer,
        attempt,
        handoff_slot,
        binding,
        S::MAX_DECODE_WORKING_SET_BYTES,
        hooks,
    )
}

fn consume_in_slot_engine_with_working_set_limit<S: HandoffSchema>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_slot: S::Slot,
    binding: S::Binding,
    maximum_working_set_bytes: usize,
    hooks: &mut impl HandoffHooks,
) -> Result<ConsumedHandoff<S>, HandoffEngineError> {
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    consume_in_slot_engine_locked::<S>(
        &output,
        producer,
        attempt,
        handoff_slot,
        binding,
        maximum_working_set_bytes,
        hooks,
    )
}

fn consume_in_slot_engine_locked<S: HandoffSchema>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    handoff_slot: S::Slot,
    binding: S::Binding,
    maximum_working_set_bytes: usize,
    hooks: &mut impl HandoffHooks,
) -> Result<ConsumedHandoff<S>, HandoffEngineError> {
    output.verify_path_identity()?;
    authorize_for_custody(output, producer, attempt, S::ATTEMPT_CUSTODY)?;
    let producer_id = producer_identity_for::<S>(producer);
    let slot_id = slot_identity_for::<S>(producer_id, attempt, handoff_slot);
    let parent = open_private_directory(
        &output.fd,
        &output.display_path,
        format!("{}{}", S::PARENT_PREFIX, hex(&producer_id)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    cleanup_stale_slots::<S>(&parent, producer_id, attempt)?;
    let slot = open_private_directory(
        &parent.fd,
        &parent.path,
        format!("{}{}", S::SLOT_PREFIX, hex(&slot_id)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    recover_slot::<S>(&slot)?;
    if entry_exists(&slot, CONSUMED_ENTRY)? {
        read_bound_record::<S>(
            &slot,
            CONSUMED_ENTRY,
            producer_id,
            slot_id,
            attempt,
            binding,
        )?;
        cleanup_consumed_payload(&slot);
        return Err(CompilerModuleHandoffErrorV1::AlreadyConsumed.into());
    }
    let record =
        read_bound_record::<S>(&slot, READY_ENTRY, producer_id, slot_id, attempt, binding)?;
    let bytes = read_payload::<S>(&slot, &record, maximum_working_set_bytes)?;
    let payload = S::decode_payload(record.binding, bytes)?;
    hooks.hit(FaultPoint::PayloadValidated)?;
    slot.verify()?;
    renameat_with(
        &slot.fd,
        READY_ENTRY,
        &slot.fd,
        CONSUMED_ENTRY,
        RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::ConsumedRenamed)?;
    fsync(&slot.fd).map_err(std::io::Error::from)?;
    hooks.hit(FaultPoint::ConsumedSynced)?;
    slot.verify()?;
    cleanup_consumed_payload(&slot);
    Ok(ConsumedHandoff::<S> {
        attempt,
        slot: handoff_slot,
        binding: record.binding,
        identity: record.identity,
        payload,
    })
}

fn authorize(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), CompilerModuleHandoffErrorV1> {
    authorize_for_custody(
        output,
        producer,
        attempt,
        HandoffAttemptCustodyV1::FrontendBuilding,
    )
}

fn prepare_publication_attempt<S: HandoffSchema>(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), CompilerModuleHandoffErrorV1> {
    if S::ATTEMPT_CUSTODY != HandoffAttemptCustodyV1::SimulationBackendClaimed {
        return Ok(());
    }
    if attempt.session() == BuildSession::DIRECT {
        return Err(attempt_error(
            "direct compiler attempts cannot own a simulation KIR handoff slot",
        ));
    }
    let mut attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|error| attempt_error(error.to_string()))?;
    if record.crate_name != producer.crate_name {
        return Err(attempt_error(
            "build attempt crate name does not match the simulation KIR producer",
        ));
    }
    match (record.phase, record.backend_receipt) {
        (AttemptPhase::Building, None) => {
            attempts
                .claim_backend(&producer.stable_source, attempt)
                .map_err(|error| attempt_error(error.to_string()))?;
            commit_attempt_registry_direct(output, &attempts)?;
            Ok(())
        }
        (AttemptPhase::BackendClaimed, None) => Ok(()),
        _ => Err(CompilerModuleHandoffErrorV1::AttemptNotClaimable),
    }
}

fn authorize_for_custody(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    custody: HandoffAttemptCustodyV1,
) -> Result<(), CompilerModuleHandoffErrorV1> {
    if attempt.session() == BuildSession::DIRECT {
        return Err(attempt_error(
            "direct compiler attempts cannot own a handoff slot",
        ));
    }
    let attempts = read_attempt_registry(output)?;
    let record = attempts
        .record_exact(&producer.stable_source, attempt)
        .map_err(|error| attempt_error(error.to_string()))?;
    if record.crate_name != producer.crate_name {
        return Err(attempt_error(
            "build attempt crate name does not match the producer",
        ));
    }
    let claimable = match custody {
        HandoffAttemptCustodyV1::FrontendBuilding => {
            record.phase == AttemptPhase::Building && record.backend_receipt.is_none()
        }
        HandoffAttemptCustodyV1::SimulationBackendClaimed => {
            record.phase == AttemptPhase::BackendClaimed && record.backend_receipt.is_none()
        }
    };
    if !claimable {
        return Err(CompilerModuleHandoffErrorV1::AttemptNotClaimable);
    }
    Ok(())
}

fn producer_identity_for<S: HandoffSchema>(producer: &ProducerIdentity) -> [u8; 32] {
    sha256_parts(&[
        S::PRODUCER_DOMAIN,
        &(producer.stable_source.len() as u64).to_le_bytes(),
        producer.stable_source.as_bytes(),
        &(producer.crate_name.len() as u64).to_le_bytes(),
        producer.crate_name.as_bytes(),
    ])
}

#[cfg(test)]
fn producer_identity(producer: &ProducerIdentity) -> [u8; 32] {
    producer_identity_for::<HandoffV1Schema>(producer)
}

fn slot_identity_for<S: HandoffSchema>(
    producer: [u8; 32],
    attempt: BuildAttempt,
    slot: S::Slot,
) -> [u8; 32] {
    let generation = attempt.generation().to_le_bytes();
    if slot == S::default_slot() {
        return sha256_parts(&[
            S::SLOT_DOMAIN,
            &producer,
            &generation,
            attempt.session().as_bytes(),
            attempt.invocation().as_bytes(),
        ]);
    }
    sha256_parts(&[
        S::NAMED_SLOT_DOMAIN,
        &producer,
        &generation,
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
        &[S::slot_tag(slot)],
    ])
}

#[cfg(test)]
fn slot_identity(
    producer: [u8; 32],
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
) -> [u8; 32] {
    slot_identity_for::<HandoffV1Schema>(producer, attempt, slot)
}

fn open_or_create_private_directory(
    parent_fd: &OwnedFd,
    parent_path: &Path,
    name: &str,
    hooks: &mut impl HandoffHooks,
) -> Result<PinnedDirectory, CompilerModuleHandoffErrorV1> {
    match mkdirat(parent_fd, name, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
        Ok(()) => {
            fsync(parent_fd).map_err(std::io::Error::from)?;
            hooks.hit(FaultPoint::DirectoryCreated)?;
        }
        Err(error) if error == rustix::io::Errno::EXIST => {}
        Err(error) => return Err(std::io::Error::from(error).into()),
    }
    open_private_directory(parent_fd, parent_path, name)?.ok_or_else(|| {
        invalid_slot(
            &parent_path.join(name),
            "directory disappeared while opening",
        )
    })
}

fn open_private_directory(
    parent_fd: &OwnedFd,
    parent_path: &Path,
    name: impl AsRef<Path>,
) -> Result<Option<PinnedDirectory>, CompilerModuleHandoffErrorV1> {
    let name = name.as_ref();
    let fd = match openat(
        parent_fd,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
        Err(error) => {
            return Err(invalid_slot(
                &parent_path.join(name),
                std::io::Error::from(error).to_string(),
            ));
        }
    };
    let stat = fstat(&fd).map_err(std::io::Error::from)?;
    if !is_private_directory(&stat) {
        return Err(invalid_slot(
            &parent_path.join(name),
            "expected a private 0700 directory",
        ));
    }
    let directory = PinnedDirectory {
        fd,
        parent_fd: rustix::io::fcntl_dupfd_cloexec(parent_fd, 0).map_err(std::io::Error::from)?,
        name: name.to_path_buf(),
        path: parent_path.join(name),
        device: stat.st_dev,
        inode: stat.st_ino,
    };
    directory.verify()?;
    Ok(Some(directory))
}

fn cleanup_stale_slots<S: HandoffSchema>(
    parent: &PinnedDirectory,
    producer: [u8; 32],
    attempt: BuildAttempt,
) -> Result<(), HandoffEngineError> {
    let current = S::ALL_SLOTS
        .iter()
        .copied()
        .map(|slot| {
            format!(
                "{}{}",
                S::SLOT_PREFIX,
                hex(&slot_identity_for::<S>(producer, attempt, slot))
            )
        })
        .collect::<Vec<_>>();
    let scan = rustix::io::fcntl_dupfd_cloexec(&parent.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = Dir::read_from(&scan).map_err(std::io::Error::from)?;
    let mut stale = Vec::new();
    let maximum_entries = MAX_STALE_SLOTS.checked_add(current.len()).ok_or_else(|| {
        invalid_slot(
            &parent.path,
            "producer directory scan entry bound overflowed",
        )
    })?;
    let mut scanned_entries = 0usize;
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name_bytes = entry.file_name().to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }
        scanned_entries = scanned_entries.checked_add(1).ok_or_else(|| {
            invalid_slot(
                &parent.path,
                "producer directory entry count overflowed during stale cleanup",
            )
        })?;
        if scanned_entries > maximum_entries {
            return Err(invalid_slot(&parent.path, "too many stale handoff slots").into());
        }
        if current
            .iter()
            .any(|current| name_bytes == current.as_bytes())
        {
            continue;
        }
        let name = PathBuf::from(std::ffi::OsStr::from_bytes(name_bytes));
        if !name_bytes.starts_with(S::SLOT_PREFIX.as_bytes()) {
            return Err(invalid_slot(
                &parent.path.join(&name),
                "unexpected producer handoff entry",
            )
            .into());
        }
        if stale.len() == MAX_STALE_SLOTS {
            return Err(invalid_slot(&parent.path, "too many stale handoff slots").into());
        }
        stale.push(name);
    }
    for name in stale {
        remove_slot_entry(parent, &name)?;
    }
    if parent_entry_count(parent, current.len())? > current.len() {
        return Err(invalid_slot(
            &parent.path,
            "handoff producer directory exceeds its entry bound",
        )
        .into());
    }
    parent.verify()?;
    Ok(())
}

fn remove_slot_entry(
    parent: &PinnedDirectory,
    name: &Path,
) -> Result<(), CompilerModuleHandoffErrorV1> {
    let stat = statat(&parent.fd, name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return Err(invalid_slot(
            &parent.path.join(name),
            "stale slot is not a directory",
        ));
    }
    let slot = open_private_directory(&parent.fd, &parent.path, name)?
        .ok_or_else(|| invalid_slot(&parent.path.join(name), "stale slot disappeared"))?;
    remove_all_slot_entries(&slot)?;
    slot.verify()?;
    unlinkat(&parent.fd, name, AtFlags::REMOVEDIR).map_err(std::io::Error::from)?;
    fsync(&parent.fd).map_err(std::io::Error::from)?;
    Ok(())
}

fn remove_all_slot_entries(slot: &PinnedDirectory) -> Result<(), CompilerModuleHandoffErrorV1> {
    let scan = rustix::io::fcntl_dupfd_cloexec(&slot.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = Dir::read_from(&scan).map_err(std::io::Error::from)?;
    let mut names = Vec::new();
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name_bytes = entry.file_name().to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }
        if names.len() == MAX_SLOT_ENTRIES {
            return Err(invalid_slot(&slot.path, "slot exceeds its entry bound"));
        }
        let name = PathBuf::from(std::ffi::OsStr::from_bytes(name_bytes));
        let stat =
            statat(&slot.fd, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            return Err(invalid_slot(
                &slot.path.join(&name),
                "nested directories are forbidden",
            ));
        }
        names.push(name);
    }
    for name in names {
        unlinkat(&slot.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
    }
    fsync(&slot.fd).map_err(std::io::Error::from)?;
    Ok(())
}

fn recover_slot<S: HandoffSchema>(slot: &PinnedDirectory) -> Result<(), HandoffEngineError> {
    let names = slot_entries(slot)?;
    for name in &names {
        let name_bytes = name.as_os_str().as_bytes();
        if !matches!(name_bytes, b"module" | b"ready" | b"consumed")
            && !name_bytes.starts_with(TEMP_PREFIX.as_bytes())
        {
            return Err(invalid_slot(&slot.path.join(name), "unexpected slot entry").into());
        }
    }
    if names.iter().any(|name| name == READY_ENTRY)
        && names.iter().any(|name| name == CONSUMED_ENTRY)
    {
        return Err(invalid_slot(&slot.path, "ready and consumed records coexist").into());
    }
    let committed_entry = names.iter().find_map(|name| {
        let name = name.as_os_str().as_bytes();
        if name == READY_ENTRY.as_bytes() {
            Some(READY_ENTRY)
        } else if name == CONSUMED_ENTRY.as_bytes() {
            Some(CONSUMED_ENTRY)
        } else {
            None
        }
    });
    let residue = names
        .into_iter()
        .filter(|name| {
            let name = name.as_os_str().as_bytes();
            name.starts_with(TEMP_PREFIX.as_bytes())
                || (committed_entry.is_none() && name == PAYLOAD_ENTRY.as_bytes())
        })
        .collect::<Vec<_>>();
    for name in &residue {
        reject_nonregular_before_cleanup(slot, name)?;
    }
    if S::VALIDATE_RECORD_DURING_RECOVERY
        && let Some(entry) = committed_entry
    {
        let record = read_private_file(slot, entry, S::RECORD_BYTES)?
            .ok_or_else(|| invalid_slot(&slot.path.join(entry), "record disappeared"))?;
        HandoffRecord::<S>::decode(&record)
            .map_err(|reason| invalid_slot(&slot.path.join(entry), reason))?;
    }
    for name in residue {
        let name_bytes = name.as_os_str().as_bytes();
        if name_bytes.starts_with(TEMP_PREFIX.as_bytes()) || name_bytes == PAYLOAD_ENTRY.as_bytes()
        {
            unlinkat(&slot.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        }
    }
    if committed_entry.is_none() {
        fsync(&slot.fd).map_err(std::io::Error::from)?;
    }
    slot.verify()?;
    Ok(())
}

fn slot_entries(slot: &PinnedDirectory) -> Result<Vec<PathBuf>, CompilerModuleHandoffErrorV1> {
    let scan = rustix::io::fcntl_dupfd_cloexec(&slot.fd, 0).map_err(std::io::Error::from)?;
    let mut directory = Dir::read_from(&scan).map_err(std::io::Error::from)?;
    let mut names = Vec::new();
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name_bytes = entry.file_name().to_bytes();
        if name_bytes == b"." || name_bytes == b".." {
            continue;
        }
        if names.len() == MAX_SLOT_ENTRIES {
            return Err(invalid_slot(&slot.path, "slot exceeds its entry bound"));
        }
        names.push(PathBuf::from(std::ffi::OsStr::from_bytes(name_bytes)));
    }
    Ok(names)
}

fn parent_entry_count(
    parent: &PinnedDirectory,
    maximum: usize,
) -> Result<usize, CompilerModuleHandoffErrorV1> {
    let scan = rustix::io::fcntl_dupfd_cloexec(&parent.fd, 0).map_err(std::io::Error::from)?;
    let mut directory = Dir::read_from(&scan).map_err(std::io::Error::from)?;
    let mut count = 0usize;
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            count = count.checked_add(1).ok_or_else(|| {
                invalid_slot(
                    &parent.path,
                    "producer directory entry count overflowed during verification",
                )
            })?;
            if count > maximum {
                return Ok(count);
            }
        }
    }
    Ok(count)
}

fn create_temp(
    slot: &PinnedDirectory,
    purpose: &str,
) -> Result<(String, fs::File), CompilerModuleHandoffErrorV1> {
    let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
    for offset in 0..MAX_TEMP_ATTEMPTS {
        let name = format!(
            "{TEMP_PREFIX}{purpose}-{}-{}",
            std::process::id(),
            start.wrapping_add(offset)
        );
        match openat(
            &slot.fd,
            &name,
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        ) {
            Ok(fd) => return Ok((name, fs::File::from(fd))),
            Err(error) if error == rustix::io::Errno::EXIST => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Err(invalid_slot(
        &slot.path,
        "could not reserve a private temporary entry",
    ))
}

fn read_payload<S: HandoffSchema>(
    slot: &PinnedDirectory,
    record: &HandoffRecord<S>,
    maximum_working_set_bytes: usize,
) -> Result<Vec<u8>, HandoffEngineError> {
    let fd = openat(
        &slot.fd,
        PAYLOAD_ENTRY,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        invalid_slot(
            &slot.path.join(PAYLOAD_ENTRY),
            std::io::Error::from(error).to_string(),
        )
    })?;
    let mut file = fs::File::from(fd);
    let before = fstat(&file).map_err(std::io::Error::from)?;
    let named =
        statat(&slot.fd, PAYLOAD_ENTRY, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !record.file.matches(&before) || !record.file.matches(&named) {
        return Err(invalid_slot(
            &slot.path.join(PAYLOAD_ENTRY),
            "payload identity metadata mismatch",
        )
        .into());
    }
    validate_decode_working_set::<S>(record.length, maximum_working_set_bytes)?;
    let mut bytes = try_allocate_payload_buffer(record.length)?;
    Read::by_ref(&mut file)
        .take(record.length as u64)
        .read_to_end(&mut bytes)?;
    let mut trailing = [0_u8; 1];
    let has_trailing_bytes = file.read(&mut trailing)? != 0;
    let after = fstat(&file).map_err(std::io::Error::from)?;
    let still_named =
        statat(&slot.fd, PAYLOAD_ENTRY, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if bytes.len() != record.length
        || has_trailing_bytes
        || !record.file.matches(&after)
        || !record.file.matches(&still_named)
    {
        return Err(invalid_slot(
            &slot.path.join(PAYLOAD_ENTRY),
            "payload changed while its descriptor was read",
        )
        .into());
    }
    let identity = S::derive_identity(
        record.producer,
        record.slot,
        record.attempt,
        record.binding,
        &bytes,
    );
    if identity != record.identity {
        return Err(CompilerModuleHandoffErrorV1::DigestMismatch.into());
    }
    Ok(bytes)
}

fn validate_decode_working_set<S: HandoffSchema>(
    payload_bytes: usize,
    maximum: usize,
) -> Result<usize, HandoffEngineError> {
    let required = payload_bytes
        .checked_mul(S::DECODE_WORKING_SET_MULTIPLIER)
        .and_then(|bytes| bytes.checked_add(S::DECODE_WORKING_SET_FIXED_BYTES))
        .ok_or(HandoffEngineError::WorkingSetBudgetExceeded {
            required: usize::MAX,
            maximum,
        })?;
    if required > maximum {
        return Err(HandoffEngineError::WorkingSetBudgetExceeded { required, maximum });
    }
    Ok(required)
}

fn try_allocate_payload_buffer(length: usize) -> Result<Vec<u8>, HandoffEngineError> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| HandoffEngineError::PayloadAllocationFailed { requested: length })?;
    Ok(bytes)
}

fn read_bound_record<S: HandoffSchema>(
    slot: &PinnedDirectory,
    entry: &str,
    producer: [u8; 32],
    slot_identity: [u8; 32],
    attempt: BuildAttempt,
    binding: S::Binding,
) -> Result<HandoffRecord<S>, HandoffEngineError> {
    let bytes = read_private_file(slot, entry, S::RECORD_BYTES)?
        .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    let record = HandoffRecord::<S>::decode(&bytes)
        .map_err(|reason| invalid_slot(&slot.path.join(entry), reason))?;
    if record.slot != slot_identity || record.producer != producer || record.attempt != attempt {
        return Err(invalid_slot(
            &slot.path.join(entry),
            "record binding does not match the requested attempt and producer",
        )
        .into());
    }
    if record.binding != binding {
        return Err(HandoffEngineError::WrongBinding);
    }
    Ok(record)
}

fn read_private_file(
    directory: &PinnedDirectory,
    entry: &str,
    exact_length: usize,
) -> Result<Option<Vec<u8>>, CompilerModuleHandoffErrorV1> {
    let fd = match openat(
        &directory.fd,
        entry,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
        Err(error) => {
            return Err(invalid_slot(
                &directory.path.join(entry),
                std::io::Error::from(error).to_string(),
            ));
        }
    };
    let mut file = fs::File::from(fd);
    let before = fstat(&file).map_err(std::io::Error::from)?;
    if !is_private_file(&before) || usize::try_from(before.st_size).ok() != Some(exact_length) {
        return Err(invalid_slot(
            &directory.path.join(entry),
            "expected a private single-link regular file with canonical length",
        ));
    }
    let mut bytes = Vec::with_capacity(exact_length);
    Read::by_ref(&mut file)
        .take((exact_length + 1) as u64)
        .read_to_end(&mut bytes)?;
    let after = fstat(&file).map_err(std::io::Error::from)?;
    let named =
        statat(&directory.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if bytes.len() != exact_length
        || !same_private_file(&before, &after, exact_length)
        || !same_private_file(&before, &named, exact_length)
    {
        return Err(invalid_slot(
            &directory.path.join(entry),
            "file changed while its descriptor was read",
        ));
    }
    Ok(Some(bytes))
}

fn validate_named_record<S: HandoffSchema>(
    slot: &PinnedDirectory,
    entry: &str,
    expected: &[u8],
) -> Result<(), HandoffEngineError> {
    let actual = read_private_file(slot, entry, S::RECORD_BYTES)?
        .ok_or_else(|| invalid_slot(&slot.path.join(entry), "record disappeared after commit"))?;
    if actual != expected {
        return Err(invalid_slot(&slot.path.join(entry), "record changed after commit").into());
    }
    Ok(())
}

fn entry_exists(slot: &PinnedDirectory, entry: &str) -> Result<bool, CompilerModuleHandoffErrorV1> {
    match statat(&slot.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if !is_private_file(&stat) {
                return Err(invalid_slot(
                    &slot.path.join(entry),
                    "entry is not a private single-link regular file",
                ));
            }
            Ok(true)
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(false),
        Err(error) => Err(std::io::Error::from(error).into()),
    }
}

fn cleanup_consumed_payload(slot: &PinnedDirectory) {
    if let Ok(stat) = statat(&slot.fd, PAYLOAD_ENTRY, AtFlags::SYMLINK_NOFOLLOW)
        && is_private_file(&stat)
        && unlinkat(&slot.fd, PAYLOAD_ENTRY, AtFlags::empty()).is_ok()
    {
        let _ = fsync(&slot.fd);
    }
}

fn reject_nonregular_before_cleanup(
    slot: &PinnedDirectory,
    entry: &Path,
) -> Result<(), CompilerModuleHandoffErrorV1> {
    let stat = statat(&slot.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if !is_private_file(&stat) {
        return Err(invalid_slot(
            &slot.path.join(entry),
            "recovery residue is not a private single-link regular file",
        ));
    }
    Ok(())
}

fn validate_handoff_size<S: HandoffSchema>(
    length: usize,
) -> Result<(), CompilerModuleHandoffErrorV1> {
    if length == 0 || length > S::MAX_HANDOFF_BYTES {
        return Err(CompilerModuleHandoffErrorV1::InvalidHandoffSize {
            actual: length,
            maximum: S::MAX_HANDOFF_BYTES,
        });
    }
    Ok(())
}

fn is_private_directory(stat: &rustix::fs::Stat) -> bool {
    FileType::from_raw_mode(stat.st_mode) == FileType::Directory && stat.st_mode & 0o777 == 0o700
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

fn invalid_slot(path: &Path, reason: impl Into<String>) -> CompilerModuleHandoffErrorV1 {
    CompilerModuleHandoffErrorV1::InvalidSlot {
        path: path.to_path_buf(),
        reason: reason.into(),
    }
}

fn attempt_error(reason: impl Into<String>) -> CompilerModuleHandoffErrorV1 {
    CompilerModuleHandoffErrorV1::Attempt {
        reason: reason.into(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FaultPoint {
    DirectoryCreated,
    PayloadCreated,
    PayloadWritten,
    PayloadSynced,
    PayloadRenamed,
    RecordWritten,
    RecordSynced,
    RecordRenamed,
    PublishedSynced,
    PayloadValidated,
    ConsumedRenamed,
    ConsumedSynced,
}

trait HandoffHooks {
    fn hit(&mut self, _point: FaultPoint) -> std::io::Result<()> {
        Ok(())
    }
}

struct NoFaults;
impl HandoffHooks for NoFaults {}

mod protected_v2 {
    use super::*;

    const PARENT_PREFIX_V2: &str = ".fe2o3-compiler-module-handoff-v2-";
    const SLOT_PREFIX_V2: &str = "attempt-";
    const RECORD_MAGIC_V2: &[u8] = b"FE2O3-COMPILER-MODULE-HANDOFF-V2\0";
    const RECORD_VERSION_V2: u16 = 2;
    const PRODUCER_DOMAIN_V2: &[u8] = b"fe2o3.compiler-module-handoff.producer.v2\0";
    const SLOT_DOMAIN_V2: &[u8] = b"fe2o3.compiler-module-handoff.slot.v2\0";
    const NAMED_SLOT_DOMAIN_V2: &[u8] = b"fe2o3.compiler-module-handoff.named-slot.v2\0";
    const HANDOFF_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.compiler-module-handoff.identity.v2\0";
    const RECORD_DOMAIN_V2: &[u8] = b"fe2o3.compiler-module-handoff.record.v2\0";
    const COMPILER_CLOSURE_BYTES_V2: usize = (6 * 32) + 2 + 32;
    const RECORD_BYTES_V2: usize = RECORD_MAGIC_V2.len()
        + 2
        + 32
        + 8
        + 16
        + 32
        + 32
        + COMPILER_CLOSURE_BYTES_V2
        + 32
        + 8
        + (7 * 8)
        + 32;

    /// Closed attempt-local transport slot for a closure-protected compiler module handoff.
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(u8)]
    pub enum CompilerModuleHandoffSlotV2 {
        /// Original one-module transport slot.
        Default = 0,
        /// Issue #138 reference wave64 XOR4 schedule.
        GeneralGemmReference = 1,
        /// Issue #138 A-only BF16 vector-transfer schedule.
        GeneralGemmVectorizedAOnly = 2,
    }

    /// SHA-256 commitment to a V2 closure, slot, attempt, producer, and exact module bytes.
    ///
    /// The identity authenticates equality inside this cooperative protocol only. It grants no
    /// compiler, publication, linking, loading, launch, or execution authority.
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct CompilerModuleHandoffIdentityV2([u8; 32]);

    impl CompilerModuleHandoffIdentityV2 {
        /// Constructs an identity from its exact SHA-256 representation.
        pub const fn from_bytes(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }

        /// Returns the exact SHA-256 representation.
        pub const fn as_bytes(&self) -> &[u8; 32] {
            &self.0
        }
    }

    /// Durable inert receipt for one closure-protected compiler module handoff.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CompilerModuleHandoffReceiptV2 {
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV2,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        identity: CompilerModuleHandoffIdentityV2,
        length: usize,
    }

    impl CompilerModuleHandoffReceiptV2 {
        pub const fn attempt(self) -> BuildAttempt {
            self.attempt
        }

        /// Returns the exact attempt-local transport slot.
        pub const fn slot(self) -> CompilerModuleHandoffSlotV2 {
            self.slot
        }

        /// Returns the complete canonical compiler-closure preimage bound to the handoff.
        pub const fn compiler_closure(self) -> fe2o3_build_authority::CompilerClosureV2 {
            self.compiler_closure
        }

        pub const fn identity(self) -> CompilerModuleHandoffIdentityV2 {
            self.identity
        }

        pub const fn length(self) -> usize {
            self.length
        }

        /// A handoff receipt is inert coordination evidence.
        pub const fn grants_publication_authority(self) -> bool {
            false
        }

        /// Closure possession does not authenticate compiler authorship.
        pub const fn grants_compiler_authority(self) -> bool {
            false
        }
    }

    /// Immutable bytes returned by the one successful consumption of a V2 handoff slot.
    #[derive(Clone, Debug)]
    pub struct ConsumedCompilerModuleHandoffV2 {
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV2,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        identity: CompilerModuleHandoffIdentityV2,
        bytes: Arc<[u8]>,
    }

    impl ConsumedCompilerModuleHandoffV2 {
        pub const fn attempt(&self) -> BuildAttempt {
            self.attempt
        }

        /// Returns the exact attempt-local transport slot.
        pub const fn slot(&self) -> CompilerModuleHandoffSlotV2 {
            self.slot
        }

        /// Returns the complete canonical compiler-closure preimage bound to the bytes.
        pub const fn compiler_closure(&self) -> fe2o3_build_authority::CompilerClosureV2 {
            self.compiler_closure
        }

        pub const fn identity(&self) -> CompilerModuleHandoffIdentityV2 {
            self.identity
        }

        pub fn bytes(&self) -> &[u8] {
            &self.bytes
        }

        /// Consumed bytes still require the finalizer's independent validation chain.
        pub const fn grants_publication_authority(&self) -> bool {
            false
        }

        /// Closure possession does not authenticate compiler authorship.
        pub const fn grants_compiler_authority(&self) -> bool {
            false
        }

        /// Consumed handoff bytes do not authorize linking.
        pub const fn grants_link_authority(&self) -> bool {
            false
        }

        /// Consumed handoff bytes do not authorize module loading.
        pub const fn grants_load_authority(&self) -> bool {
            false
        }

        /// Consumed handoff bytes do not authorize kernel launch.
        pub const fn grants_launch_authority(&self) -> bool {
            false
        }
    }

    /// Failure to publish, recover, or consume a closure-protected compiler module handoff.
    #[derive(Debug)]
    pub enum CompilerModuleHandoffErrorV2 {
        Io(std::io::Error),
        Attempt { reason: String },
        InvalidSlot { path: PathBuf, reason: String },
        InvalidHandoffSize { actual: usize, maximum: usize },
        AlreadyPublished,
        ConflictingPublication,
        AlreadyConsumed,
        NotPublished,
        DigestMismatch,
        WrongCompilerClosure,
    }

    impl fmt::Display for CompilerModuleHandoffErrorV2 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Io(error) => write!(formatter, "{error}"),
                Self::Attempt { reason } => {
                    write!(formatter, "invalid V2 build-attempt handoff: {reason}")
                }
                Self::InvalidSlot { path, reason } => write!(
                    formatter,
                    "invalid V2 compiler module handoff {}: {reason}",
                    path.display()
                ),
                Self::InvalidHandoffSize { actual, maximum } => write!(
                    formatter,
                    "canonical compiler module handoff size {actual} is outside 1..={maximum} bytes"
                ),
                Self::AlreadyPublished => {
                    formatter.write_str("V2 compiler module handoff is already published")
                }
                Self::ConflictingPublication => formatter
                    .write_str("V2 compiler module handoff conflicts with the committed module"),
                Self::AlreadyConsumed => {
                    formatter.write_str("V2 compiler module handoff was already consumed")
                }
                Self::NotPublished => {
                    formatter.write_str("V2 compiler module handoff is not published")
                }
                Self::DigestMismatch => formatter
                    .write_str("V2 compiler module handoff closure-bound identity mismatch"),
                Self::WrongCompilerClosure => formatter.write_str(
                    "V2 compiler module handoff is bound to a different compiler closure",
                ),
            }
        }
    }

    impl std::error::Error for CompilerModuleHandoffErrorV2 {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Io(error) => Some(error),
                _ => None,
            }
        }
    }

    impl From<std::io::Error> for CompilerModuleHandoffErrorV2 {
        fn from(error: std::io::Error) -> Self {
            Self::Io(error)
        }
    }

    impl From<EmitError> for CompilerModuleHandoffErrorV2 {
        fn from(error: EmitError) -> Self {
            match error {
                EmitError::BuildAttempt { reason } => Self::Attempt { reason },
                error => Self::InvalidSlot {
                    path: PathBuf::new(),
                    reason: error.to_string(),
                },
            }
        }
    }

    impl From<CompilerModuleHandoffErrorV1> for CompilerModuleHandoffErrorV2 {
        fn from(error: CompilerModuleHandoffErrorV1) -> Self {
            match error {
                CompilerModuleHandoffErrorV1::Io(error) => Self::Io(error),
                CompilerModuleHandoffErrorV1::Attempt { reason } => Self::Attempt { reason },
                CompilerModuleHandoffErrorV1::AttemptNotClaimable => Self::Attempt {
                    reason: "build attempt is not in the claimable building phase".to_owned(),
                },
                CompilerModuleHandoffErrorV1::InvalidSlot { path, reason } => {
                    Self::InvalidSlot { path, reason }
                }
                CompilerModuleHandoffErrorV1::InvalidHandoffSize { actual, maximum } => {
                    Self::InvalidHandoffSize { actual, maximum }
                }
                CompilerModuleHandoffErrorV1::AlreadyPublished => Self::AlreadyPublished,
                CompilerModuleHandoffErrorV1::ConflictingPublication => {
                    Self::ConflictingPublication
                }
                CompilerModuleHandoffErrorV1::AlreadyConsumed => Self::AlreadyConsumed,
                CompilerModuleHandoffErrorV1::NotPublished => Self::NotPublished,
                CompilerModuleHandoffErrorV1::DigestMismatch => Self::DigestMismatch,
            }
        }
    }

    /// Atomically publishes bytes under one attempt and complete canonical compiler closure.
    ///
    /// Success establishes only a durable cooperative-protocol commitment. The receipt remains inert
    /// and does not claim that any compiler authored the bytes or authorize their publication.
    pub fn publish_compiler_module_handoff_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        handoff_bytes: &[u8],
    ) -> Result<CompilerModuleHandoffReceiptV2, CompilerModuleHandoffErrorV2> {
        publish_with_hooks_v2(
            output_dir,
            producer,
            attempt,
            compiler_closure,
            handoff_bytes,
            &mut NoFaults,
        )
    }

    /// Atomically publishes closure-protected bytes in one closed attempt-local named slot.
    pub fn publish_compiler_module_handoff_in_slot_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV2,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        handoff_bytes: &[u8],
    ) -> Result<CompilerModuleHandoffReceiptV2, CompilerModuleHandoffErrorV2> {
        publish_in_slot_with_hooks_v2(
            output_dir,
            producer,
            attempt,
            slot,
            compiler_closure,
            handoff_bytes,
            &mut NoFaults,
        )
    }

    /// Consumes one attempt's handoff exactly once under the expected compiler closure.
    pub fn consume_compiler_module_handoff_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
    ) -> Result<ConsumedCompilerModuleHandoffV2, CompilerModuleHandoffErrorV2> {
        consume_with_hooks_v2(
            output_dir,
            producer,
            attempt,
            compiler_closure,
            &mut NoFaults,
        )
    }

    /// Consumes one closure-protected named slot exactly once.
    pub fn consume_compiler_module_handoff_in_slot_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV2,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
    ) -> Result<ConsumedCompilerModuleHandoffV2, CompilerModuleHandoffErrorV2> {
        consume_in_slot_with_hooks_v2(
            output_dir,
            producer,
            attempt,
            slot,
            compiler_closure,
            &mut NoFaults,
        )
    }

    pub(super) struct HandoffV2Schema;

    impl HandoffSchema for HandoffV2Schema {
        type Slot = CompilerModuleHandoffSlotV2;
        type Binding = fe2o3_build_authority::CompilerClosureV2;
        type Payload = Arc<[u8]>;

        const PARENT_PREFIX: &'static str = PARENT_PREFIX_V2;
        const SLOT_PREFIX: &'static str = SLOT_PREFIX_V2;
        const RECORD_MAGIC: &'static [u8] = RECORD_MAGIC_V2;
        const RECORD_VERSION: u16 = RECORD_VERSION_V2;
        const PRODUCER_DOMAIN: &'static [u8] = PRODUCER_DOMAIN_V2;
        const SLOT_DOMAIN: &'static [u8] = SLOT_DOMAIN_V2;
        const NAMED_SLOT_DOMAIN: &'static [u8] = NAMED_SLOT_DOMAIN_V2;
        const RECORD_DOMAIN: &'static [u8] = RECORD_DOMAIN_V2;
        const RECORD_BYTES: usize = RECORD_BYTES_V2;
        const MAX_HANDOFF_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES;
        const DECODE_WORKING_SET_MULTIPLIER: usize = 1;
        const DECODE_WORKING_SET_FIXED_BYTES: usize = 0;
        const MAX_DECODE_WORKING_SET_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES;
        const VALIDATE_RECORD_DURING_RECOVERY: bool = true;
        const ALL_SLOTS: &'static [Self::Slot] = &[
            CompilerModuleHandoffSlotV2::Default,
            CompilerModuleHandoffSlotV2::GeneralGemmReference,
            CompilerModuleHandoffSlotV2::GeneralGemmVectorizedAOnly,
        ];

        fn default_slot() -> Self::Slot {
            CompilerModuleHandoffSlotV2::Default
        }

        fn slot_tag(slot: Self::Slot) -> u8 {
            slot as u8
        }

        fn encode_binding(binding: Self::Binding, bytes: &mut Vec<u8>) {
            bytes.extend_from_slice(&compiler_closure_bytes_v2(binding));
        }

        fn decode_binding(decoder: &mut Decoder<'_>) -> Result<Self::Binding, &'static str> {
            decode_compiler_closure_v2(decoder.take(COMPILER_CLOSURE_BYTES_V2)?)
        }

        fn binding_matches_length(_binding: Self::Binding, _length: usize) -> bool {
            true
        }

        fn decode_payload(
            _binding: Self::Binding,
            bytes: Vec<u8>,
        ) -> Result<Self::Payload, HandoffEngineError> {
            Ok(Arc::from(bytes))
        }

        fn derive_identity(
            producer: [u8; 32],
            slot: [u8; 32],
            attempt: BuildAttempt,
            binding: Self::Binding,
            handoff_bytes: &[u8],
        ) -> [u8; 32] {
            let generation = attempt.generation().to_le_bytes();
            let length = (handoff_bytes.len() as u64).to_le_bytes();
            sha256_parts(&[
                HANDOFF_IDENTITY_DOMAIN_V2,
                &compiler_closure_bytes_v2(binding),
                &slot,
                &producer,
                &generation,
                attempt.session().as_bytes(),
                attempt.invocation().as_bytes(),
                &length,
                handoff_bytes,
            ])
        }
    }

    #[cfg(test)]
    type HandoffRecordV2 = HandoffRecord<HandoffV2Schema>;

    fn compiler_closure_bytes_v2(
        closure: fe2o3_build_authority::CompilerClosureV2,
    ) -> [u8; COMPILER_CLOSURE_BYTES_V2] {
        let mut bytes = [0; COMPILER_CLOSURE_BYTES_V2];
        bytes[0..32].copy_from_slice(&closure.cargo_executable_sha256());
        bytes[32..64].copy_from_slice(&closure.cargo_binding_trampoline_sha256());
        bytes[64..96].copy_from_slice(&closure.cargo_fe2o3_binding_wrapper_sha256());
        bytes[96..128].copy_from_slice(&closure.rustc_executable_sha256());
        bytes[128..160].copy_from_slice(&closure.rustc_runtime_tree_sha256());
        bytes[160..192].copy_from_slice(&closure.codegen_backend_sha256());
        bytes[192..194].copy_from_slice(
            &closure
                .cargo_binding_transition_protocol_version()
                .to_le_bytes(),
        );
        bytes[194..226].copy_from_slice(&closure.identity_sha256());
        bytes
    }

    fn decode_compiler_closure_v2(
        bytes: &[u8],
    ) -> Result<fe2o3_build_authority::CompilerClosureV2, &'static str> {
        let mut decoder = Decoder::new(bytes);
        let cargo_executable = decoder.array()?;
        let cargo_binding_trampoline = decoder.array()?;
        let cargo_fe2o3_binding_wrapper = decoder.array()?;
        let rustc_executable = decoder.array()?;
        let rustc_runtime_tree = decoder.array()?;
        let codegen_backend = decoder.array()?;
        let transition_protocol = decoder.u16()?;
        let identity = decoder.array()?;
        if !decoder.finished() {
            return Err("compiler closure preimage has trailing bytes");
        }
        fe2o3_build_authority::CompilerClosureV2::from_pins_and_identity(
            cargo_executable,
            cargo_binding_trampoline,
            cargo_fe2o3_binding_wrapper,
            rustc_executable,
            rustc_runtime_tree,
            codegen_backend,
            transition_protocol,
            identity,
        )
        .map_err(|error| {
            match error {
        fe2o3_build_authority::CompilerClosureErrorV2::ZeroDigest { .. } => {
            "compiler closure preimage contains a zero digest"
        }
        fe2o3_build_authority::CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion {
            ..
        } => "compiler closure preimage has an unsupported transition protocol version",
        fe2o3_build_authority::CompilerClosureErrorV2::IdentityMismatch => {
            "compiler closure preimage identity mismatch"
        }
        _ => "compiler closure preimage is not canonical",
    }
        })
    }

    fn publish_with_hooks_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        handoff_bytes: &[u8],
        hooks: &mut impl HandoffHooks,
    ) -> Result<CompilerModuleHandoffReceiptV2, CompilerModuleHandoffErrorV2> {
        publish_in_slot_with_hooks_v2(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV2::Default,
            compiler_closure,
            handoff_bytes,
            hooks,
        )
    }

    fn publish_in_slot_with_hooks_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        handoff_slot: CompilerModuleHandoffSlotV2,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        handoff_bytes: &[u8],
        hooks: &mut impl HandoffHooks,
    ) -> Result<CompilerModuleHandoffReceiptV2, CompilerModuleHandoffErrorV2> {
        publish_in_slot_engine::<HandoffV2Schema>(
            output_dir,
            producer,
            attempt,
            handoff_slot,
            compiler_closure,
            handoff_bytes,
            hooks,
        )
        .map(|published| CompilerModuleHandoffReceiptV2 {
            attempt: published.attempt,
            slot: published.slot,
            compiler_closure: published.binding,
            identity: CompilerModuleHandoffIdentityV2(published.identity),
            length: published.length,
        })
        .map_err(engine_error_v2)
    }

    fn consume_with_hooks_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        hooks: &mut impl HandoffHooks,
    ) -> Result<ConsumedCompilerModuleHandoffV2, CompilerModuleHandoffErrorV2> {
        consume_in_slot_with_hooks_v2(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV2::Default,
            compiler_closure,
            hooks,
        )
    }

    fn consume_in_slot_with_hooks_v2(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        handoff_slot: CompilerModuleHandoffSlotV2,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        hooks: &mut impl HandoffHooks,
    ) -> Result<ConsumedCompilerModuleHandoffV2, CompilerModuleHandoffErrorV2> {
        consume_in_slot_engine::<HandoffV2Schema>(
            output_dir,
            producer,
            attempt,
            handoff_slot,
            compiler_closure,
            hooks,
        )
        .map(|consumed| ConsumedCompilerModuleHandoffV2 {
            attempt: consumed.attempt,
            slot: consumed.slot,
            compiler_closure: consumed.binding,
            identity: CompilerModuleHandoffIdentityV2(consumed.identity),
            bytes: consumed.payload,
        })
        .map_err(engine_error_v2)
    }

    #[cfg(test)]
    fn producer_identity_v2(producer: &ProducerIdentity) -> [u8; 32] {
        producer_identity_for::<HandoffV2Schema>(producer)
    }

    #[cfg(test)]
    fn slot_identity_v2(
        producer: [u8; 32],
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV2,
    ) -> [u8; 32] {
        slot_identity_for::<HandoffV2Schema>(producer, attempt, slot)
    }

    #[cfg(test)]
    fn handoff_identity_v2(
        producer: [u8; 32],
        slot: [u8; 32],
        attempt: BuildAttempt,
        compiler_closure: fe2o3_build_authority::CompilerClosureV2,
        handoff_bytes: &[u8],
    ) -> [u8; 32] {
        HandoffV2Schema::derive_identity(producer, slot, attempt, compiler_closure, handoff_bytes)
    }

    fn engine_error_v2(error: HandoffEngineError) -> CompilerModuleHandoffErrorV2 {
        match error {
            HandoffEngineError::Common(error) => error.into(),
            HandoffEngineError::WrongBinding => CompilerModuleHandoffErrorV2::WrongCompilerClosure,
            HandoffEngineError::PayloadBindingMismatch => {
                CompilerModuleHandoffErrorV2::InvalidSlot {
                    path: PathBuf::new(),
                    reason: "V2 handoff unexpectedly failed a protocol payload binding".into(),
                }
            }
            HandoffEngineError::WorkingSetBudgetExceeded { required, maximum } => {
                CompilerModuleHandoffErrorV2::InvalidSlot {
                    path: PathBuf::new(),
                    reason: format!(
                        "V2 handoff unexpectedly required {required} bytes of decode working set with a {maximum}-byte limit"
                    ),
                }
            }
            HandoffEngineError::PayloadAllocationFailed { requested } => {
                CompilerModuleHandoffErrorV2::InvalidSlot {
                    path: PathBuf::new(),
                    reason: format!(
                        "could not reserve {requested} bytes for the V2 handoff payload"
                    ),
                }
            }
            HandoffEngineError::InvalidCanonicalV3(error) => {
                CompilerModuleHandoffErrorV2::InvalidSlot {
                    path: PathBuf::new(),
                    reason: format!("V2 handoff unexpectedly required V3 decoding: {error}"),
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{BuildInvocation, BuildSession, begin_build_attempt};
        use std::os::unix::fs::PermissionsExt;
        use std::sync::{Arc, Barrier};
        use std::thread;

        #[test]
        fn v2_schema_has_exactly_three_slots() {
            assert_eq!(HandoffV2Schema::ALL_SLOTS.len(), 3);
        }

        struct TestDirectory(PathBuf);

        impl TestDirectory {
            fn new() -> Self {
                static NEXT: AtomicU64 = AtomicU64::new(1);
                let path = std::env::temp_dir().join(format!(
                    "fe2o3-protected-module-handoff-v2-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&path).unwrap();
                Self(path)
            }
        }

        impl Drop for TestDirectory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        fn producer(name: &str) -> ProducerIdentity {
            ProducerIdentity::from_codegen(name, Some(Path::new("/src/protected-kernel.rs")))
                .unwrap()
        }

        fn begin(path: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
            begin_build_attempt(
                path,
                producer,
                BuildInvocation::from_bytes([seed; 32]),
                BuildSession::from_bytes([seed.wrapping_add(1); 16]),
            )
            .unwrap()
        }

        fn closure(seed: u8) -> fe2o3_build_authority::CompilerClosureV2 {
            fe2o3_build_authority::CompilerClosureV2::new(
                [seed; 32],
                [seed.wrapping_add(1); 32],
                [seed.wrapping_add(2); 32],
                [seed.wrapping_add(3); 32],
                [seed.wrapping_add(4); 32],
                [seed.wrapping_add(5); 32],
            )
            .unwrap()
        }

        fn slot_path_v2(
            path: &Path,
            producer: &ProducerIdentity,
            attempt: BuildAttempt,
            slot: CompilerModuleHandoffSlotV2,
        ) -> PathBuf {
            let producer_id = producer_identity_v2(producer);
            path.join(format!("{PARENT_PREFIX_V2}{}", hex(&producer_id)))
                .join(format!(
                    "{SLOT_PREFIX_V2}{}",
                    hex(&slot_identity_v2(producer_id, attempt, slot))
                ))
        }

        fn slot_path_v1(
            path: &Path,
            producer: &ProducerIdentity,
            attempt: BuildAttempt,
        ) -> PathBuf {
            let producer_id = producer_identity(producer);
            path.join(format!("{PARENT_PREFIX}{}", hex(&producer_id)))
                .join(format!(
                    "{SLOT_PREFIX}{}",
                    hex(&slot_identity(
                        producer_id,
                        attempt,
                        CompilerModuleHandoffSlotV1::Default
                    ))
                ))
        }

        fn closure_offset() -> usize {
            RECORD_MAGIC_V2.len() + 2 + 32 + 8 + 16 + 32 + 32
        }

        fn refresh_record_checksum(bytes: &mut [u8]) {
            let body_length = bytes.len() - 32;
            let checksum = sha256_parts(&[RECORD_DOMAIN_V2, &bytes[..body_length]]);
            bytes[body_length..].copy_from_slice(&checksum);
        }

        fn rewrite_ready_record(slot: &Path, mutate: impl FnOnce(&mut [u8])) -> Vec<u8> {
            let path = slot.join(READY_ENTRY);
            let mut bytes = fs::read(&path).unwrap();
            mutate(&mut bytes);
            refresh_record_checksum(&mut bytes);
            fs::write(path, &bytes).unwrap();
            bytes
        }

        #[test]
        fn canonical_closure_preimage_receipt_and_consumed_value_are_exact_and_inert() {
            let temp = TestDirectory::new();
            let producer = producer("protected");
            let attempt = begin(&temp.0, &producer, 31);
            let closure = closure(11);
            let module = b"closure-protected compiler module";
            let closure_bytes = compiler_closure_bytes_v2(closure);

            assert_eq!(closure_bytes.len(), 226);
            assert_eq!(&closure_bytes[0..32], &[11; 32]);
            assert_eq!(&closure_bytes[32..64], &[12; 32]);
            assert_eq!(&closure_bytes[64..96], &[13; 32]);
            assert_eq!(&closure_bytes[96..128], &[14; 32]);
            assert_eq!(&closure_bytes[128..160], &[15; 32]);
            assert_eq!(&closure_bytes[160..192], &[16; 32]);
            assert_eq!(&closure_bytes[192..194], &1u16.to_le_bytes());
            assert_eq!(&closure_bytes[194..226], &closure.identity_sha256());
            assert_eq!(decode_compiler_closure_v2(&closure_bytes).unwrap(), closure);

            let receipt =
                publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure, module)
                    .unwrap();
            assert_eq!(receipt.attempt(), attempt);
            assert_eq!(receipt.slot(), CompilerModuleHandoffSlotV2::Default);
            assert_eq!(receipt.compiler_closure(), closure);
            assert_eq!(receipt.length(), module.len());
            assert!(!receipt.grants_publication_authority());
            assert!(!receipt.grants_compiler_authority());

            let slot = slot_path_v2(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV2::Default,
            );
            let record_bytes = fs::read(slot.join(READY_ENTRY)).unwrap();
            let record = HandoffRecordV2::decode(&record_bytes).unwrap();
            assert_eq!(record.binding, closure);
            assert_eq!(
                CompilerModuleHandoffIdentityV2(record.identity),
                receipt.identity()
            );
            assert!(record_bytes.starts_with(RECORD_MAGIC_V2));
            assert_eq!(
                &record_bytes[closure_offset()..closure_offset() + COMPILER_CLOSURE_BYTES_V2],
                &closure_bytes
            );
            assert_eq!(
                fs::metadata(slot.join(READY_ENTRY))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );

            let consumed =
                consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure).unwrap();
            assert_eq!(consumed.attempt(), attempt);
            assert_eq!(consumed.slot(), CompilerModuleHandoffSlotV2::Default);
            assert_eq!(consumed.compiler_closure(), closure);
            assert_eq!(consumed.identity(), receipt.identity());
            assert_eq!(consumed.bytes(), module);
            assert!(!consumed.grants_publication_authority());
            assert!(!consumed.grants_compiler_authority());
            assert!(!consumed.grants_link_authority());
            assert!(!consumed.grants_load_authority());
            assert!(!consumed.grants_launch_authority());
            assert!(matches!(
                consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure),
                Err(CompilerModuleHandoffErrorV2::AlreadyConsumed)
            ));
        }

        #[test]
        fn identity_binds_closure_slot_attempt_producer_and_module_bytes() {
            let temp = TestDirectory::new();
            let primary_producer = producer("binding");
            let other_producer = producer("binding_other");
            let attempt = begin(&temp.0, &primary_producer, 32);
            let producer_id = producer_identity_v2(&primary_producer);
            let slot = slot_identity_v2(producer_id, attempt, CompilerModuleHandoffSlotV2::Default);
            let compiler_closure = closure(21);
            let baseline =
                handoff_identity_v2(producer_id, slot, attempt, compiler_closure, b"module");

            assert_ne!(
                baseline,
                handoff_identity_v2(
                    producer_identity_v2(&other_producer),
                    slot,
                    attempt,
                    compiler_closure,
                    b"module"
                )
            );
            assert_ne!(
                baseline,
                handoff_identity_v2(
                    producer_id,
                    slot_identity_v2(
                        producer_id,
                        attempt,
                        CompilerModuleHandoffSlotV2::GeneralGemmReference
                    ),
                    attempt,
                    compiler_closure,
                    b"module"
                )
            );
            let other_attempt = BuildAttempt::from_env_value(&format!(
                "{}:{}:{}",
                attempt.generation().wrapping_add(1),
                attempt.session().to_hex(),
                attempt.invocation().to_hex()
            ))
            .unwrap();
            assert_ne!(
                baseline,
                handoff_identity_v2(
                    producer_id,
                    slot,
                    other_attempt,
                    compiler_closure,
                    b"module"
                )
            );
            assert_ne!(
                baseline,
                handoff_identity_v2(producer_id, slot, attempt, closure(22), b"module")
            );
            assert_ne!(
                baseline,
                handoff_identity_v2(producer_id, slot, attempt, compiler_closure, b"module!")
            );
        }

        #[test]
        fn named_slot_record_cannot_be_replayed_as_default() {
            let temp = TestDirectory::new();
            let producer = producer("slot_binding");
            let attempt = begin(&temp.0, &producer, 33);
            let closure = closure(31);
            let module = b"same module";
            let default =
                publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure, module)
                    .unwrap();
            let named = publish_compiler_module_handoff_in_slot_v2(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV2::GeneralGemmReference,
                closure,
                module,
            )
            .unwrap();
            assert_ne!(default.identity(), named.identity());

            let default_slot = slot_path_v2(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV2::Default,
            );
            let named_slot = slot_path_v2(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV2::GeneralGemmReference,
            );
            fs::remove_file(default_slot.join(READY_ENTRY)).unwrap();
            fs::copy(named_slot.join(READY_ENTRY), default_slot.join(READY_ENTRY)).unwrap();
            assert!(matches!(
                consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure),
                Err(CompilerModuleHandoffErrorV2::InvalidSlot { .. })
            ));
            assert_eq!(
                consume_compiler_module_handoff_in_slot_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV2::GeneralGemmReference,
                    closure,
                )
                .unwrap()
                .bytes(),
                module
            );
        }

        #[test]
        fn wrong_closure_fails_without_consuming_the_committed_bytes() {
            let temp = TestDirectory::new();
            let producer = producer("wrong_closure");
            let attempt = begin(&temp.0, &producer, 34);
            let expected = closure(41);
            let wrong = closure(51);
            publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, expected, b"module")
                .unwrap();

            assert!(matches!(
                consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, wrong),
                Err(CompilerModuleHandoffErrorV2::WrongCompilerClosure)
            ));
            assert!(matches!(
                publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, wrong, b"module"),
                Err(CompilerModuleHandoffErrorV2::WrongCompilerClosure)
            ));
            assert_eq!(
                consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, expected)
                    .unwrap()
                    .bytes(),
                b"module"
            );
        }

        #[test]
        fn hostile_payload_record_and_closure_mutations_fail_closed() {
            for attack in ["payload", "record-identity", "closure-pin"] {
                let temp = TestDirectory::new();
                let producer = producer("hostile_mutation");
                let attempt = begin(&temp.0, &producer, 35);
                let closure = closure(61);
                publish_compiler_module_handoff_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    closure,
                    b"original",
                )
                .unwrap();
                let slot = slot_path_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV2::Default,
                );
                match attack {
                    "payload" => fs::write(slot.join(PAYLOAD_ENTRY), b"changed!").unwrap(),
                    "record-identity" => {
                        rewrite_ready_record(&slot, |bytes| {
                            let identity_offset = closure_offset() + COMPILER_CLOSURE_BYTES_V2;
                            bytes[identity_offset] ^= 0x80;
                        });
                    }
                    "closure-pin" => {
                        rewrite_ready_record(&slot, |bytes| {
                            bytes[closure_offset() + 7] ^= 0x80;
                        });
                    }
                    _ => unreachable!(),
                }
                assert!(
                    consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure)
                        .is_err(),
                    "attack={attack}"
                );
            }
        }

        #[test]
        fn compiler_closure_digest_roles_cannot_be_swapped() {
            let temp = TestDirectory::new();
            let producer = producer("role_mismatch");
            let attempt = begin(&temp.0, &producer, 36);
            let closure = closure(71);
            publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure, b"module")
                .unwrap();
            let slot = slot_path_v2(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV2::Default,
            );
            rewrite_ready_record(&slot, |bytes| {
                let offset = closure_offset();
                let first: [u8; 32] = bytes[offset..offset + 32].try_into().unwrap();
                let second: [u8; 32] = bytes[offset + 32..offset + 64].try_into().unwrap();
                bytes[offset..offset + 32].copy_from_slice(&second);
                bytes[offset + 32..offset + 64].copy_from_slice(&first);
            });
            assert!(matches!(
                consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure),
                Err(CompilerModuleHandoffErrorV2::InvalidSlot { .. })
            ));
        }

        #[test]
        fn unknown_record_version_and_closure_protocol_fail_closed() {
            for attack in ["record-version", "closure-protocol"] {
                let temp = TestDirectory::new();
                let producer = producer("unknown_version");
                let attempt = begin(&temp.0, &producer, 37);
                let closure = closure(81);
                publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure, b"module")
                    .unwrap();
                let slot = slot_path_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV2::Default,
                );
                rewrite_ready_record(&slot, |bytes| match attack {
                    "record-version" => bytes[RECORD_MAGIC_V2.len()..RECORD_MAGIC_V2.len() + 2]
                        .copy_from_slice(&3u16.to_le_bytes()),
                    "closure-protocol" => bytes[closure_offset() + 192..closure_offset() + 194]
                        .copy_from_slice(&2u16.to_le_bytes()),
                    _ => unreachable!(),
                });
                assert!(matches!(
                    consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure),
                    Err(CompilerModuleHandoffErrorV2::InvalidSlot { .. })
                ));
            }
        }

        #[test]
        fn v1_record_downgrade_is_rejected_by_v2_recovery_and_consume() {
            for operation in ["recover-publish", "consume"] {
                let temp = TestDirectory::new();
                let producer = producer("v1_downgrade");
                let attempt = begin(&temp.0, &producer, 38);
                let closure = closure(91);
                publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"V1 module")
                    .unwrap();
                publish_compiler_module_handoff_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    closure,
                    b"V2 module",
                )
                .unwrap();
                let v1_slot = slot_path_v1(&temp.0, &producer, attempt);
                let v2_slot = slot_path_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV2::Default,
                );
                assert_ne!(v1_slot, v2_slot);
                assert!(
                    fs::read(v1_slot.join(READY_ENTRY))
                        .unwrap()
                        .starts_with(RECORD_MAGIC)
                );
                assert!(
                    fs::read(v2_slot.join(READY_ENTRY))
                        .unwrap()
                        .starts_with(RECORD_MAGIC_V2)
                );
                fs::remove_file(v2_slot.join(READY_ENTRY)).unwrap();
                fs::copy(v1_slot.join(READY_ENTRY), v2_slot.join(READY_ENTRY)).unwrap();

                let result = match operation {
                    "recover-publish" => publish_compiler_module_handoff_v2(
                        &temp.0,
                        &producer,
                        attempt,
                        closure,
                        b"V2 module",
                    )
                    .map(|_| ()),
                    "consume" => {
                        consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure)
                            .map(|_| ())
                    }
                    _ => unreachable!(),
                };
                assert!(matches!(
                    result,
                    Err(CompilerModuleHandoffErrorV2::InvalidSlot { .. })
                ));
                assert_eq!(
                    consume_compiler_module_handoff_v1(&temp.0, &producer, attempt)
                        .unwrap()
                        .bytes(),
                    b"V1 module"
                );
            }
        }

        #[test]
        fn concurrent_v2_publish_and_consume_have_single_winners() {
            let temp = Arc::new(TestDirectory::new());
            let producer = Arc::new(producer("concurrent_v2"));
            let attempt = begin(&temp.0, &producer, 39);
            let closure = closure(101);
            let barrier = Arc::new(Barrier::new(8));
            let publishers = (0..8)
                .map(|_| {
                    let temp = Arc::clone(&temp);
                    let producer = Arc::clone(&producer);
                    let barrier = Arc::clone(&barrier);
                    thread::spawn(move || {
                        barrier.wait();
                        publish_compiler_module_handoff_v2(
                            &temp.0,
                            &producer,
                            attempt,
                            closure,
                            b"concurrent V2 module",
                        )
                    })
                })
                .collect::<Vec<_>>();
            let results = publishers
                .into_iter()
                .map(|join| join.join().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
            assert_eq!(
                results
                    .iter()
                    .filter(|result| matches!(
                        result,
                        Err(CompilerModuleHandoffErrorV2::AlreadyPublished)
                    ))
                    .count(),
                7
            );

            let barrier = Arc::new(Barrier::new(8));
            let consumers = (0..8)
                .map(|_| {
                    let temp = Arc::clone(&temp);
                    let producer = Arc::clone(&producer);
                    let barrier = Arc::clone(&barrier);
                    thread::spawn(move || {
                        barrier.wait();
                        consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure)
                    })
                })
                .collect::<Vec<_>>();
            let results = consumers
                .into_iter()
                .map(|join| join.join().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
            assert_eq!(
                results
                    .iter()
                    .filter(|result| matches!(
                        result,
                        Err(CompilerModuleHandoffErrorV2::AlreadyConsumed)
                    ))
                    .count(),
                7
            );
        }

        struct FailAt(FaultPoint);

        impl HandoffHooks for FailAt {
            fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
                if point == self.0 {
                    Err(std::io::Error::other("simulated V2 crash"))
                } else {
                    Ok(())
                }
            }
        }

        #[test]
        fn v2_publish_crash_residue_recovers_or_preserves_committed_state() {
            let points = [
                FaultPoint::DirectoryCreated,
                FaultPoint::PayloadCreated,
                FaultPoint::PayloadWritten,
                FaultPoint::PayloadSynced,
                FaultPoint::PayloadRenamed,
                FaultPoint::RecordWritten,
                FaultPoint::RecordSynced,
                FaultPoint::RecordRenamed,
                FaultPoint::PublishedSynced,
            ];
            for point in points {
                let temp = TestDirectory::new();
                let producer = producer("publish_crash_v2");
                let attempt = begin(&temp.0, &producer, 40);
                let closure = closure(111);
                assert!(
                    publish_with_hooks_v2(
                        &temp.0,
                        &producer,
                        attempt,
                        closure,
                        b"module",
                        &mut FailAt(point),
                    )
                    .is_err()
                );
                let retry = publish_compiler_module_handoff_v2(
                    &temp.0, &producer, attempt, closure, b"module",
                );
                if matches!(
                    point,
                    FaultPoint::RecordRenamed | FaultPoint::PublishedSynced
                ) {
                    assert!(matches!(
                        retry,
                        Err(CompilerModuleHandoffErrorV2::AlreadyPublished)
                    ));
                } else {
                    retry.unwrap();
                }
                let consumed =
                    consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure)
                        .unwrap();
                assert_eq!(consumed.compiler_closure(), closure);
                assert_eq!(consumed.bytes(), b"module");
            }
        }

        #[test]
        fn v2_consumption_crashes_are_exactly_once_or_retryable_at_the_commit_boundary() {
            for point in [FaultPoint::ConsumedRenamed, FaultPoint::ConsumedSynced] {
                let temp = TestDirectory::new();
                let producer = producer("consume_crash_v2");
                let attempt = begin(&temp.0, &producer, 41);
                let closure = closure(121);
                publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure, b"module")
                    .unwrap();
                assert!(
                consume_with_hooks_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    closure,
                    &mut FailAt(point),
                )
                .is_err()
            );
                assert!(matches!(
                    consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure),
                    Err(CompilerModuleHandoffErrorV2::AlreadyConsumed)
                ));
            }

            let temp = TestDirectory::new();
            let producer = producer("consume_retry_v2");
            let attempt = begin(&temp.0, &producer, 42);
            let closure = closure(131);
            publish_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure, b"module")
                .unwrap();
            assert!(
                consume_with_hooks_v2(
                    &temp.0,
                    &producer,
                    attempt,
                    closure,
                    &mut FailAt(FaultPoint::PayloadValidated),
                )
                .is_err()
            );
            assert_eq!(
                consume_compiler_module_handoff_v2(&temp.0, &producer, attempt, closure)
                    .unwrap()
                    .bytes(),
                b"module"
            );
        }
    }
}

pub use protected_v2::{
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2, CompilerModuleHandoffReceiptV2,
    CompilerModuleHandoffSlotV2, ConsumedCompilerModuleHandoffV2,
    consume_compiler_module_handoff_in_slot_v2, consume_compiler_module_handoff_v2,
    publish_compiler_module_handoff_in_slot_v2, publish_compiler_module_handoff_v2,
};

mod semantic_v3 {
    use super::*;

    const PARENT_PREFIX_V3: &str = ".fe2o3-compiler-module-handoff-v3-";
    const SLOT_PREFIX_V3: &str = "attempt-";
    const RECORD_MAGIC_V3: &[u8] = b"FE2O3-COMPILER-MODULE-HANDOFF-V3\0";
    const RECORD_VERSION_V3: u16 = 3;
    const PRODUCER_DOMAIN_V3: &[u8] = b"fe2o3.compiler-module-handoff.producer.v3\0";
    const SLOT_DOMAIN_V3: &[u8] = b"fe2o3.compiler-module-handoff.slot.v3\0";
    const NAMED_SLOT_DOMAIN_V3: &[u8] = b"fe2o3.compiler-module-handoff.named-slot.v3\0";
    const TRANSACTION_IDENTITY_DOMAIN_V3: &[u8] =
        b"fe2o3.compiler-module-handoff.transaction-identity.v3\0";
    const RECORD_DOMAIN_V3: &[u8] = b"fe2o3.compiler-module-handoff.record.v3\0";
    const HANDOFF_BINDING_BYTES_V3: usize = 32 + 8;
    const RECORD_BYTES_V3: usize = RECORD_MAGIC_V3.len()
        + 2
        + 32
        + 8
        + 16
        + 32
        + 32
        + HANDOFF_BINDING_BYTES_V3
        + 32
        + 8
        + (7 * 8)
        + 32;

    /// Maximum strict compiler-FFI V3 handoff accepted by the V3 transaction.
    ///
    /// This is a per-version ceiling. The V1 and V2 transaction schemas retain
    /// [`MAX_COMPILER_MODULE_HANDOFF_BYTES`] unchanged.
    pub const MAX_COMPILER_MODULE_HANDOFF_BYTES_V3: usize =
        fe2o3_compiler_ffi::MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3;

    // The admitted payload is the one shared canonical backing. This fixed allowance covers three
    // bounded representations of the envelope and symbol manifest plus 8 MiB for the bounded
    // invocation, collection metadata, and fixed decoded owners. Large module and receipt payloads
    // are ranges in the canonical backing rather than additional complete buffers.
    const V3_DECODE_WORKING_SET_MULTIPLIER: usize = 1;
    const V3_DECODE_FIXED_BYTES: usize = 3 * fe2o3_compiler_ffi::MAX_COMPILER_FFI_ENVELOPE_BYTES_V1
        + 3 * fe2o3_compiler_ffi::MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1
        + 8 * 1024 * 1024;
    const MAX_V3_DECODE_WORKING_SET_BYTES: usize =
        MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 + V3_DECODE_FIXED_BYTES;
    const STREAM_BUFFER_BYTES_V3: usize = 16 * 1024;

    /// Closed attempt-local transport slot for one strict semantic V3 handoff.
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    #[repr(u8)]
    pub enum CompilerModuleHandoffSlotV3 {
        /// Original one-module transport slot.
        Default = 0,
        /// Issue #138 reference wave64 XOR4 schedule.
        GeneralGemmReference = 1,
        /// Issue #138 A-only BF16 vector-transfer schedule.
        GeneralGemmVectorizedAOnly = 2,
    }

    /// Domain-separated transaction identity for exact V3 bytes and bindings.
    ///
    /// This identity establishes content equality inside the cooperative
    /// attempt protocol. It grants no compiler, publication, link, load, or
    /// launch authority.
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub struct CompilerModuleHandoffTransactionIdentityV3([u8; 32]);

    impl CompilerModuleHandoffTransactionIdentityV3 {
        /// Constructs an identity from its exact SHA-256 representation.
        pub const fn from_bytes(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }

        /// Returns the exact domain-separated SHA-256 representation.
        pub const fn as_bytes(&self) -> &[u8; 32] {
            &self.0
        }
    }

    /// Durable inert receipt for one strict semantic V3 handoff.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CompilerModuleHandoffReceiptV3 {
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        handoff_identity: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3,
        transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
        length: usize,
    }

    impl CompilerModuleHandoffReceiptV3 {
        /// Returns the exact build attempt that owned this transaction.
        pub const fn attempt(self) -> BuildAttempt {
            self.attempt
        }

        /// Returns the exact attempt-local transport slot.
        pub const fn slot(self) -> CompilerModuleHandoffSlotV3 {
            self.slot
        }

        /// Returns the native terminal identity of the strict compiler-FFI handoff.
        pub const fn handoff_identity(
            self,
        ) -> fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3 {
            self.handoff_identity
        }

        /// Returns the attempt-, producer-, slot-, and byte-bound transaction identity.
        pub const fn transaction_identity(self) -> CompilerModuleHandoffTransactionIdentityV3 {
            self.transaction_identity
        }

        /// Returns the exact canonical handoff length.
        pub const fn length(self) -> usize {
            self.length
        }

        /// A V3 transaction receipt is inert coordination evidence.
        pub const fn grants_publication_authority(self) -> bool {
            false
        }

        /// Attempt possession and content identity do not authenticate compiler authorship.
        pub const fn grants_compiler_authority(self) -> bool {
            false
        }
    }

    /// Strictly decoded handoff returned by the one successful V3 consumption.
    #[derive(Debug)]
    pub struct ConsumedCompilerModuleHandoffV3 {
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
        handoff: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    }

    impl ConsumedCompilerModuleHandoffV3 {
        /// Returns the exact build attempt that owned this transaction.
        pub const fn attempt(&self) -> BuildAttempt {
            self.attempt
        }

        /// Returns the exact attempt-local transport slot.
        pub const fn slot(&self) -> CompilerModuleHandoffSlotV3 {
            self.slot
        }

        /// Returns the native terminal identity of the decoded handoff.
        pub const fn handoff_identity(
            &self,
        ) -> fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3 {
            self.handoff.identity()
        }

        /// Returns the attempt-, producer-, slot-, and byte-bound transaction identity.
        pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV3 {
            self.transaction_identity
        }

        /// Returns the complete strictly decoded compiler-FFI handoff.
        pub const fn handoff(&self) -> &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3 {
            &self.handoff
        }

        /// Returns the exact canonical bytes retained by the strict handoff owner.
        pub fn bytes(&self) -> &[u8] {
            self.handoff.canonical_bytes()
        }

        /// Moves the strict compiler-FFI handoff out of this inert transaction result.
        pub fn into_handoff(self) -> fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3 {
            self.handoff
        }

        /// Consumed bytes do not grant publication authority.
        pub const fn grants_publication_authority(&self) -> bool {
            false
        }

        /// Consumed bytes do not authenticate compiler authorship.
        pub const fn grants_compiler_authority(&self) -> bool {
            false
        }

        /// Consumed bytes do not authorize linking.
        pub const fn grants_link_authority(&self) -> bool {
            false
        }

        /// Consumed bytes do not authorize module loading.
        pub const fn grants_load_authority(&self) -> bool {
            false
        }

        /// Consumed bytes do not authorize kernel launch.
        pub const fn grants_launch_authority(&self) -> bool {
            false
        }
    }

    /// Publication result carrying inert evidence and one exact currentness lease.
    ///
    /// The result is deliberately move-only. The copyable [`CompilerModuleHandoffReceiptV3`]
    /// remains inert, while [`Self::into_current_lease`] transfers the private filesystem custody
    /// needed to prove that this exact V3 publication is still current.
    #[derive(Debug)]
    pub struct CompilerModuleHandoffPublicationV3 {
        receipt: CompilerModuleHandoffReceiptV3,
        lease: CompilerModuleHandoffCurrentnessLeaseV3,
    }

    impl CompilerModuleHandoffPublicationV3 {
        /// Returns the inert durable receipt for this publication.
        pub const fn receipt(&self) -> CompilerModuleHandoffReceiptV3 {
            self.receipt
        }

        /// Moves the private currentness lease into the consumer path.
        pub fn into_current_lease(self) -> CompilerModuleHandoffCurrentnessLeaseV3 {
            self.lease
        }

        /// Separates the inert receipt from the move-only lease.
        pub fn into_parts(
            self,
        ) -> (
            CompilerModuleHandoffReceiptV3,
            CompilerModuleHandoffCurrentnessLeaseV3,
        ) {
            (self.receipt, self.lease)
        }
    }

    /// Move-only custody of one exact, currently committed V3 handoff publication.
    ///
    /// The lease retains pinned descriptors and metadata for the output, V3 producer namespace,
    /// attempt slot, ready record, and payload. Its private binding includes the exact attempt,
    /// producer, slot, transaction identity, native outer V3 identity, and committed generation.
    /// It grants no compiler, publication, link, load, or launch authority.
    ///
    /// ```compile_fail
    /// use fe2o3_artifact_transaction::CompilerModuleHandoffCurrentnessLeaseV3;
    ///
    /// fn cannot_clone(
    ///     lease: CompilerModuleHandoffCurrentnessLeaseV3,
    /// ) -> (
    ///     CompilerModuleHandoffCurrentnessLeaseV3,
    ///     CompilerModuleHandoffCurrentnessLeaseV3,
    /// ) {
    ///     (lease.clone(), lease)
    /// }
    /// ```
    pub struct CompilerModuleHandoffCurrentnessLeaseV3 {
        binding: Arc<CompilerModuleHandoffCurrentnessBindingV3>,
    }

    struct CompilerModuleHandoffCurrentnessBindingV3 {
        output: PinnedOutput,
        producer: ProducerIdentity,
        producer_identity: [u8; 32],
        parent: PinnedDirectory,
        slot_directory: PinnedDirectory,
        ready_file: PinnedHandoffFileV3,
        payload_file: PinnedHandoffFileV3,
        receipt: CompilerModuleHandoffReceiptV3,
        slot_identity: [u8; 32],
        committed_generation: u64,
    }

    struct PinnedHandoffFileV3 {
        file: fs::File,
        identity: FileIdentity,
    }

    impl fmt::Debug for CompilerModuleHandoffCurrentnessLeaseV3 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let binding = &self.binding;
            formatter
                .debug_struct("CompilerModuleHandoffCurrentnessLeaseV3")
                .field("attempt", &binding.receipt.attempt())
                .field("slot", &binding.receipt.slot())
                .field(
                    "transaction_identity",
                    &binding.receipt.transaction_identity(),
                )
                .field("handoff_identity", &binding.receipt.handoff_identity())
                .field("committed_generation", &binding.committed_generation)
                .finish_non_exhaustive()
        }
    }

    impl CompilerModuleHandoffCurrentnessLeaseV3 {
        /// Returns the inert receipt whose exact publication is pinned by this lease.
        pub fn receipt(&self) -> CompilerModuleHandoffReceiptV3 {
            self.binding.receipt
        }

        /// Revalidates this lease as the current exact V3 publication.
        pub fn revalidate(&self) -> Result<(), CompilerModuleHandoffErrorV3> {
            let token = self.acquire_current_token()?;
            token.revalidate_locked_currentness()
        }

        /// Acquires the cooperative lock and mints a single-use consumption token.
        ///
        /// Strict V3 decoding and every retained binding are checked before the token is returned.
        /// The token keeps the lock held until it is consumed or dropped.
        pub fn acquire_current_token(
            &self,
        ) -> Result<CompilerModuleHandoffConsumptionTokenV3, CompilerModuleHandoffErrorV3> {
            let lock = self
                .binding
                .output
                .try_lock()
                .map_err(CompilerModuleHandoffErrorV3::from)?
                .ok_or(CompilerModuleHandoffErrorV3::Busy)?;
            let handoff = load_current_handoff_locked(&self.binding)?;
            Ok(CompilerModuleHandoffConsumptionTokenV3 {
                binding: Arc::clone(&self.binding),
                handoff,
                _lock: lock,
            })
        }

        /// Checks that a token was minted from this exact lease instance.
        pub fn validate_current_token(
            &self,
            token: &CompilerModuleHandoffConsumptionTokenV3,
        ) -> Result<(), CompilerModuleHandoffErrorV3> {
            if Arc::ptr_eq(&self.binding, &token.binding) {
                Ok(())
            } else {
                Err(CompilerModuleHandoffErrorV3::MismatchedCurrentnessToken)
            }
        }

        /// A currentness lease is local custody, not compiler authority.
        pub const fn grants_compiler_authority(&self) -> bool {
            false
        }

        /// A currentness lease does not authorize linking.
        pub const fn grants_link_authority(&self) -> bool {
            false
        }

        /// A currentness lease does not authorize loading.
        pub const fn grants_load_authority(&self) -> bool {
            false
        }

        /// A currentness lease does not authorize launch.
        pub const fn grants_launch_authority(&self) -> bool {
            false
        }
    }

    /// Single-use proof that one exact V3 handoff remained current under the cooperative lock.
    ///
    /// The only successful consumption API takes this value by ownership. The token cannot be
    /// reconstructed, serialized, or combined with a lease from another publication.
    ///
    /// ```compile_fail
    /// use fe2o3_artifact_transaction::CompilerModuleHandoffConsumptionTokenV3;
    ///
    /// fn cannot_clone(
    ///     token: CompilerModuleHandoffConsumptionTokenV3,
    /// ) -> (
    ///     CompilerModuleHandoffConsumptionTokenV3,
    ///     CompilerModuleHandoffConsumptionTokenV3,
    /// ) {
    ///     (token.clone(), token)
    /// }
    /// ```
    pub struct CompilerModuleHandoffConsumptionTokenV3 {
        binding: Arc<CompilerModuleHandoffCurrentnessBindingV3>,
        handoff: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
        _lock: crate::OutputLock,
    }

    impl fmt::Debug for CompilerModuleHandoffConsumptionTokenV3 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter
                .debug_struct("CompilerModuleHandoffConsumptionTokenV3")
                .field("attempt", &self.binding.receipt.attempt())
                .field("slot", &self.binding.receipt.slot())
                .field(
                    "transaction_identity",
                    &self.binding.receipt.transaction_identity(),
                )
                .finish_non_exhaustive()
        }
    }

    impl CompilerModuleHandoffConsumptionTokenV3 {
        /// Borrows the strictly decoded inert handoff while this token keeps
        /// the cooperative lock held.
        ///
        /// This allows a caller to apply its private authority checks before
        /// committing the one-shot tombstone. The borrowed content remains
        /// inert and grants no compiler, link, load, or launch authority.
        pub const fn handoff(&self) -> &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3 {
            &self.handoff
        }

        /// Revalidates the exact files and generation while this token keeps the lock held.
        pub fn revalidate_locked_currentness(&self) -> Result<(), CompilerModuleHandoffErrorV3> {
            validate_current_metadata_locked(&self.binding)
        }

        /// A currentness token is not compiler authority.
        pub const fn grants_compiler_authority(&self) -> bool {
            false
        }

        /// A currentness token does not authorize linking.
        pub const fn grants_link_authority(&self) -> bool {
            false
        }

        /// A currentness token does not authorize loading.
        pub const fn grants_load_authority(&self) -> bool {
            false
        }

        /// A currentness token does not authorize launch.
        pub const fn grants_launch_authority(&self) -> bool {
            false
        }
    }

    /// Failure to publish, recover, strictly decode, or consume a V3 handoff.
    #[derive(Debug)]
    pub enum CompilerModuleHandoffErrorV3 {
        /// Another cooperating operation currently owns the artifact-store lock.
        Busy,
        /// Filesystem operation failed.
        Io(std::io::Error),
        /// The exact build attempt is not authorized for this producer.
        Attempt { reason: String },
        /// The private transaction namespace is malformed or was substituted.
        InvalidSlot { path: PathBuf, reason: String },
        /// The complete handoff is empty or exceeds the V3-only byte ceiling.
        InvalidHandoffSize { actual: usize, maximum: usize },
        /// The same exact V3 handoff was already published.
        AlreadyPublished,
        /// Different bytes conflict with the committed V3 handoff.
        ConflictingPublication,
        /// The V3 handoff was already consumed.
        AlreadyConsumed,
        /// No V3 handoff exists for this exact attempt and slot.
        NotPublished,
        /// The durable transaction identity does not match the exact payload bytes.
        DigestMismatch,
        /// The transaction is bound to a different native V3 handoff identity.
        WrongHandoffIdentity,
        /// Strict compiler-FFI V3 decoding rejected the committed payload.
        NonCanonicalHandoff(fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3),
        /// Strictly decoded bytes disagree with the native identity in the transaction record.
        HandoffIdentityMismatch,
        /// The admitted payload would exceed the explicit V3 decode working-set budget.
        WorkingSetBudgetExceeded { required: usize, maximum: usize },
        /// Reserving the complete V3 input buffer failed before any tombstone was committed.
        PayloadAllocationFailed { requested: usize },
        /// A consumption token was minted from a different private lease instance.
        MismatchedCurrentnessToken,
    }

    impl fmt::Display for CompilerModuleHandoffErrorV3 {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Busy => formatter
                    .write_str("the V3 compiler module handoff currentness lock is already held"),
                Self::Io(error) => write!(formatter, "{error}"),
                Self::Attempt { reason } => {
                    write!(formatter, "invalid V3 build-attempt handoff: {reason}")
                }
                Self::InvalidSlot { path, reason } => write!(
                    formatter,
                    "invalid V3 compiler module handoff {}: {reason}",
                    path.display()
                ),
                Self::InvalidHandoffSize { actual, maximum } => write!(
                    formatter,
                    "canonical V3 compiler module handoff size {actual} is outside 1..={maximum} bytes"
                ),
                Self::AlreadyPublished => {
                    formatter.write_str("V3 compiler module handoff is already published")
                }
                Self::ConflictingPublication => formatter
                    .write_str("V3 compiler module handoff conflicts with the committed handoff"),
                Self::AlreadyConsumed => {
                    formatter.write_str("V3 compiler module handoff was already consumed")
                }
                Self::NotPublished => {
                    formatter.write_str("V3 compiler module handoff is not published")
                }
                Self::DigestMismatch => {
                    formatter.write_str("V3 compiler module handoff transaction identity mismatch")
                }
                Self::WrongHandoffIdentity => formatter.write_str(
                    "V3 compiler module handoff is bound to a different native handoff identity",
                ),
                Self::NonCanonicalHandoff(error) => {
                    write!(
                        formatter,
                        "noncanonical strict compiler-FFI V3 handoff: {error}"
                    )
                }
                Self::HandoffIdentityMismatch => formatter.write_str(
                    "strictly decoded V3 handoff identity disagrees with the transaction binding",
                ),
                Self::WorkingSetBudgetExceeded { required, maximum } => write!(
                    formatter,
                    "V3 handoff decode requires {required} bytes of working set, exceeding the {maximum}-byte limit"
                ),
                Self::PayloadAllocationFailed { requested } => write!(
                    formatter,
                    "could not reserve the {requested}-byte V3 handoff input buffer"
                ),
                Self::MismatchedCurrentnessToken => formatter
                    .write_str("V3 currentness token belongs to a different publication lease"),
            }
        }
    }

    impl std::error::Error for CompilerModuleHandoffErrorV3 {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Io(error) => Some(error),
                Self::NonCanonicalHandoff(error) => Some(error),
                _ => None,
            }
        }
    }

    impl From<CompilerModuleHandoffErrorV1> for CompilerModuleHandoffErrorV3 {
        fn from(error: CompilerModuleHandoffErrorV1) -> Self {
            match error {
                CompilerModuleHandoffErrorV1::Io(error) => Self::Io(error),
                CompilerModuleHandoffErrorV1::Attempt { reason } => Self::Attempt { reason },
                CompilerModuleHandoffErrorV1::AttemptNotClaimable => Self::Attempt {
                    reason: "build attempt is not in the claimable building phase".to_owned(),
                },
                CompilerModuleHandoffErrorV1::InvalidSlot { path, reason } => {
                    Self::InvalidSlot { path, reason }
                }
                CompilerModuleHandoffErrorV1::InvalidHandoffSize { actual, maximum } => {
                    Self::InvalidHandoffSize { actual, maximum }
                }
                CompilerModuleHandoffErrorV1::AlreadyPublished => Self::AlreadyPublished,
                CompilerModuleHandoffErrorV1::ConflictingPublication => {
                    Self::ConflictingPublication
                }
                CompilerModuleHandoffErrorV1::AlreadyConsumed => Self::AlreadyConsumed,
                CompilerModuleHandoffErrorV1::NotPublished => Self::NotPublished,
                CompilerModuleHandoffErrorV1::DigestMismatch => Self::DigestMismatch,
            }
        }
    }

    impl From<EmitError> for CompilerModuleHandoffErrorV3 {
        fn from(error: EmitError) -> Self {
            Self::from(CompilerModuleHandoffErrorV1::from(error))
        }
    }

    impl From<std::io::Error> for CompilerModuleHandoffErrorV3 {
        fn from(error: std::io::Error) -> Self {
            Self::Io(error)
        }
    }

    /// Atomically publishes one exact strict compiler-FFI V3 handoff.
    ///
    /// The public API accepts the strict typed owner, never unclassified bytes,
    /// and writes only to the isolated V3 namespace. There is no V2 or V1
    /// decoding fallback.
    pub fn publish_compiler_module_handoff_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        handoff: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    ) -> Result<CompilerModuleHandoffReceiptV3, CompilerModuleHandoffErrorV3> {
        publish_in_slot_v3(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV3::Default,
            handoff,
            &mut NoFaults,
        )
    }

    /// Atomically publishes one strict V3 handoff in a closed named slot.
    pub fn publish_compiler_module_handoff_in_slot_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        handoff: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    ) -> Result<CompilerModuleHandoffReceiptV3, CompilerModuleHandoffErrorV3> {
        publish_in_slot_v3(output_dir, producer, attempt, slot, handoff, &mut NoFaults)
    }

    /// Recovers the inert receipt for the exact durable V3 default-slot publication.
    ///
    /// Recovery takes the cooperative output lock, requires the requested attempt to remain
    /// claimable for the exact producer, and strictly validates the V3 ready record and complete
    /// canonical payload. It neither consumes the publication nor grants compiler, publication,
    /// link, load, or launch authority. V1 and V2 namespaces are never inspected.
    pub fn recover_compiler_module_handoff_receipt_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
    ) -> Result<CompilerModuleHandoffReceiptV3, CompilerModuleHandoffErrorV3> {
        recover_compiler_module_handoff_receipt_in_slot_v3(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV3::Default,
        )
    }

    /// Recovers the inert receipt for one exact durable named-slot V3 publication.
    ///
    /// The returned receipt is reconstructed only from an exact, current V3 ready record whose
    /// producer, attempt, slot, transaction identity, native handoff identity, file metadata, and
    /// strict canonical payload all agree under the cooperative lock. A consumed publication,
    /// crash residue without a ready record, legacy publication, or any mismatch fails closed.
    pub fn recover_compiler_module_handoff_receipt_in_slot_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
    ) -> Result<CompilerModuleHandoffReceiptV3, CompilerModuleHandoffErrorV3> {
        recover_receipt_in_slot_v3(output_dir, producer, attempt, slot)
    }

    /// Publishes the default V3 slot and returns a move-only currentness lease.
    ///
    /// This additive API preserves the inert receipt API while giving an in-process consumer exact
    /// filesystem custody. If a newer generation wins between durable publication and lease
    /// issuance, issuance fails instead of returning a stale lease. Any error from lease issuance
    /// is preserved even though the preceding publication may already be durable. A different
    /// process can discover that exact committed state with
    /// [`recover_compiler_module_handoff_receipt_v3`] and then independently request local custody
    /// with [`acquire_compiler_module_handoff_currentness_lease_v3`].
    pub fn publish_compiler_module_handoff_with_currentness_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        handoff: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    ) -> Result<CompilerModuleHandoffPublicationV3, CompilerModuleHandoffErrorV3> {
        publish_compiler_module_handoff_in_slot_with_currentness_v3(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV3::Default,
            handoff,
        )
    }

    /// Publishes one named V3 slot and returns a move-only currentness lease.
    pub fn publish_compiler_module_handoff_in_slot_with_currentness_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        handoff: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    ) -> Result<CompilerModuleHandoffPublicationV3, CompilerModuleHandoffErrorV3> {
        publish_compiler_module_handoff_in_slot_with_currentness_after_publish_v3(
            output_dir,
            producer,
            attempt,
            slot,
            handoff,
            || {},
        )
    }

    fn publish_compiler_module_handoff_in_slot_with_currentness_after_publish_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        handoff: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
        after_publish: impl FnOnce(),
    ) -> Result<CompilerModuleHandoffPublicationV3, CompilerModuleHandoffErrorV3> {
        let receipt = publish_compiler_module_handoff_in_slot_v3(
            output_dir, producer, attempt, slot, handoff,
        )?;
        after_publish();
        let lease =
            acquire_compiler_module_handoff_currentness_lease_v3(output_dir, producer, receipt)?;
        Ok(CompilerModuleHandoffPublicationV3 { receipt, lease })
    }

    /// Reacquires private currentness custody for one exact inert V3 receipt.
    ///
    /// The receipt is not authority. Issuance succeeds only after the attempt registry, producer,
    /// slot, committed record, payload, transaction identity, native outer identity, and all pinned
    /// directory/file metadata are revalidated under the cooperative lock.
    pub fn acquire_compiler_module_handoff_currentness_lease_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        receipt: CompilerModuleHandoffReceiptV3,
    ) -> Result<CompilerModuleHandoffCurrentnessLeaseV3, CompilerModuleHandoffErrorV3> {
        mint_currentness_lease_v3(output_dir, producer, receipt)
    }

    /// Consumes and strictly decodes one V3 handoff exactly once.
    ///
    /// The caller must supply the expected native terminal identity. Strict V3
    /// decoding completes before the durable consumed tombstone is committed.
    pub fn consume_compiler_module_handoff_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        expected_handoff_identity: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3,
    ) -> Result<ConsumedCompilerModuleHandoffV3, CompilerModuleHandoffErrorV3> {
        consume_in_slot_v3(
            output_dir,
            producer,
            attempt,
            CompilerModuleHandoffSlotV3::Default,
            expected_handoff_identity,
            &mut NoFaults,
        )
    }

    /// Consumes and strictly decodes one named V3 slot exactly once.
    pub fn consume_compiler_module_handoff_in_slot_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        expected_handoff_identity: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3,
    ) -> Result<ConsumedCompilerModuleHandoffV3, CompilerModuleHandoffErrorV3> {
        consume_in_slot_v3(
            output_dir,
            producer,
            attempt,
            slot,
            expected_handoff_identity,
            &mut NoFaults,
        )
    }

    /// Consumes the exact V3 publication by moving its single-use currentness token.
    ///
    /// The lease/token pair is checked by private binding identity. The token keeps the cooperative
    /// lock held while the existing durable `ready` record is atomically renamed to `consumed`.
    /// The returned value retains the same inert semantics as the original V3 consume API.
    pub fn consume_compiler_module_handoff_with_currentness_v3(
        lease: &CompilerModuleHandoffCurrentnessLeaseV3,
        token: CompilerModuleHandoffConsumptionTokenV3,
    ) -> Result<ConsumedCompilerModuleHandoffV3, CompilerModuleHandoffErrorV3> {
        lease.validate_current_token(&token)?;
        token.revalidate_locked_currentness()?;

        let CompilerModuleHandoffConsumptionTokenV3 {
            binding,
            handoff,
            _lock,
        } = token;
        let receipt = binding.receipt;
        binding.slot_directory.verify()?;
        renameat_with(
            &binding.slot_directory.fd,
            READY_ENTRY,
            &binding.slot_directory.fd,
            CONSUMED_ENTRY,
            RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)?;
        fsync(&binding.slot_directory.fd).map_err(std::io::Error::from)?;
        binding.slot_directory.verify()?;
        validate_renamed_pinned_handoff_file_v3(
            &binding.slot_directory,
            CONSUMED_ENTRY,
            &binding.ready_file,
        )?;
        validate_pinned_handoff_file_v3(
            &binding.slot_directory,
            PAYLOAD_ENTRY,
            &binding.payload_file,
        )?;
        cleanup_consumed_payload(&binding.slot_directory);

        Ok(ConsumedCompilerModuleHandoffV3 {
            attempt: receipt.attempt(),
            slot: receipt.slot(),
            transaction_identity: receipt.transaction_identity(),
            handoff,
        })
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct HandoffBindingV3 {
        sha256: [u8; 32],
        byte_len: u64,
    }

    impl From<fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3> for HandoffBindingV3 {
        fn from(
            identity: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3,
        ) -> Self {
            Self {
                sha256: *identity.sha256(),
                byte_len: identity.byte_len(),
            }
        }
    }

    struct HandoffV3Schema;

    impl HandoffSchema for HandoffV3Schema {
        type Slot = CompilerModuleHandoffSlotV3;
        type Binding = HandoffBindingV3;
        type Payload = fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;

        const PARENT_PREFIX: &'static str = PARENT_PREFIX_V3;
        const SLOT_PREFIX: &'static str = SLOT_PREFIX_V3;
        const RECORD_MAGIC: &'static [u8] = RECORD_MAGIC_V3;
        const RECORD_VERSION: u16 = RECORD_VERSION_V3;
        const PRODUCER_DOMAIN: &'static [u8] = PRODUCER_DOMAIN_V3;
        const SLOT_DOMAIN: &'static [u8] = SLOT_DOMAIN_V3;
        const NAMED_SLOT_DOMAIN: &'static [u8] = NAMED_SLOT_DOMAIN_V3;
        const RECORD_DOMAIN: &'static [u8] = RECORD_DOMAIN_V3;
        const RECORD_BYTES: usize = RECORD_BYTES_V3;
        const MAX_HANDOFF_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES_V3;
        const DECODE_WORKING_SET_MULTIPLIER: usize = V3_DECODE_WORKING_SET_MULTIPLIER;
        const DECODE_WORKING_SET_FIXED_BYTES: usize = V3_DECODE_FIXED_BYTES;
        const MAX_DECODE_WORKING_SET_BYTES: usize = MAX_V3_DECODE_WORKING_SET_BYTES;
        const VALIDATE_RECORD_DURING_RECOVERY: bool = true;
        const ALL_SLOTS: &'static [Self::Slot] = &[
            CompilerModuleHandoffSlotV3::Default,
            CompilerModuleHandoffSlotV3::GeneralGemmReference,
            CompilerModuleHandoffSlotV3::GeneralGemmVectorizedAOnly,
        ];

        fn default_slot() -> Self::Slot {
            CompilerModuleHandoffSlotV3::Default
        }

        fn slot_tag(slot: Self::Slot) -> u8 {
            slot as u8
        }

        fn encode_binding(binding: Self::Binding, bytes: &mut Vec<u8>) {
            bytes.extend_from_slice(&binding.sha256);
            bytes.extend_from_slice(&binding.byte_len.to_le_bytes());
        }

        fn decode_binding(decoder: &mut Decoder<'_>) -> Result<Self::Binding, &'static str> {
            let sha256 = decoder.array()?;
            let byte_len = decoder.u64()?;
            if sha256 == [0; 32] {
                return Err("native V3 handoff identity is zero");
            }
            Ok(HandoffBindingV3 { sha256, byte_len })
        }

        fn binding_matches_length(binding: Self::Binding, length: usize) -> bool {
            usize::try_from(binding.byte_len).ok() == Some(length)
                && length <= MAX_COMPILER_MODULE_HANDOFF_BYTES_V3
        }

        fn decode_payload(
            binding: Self::Binding,
            bytes: Vec<u8>,
        ) -> Result<Self::Payload, HandoffEngineError> {
            let handoff =
                fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3::decode_shared_vec(
                    Arc::new(bytes),
                )
                .map_err(HandoffEngineError::InvalidCanonicalV3)?;
            if HandoffBindingV3::from(handoff.identity()) != binding {
                return Err(HandoffEngineError::PayloadBindingMismatch);
            }
            Ok(handoff)
        }

        fn derive_identity(
            producer: [u8; 32],
            slot: [u8; 32],
            attempt: BuildAttempt,
            binding: Self::Binding,
            handoff_bytes: &[u8],
        ) -> [u8; 32] {
            let mut digest = transaction_identity_digest_v3(
                producer,
                slot,
                attempt,
                binding,
                handoff_bytes.len(),
            );
            digest.update(handoff_bytes);
            digest.finalize().into()
        }
    }

    fn transaction_identity_digest_v3(
        producer: [u8; 32],
        slot: [u8; 32],
        attempt: BuildAttempt,
        binding: HandoffBindingV3,
        payload_length: usize,
    ) -> Sha256 {
        let mut digest = Sha256::new();
        digest.update(TRANSACTION_IDENTITY_DOMAIN_V3);
        digest.update(binding.sha256);
        digest.update(binding.byte_len.to_le_bytes());
        digest.update(slot);
        digest.update(producer);
        digest.update(attempt.generation().to_le_bytes());
        digest.update(attempt.session().as_bytes());
        digest.update(attempt.invocation().as_bytes());
        digest.update((payload_length as u64).to_le_bytes());
        digest
    }

    fn recover_receipt_in_slot_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
    ) -> Result<CompilerModuleHandoffReceiptV3, CompilerModuleHandoffErrorV3> {
        let output = PinnedOutput::open_existing(output_dir)?;
        let _lock = output.lock()?;
        output.verify_path_identity()?;
        authorize(&output, producer, attempt)?;

        let producer_identity = producer_identity_for::<HandoffV3Schema>(producer);
        let slot_identity = slot_identity_for::<HandoffV3Schema>(producer_identity, attempt, slot);
        let parent = open_private_directory(
            &output.fd,
            &output.display_path,
            format!("{PARENT_PREFIX_V3}{}", hex(&producer_identity)),
        )?
        .ok_or(CompilerModuleHandoffErrorV3::NotPublished)?;
        cleanup_stale_slots::<HandoffV3Schema>(&parent, producer_identity, attempt)
            .map_err(engine_error_v3)?;
        let slot_directory = open_private_directory(
            &parent.fd,
            &parent.path,
            format!("{SLOT_PREFIX_V3}{}", hex(&slot_identity)),
        )?
        .ok_or(CompilerModuleHandoffErrorV3::NotPublished)?;
        recover_slot::<HandoffV3Schema>(&slot_directory).map_err(engine_error_v3)?;
        require_current_slot_shape_v3(&slot_directory)?;

        let ready_file = open_pinned_handoff_file_v3(
            &slot_directory,
            READY_ENTRY,
            HandoffV3Schema::RECORD_BYTES,
        )?;
        let record_bytes = read_pinned_handoff_file_v3(
            &slot_directory,
            READY_ENTRY,
            &ready_file,
            HandoffV3Schema::RECORD_BYTES,
            HandoffV3Schema::RECORD_BYTES,
        )?;
        let record = HandoffRecord::<HandoffV3Schema>::decode(&record_bytes).map_err(|reason| {
            CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot_directory.path.join(READY_ENTRY),
                reason: reason.to_string(),
            }
        })?;
        if record.producer != producer_identity
            || record.attempt != attempt
            || record.slot != slot_identity
        {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot_directory.path.join(READY_ENTRY),
                reason: "record binding does not match the requested producer, attempt, and slot"
                    .to_string(),
            });
        }

        let payload_file =
            open_pinned_handoff_file_v3(&slot_directory, PAYLOAD_ENTRY, record.length)?;
        if record.file != payload_file.identity {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot_directory.path.join(PAYLOAD_ENTRY),
                reason: "payload metadata does not match the durable ready record".to_string(),
            });
        }
        validate_decode_working_set::<HandoffV3Schema>(
            record.length,
            MAX_V3_DECODE_WORKING_SET_BYTES,
        )
        .map_err(engine_error_v3)?;
        let payload_bytes = read_pinned_handoff_file_v3(
            &slot_directory,
            PAYLOAD_ENTRY,
            &payload_file,
            record.length,
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
        )?;
        let transaction_identity = HandoffV3Schema::derive_identity(
            record.producer,
            record.slot,
            record.attempt,
            record.binding,
            &payload_bytes,
        );
        if transaction_identity != record.identity {
            return Err(CompilerModuleHandoffErrorV3::DigestMismatch);
        }
        let handoff = HandoffV3Schema::decode_payload(record.binding, payload_bytes)
            .map_err(engine_error_v3)?;
        let handoff_identity = handoff.identity();
        if HandoffBindingV3::from(handoff_identity) != record.binding {
            return Err(CompilerModuleHandoffErrorV3::HandoffIdentityMismatch);
        }

        authorize(&output, producer, attempt)?;
        output.verify_path_identity()?;
        parent.verify()?;
        slot_directory.verify()?;
        require_current_slot_shape_v3(&slot_directory)?;
        let final_record_bytes = read_pinned_handoff_file_v3(
            &slot_directory,
            READY_ENTRY,
            &ready_file,
            HandoffV3Schema::RECORD_BYTES,
            HandoffV3Schema::RECORD_BYTES,
        )?;
        if final_record_bytes != record_bytes {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot_directory.path.join(READY_ENTRY),
                reason: "ready record changed while its exact payload was validated".to_string(),
            });
        }
        validate_pinned_handoff_file_v3(&slot_directory, PAYLOAD_ENTRY, &payload_file)?;

        Ok(CompilerModuleHandoffReceiptV3 {
            attempt,
            slot,
            handoff_identity,
            transaction_identity: CompilerModuleHandoffTransactionIdentityV3(transaction_identity),
            length: record.length,
        })
    }

    fn mint_currentness_lease_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        receipt: CompilerModuleHandoffReceiptV3,
    ) -> Result<CompilerModuleHandoffCurrentnessLeaseV3, CompilerModuleHandoffErrorV3> {
        if usize::try_from(receipt.handoff_identity().byte_len()).ok() != Some(receipt.length()) {
            return Err(CompilerModuleHandoffErrorV3::HandoffIdentityMismatch);
        }

        let output = PinnedOutput::open_existing(output_dir)?;
        let _lock = output
            .try_lock()?
            .ok_or(CompilerModuleHandoffErrorV3::Busy)?;
        output.verify_path_identity()?;
        authorize(&output, producer, receipt.attempt())?;

        let producer_identity = producer_identity_for::<HandoffV3Schema>(producer);
        let slot_identity = slot_identity_for::<HandoffV3Schema>(
            producer_identity,
            receipt.attempt(),
            receipt.slot(),
        );
        let parent = open_private_directory(
            &output.fd,
            &output.display_path,
            format!("{PARENT_PREFIX_V3}{}", hex(&producer_identity)),
        )?
        .ok_or(CompilerModuleHandoffErrorV3::NotPublished)?;
        cleanup_stale_slots::<HandoffV3Schema>(&parent, producer_identity, receipt.attempt())
            .map_err(engine_error_v3)?;
        let slot_directory = open_private_directory(
            &parent.fd,
            &parent.path,
            format!("{SLOT_PREFIX_V3}{}", hex(&slot_identity)),
        )?
        .ok_or(CompilerModuleHandoffErrorV3::NotPublished)?;
        recover_slot::<HandoffV3Schema>(&slot_directory).map_err(engine_error_v3)?;
        require_current_slot_shape_v3(&slot_directory)?;

        let ready_file = open_pinned_handoff_file_v3(
            &slot_directory,
            READY_ENTRY,
            HandoffV3Schema::RECORD_BYTES,
        )?;
        let payload_file =
            open_pinned_handoff_file_v3(&slot_directory, PAYLOAD_ENTRY, receipt.length())?;
        let binding = Arc::new(CompilerModuleHandoffCurrentnessBindingV3 {
            output,
            producer: producer.clone(),
            producer_identity,
            parent,
            slot_directory,
            ready_file,
            payload_file,
            receipt,
            slot_identity,
            committed_generation: receipt.attempt().generation(),
        });
        validate_current_payload_identity_locked(&binding)?;
        Ok(CompilerModuleHandoffCurrentnessLeaseV3 { binding })
    }

    fn require_current_slot_shape_v3(
        slot: &PinnedDirectory,
    ) -> Result<(), CompilerModuleHandoffErrorV3> {
        let entries = slot_entries(slot)?;
        if entries.iter().any(|entry| entry == CONSUMED_ENTRY) {
            return Err(CompilerModuleHandoffErrorV3::AlreadyConsumed);
        }
        if entries.len() == 2
            && entries.iter().any(|entry| entry == READY_ENTRY)
            && entries.iter().any(|entry| entry == PAYLOAD_ENTRY)
        {
            return Ok(());
        }
        if !entries.iter().any(|entry| entry == READY_ENTRY) {
            return Err(CompilerModuleHandoffErrorV3::NotPublished);
        }
        Err(CompilerModuleHandoffErrorV3::InvalidSlot {
            path: slot.path.clone(),
            reason: "current V3 slot must contain exactly the ready record and payload".to_string(),
        })
    }

    fn open_pinned_handoff_file_v3(
        slot: &PinnedDirectory,
        entry: &str,
        exact_length: usize,
    ) -> Result<PinnedHandoffFileV3, CompilerModuleHandoffErrorV3> {
        let fd = openat(
            &slot.fd,
            entry,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| CompilerModuleHandoffErrorV3::InvalidSlot {
            path: slot.path.join(entry),
            reason: std::io::Error::from(error).to_string(),
        })?;
        let file = fs::File::from(fd);
        let opened = fstat(&file).map_err(std::io::Error::from)?;
        let named =
            statat(&slot.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
        if !same_private_file(&opened, &named, exact_length) {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot.path.join(entry),
                reason: "file does not match its pinned private descriptor".to_string(),
            });
        }
        Ok(PinnedHandoffFileV3 {
            file,
            identity: FileIdentity::from_stat(&opened),
        })
    }

    fn validate_pinned_handoff_file_v3(
        slot: &PinnedDirectory,
        entry: &str,
        pinned: &PinnedHandoffFileV3,
    ) -> Result<(), CompilerModuleHandoffErrorV3> {
        let opened = fstat(&pinned.file).map_err(std::io::Error::from)?;
        let named = statat(&slot.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot.path.join(entry),
                reason: std::io::Error::from(error).to_string(),
            }
        })?;
        if !pinned.identity.matches(&opened) || !pinned.identity.matches(&named) {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot.path.join(entry),
                reason: "file no longer matches the publication lease".to_string(),
            });
        }
        Ok(())
    }

    fn validate_renamed_pinned_handoff_file_v3(
        slot: &PinnedDirectory,
        entry: &str,
        pinned: &PinnedHandoffFileV3,
    ) -> Result<(), CompilerModuleHandoffErrorV3> {
        let opened = fstat(&pinned.file).map_err(std::io::Error::from)?;
        let named = statat(&slot.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot.path.join(entry),
                reason: std::io::Error::from(error).to_string(),
            }
        })?;
        let length = usize::try_from(pinned.identity.length).map_err(|_| {
            CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot.path.join(entry),
                reason: "pinned record length is invalid".to_string(),
            }
        })?;
        if pinned.identity.device != opened.st_dev
            || pinned.identity.inode != opened.st_ino
            || !same_private_file(&opened, &named, length)
        {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot.path.join(entry),
                reason: "consumed record is not the exact renamed ready record".to_string(),
            });
        }
        Ok(())
    }

    fn read_pinned_handoff_file_v3(
        slot: &PinnedDirectory,
        entry: &str,
        pinned: &PinnedHandoffFileV3,
        exact_length: usize,
        maximum: usize,
    ) -> Result<Vec<u8>, CompilerModuleHandoffErrorV3> {
        if exact_length == 0 || exact_length > maximum {
            return Err(CompilerModuleHandoffErrorV3::InvalidHandoffSize {
                actual: exact_length,
                maximum,
            });
        }
        validate_pinned_handoff_file_v3(slot, entry, pinned)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(exact_length).map_err(|_| {
            CompilerModuleHandoffErrorV3::PayloadAllocationFailed {
                requested: exact_length,
            }
        })?;
        bytes.resize(exact_length, 0);
        pinned.file.read_exact_at(&mut bytes, 0)?;
        let mut trailing = [0_u8; 1];
        if pinned.file.read_at(&mut trailing, exact_length as u64)? != 0 {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: slot.path.join(entry),
                reason: "pinned file grew beyond its committed length".to_string(),
            });
        }
        validate_pinned_handoff_file_v3(slot, entry, pinned)?;
        Ok(bytes)
    }

    fn read_current_record_locked(
        binding: &CompilerModuleHandoffCurrentnessBindingV3,
    ) -> Result<HandoffRecord<HandoffV3Schema>, CompilerModuleHandoffErrorV3> {
        binding.output.verify_path_identity()?;
        authorize(
            &binding.output,
            &binding.producer,
            binding.receipt.attempt(),
        )?;
        if binding.committed_generation != binding.receipt.attempt().generation() {
            return Err(CompilerModuleHandoffErrorV3::Attempt {
                reason: "lease generation no longer matches its committed attempt".to_string(),
            });
        }
        binding.parent.verify()?;
        binding.slot_directory.verify()?;
        require_current_slot_shape_v3(&binding.slot_directory)?;
        validate_pinned_handoff_file_v3(&binding.slot_directory, READY_ENTRY, &binding.ready_file)?;
        validate_pinned_handoff_file_v3(
            &binding.slot_directory,
            PAYLOAD_ENTRY,
            &binding.payload_file,
        )?;

        let bytes = read_pinned_handoff_file_v3(
            &binding.slot_directory,
            READY_ENTRY,
            &binding.ready_file,
            HandoffV3Schema::RECORD_BYTES,
            HandoffV3Schema::RECORD_BYTES,
        )?;
        let record = HandoffRecord::<HandoffV3Schema>::decode(&bytes).map_err(|reason| {
            CompilerModuleHandoffErrorV3::InvalidSlot {
                path: binding.slot_directory.path.join(READY_ENTRY),
                reason: reason.to_string(),
            }
        })?;
        let receipt = binding.receipt;
        if record.attempt != receipt.attempt()
            || record.attempt.generation() != binding.committed_generation
            || record.producer != binding.producer_identity
            || record.slot != binding.slot_identity
        {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: binding.slot_directory.path.join(READY_ENTRY),
                reason: "record no longer matches the lease attempt, producer, slot, or generation"
                    .to_string(),
            });
        }
        if record.binding != HandoffBindingV3::from(receipt.handoff_identity()) {
            return Err(CompilerModuleHandoffErrorV3::WrongHandoffIdentity);
        }
        if record.identity != *receipt.transaction_identity().as_bytes()
            || record.length != receipt.length()
        {
            return Err(CompilerModuleHandoffErrorV3::DigestMismatch);
        }
        if record.file != binding.payload_file.identity {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: binding.slot_directory.path.join(PAYLOAD_ENTRY),
                reason: "payload metadata no longer matches the committed record".to_string(),
            });
        }
        Ok(record)
    }

    fn validate_current_metadata_locked(
        binding: &CompilerModuleHandoffCurrentnessBindingV3,
    ) -> Result<(), CompilerModuleHandoffErrorV3> {
        read_current_record_locked(binding)?;
        binding.output.verify_path_identity()?;
        binding.parent.verify()?;
        binding.slot_directory.verify()?;
        Ok(())
    }

    fn load_current_handoff_locked(
        binding: &CompilerModuleHandoffCurrentnessBindingV3,
    ) -> Result<
        fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
        CompilerModuleHandoffErrorV3,
    > {
        let record = read_current_record_locked(binding)?;
        validate_decode_working_set::<HandoffV3Schema>(
            record.length,
            MAX_V3_DECODE_WORKING_SET_BYTES,
        )
        .map_err(engine_error_v3)?;
        let bytes = read_pinned_handoff_file_v3(
            &binding.slot_directory,
            PAYLOAD_ENTRY,
            &binding.payload_file,
            record.length,
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
        )?;
        let identity = HandoffV3Schema::derive_identity(
            record.producer,
            record.slot,
            record.attempt,
            record.binding,
            &bytes,
        );
        if identity != record.identity
            || identity != *binding.receipt.transaction_identity().as_bytes()
        {
            return Err(CompilerModuleHandoffErrorV3::DigestMismatch);
        }
        let handoff =
            HandoffV3Schema::decode_payload(record.binding, bytes).map_err(engine_error_v3)?;
        if handoff.identity() != binding.receipt.handoff_identity() {
            return Err(CompilerModuleHandoffErrorV3::HandoffIdentityMismatch);
        }
        validate_current_metadata_locked(binding)?;
        Ok(handoff)
    }

    fn validate_current_payload_identity_locked(
        binding: &CompilerModuleHandoffCurrentnessBindingV3,
    ) -> Result<(), CompilerModuleHandoffErrorV3> {
        let record = read_current_record_locked(binding)?;
        validate_pinned_handoff_file_v3(
            &binding.slot_directory,
            PAYLOAD_ENTRY,
            &binding.payload_file,
        )?;

        let mut digest = transaction_identity_digest_v3(
            record.producer,
            record.slot,
            record.attempt,
            record.binding,
            record.length,
        );
        let mut offset = 0_u64;
        let mut remaining = record.length;
        let mut buffer = [0_u8; STREAM_BUFFER_BYTES_V3];
        while remaining != 0 {
            let requested = remaining.min(buffer.len());
            let read = binding
                .payload_file
                .file
                .read_at(&mut buffer[..requested], offset)?;
            if read == 0 {
                return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                    path: binding.slot_directory.path.join(PAYLOAD_ENTRY),
                    reason: "pinned payload ended before its committed length".to_string(),
                });
            }
            digest.update(&buffer[..read]);
            remaining -= read;
            offset += read as u64;
        }
        let mut trailing = [0_u8; 1];
        if binding.payload_file.file.read_at(&mut trailing, offset)? != 0 {
            return Err(CompilerModuleHandoffErrorV3::InvalidSlot {
                path: binding.slot_directory.path.join(PAYLOAD_ENTRY),
                reason: "pinned payload grew beyond its committed length".to_string(),
            });
        }
        validate_pinned_handoff_file_v3(
            &binding.slot_directory,
            PAYLOAD_ENTRY,
            &binding.payload_file,
        )?;

        let identity: [u8; 32] = digest.finalize().into();
        if identity != record.identity
            || identity != *binding.receipt.transaction_identity().as_bytes()
        {
            return Err(CompilerModuleHandoffErrorV3::DigestMismatch);
        }
        validate_current_metadata_locked(binding)
    }

    fn publish_in_slot_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        handoff: &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
        hooks: &mut impl HandoffHooks,
    ) -> Result<CompilerModuleHandoffReceiptV3, CompilerModuleHandoffErrorV3> {
        let bytes = handoff.canonical_bytes();
        validate_handoff_size::<HandoffV3Schema>(bytes.len())
            .map_err(CompilerModuleHandoffErrorV3::from)?;
        let handoff_identity = handoff.identity();
        if !handoff_identity.matches_canonical_bytes(bytes)
            || usize::try_from(handoff_identity.byte_len()).ok() != Some(bytes.len())
        {
            return Err(CompilerModuleHandoffErrorV3::HandoffIdentityMismatch);
        }
        publish_in_slot_engine::<HandoffV3Schema>(
            output_dir,
            producer,
            attempt,
            slot,
            handoff_identity.into(),
            bytes,
            hooks,
        )
        .map(|published| CompilerModuleHandoffReceiptV3 {
            attempt: published.attempt,
            slot: published.slot,
            handoff_identity,
            transaction_identity: CompilerModuleHandoffTransactionIdentityV3(published.identity),
            length: published.length,
        })
        .map_err(engine_error_v3)
    }

    fn consume_in_slot_v3(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        expected_handoff_identity: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3,
        hooks: &mut impl HandoffHooks,
    ) -> Result<ConsumedCompilerModuleHandoffV3, CompilerModuleHandoffErrorV3> {
        consume_in_slot_engine::<HandoffV3Schema>(
            output_dir,
            producer,
            attempt,
            slot,
            expected_handoff_identity.into(),
            hooks,
        )
        .map(|consumed| ConsumedCompilerModuleHandoffV3 {
            attempt: consumed.attempt,
            slot: consumed.slot,
            transaction_identity: CompilerModuleHandoffTransactionIdentityV3(consumed.identity),
            handoff: consumed.payload,
        })
        .map_err(engine_error_v3)
    }

    fn engine_error_v3(error: HandoffEngineError) -> CompilerModuleHandoffErrorV3 {
        match error {
            HandoffEngineError::Common(error) => error.into(),
            HandoffEngineError::WrongBinding => CompilerModuleHandoffErrorV3::WrongHandoffIdentity,
            HandoffEngineError::PayloadBindingMismatch => {
                CompilerModuleHandoffErrorV3::HandoffIdentityMismatch
            }
            HandoffEngineError::WorkingSetBudgetExceeded { required, maximum } => {
                CompilerModuleHandoffErrorV3::WorkingSetBudgetExceeded { required, maximum }
            }
            HandoffEngineError::PayloadAllocationFailed { requested } => {
                CompilerModuleHandoffErrorV3::PayloadAllocationFailed { requested }
            }
            HandoffEngineError::InvalidCanonicalV3(error) => {
                CompilerModuleHandoffErrorV3::NonCanonicalHandoff(error)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{BuildInvocation, BuildSession, begin_build_attempt};
        use fe2o3_build_authority::CompilerClosureV2;
        use fe2o3_compiler_ffi::{
            CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffV2,
            CompilerModuleKindV1, CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
            DeviceTargetV1, InertFinalCompilerModuleCommitmentV3,
            InertSemanticCompilerModuleHandoffV3,
        };
        use fe2o3_compiler_lineage::{
            InertAbiReceiptV3, InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirReceiptV3,
            InertDataLayoutReceiptV3, InertExportManifestReceiptV3,
            InertFinalCompilerModuleCommitmentReceiptV3, InertFormalMemoryReceiptV3,
            InertKernelIrReceiptV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
            InertProductionSemanticCapsuleV3, InertProofBindingReceiptV3,
            InertRustcIdentityInventoryReceiptV3, InertRustcPreflightPlanReceiptV3,
            InertSemanticToLlvmReceiptV3, InertTargetBindingReceiptV3,
            OrderedInertSemanticLineageReceiptsV3,
        };

        #[test]
        fn v3_schema_has_exactly_three_slots() {
            assert_eq!(HandoffV3Schema::ALL_SLOTS.len(), 3);
        }
        use fe2o3_rustc_invocation::{
            CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3,
            RustcUnitV2,
        };
        use std::ffi::OsString;
        use std::os::unix::fs::{PermissionsExt, symlink};

        const TARGET: &str = "gfx942:xnack-";

        struct TestDirectory(PathBuf);

        impl TestDirectory {
            fn new() -> Self {
                static NEXT: AtomicU64 = AtomicU64::new(1);
                let path = std::env::temp_dir().join(format!(
                    "fe2o3-semantic-module-handoff-v3-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&path).unwrap();
                Self(path)
            }
        }

        impl Drop for TestDirectory {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        fn producer(name: &str) -> ProducerIdentity {
            ProducerIdentity::from_codegen(name, Some(Path::new("/src/semantic-kernel.rs")))
                .unwrap()
        }

        fn begin(path: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
            begin_build_attempt(
                path,
                producer,
                BuildInvocation::from_bytes([seed; 32]),
                BuildSession::from_bytes([seed.wrapping_add(1); 16]),
            )
            .unwrap()
        }

        fn target() -> DeviceTargetV1 {
            DeviceTargetV1::parse(TARGET).unwrap()
        }

        fn closure(seed: u8) -> CompilerClosureV2 {
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

        fn invocation(seed: u8) -> RustcInvocationDescriptorV3 {
            let closure = closure(seed.wrapping_add(1));
            let rustc = RustcUnitV2::new(
                "/workspace/fe2o3",
                vec![
                    "/opt/fe2o3/rustc".into(),
                    "--crate-name".into(),
                    format!("transaction_v3_{seed:02x}"),
                    "crates/transaction-v3-fixture/src/lib.rs".into(),
                    "--crate-type=lib".into(),
                    "--edition=2024".into(),
                    "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
                ],
            )
            .unwrap();
            let environment = CompileEnvironmentV2::from_child_environment(
                [
                    ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
                    ("FE2O3_HSACO_DIR", "/workspace/fe2o3/target/fe2o3"),
                    ("FE2O3_TARGET", TARGET),
                    ("FE2O3_VERIFY_KERNEL_IR", "1"),
                ]
                .map(|(key, value)| (OsString::from(key), OsString::from(value))),
            )
            .unwrap();
            let v2 = RustcInvocationDescriptorV2::new(
                closure.rustc_executable_sha256(),
                closure.codegen_backend_sha256(),
                rustc,
                environment,
            )
            .unwrap();
            RustcInvocationDescriptorV3::new(v2, closure).unwrap()
        }

        fn payload(label: &str, seed: u8) -> Vec<u8> {
            format!("fe2o3-transaction-v3/{label}/seed-{seed:03}").into_bytes()
        }

        fn llvm_module(seed: u8) -> Vec<u8> {
            format!(
                "; ModuleID = 'transaction-v3-{seed:02x}'\ndefine amdgpu_kernel void @kernel() {{ ret void }}\n"
            )
            .into_bytes()
        }

        fn receipts(seed: u8, final_commitment: &[u8]) -> OrderedInertSemanticLineageReceiptsV3 {
            OrderedInertSemanticLineageReceiptsV3::new(
                InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(payload(
                    "inventory",
                    seed,
                ))
                .unwrap(),
                InertRustcPreflightPlanReceiptV3::from_canonical_preimage(payload(
                    "preflight",
                    seed,
                ))
                .unwrap(),
                InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(payload(
                    "semantic-mir",
                    seed,
                ))
                .unwrap(),
                InertMiddleEndReceiptV3::from_canonical_preimage(payload("middle-end", seed))
                    .unwrap(),
                InertKernelIrReceiptV3::from_canonical_preimage(payload("kernel-ir", seed))
                    .unwrap(),
                InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(payload(
                    "mir-to-kir",
                    seed,
                ))
                .unwrap(),
                InertFormalMemoryReceiptV3::from_canonical_preimage(payload("formal-memory", seed))
                    .unwrap(),
                InertProofBindingReceiptV3::from_canonical_preimage(payload("proof-binding", seed))
                    .unwrap(),
                InertTargetBindingReceiptV3::from_canonical_preimage(payload(
                    "target-binding",
                    seed,
                ))
                .unwrap(),
                InertDataLayoutReceiptV3::from_canonical_preimage(payload("data-layout", seed))
                    .unwrap(),
                InertAbiReceiptV3::from_canonical_preimage(payload("abi", seed)).unwrap(),
                InertExportManifestReceiptV3::from_canonical_preimage(payload(
                    "export-manifest",
                    seed,
                ))
                .unwrap(),
                InertAmdgpuLoweringReceiptV3::from_canonical_preimage(payload(
                    "amdgpu-lowering",
                    seed,
                ))
                .unwrap(),
                InertSemanticToLlvmReceiptV3::from_canonical_preimage(payload(
                    "semantic-to-llvm",
                    seed,
                ))
                .unwrap(),
                InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(
                    final_commitment.to_vec(),
                )
                .unwrap(),
            )
        }

        fn outer(seed: u8) -> InertSemanticCompilerModuleHandoffV3 {
            let llvm = llvm_module(seed);
            let envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(
                target(),
                CodeObjectVersion::V5,
            )
            .unwrap();
            let manifest = CompilerModuleSymbolManifestV1::new([
                (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
                (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
            ])
            .unwrap();
            let module = CompilerModuleHandoffV2::new(
                CompilerModuleKindV1::LlvmTextIr,
                target(),
                CodeObjectVersion::V5,
                envelope,
                manifest,
                &llvm,
            )
            .unwrap();
            let final_commitment =
                InertFinalCompilerModuleCommitmentV3::from_handoff(&module).unwrap();
            let capsule = InertProductionSemanticCapsuleV3::new(
                invocation(seed),
                target(),
                receipts(seed, final_commitment.canonical_bytes()),
            )
            .unwrap();
            InertSemanticCompilerModuleHandoffV3::new(capsule, module).unwrap()
        }

        fn slot_path(
            path: &Path,
            producer: &ProducerIdentity,
            attempt: BuildAttempt,
            slot: CompilerModuleHandoffSlotV3,
        ) -> PathBuf {
            let producer_id = producer_identity_for::<HandoffV3Schema>(producer);
            path.join(format!("{PARENT_PREFIX_V3}{}", hex(&producer_id)))
                .join(format!(
                    "{SLOT_PREFIX_V3}{}",
                    hex(&slot_identity_for::<HandoffV3Schema>(
                        producer_id,
                        attempt,
                        slot
                    ))
                ))
        }

        fn rewrite_record_for_payload(
            slot: &Path,
            payload: &[u8],
            update_transaction_identity: bool,
        ) {
            let ready = slot.join(READY_ENTRY);
            let mut record =
                HandoffRecord::<HandoffV3Schema>::decode(&fs::read(&ready).unwrap()).unwrap();
            fs::write(slot.join(PAYLOAD_ENTRY), payload).unwrap();
            let file = fs::File::open(slot.join(PAYLOAD_ENTRY)).unwrap();
            let stat = fstat(&file).unwrap();
            record.file = FileIdentity::from_stat(&stat);
            if update_transaction_identity {
                record.identity = HandoffV3Schema::derive_identity(
                    record.producer,
                    record.slot,
                    record.attempt,
                    record.binding,
                    payload,
                );
            }
            fs::write(ready, record.encode()).unwrap();
        }

        struct FailAt(FaultPoint);

        impl HandoffHooks for FailAt {
            fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
                if point == self.0 {
                    Err(std::io::Error::other("injected V3 transaction fault"))
                } else {
                    Ok(())
                }
            }
        }

        #[test]
        fn v3_round_trip_binds_exact_bytes_identities_and_attempt() {
            let temp = TestDirectory::new();
            let producer = producer("round_trip_v3");
            let attempt = begin(&temp.0, &producer, 1);
            let handoff = outer(11);
            let expected_bytes = handoff.canonical_bytes().to_vec();
            let expected_identity = handoff.identity();

            let receipt =
                publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            assert_eq!(receipt.attempt(), attempt);
            assert_eq!(receipt.slot(), CompilerModuleHandoffSlotV3::Default);
            assert_eq!(receipt.handoff_identity(), expected_identity);
            assert_eq!(receipt.length(), expected_bytes.len());
            assert!(!receipt.grants_compiler_authority());
            assert!(!receipt.grants_publication_authority());

            let producer_id = producer_identity_for::<HandoffV3Schema>(&producer);
            let slot_id = slot_identity_for::<HandoffV3Schema>(
                producer_id,
                attempt,
                CompilerModuleHandoffSlotV3::Default,
            );
            let binding = HandoffBindingV3::from(expected_identity);
            let generation = attempt.generation().to_le_bytes();
            let length = (expected_bytes.len() as u64).to_le_bytes();
            let independently_derived = sha256_parts(&[
                TRANSACTION_IDENTITY_DOMAIN_V3,
                &binding.sha256,
                &binding.byte_len.to_le_bytes(),
                &slot_id,
                &producer_id,
                &generation,
                attempt.session().as_bytes(),
                attempt.invocation().as_bytes(),
                &length,
                &expected_bytes,
            ]);
            assert_eq!(
                receipt.transaction_identity().as_bytes(),
                &independently_derived
            );

            let consumed =
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, expected_identity)
                    .unwrap();
            assert_eq!(consumed.attempt(), attempt);
            assert_eq!(consumed.handoff_identity(), expected_identity);
            assert_eq!(
                consumed.transaction_identity(),
                receipt.transaction_identity()
            );
            assert_eq!(consumed.bytes(), expected_bytes);
            assert!(!consumed.grants_compiler_authority());
            assert!(!consumed.grants_link_authority());
            assert!(!consumed.grants_publication_authority());
            assert!(!consumed.grants_load_authority());
            assert!(!consumed.grants_launch_authority());
            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, expected_identity),
                Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
            ));
        }

        #[test]
        fn receipt_recovery_is_exact_inert_repeatable_and_nonconsuming() {
            let temp = TestDirectory::new();
            let producer = producer("recover_receipt_v3");
            let attempt = begin(&temp.0, &producer, 31);
            let default_handoff = outer(131);
            let default =
                publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &default_handoff)
                    .unwrap();
            let named_handoff = outer(132);
            let named = publish_compiler_module_handoff_in_slot_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmReference,
                &named_handoff,
            )
            .unwrap();

            let recovered_default =
                recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt).unwrap();
            let recovered_named = recover_compiler_module_handoff_receipt_in_slot_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmReference,
            )
            .unwrap();
            assert_eq!(recovered_default, default);
            assert_eq!(recovered_named, named);
            assert_eq!(
                recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt).unwrap(),
                default
            );
            assert!(!recovered_default.grants_compiler_authority());
            assert!(!recovered_default.grants_publication_authority());

            for slot in [
                CompilerModuleHandoffSlotV3::Default,
                CompilerModuleHandoffSlotV3::GeneralGemmReference,
            ] {
                let path = slot_path(&temp.0, &producer, attempt, slot);
                assert!(path.join(READY_ENTRY).is_file());
                assert!(path.join(PAYLOAD_ENTRY).is_file());
                assert!(!path.join(CONSUMED_ENTRY).exists());
            }

            let lease = acquire_compiler_module_handoff_currentness_lease_v3(
                &temp.0,
                &producer,
                recovered_default,
            )
            .unwrap();
            lease.revalidate().unwrap();
        }

        #[test]
        fn receipt_recovery_rejects_absent_consumed_and_wrong_bindings() {
            let absent = TestDirectory::new();
            let absent_producer = producer("recover_absent_v3");
            let absent_attempt = begin(&absent.0, &absent_producer, 32);
            assert!(matches!(
                recover_compiler_module_handoff_receipt_v3(
                    &absent.0,
                    &absent_producer,
                    absent_attempt
                ),
                Err(CompilerModuleHandoffErrorV3::NotPublished)
            ));

            let temp = TestDirectory::new();
            let producer = producer("recover_bindings_v3");
            let attempt = begin(&temp.0, &producer, 33);
            let handoff = outer(133);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();

            assert!(matches!(
                recover_compiler_module_handoff_receipt_in_slot_v3(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::GeneralGemmVectorizedAOnly,
                ),
                Err(CompilerModuleHandoffErrorV3::NotPublished)
            ));
            let wrong_producer = ProducerIdentity::from_codegen(
                "recover_bindings_wrong_crate_v3",
                Some(Path::new("/src/semantic-kernel.rs")),
            )
            .unwrap();
            assert!(matches!(
                recover_compiler_module_handoff_receipt_v3(&temp.0, &wrong_producer, attempt),
                Err(CompilerModuleHandoffErrorV3::Attempt { .. })
            ));

            let newer_attempt = begin(&temp.0, &producer, 34);
            assert!(newer_attempt.generation() > attempt.generation());
            assert!(matches!(
                recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt),
                Err(CompilerModuleHandoffErrorV3::Attempt { .. })
            ));
            assert!(matches!(
                recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, newer_attempt),
                Err(CompilerModuleHandoffErrorV3::NotPublished)
            ));

            let consumed = TestDirectory::new();
            let consumed_producer = self::producer("recover_consumed_v3");
            let consumed_attempt = begin(&consumed.0, &consumed_producer, 35);
            let consumed_handoff = outer(134);
            publish_compiler_module_handoff_v3(
                &consumed.0,
                &consumed_producer,
                consumed_attempt,
                &consumed_handoff,
            )
            .unwrap();
            consume_compiler_module_handoff_v3(
                &consumed.0,
                &consumed_producer,
                consumed_attempt,
                consumed_handoff.identity(),
            )
            .unwrap();
            assert!(matches!(
                recover_compiler_module_handoff_receipt_v3(
                    &consumed.0,
                    &consumed_producer,
                    consumed_attempt
                ),
                Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
            ));
        }

        #[test]
        fn receipt_recovery_rejects_record_payload_and_identity_tamper() {
            for (seed, mutation) in [
                (36, "record-checksum"),
                (37, "payload-transaction"),
                (38, "native-identity"),
                (39, "transaction-identity"),
                (40, "payload-replacement"),
            ] {
                let temp = TestDirectory::new();
                let producer =
                    producer(&format!("recover_tamper_{}_v3", mutation.replace('-', "_")));
                let attempt = begin(&temp.0, &producer, seed);
                let handoff = outer(seed.wrapping_add(100));
                publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
                let slot = slot_path(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::Default,
                );

                match mutation {
                    "record-checksum" => {
                        let ready = slot.join(READY_ENTRY);
                        let mut bytes = fs::read(&ready).unwrap();
                        bytes[RECORD_MAGIC_V3.len() + 2] ^= 1;
                        fs::write(ready, bytes).unwrap();
                    }
                    "payload-transaction" => {
                        let mut bytes = handoff.canonical_bytes().to_vec();
                        bytes[0] ^= 1;
                        rewrite_record_for_payload(&slot, &bytes, false);
                    }
                    "native-identity" => {
                        let ready = slot.join(READY_ENTRY);
                        let mut record =
                            HandoffRecord::<HandoffV3Schema>::decode(&fs::read(&ready).unwrap())
                                .unwrap();
                        record.binding.sha256[0] ^= 1;
                        record.identity = HandoffV3Schema::derive_identity(
                            record.producer,
                            record.slot,
                            record.attempt,
                            record.binding,
                            handoff.canonical_bytes(),
                        );
                        fs::write(ready, record.encode()).unwrap();
                    }
                    "transaction-identity" => {
                        let ready = slot.join(READY_ENTRY);
                        let mut record =
                            HandoffRecord::<HandoffV3Schema>::decode(&fs::read(&ready).unwrap())
                                .unwrap();
                        record.identity[0] ^= 1;
                        fs::write(ready, record.encode()).unwrap();
                    }
                    "payload-replacement" => {
                        let payload = slot.join(PAYLOAD_ENTRY);
                        let displaced = slot.join("module.displaced");
                        fs::rename(&payload, &displaced).unwrap();
                        fs::copy(&displaced, &payload).unwrap();
                        fs::set_permissions(&payload, fs::Permissions::from_mode(0o600)).unwrap();
                    }
                    _ => unreachable!(),
                }

                let rejected =
                    recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt);
                assert!(
                    match mutation {
                        "record-checksum" | "payload-replacement" => matches!(
                            rejected,
                            Err(CompilerModuleHandoffErrorV3::InvalidSlot { .. })
                        ),
                        "payload-transaction" | "transaction-identity" =>
                            matches!(rejected, Err(CompilerModuleHandoffErrorV3::DigestMismatch)),
                        "native-identity" => matches!(
                            rejected,
                            Err(CompilerModuleHandoffErrorV3::HandoffIdentityMismatch)
                        ),
                        _ => unreachable!(),
                    },
                    "mutation={mutation}"
                );
                assert!(slot.join(READY_ENTRY).exists());
                assert!(!slot.join(CONSUMED_ENTRY).exists());
            }
        }

        #[test]
        fn receipt_recovery_handles_every_publication_crash_boundary() {
            let points = [
                FaultPoint::DirectoryCreated,
                FaultPoint::PayloadCreated,
                FaultPoint::PayloadWritten,
                FaultPoint::PayloadSynced,
                FaultPoint::PayloadRenamed,
                FaultPoint::RecordWritten,
                FaultPoint::RecordSynced,
                FaultPoint::RecordRenamed,
                FaultPoint::PublishedSynced,
            ];
            for (index, point) in points.into_iter().enumerate() {
                let temp = TestDirectory::new();
                let producer = producer(&format!("recover_crash_{point:?}_v3"));
                let attempt = begin(&temp.0, &producer, 41 + index as u8);
                let handoff = outer(141 + index as u8);
                assert!(
                    publish_in_slot_v3(
                        &temp.0,
                        &producer,
                        attempt,
                        CompilerModuleHandoffSlotV3::Default,
                        &handoff,
                        &mut FailAt(point),
                    )
                    .is_err()
                );

                let recovered =
                    recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt);
                if matches!(
                    point,
                    FaultPoint::RecordRenamed | FaultPoint::PublishedSynced
                ) {
                    let recovered = recovered.unwrap();
                    assert_eq!(recovered.handoff_identity(), handoff.identity());
                    assert_eq!(recovered.length(), handoff.canonical_bytes().len());
                } else {
                    assert!(matches!(
                        recovered,
                        Err(CompilerModuleHandoffErrorV3::NotPublished)
                    ));
                }
            }
        }

        #[test]
        fn receipt_recovery_rejects_replay_symlink_and_replacement() {
            let replay = TestDirectory::new();
            let replay_producer = producer("recover_replay_v3");
            let replay_attempt = begin(&replay.0, &replay_producer, 50);
            let replay_handoff = outer(150);
            publish_compiler_module_handoff_v3(
                &replay.0,
                &replay_producer,
                replay_attempt,
                &replay_handoff,
            )
            .unwrap();
            let source = slot_path(
                &replay.0,
                &replay_producer,
                replay_attempt,
                CompilerModuleHandoffSlotV3::Default,
            );
            let destination = slot_path(
                &replay.0,
                &replay_producer,
                replay_attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmReference,
            );
            fs::create_dir(&destination).unwrap();
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o700)).unwrap();
            fs::rename(source.join(PAYLOAD_ENTRY), destination.join(PAYLOAD_ENTRY)).unwrap();
            fs::rename(source.join(READY_ENTRY), destination.join(READY_ENTRY)).unwrap();
            assert!(matches!(
                recover_compiler_module_handoff_receipt_in_slot_v3(
                    &replay.0,
                    &replay_producer,
                    replay_attempt,
                    CompilerModuleHandoffSlotV3::GeneralGemmReference,
                ),
                Err(CompilerModuleHandoffErrorV3::InvalidSlot { .. })
            ));

            for attack in ["ready-symlink", "slot-copy"] {
                let temp = TestDirectory::new();
                let producer = producer(&format!("recover_{}_v3", attack.replace('-', "_")));
                let attempt = begin(&temp.0, &producer, 51);
                let handoff = outer(151);
                publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
                let slot = slot_path(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::Default,
                );
                match attack {
                    "ready-symlink" => {
                        let ready = slot.join(READY_ENTRY);
                        let displaced = slot.join("ready.displaced");
                        fs::rename(&ready, &displaced).unwrap();
                        symlink(&displaced, &ready).unwrap();
                    }
                    "slot-copy" => {
                        let displaced = slot.with_extension("displaced");
                        fs::rename(&slot, &displaced).unwrap();
                        fs::create_dir(&slot).unwrap();
                        fs::set_permissions(&slot, fs::Permissions::from_mode(0o700)).unwrap();
                        for entry in [PAYLOAD_ENTRY, READY_ENTRY] {
                            fs::copy(displaced.join(entry), slot.join(entry)).unwrap();
                            fs::set_permissions(
                                slot.join(entry),
                                fs::Permissions::from_mode(0o600),
                            )
                            .unwrap();
                        }
                    }
                    _ => unreachable!(),
                }
                assert!(
                    recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt)
                        .is_err(),
                    "attack={attack}"
                );
            }

            let output = TestDirectory::new();
            let output_producer = producer("recover_output_symlink_v3");
            let output_attempt = begin(&output.0, &output_producer, 52);
            let output_handoff = outer(152);
            publish_compiler_module_handoff_v3(
                &output.0,
                &output_producer,
                output_attempt,
                &output_handoff,
            )
            .unwrap();
            let parked = output.0.with_extension("parked");
            fs::rename(&output.0, &parked).unwrap();
            symlink(&parked, &output.0).unwrap();
            assert!(
                recover_compiler_module_handoff_receipt_v3(
                    &output.0,
                    &output_producer,
                    output_attempt
                )
                .is_err()
            );
            fs::remove_file(&output.0).unwrap();
            fs::remove_dir_all(&parked).unwrap();
        }

        #[test]
        fn concurrent_recovery_and_publication_never_expose_partial_state() {
            let temp = TestDirectory::new();
            let producer = Arc::new(producer("recover_publish_race_v3"));
            let attempt = begin(&temp.0, &producer, 53);
            let handoff = Arc::new(outer(153));
            let barrier = Arc::new(std::sync::Barrier::new(2));

            let publish_path = temp.0.clone();
            let publish_producer = Arc::clone(&producer);
            let publish_handoff = Arc::clone(&handoff);
            let publish_barrier = Arc::clone(&barrier);
            let publisher = std::thread::spawn(move || {
                publish_barrier.wait();
                publish_compiler_module_handoff_v3(
                    &publish_path,
                    &publish_producer,
                    attempt,
                    &publish_handoff,
                )
            });

            let recover_path = temp.0.clone();
            let recover_producer = Arc::clone(&producer);
            let recover_barrier = Arc::clone(&barrier);
            let recovery = std::thread::spawn(move || {
                recover_barrier.wait();
                recover_compiler_module_handoff_receipt_v3(
                    &recover_path,
                    &recover_producer,
                    attempt,
                )
            });

            let published = publisher.join().unwrap().unwrap();
            match recovery.join().unwrap() {
                Ok(recovered) => assert_eq!(recovered, published),
                Err(CompilerModuleHandoffErrorV3::NotPublished) => {}
                Err(error) => panic!("unexpected concurrent recovery error: {error}"),
            }
            assert_eq!(
                recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt).unwrap(),
                published
            );
        }

        #[test]
        fn committed_publication_survives_transient_lease_mint_failure() {
            let temp = TestDirectory::new();
            let producer = producer("recover_after_lease_failure_v3");
            let attempt = begin(&temp.0, &producer, 54);
            let handoff = outer(154);
            let mut held_lock = None;
            assert!(matches!(
                publish_compiler_module_handoff_in_slot_with_currentness_after_publish_v3(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::Default,
                    &handoff,
                    || {
                        let output = PinnedOutput::open_existing(&temp.0).unwrap();
                        held_lock = Some(output.lock().unwrap());
                    },
                ),
                Err(CompilerModuleHandoffErrorV3::Busy)
            ));
            drop(held_lock);

            let recovered =
                recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt).unwrap();
            assert_eq!(recovered.handoff_identity(), handoff.identity());
            assert_eq!(recovered.length(), handoff.canonical_bytes().len());
            let lease =
                acquire_compiler_module_handoff_currentness_lease_v3(&temp.0, &producer, recovered)
                    .unwrap();
            lease.revalidate().unwrap();
        }

        #[test]
        fn currentness_token_consumes_once_and_preserves_inert_semantics() {
            let temp = TestDirectory::new();
            let producer = producer("currentness_round_trip_v3");
            let attempt = begin(&temp.0, &producer, 13);
            let handoff = outer(111);
            let publication = publish_compiler_module_handoff_with_currentness_v3(
                &temp.0, &producer, attempt, &handoff,
            )
            .unwrap();
            let receipt = publication.receipt();
            let lease = publication.into_current_lease();
            assert_eq!(lease.receipt(), receipt);
            assert!(!lease.grants_compiler_authority());
            assert!(!lease.grants_link_authority());
            assert!(!lease.grants_load_authority());
            assert!(!lease.grants_launch_authority());
            lease.revalidate().unwrap();

            let token = lease.acquire_current_token().unwrap();
            lease.validate_current_token(&token).unwrap();
            token.revalidate_locked_currentness().unwrap();
            assert!(!token.grants_compiler_authority());
            assert!(!token.grants_link_authority());
            assert!(!token.grants_load_authority());
            assert!(!token.grants_launch_authority());
            let consumed =
                consume_compiler_module_handoff_with_currentness_v3(&lease, token).unwrap();
            assert_eq!(consumed.attempt(), attempt);
            assert_eq!(consumed.slot(), CompilerModuleHandoffSlotV3::Default);
            assert_eq!(consumed.handoff_identity(), receipt.handoff_identity());
            assert_eq!(
                consumed.transaction_identity(),
                receipt.transaction_identity()
            );
            assert_eq!(consumed.bytes(), handoff.canonical_bytes());
            assert!(!consumed.grants_publication_authority());
            assert!(matches!(
                lease.acquire_current_token(),
                Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
            ));
            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity()),
                Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
            ));
        }

        #[test]
        fn prior_lease_rejects_a_newer_committed_attempt_generation() {
            let temp = TestDirectory::new();
            let producer = producer("stale_currentness_v3");
            let first_attempt = begin(&temp.0, &producer, 14);
            let handoff = outer(112);
            let lease = publish_compiler_module_handoff_with_currentness_v3(
                &temp.0,
                &producer,
                first_attempt,
                &handoff,
            )
            .unwrap()
            .into_current_lease();

            let second_attempt = begin(&temp.0, &producer, 15);
            assert!(second_attempt.generation() > first_attempt.generation());
            assert!(matches!(
                lease.acquire_current_token(),
                Err(CompilerModuleHandoffErrorV3::Attempt { .. })
            ));
        }

        #[test]
        fn token_and_lease_from_different_v3_slots_never_combine() {
            let temp = TestDirectory::new();
            let producer = producer("cross_publication_token_v3");
            let attempt = begin(&temp.0, &producer, 16);
            let handoff = outer(113);
            let reference = publish_compiler_module_handoff_in_slot_with_currentness_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmReference,
                &handoff,
            )
            .unwrap()
            .into_current_lease();
            let vectorized = publish_compiler_module_handoff_in_slot_with_currentness_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmVectorizedAOnly,
                &handoff,
            )
            .unwrap()
            .into_current_lease();

            let wrong_token = reference.acquire_current_token().unwrap();
            assert!(matches!(
                vectorized.validate_current_token(&wrong_token),
                Err(CompilerModuleHandoffErrorV3::MismatchedCurrentnessToken)
            ));
            assert!(matches!(
                consume_compiler_module_handoff_with_currentness_v3(&vectorized, wrong_token),
                Err(CompilerModuleHandoffErrorV3::MismatchedCurrentnessToken)
            ));

            let token = reference.acquire_current_token().unwrap();
            consume_compiler_module_handoff_with_currentness_v3(&reference, token).unwrap();
            vectorized.revalidate().unwrap();
        }

        #[test]
        fn currentness_rejects_file_tamper_and_file_or_directory_replacement() {
            for (seed, mutation) in [
                (17, "payload-tamper"),
                (18, "ready-replacement"),
                (19, "slot-replacement"),
                (20, "output-replacement"),
            ] {
                let temp = TestDirectory::new();
                let producer = producer(&format!("currentness_replacement_{seed}"));
                let attempt = begin(&temp.0, &producer, seed);
                let handoff = outer(seed.wrapping_add(100));
                let lease = publish_compiler_module_handoff_with_currentness_v3(
                    &temp.0, &producer, attempt, &handoff,
                )
                .unwrap()
                .into_current_lease();
                let slot = slot_path(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::Default,
                );

                match mutation {
                    "payload-tamper" => {
                        let file = fs::OpenOptions::new()
                            .write(true)
                            .open(slot.join(PAYLOAD_ENTRY))
                            .unwrap();
                        file.write_all_at(b"X", 0).unwrap();
                        file.sync_all().unwrap();
                    }
                    "ready-replacement" => {
                        let ready = slot.join(READY_ENTRY);
                        let displaced = slot.join("ready.displaced");
                        fs::rename(&ready, &displaced).unwrap();
                        fs::copy(&displaced, &ready).unwrap();
                        fs::set_permissions(&ready, fs::Permissions::from_mode(0o600)).unwrap();
                    }
                    "slot-replacement" => {
                        let displaced = slot.with_extension("displaced");
                        fs::rename(&slot, &displaced).unwrap();
                        fs::create_dir(&slot).unwrap();
                        fs::set_permissions(&slot, fs::Permissions::from_mode(0o700)).unwrap();
                    }
                    "output-replacement" => {
                        let displaced = temp.0.with_extension("displaced");
                        fs::rename(&temp.0, &displaced).unwrap();
                        fs::create_dir(&temp.0).unwrap();
                    }
                    _ => unreachable!(),
                }

                assert!(lease.acquire_current_token().is_err(), "{mutation}");
                if mutation == "output-replacement" {
                    drop(lease);
                    fs::remove_dir_all(temp.0.with_extension("displaced")).unwrap();
                }
            }
        }

        #[test]
        fn only_one_concurrent_currentness_token_holds_the_v3_lock() {
            let temp = TestDirectory::new();
            let producer = producer("currentness_concurrency_v3");
            let attempt = begin(&temp.0, &producer, 21);
            let handoff = outer(121);
            let lease = Arc::new(
                publish_compiler_module_handoff_with_currentness_v3(
                    &temp.0, &producer, attempt, &handoff,
                )
                .unwrap()
                .into_current_lease(),
            );
            let winner = lease.acquire_current_token().unwrap();
            let contender_lease = Arc::clone(&lease);
            let contender = std::thread::spawn(move || {
                matches!(
                    contender_lease.acquire_current_token(),
                    Err(CompilerModuleHandoffErrorV3::Busy)
                )
            });
            assert!(contender.join().unwrap());
            consume_compiler_module_handoff_with_currentness_v3(&lease, winner).unwrap();
            assert!(matches!(
                lease.acquire_current_token(),
                Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
            ));
        }

        #[test]
        fn v3_wrong_expected_identity_is_retryable_and_same_publish_is_idempotent() {
            let temp = TestDirectory::new();
            let producer = producer("identity_v3");
            let attempt = begin(&temp.0, &producer, 2);
            let handoff = outer(21);
            let other = outer(22);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();

            assert!(matches!(
                publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff),
                Err(CompilerModuleHandoffErrorV3::AlreadyPublished)
            ));
            assert!(matches!(
                publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &other),
                Err(CompilerModuleHandoffErrorV3::WrongHandoffIdentity)
            ));
            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, other.identity()),
                Err(CompilerModuleHandoffErrorV3::WrongHandoffIdentity)
            ));
            consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity())
                .unwrap();
        }

        #[test]
        fn v3_namespaces_never_fall_back_to_v1_or_v2() {
            let temp = TestDirectory::new();
            let producer = producer("no_fallback_legacy");
            let attempt = begin(&temp.0, &producer, 3);
            let handoff = outer(31);
            let closure = closure(90);
            publish_compiler_module_handoff_v1(
                &temp.0,
                &producer,
                attempt,
                handoff.canonical_bytes(),
            )
            .unwrap();
            publish_compiler_module_handoff_v2(
                &temp.0,
                &producer,
                attempt,
                closure,
                handoff.canonical_bytes(),
            )
            .unwrap();
            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity()),
                Err(CompilerModuleHandoffErrorV3::NotPublished)
            ));
            assert!(matches!(
                recover_compiler_module_handoff_receipt_v3(&temp.0, &producer, attempt),
                Err(CompilerModuleHandoffErrorV3::NotPublished)
            ));

            let temp = TestDirectory::new();
            let v3_producer = self::producer("no_fallback_v3");
            let attempt = begin(&temp.0, &v3_producer, 4);
            publish_compiler_module_handoff_v3(&temp.0, &v3_producer, attempt, &handoff).unwrap();
            assert!(matches!(
                consume_compiler_module_handoff_v1(&temp.0, &v3_producer, attempt),
                Err(CompilerModuleHandoffErrorV1::NotPublished)
            ));
            assert!(matches!(
                consume_compiler_module_handoff_v2(&temp.0, &v3_producer, attempt, closure),
                Err(CompilerModuleHandoffErrorV2::NotPublished)
            ));
        }

        #[test]
        fn per_version_bounds_preserve_v1_and_v2_without_allocating_the_v3_limit() {
            let original_max = (64 * 1024 * 1024) + (512 * 1024) + 128 + 83;
            assert_eq!(MAX_COMPILER_MODULE_HANDOFF_BYTES, original_max);
            assert_eq!(HandoffV1Schema::MAX_HANDOFF_BYTES, original_max);
            assert_eq!(
                protected_v2::HandoffV2Schema::MAX_HANDOFF_BYTES,
                original_max
            );
            assert_eq!(
                HandoffV3Schema::MAX_HANDOFF_BYTES,
                fe2o3_compiler_ffi::MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3
            );
            assert!(HandoffV3Schema::MAX_HANDOFF_BYTES > original_max);
            assert!(matches!(
                validate_handoff_size::<HandoffV3Schema>(
                    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 + 1
                ),
                Err(CompilerModuleHandoffErrorV1::InvalidHandoffSize {
                    actual,
                    maximum
                }) if actual == MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 + 1
                    && maximum == MAX_COMPILER_MODULE_HANDOFF_BYTES_V3
            ));
        }

        #[test]
        fn v3_decode_working_set_has_an_exact_maximum_and_fallible_input_allocation() {
            assert!(matches!(
                validate_decode_working_set::<HandoffV3Schema>(
                    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
                    MAX_V3_DECODE_WORKING_SET_BYTES,
                ),
                Ok(required) if required == MAX_V3_DECODE_WORKING_SET_BYTES
            ));
            assert!(matches!(
                validate_decode_working_set::<HandoffV3Schema>(
                    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
                    MAX_V3_DECODE_WORKING_SET_BYTES - 1,
                ),
                Err(HandoffEngineError::WorkingSetBudgetExceeded { required, maximum })
                    if required == MAX_V3_DECODE_WORKING_SET_BYTES
                        && maximum == MAX_V3_DECODE_WORKING_SET_BYTES - 1
            ));
            assert!(matches!(
                validate_decode_working_set::<HandoffV3Schema>(usize::MAX, usize::MAX),
                Err(HandoffEngineError::WorkingSetBudgetExceeded {
                    required: usize::MAX,
                    maximum: usize::MAX,
                })
            ));
            assert!(matches!(
                try_allocate_payload_buffer(usize::MAX),
                Err(HandoffEngineError::PayloadAllocationFailed {
                    requested: usize::MAX,
                })
            ));
        }

        #[test]
        fn v3_schema_decode_retains_one_shared_canonical_payload_allocation() {
            let expected = outer(204);
            let bytes = expected.canonical_bytes().to_vec();
            let allocation = bytes.as_ptr();
            let binding = HandoffBindingV3::from(expected.identity());

            let decoded = match HandoffV3Schema::decode_payload(binding, bytes) {
                Ok(decoded) => decoded,
                Err(_) => panic!("canonical shared V3 fixture must decode"),
            };

            assert_eq!(decoded.canonical_bytes().as_ptr(), allocation);
            let outer_start = allocation as usize;
            let outer_end = outer_start + decoded.canonical_bytes().len();
            for retained in [
                decoded.capsule().canonical_bytes(),
                decoded
                    .capsule()
                    .receipts()
                    .semantic_mir()
                    .canonical_preimage(),
                decoded.module_handoff().canonical_bytes(),
                decoded.module_handoff().module_bytes(),
            ] {
                let retained_start = retained.as_ptr() as usize;
                let retained_end = retained_start + retained.len();
                assert!(retained_start >= outer_start);
                assert!(retained_end <= outer_end);
            }
        }

        #[test]
        fn v3_working_set_rejection_is_retryable_before_tombstone() {
            let temp = TestDirectory::new();
            let producer = producer("working_set_retry_v3");
            let attempt = begin(&temp.0, &producer, 105);
            let handoff = outer(205);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            let slot = slot_path(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::Default,
            );
            let required = handoff.canonical_bytes().len() * V3_DECODE_WORKING_SET_MULTIPLIER
                + V3_DECODE_FIXED_BYTES;

            assert!(matches!(
                consume_in_slot_engine_with_working_set_limit::<HandoffV3Schema>(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::Default,
                    handoff.identity().into(),
                    required - 1,
                    &mut NoFaults,
                ),
                Err(HandoffEngineError::WorkingSetBudgetExceeded {
                    required: actual,
                    maximum,
                }) if actual == required && maximum == required - 1
            ));
            assert!(slot.join(READY_ENTRY).exists());
            assert!(slot.join(PAYLOAD_ENTRY).exists());
            assert!(!slot.join(CONSUMED_ENTRY).exists());

            consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity())
                .unwrap();
        }

        #[test]
        fn oversized_v3_record_is_rejected_before_payload_allocation() {
            let temp = TestDirectory::new();
            let producer = producer("oversized_record_v3");
            let attempt = begin(&temp.0, &producer, 5);
            let handoff = outer(41);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            let slot = slot_path(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::Default,
            );
            let ready = slot.join(READY_ENTRY);
            let original_record = fs::read(&ready).unwrap();
            let mut record = HandoffRecord::<HandoffV3Schema>::decode(&original_record).unwrap();
            record.length = MAX_COMPILER_MODULE_HANDOFF_BYTES_V3 + 1;
            record.binding.byte_len = record.length as u64;
            fs::write(ready, record.encode()).unwrap();

            assert!(matches!(
                consume_compiler_module_handoff_v3(
                    &temp.0,
                    &producer,
                    attempt,
                    handoff.identity()
                ),
                Err(CompilerModuleHandoffErrorV3::InvalidSlot { ref reason, .. })
                    if reason == "record contains an invalid module length"
            ));
            assert!(slot.join(READY_ENTRY).exists());
            assert!(!slot.join(CONSUMED_ENTRY).exists());

            fs::write(slot.join(READY_ENTRY), original_record).unwrap();
            consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity())
                .unwrap();
        }

        #[test]
        fn payload_tamper_is_rejected_by_exact_transaction_digest() {
            let temp = TestDirectory::new();
            let producer = producer("digest_tamper_v3");
            let attempt = begin(&temp.0, &producer, 6);
            let handoff = outer(51);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            let slot = slot_path(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::Default,
            );
            let mut tampered = handoff.canonical_bytes().to_vec();
            tampered[0] ^= 1;
            rewrite_record_for_payload(&slot, &tampered, false);

            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity()),
                Err(CompilerModuleHandoffErrorV3::DigestMismatch)
            ));
            assert!(slot.join(READY_ENTRY).exists());
            assert!(!slot.join(CONSUMED_ENTRY).exists());
        }

        #[test]
        fn independently_rehashed_noncanonical_payload_fails_before_tombstone() {
            let temp = TestDirectory::new();
            let producer = producer("strict_decode_v3");
            let attempt = begin(&temp.0, &producer, 7);
            let handoff = outer(61);
            let original = handoff.canonical_bytes().to_vec();
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            let slot = slot_path(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::Default,
            );
            let mut tampered = original.clone();
            tampered[0] ^= 1;
            rewrite_record_for_payload(&slot, &tampered, true);

            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity()),
                Err(CompilerModuleHandoffErrorV3::NonCanonicalHandoff(
                    fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV3::InvalidMagic
                ))
            ));
            assert!(slot.join(READY_ENTRY).exists());
            assert!(!slot.join(CONSUMED_ENTRY).exists());

            rewrite_record_for_payload(&slot, &original, true);
            consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity())
                .unwrap();
        }

        #[test]
        fn record_identity_splice_is_rejected_even_with_valid_checksum() {
            let temp = TestDirectory::new();
            let producer = producer("binding_splice_v3");
            let attempt = begin(&temp.0, &producer, 8);
            let handoff = outer(71);
            let replacement = outer(72);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            let slot = slot_path(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::Default,
            );
            let ready = slot.join(READY_ENTRY);
            let mut record =
                HandoffRecord::<HandoffV3Schema>::decode(&fs::read(&ready).unwrap()).unwrap();
            record.binding.sha256 = *replacement.identity().sha256();
            record.identity = HandoffV3Schema::derive_identity(
                record.producer,
                record.slot,
                record.attempt,
                record.binding,
                handoff.canonical_bytes(),
            );
            fs::write(ready, record.encode()).unwrap();

            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity()),
                Err(CompilerModuleHandoffErrorV3::WrongHandoffIdentity)
            ));
            assert!(slot.join(READY_ENTRY).exists());
        }

        #[test]
        fn named_v3_slots_are_independent_and_slot_bound() {
            let temp = TestDirectory::new();
            let producer = producer("named_slots_v3");
            let attempt = begin(&temp.0, &producer, 9);
            let handoff = outer(81);
            let reference = publish_compiler_module_handoff_in_slot_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmReference,
                &handoff,
            )
            .unwrap();
            let vectorized = publish_compiler_module_handoff_in_slot_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmVectorizedAOnly,
                &handoff,
            )
            .unwrap();
            assert_ne!(
                reference.transaction_identity(),
                vectorized.transaction_identity()
            );

            let first = consume_compiler_module_handoff_in_slot_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmReference,
                handoff.identity(),
            )
            .unwrap();
            let second = consume_compiler_module_handoff_in_slot_v3(
                &temp.0,
                &producer,
                attempt,
                CompilerModuleHandoffSlotV3::GeneralGemmVectorizedAOnly,
                handoff.identity(),
            )
            .unwrap();
            assert_eq!(first.bytes(), handoff.canonical_bytes());
            assert_eq!(second.bytes(), handoff.canonical_bytes());
        }

        #[test]
        fn concurrent_v3_publish_and_consume_have_single_winners() {
            let temp = TestDirectory::new();
            let producer = Arc::new(producer("concurrent_v3"));
            let attempt = begin(&temp.0, &producer, 10);
            let handoff = Arc::new(outer(91));
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let mut publishers = Vec::new();
            for _ in 0..2 {
                let path = temp.0.clone();
                let producer = Arc::clone(&producer);
                let handoff = Arc::clone(&handoff);
                let barrier = Arc::clone(&barrier);
                publishers.push(std::thread::spawn(move || {
                    barrier.wait();
                    publish_compiler_module_handoff_v3(&path, &producer, attempt, &handoff)
                }));
            }
            let publish_results = publishers
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                publish_results
                    .iter()
                    .filter(|result| result.is_ok())
                    .count(),
                1
            );
            assert_eq!(
                publish_results
                    .iter()
                    .filter(|result| matches!(
                        result,
                        Err(CompilerModuleHandoffErrorV3::AlreadyPublished)
                    ))
                    .count(),
                1
            );

            let barrier = Arc::new(std::sync::Barrier::new(2));
            let mut consumers = Vec::new();
            for _ in 0..2 {
                let path = temp.0.clone();
                let producer = Arc::clone(&producer);
                let barrier = Arc::clone(&barrier);
                let identity = handoff.identity();
                consumers.push(std::thread::spawn(move || {
                    barrier.wait();
                    consume_compiler_module_handoff_v3(&path, &producer, attempt, identity)
                }));
            }
            let consume_results = consumers
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                consume_results
                    .iter()
                    .filter(|result| result.is_ok())
                    .count(),
                1
            );
            assert_eq!(
                consume_results
                    .iter()
                    .filter(|result| matches!(
                        result,
                        Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
                    ))
                    .count(),
                1
            );
        }

        #[test]
        fn v3_strict_decode_fault_boundary_is_retryable_only_before_tombstone() {
            let temp = TestDirectory::new();
            let producer = producer("decode_boundary_v3");
            let attempt = begin(&temp.0, &producer, 11);
            let handoff = outer(101);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            assert!(
                consume_in_slot_v3(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::Default,
                    handoff.identity(),
                    &mut FailAt(FaultPoint::PayloadValidated),
                )
                .is_err()
            );
            consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity())
                .unwrap();

            let temp = TestDirectory::new();
            let producer = self::producer("tombstone_boundary_v3");
            let attempt = begin(&temp.0, &producer, 12);
            let handoff = outer(102);
            publish_compiler_module_handoff_v3(&temp.0, &producer, attempt, &handoff).unwrap();
            assert!(
                consume_in_slot_v3(
                    &temp.0,
                    &producer,
                    attempt,
                    CompilerModuleHandoffSlotV3::Default,
                    handoff.identity(),
                    &mut FailAt(FaultPoint::ConsumedRenamed),
                )
                .is_err()
            );
            assert!(matches!(
                consume_compiler_module_handoff_v3(&temp.0, &producer, attempt, handoff.identity()),
                Err(CompilerModuleHandoffErrorV3::AlreadyConsumed)
            ));
        }
    }
}

pub use semantic_v3::{
    CompilerModuleHandoffConsumptionTokenV3, CompilerModuleHandoffCurrentnessLeaseV3,
    CompilerModuleHandoffErrorV3, CompilerModuleHandoffPublicationV3,
    CompilerModuleHandoffReceiptV3, CompilerModuleHandoffSlotV3,
    CompilerModuleHandoffTransactionIdentityV3, ConsumedCompilerModuleHandoffV3,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V3, acquire_compiler_module_handoff_currentness_lease_v3,
    consume_compiler_module_handoff_in_slot_v3, consume_compiler_module_handoff_v3,
    consume_compiler_module_handoff_with_currentness_v3,
    publish_compiler_module_handoff_in_slot_v3,
    publish_compiler_module_handoff_in_slot_with_currentness_v3,
    publish_compiler_module_handoff_v3, publish_compiler_module_handoff_with_currentness_v3,
    recover_compiler_module_handoff_receipt_in_slot_v3, recover_compiler_module_handoff_receipt_v3,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildInvocation, BuildSession, begin_build_attempt};
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::{Arc, Barrier};
    use std::thread;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-module-handoff-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn producer(name: &str) -> ProducerIdentity {
        ProducerIdentity::from_codegen(name, Some(Path::new("/src/kernel.rs"))).unwrap()
    }

    fn begin(path: &Path, producer: &ProducerIdentity, seed: u8) -> BuildAttempt {
        begin_build_attempt(
            path,
            producer,
            BuildInvocation::from_bytes([seed; 32]),
            BuildSession::from_bytes([seed.wrapping_add(1); 16]),
        )
        .unwrap()
    }

    fn slot_path(path: &Path, producer: &ProducerIdentity, attempt: BuildAttempt) -> PathBuf {
        slot_path_for(
            path,
            producer,
            attempt,
            CompilerModuleHandoffSlotV1::Default,
        )
    }

    fn slot_path_for(
        path: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV1,
    ) -> PathBuf {
        let producer_id = producer_identity(producer);
        path.join(format!("{PARENT_PREFIX}{}", hex(&producer_id)))
            .join(format!(
                "{SLOT_PREFIX}{}",
                hex(&slot_identity(producer_id, attempt, slot))
            ))
    }

    fn write_private_test_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }

    fn create_private_test_directory(path: &Path) {
        fs::create_dir(path).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn happy_path_is_private_digest_bound_and_exactly_once() {
        let temp = TestDirectory::new();
        let producer = producer("kernel");
        let attempt = begin(&temp.0, &producer, 1);
        let module = b"exact compiler module";
        let receipt =
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, module).unwrap();
        assert_eq!(receipt.slot(), CompilerModuleHandoffSlotV1::Default);
        assert_eq!(receipt.identity().as_bytes(), &sha256(module));
        assert_eq!(receipt.length(), module.len());
        assert!(!receipt.grants_publication_authority());
        assert!(!receipt.grants_compiler_authority());

        let slot = slot_path(&temp.0, &producer, attempt);
        assert_eq!(
            fs::metadata(slot.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&slot).unwrap().permissions().mode() & 0o777,
            0o700
        );
        for entry in [PAYLOAD_ENTRY, READY_ENTRY] {
            assert_eq!(
                fs::metadata(slot.join(entry)).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        let consumed = consume_compiler_module_handoff_v1(&temp.0, &producer, attempt).unwrap();
        assert_eq!(consumed.slot(), CompilerModuleHandoffSlotV1::Default);
        assert_eq!(consumed.bytes(), module);
        assert_eq!(consumed.identity(), receipt.identity());
        assert!(!consumed.grants_publication_authority());
        assert!(!consumed.grants_compiler_authority());
        assert!(!consumed.grants_link_authority());
        assert!(!consumed.grants_load_authority());
        assert!(!consumed.grants_launch_authority());
        assert!(slot.join(CONSUMED_ENTRY).is_file());
        assert!(!slot.join(PAYLOAD_ENTRY).exists());
        assert!(matches!(
            consume_compiler_module_handoff_v1(&temp.0, &producer, attempt),
            Err(CompilerModuleHandoffErrorV1::AlreadyConsumed)
        ));
        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, module),
            Err(CompilerModuleHandoffErrorV1::AlreadyConsumed)
        ));
    }

    #[test]
    fn two_general_gemm_slots_transfer_independent_modules_under_one_attempt() {
        let temp = TestDirectory::new();
        let producer = producer("general_gemm");
        let attempt = begin(&temp.0, &producer, 41);
        let reference_slot = CompilerModuleHandoffSlotV1::GeneralGemmReference;
        let vector_slot = CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly;

        let reference = publish_compiler_module_handoff_in_slot_v1(
            &temp.0,
            &producer,
            attempt,
            reference_slot,
            b"reference module",
        )
        .unwrap();
        assert_eq!(reference.slot(), reference_slot);
        assert_ne!(
            slot_path_for(&temp.0, &producer, attempt, reference_slot),
            slot_path_for(&temp.0, &producer, attempt, vector_slot)
        );
        let consumed_reference =
            consume_compiler_module_handoff_in_slot_v1(&temp.0, &producer, attempt, reference_slot)
                .unwrap();
        assert_eq!(consumed_reference.slot(), reference_slot);
        assert_eq!(consumed_reference.bytes(), b"reference module");
        assert!(matches!(
            consume_compiler_module_handoff_in_slot_v1(&temp.0, &producer, attempt, reference_slot,),
            Err(CompilerModuleHandoffErrorV1::AlreadyConsumed)
        ));

        let vector = publish_compiler_module_handoff_in_slot_v1(
            &temp.0,
            &producer,
            attempt,
            vector_slot,
            b"vectorized module",
        )
        .unwrap();
        assert_eq!(vector.slot(), vector_slot);
        let consumed_vector =
            consume_compiler_module_handoff_in_slot_v1(&temp.0, &producer, attempt, vector_slot)
                .unwrap();
        assert_eq!(consumed_vector.slot(), vector_slot);
        assert_eq!(consumed_vector.bytes(), b"vectorized module");
        assert_ne!(consumed_reference.identity(), consumed_vector.identity());
        assert_eq!(consumed_reference.attempt(), consumed_vector.attempt());
    }

    #[test]
    fn named_slot_cannot_be_replayed_through_the_default_api() {
        let temp = TestDirectory::new();
        let producer = producer("general_gemm_slot_substitution");
        let attempt = begin(&temp.0, &producer, 42);
        publish_compiler_module_handoff_in_slot_v1(
            &temp.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV1::GeneralGemmReference,
            b"reference module",
        )
        .unwrap();
        assert!(matches!(
            consume_compiler_module_handoff_v1(&temp.0, &producer, attempt),
            Err(CompilerModuleHandoffErrorV1::NotPublished)
        ));
        let consumed = consume_compiler_module_handoff_in_slot_v1(
            &temp.0,
            &producer,
            attempt,
            CompilerModuleHandoffSlotV1::GeneralGemmReference,
        )
        .unwrap();
        assert_eq!(consumed.bytes(), b"reference module");
    }

    #[test]
    fn conflicting_republication_and_corrupt_tombstone_fail_closed() {
        let temp = TestDirectory::new();
        let producer = producer("kernel");
        let attempt = begin(&temp.0, &producer, 12);
        publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"first").unwrap();
        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"second"),
            Err(CompilerModuleHandoffErrorV1::ConflictingPublication)
        ));
        consume_compiler_module_handoff_v1(&temp.0, &producer, attempt).unwrap();
        let tombstone = slot_path(&temp.0, &producer, attempt).join(CONSUMED_ENTRY);
        let mut bytes = fs::read(&tombstone).unwrap();
        bytes[RECORD_MAGIC.len()] ^= 1;
        fs::write(tombstone, bytes).unwrap();
        assert!(matches!(
            consume_compiler_module_handoff_v1(&temp.0, &producer, attempt),
            Err(CompilerModuleHandoffErrorV1::InvalidSlot { .. })
        ));
    }

    #[test]
    fn empty_and_oversize_handoffs_are_rejected_before_slot_creation() {
        let temp = TestDirectory::new();
        let producer = producer("kernel");
        let attempt = begin(&temp.0, &producer, 2);
        assert_eq!(MAX_COMPILER_MODULE_HANDOFF_BYTES, 67_633_363);
        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, &[]),
            Err(CompilerModuleHandoffErrorV1::InvalidHandoffSize { actual: 0, .. })
        ));
        let oversized = vec![0; MAX_COMPILER_MODULE_HANDOFF_BYTES + 1];
        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, &oversized),
            Err(CompilerModuleHandoffErrorV1::InvalidHandoffSize { .. })
        ));
        assert!(!slot_path(&temp.0, &producer, attempt).exists());
    }

    #[test]
    fn stale_attempt_and_mismatched_producer_are_rejected() {
        let temp = TestDirectory::new();
        let first_producer = producer("kernel");
        let stale = begin(&temp.0, &first_producer, 3);
        let current = begin(&temp.0, &first_producer, 4);
        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &first_producer, stale, b"stale"),
            Err(CompilerModuleHandoffErrorV1::Attempt { .. })
        ));
        let other = producer("other");
        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &other, current, b"mismatch"),
            Err(CompilerModuleHandoffErrorV1::Attempt { .. })
        ));
    }

    #[test]
    fn completed_attempt_has_a_typed_not_claimable_error() {
        let temp = TestDirectory::new();
        let producer = producer("host_only");
        let attempt = begin(&temp.0, &producer, 29);
        let output = PinnedOutput::open_existing(&temp.0).unwrap();
        let lock = output.lock().unwrap();
        let mut attempts = read_attempt_registry(&output).unwrap();
        attempts
            .claim_backend(&producer.stable_source, attempt)
            .unwrap();
        attempts
            .record_legacy_backend_receipt(&producer.stable_source, attempt)
            .unwrap();
        crate::commit_attempt_registry_direct(&output, &attempts).unwrap();
        drop(lock);
        drop(output);
        assert!(matches!(
            consume_compiler_module_handoff_v1(&temp.0, &producer, attempt),
            Err(CompilerModuleHandoffErrorV1::AttemptNotClaimable)
        ));
    }

    #[test]
    fn simulation_kir_uses_backend_claimed_custody_and_authority_free_completion() {
        let temp = TestDirectory::new();
        let producer = producer("simulation_kernel");
        let attempt = begin(&temp.0, &producer, 30);
        let receipt =
            publish_simulation_kernel_ir_handoff_v1(&temp.0, &producer, attempt, b"canonical-kir")
                .unwrap();
        assert_eq!(receipt.attempt(), attempt);
        assert_eq!(receipt.length(), b"canonical-kir".len());
        assert!(!receipt.grants_publication_authority());
        assert!(!receipt.grants_hardware_authority());

        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"protected-protocol"),
            Err(CompilerModuleHandoffErrorV1::AttemptNotClaimable)
        ));

        let consumed =
            consume_simulation_kernel_ir_handoff_v1(&temp.0, &producer, attempt).unwrap();
        assert_eq!(consumed.bytes(), b"canonical-kir");
        assert!(!consumed.grants_publication_authority());
        assert!(!consumed.grants_hardware_authority());
        let observation =
            complete_simulation_kernel_ir_attempt_v1(&temp.0, &producer, &consumed).unwrap();
        assert_eq!(
            observation.canonical_kir_sha256(),
            *consumed.identity().as_bytes()
        );
        assert!(!observation.grants_artifact_authority());
        assert!(!observation.proves_hardware_execution());
        let duplicate =
            complete_simulation_kernel_ir_attempt_v1(&temp.0, &producer, &consumed).unwrap_err();
        assert!(matches!(
            duplicate,
            CompilerModuleHandoffErrorV1::AttemptNotClaimable
        ));
        let generic_completion =
            crate::finish_build_attempt(&temp.0, &producer, attempt).unwrap_err();
        assert!(
            generic_completion
                .to_string()
                .contains("observation-only attempt"),
            "{generic_completion}"
        );
        assert!(crate::read_backend_publication_receipt_v1(&temp.0, &producer, attempt).is_err());
        assert!(matches!(
            crate::read_backend_publication_receipt_v2(&temp.0, &producer, attempt),
            Err(crate::AttemptScopedHsacoPublicationErrorV2::IncompatibleReceiptVersion)
        ));
        assert!(matches!(
            crate::read_backend_publication_receipt_v3(&temp.0, &producer, attempt),
            Err(crate::AttemptScopedHsacoPublicationErrorV3::IncompatibleReceiptVersion)
        ));
    }

    #[test]
    fn handoff_schema_slot_cardinality_is_exact() {
        assert_eq!(HandoffV1Schema::ALL_SLOTS.len(), 3);
        assert_eq!(SimulationKernelIrHandoffSchemaV1::ALL_SLOTS.len(), 1);
    }

    #[test]
    fn host_backend_receipt_cannot_enter_simulation_kir_custody() {
        let temp = TestDirectory::new();
        let producer = producer("host_only_simulation");
        let attempt = begin(&temp.0, &producer, 31);
        let output = PinnedOutput::open_existing(&temp.0).unwrap();
        let lock = output.lock().unwrap();
        let mut attempts = read_attempt_registry(&output).unwrap();
        attempts
            .claim_backend(&producer.stable_source, attempt)
            .unwrap();
        attempts
            .record_legacy_backend_receipt(&producer.stable_source, attempt)
            .unwrap();
        crate::commit_attempt_registry_direct(&output, &attempts).unwrap();
        drop(lock);
        drop(output);

        assert!(matches!(
            consume_simulation_kernel_ir_handoff_v1(&temp.0, &producer, attempt),
            Err(CompilerModuleHandoffErrorV1::AttemptNotClaimable)
        ));
    }

    #[test]
    fn concurrent_publish_and_consume_have_single_winners() {
        let temp = Arc::new(TestDirectory::new());
        let producer = Arc::new(producer("kernel"));
        let attempt = begin(&temp.0, &producer, 5);
        let barrier = Arc::new(Barrier::new(8));
        let publishers = (0..8)
            .map(|_| {
                let temp = Arc::clone(&temp);
                let producer = Arc::clone(&producer);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"concurrent")
                })
            })
            .collect::<Vec<_>>();
        let results = publishers
            .into_iter()
            .map(|join| join.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(CompilerModuleHandoffErrorV1::AlreadyPublished)
                ))
                .count()
                >= 1
        );

        let barrier = Arc::new(Barrier::new(8));
        let consumers = (0..8)
            .map(|_| {
                let temp = Arc::clone(&temp);
                let producer = Arc::clone(&producer);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    consume_compiler_module_handoff_v1(&temp.0, &producer, attempt)
                })
            })
            .collect::<Vec<_>>();
        let results = consumers
            .into_iter()
            .map(|join| join.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(CompilerModuleHandoffErrorV1::AlreadyConsumed)
                ))
                .count(),
            7
        );
    }

    #[test]
    fn symlink_and_replacement_attacks_fail_closed() {
        for attack in [
            "payload-symlink",
            "record-symlink",
            "replacement",
            "hardlink",
        ] {
            let temp = TestDirectory::new();
            let producer = producer("kernel");
            let attempt = begin(&temp.0, &producer, 6);
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"original").unwrap();
            let slot = slot_path(&temp.0, &producer, attempt);
            let outside = temp.0.join("outside");
            fs::write(&outside, b"replacement").unwrap();
            fs::set_permissions(&outside, fs::Permissions::from_mode(0o600)).unwrap();
            match attack {
                "payload-symlink" => {
                    fs::remove_file(slot.join(PAYLOAD_ENTRY)).unwrap();
                    symlink(&outside, slot.join(PAYLOAD_ENTRY)).unwrap();
                }
                "record-symlink" => {
                    fs::remove_file(slot.join(READY_ENTRY)).unwrap();
                    symlink(&outside, slot.join(READY_ENTRY)).unwrap();
                }
                "replacement" => {
                    fs::remove_file(slot.join(PAYLOAD_ENTRY)).unwrap();
                    fs::copy(&outside, slot.join(PAYLOAD_ENTRY)).unwrap();
                    fs::set_permissions(
                        slot.join(PAYLOAD_ENTRY),
                        fs::Permissions::from_mode(0o600),
                    )
                    .unwrap();
                }
                "hardlink" => {
                    fs::remove_file(slot.join(PAYLOAD_ENTRY)).unwrap();
                    fs::hard_link(&outside, slot.join(PAYLOAD_ENTRY)).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    consume_compiler_module_handoff_v1(&temp.0, &producer, attempt),
                    Err(CompilerModuleHandoffErrorV1::InvalidSlot { .. })
                        | Err(CompilerModuleHandoffErrorV1::Io(_))
                ),
                "attack={attack}"
            );
            assert_eq!(fs::read(&outside).unwrap(), b"replacement");
        }
    }

    #[test]
    fn slot_and_parent_substitution_fail_without_following_symlinks() {
        for attack in ["slot-symlink", "parent-symlink", "slot-copy"] {
            let temp = TestDirectory::new();
            let producer = producer("kernel");
            let attempt = begin(&temp.0, &producer, 13);
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"original").unwrap();
            let slot = slot_path(&temp.0, &producer, attempt);
            let parent = slot.parent().unwrap().to_path_buf();
            let parked = temp.0.join(format!("parked-{attack}"));
            let preserved_payload = match attack {
                "slot-symlink" => {
                    fs::rename(&slot, &parked).unwrap();
                    symlink(&parked, &slot).unwrap();
                    parked.join(PAYLOAD_ENTRY)
                }
                "parent-symlink" => {
                    fs::rename(&parent, &parked).unwrap();
                    symlink(&parked, &parent).unwrap();
                    parked.join(slot.file_name().unwrap()).join(PAYLOAD_ENTRY)
                }
                "slot-copy" => {
                    fs::rename(&slot, &parked).unwrap();
                    fs::create_dir(&slot).unwrap();
                    fs::set_permissions(&slot, fs::Permissions::from_mode(0o700)).unwrap();
                    for entry in [PAYLOAD_ENTRY, READY_ENTRY] {
                        fs::copy(parked.join(entry), slot.join(entry)).unwrap();
                        fs::set_permissions(slot.join(entry), fs::Permissions::from_mode(0o600))
                            .unwrap();
                    }
                    parked.join(PAYLOAD_ENTRY)
                }
                _ => unreachable!(),
            };
            assert!(
                consume_compiler_module_handoff_v1(&temp.0, &producer, attempt).is_err(),
                "attack={attack}"
            );
            assert_eq!(fs::read(preserved_payload).unwrap(), b"original");
        }
    }

    #[test]
    fn in_place_mutation_and_record_corruption_are_rejected() {
        for attack in ["mutate", "truncate", "grow", "record"] {
            let temp = TestDirectory::new();
            let producer = producer("kernel");
            let attempt = begin(&temp.0, &producer, 7);
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"original bytes")
                .unwrap();
            let slot = slot_path(&temp.0, &producer, attempt);
            match attack {
                "mutate" => fs::write(slot.join(PAYLOAD_ENTRY), b"changed! bytes").unwrap(),
                "truncate" => fs::write(slot.join(PAYLOAD_ENTRY), b"x").unwrap(),
                "grow" => fs::write(slot.join(PAYLOAD_ENTRY), b"original bytes plus more").unwrap(),
                "record" => {
                    let mut bytes = fs::read(slot.join(READY_ENTRY)).unwrap();
                    bytes[10] ^= 1;
                    fs::write(slot.join(READY_ENTRY), bytes).unwrap();
                }
                _ => unreachable!(),
            }
            assert!(
                consume_compiler_module_handoff_v1(&temp.0, &producer, attempt).is_err(),
                "attack={attack}"
            );
        }
    }

    struct FailAt(FaultPoint);
    impl HandoffHooks for FailAt {
        fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
            if point == self.0 {
                Err(std::io::Error::other("simulated crash"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn publish_crash_residue_recovers_or_preserves_committed_state() {
        let points = [
            FaultPoint::DirectoryCreated,
            FaultPoint::PayloadCreated,
            FaultPoint::PayloadWritten,
            FaultPoint::PayloadSynced,
            FaultPoint::PayloadRenamed,
            FaultPoint::RecordWritten,
            FaultPoint::RecordSynced,
            FaultPoint::RecordRenamed,
            FaultPoint::PublishedSynced,
        ];
        for point in points {
            let temp = TestDirectory::new();
            let producer = producer("kernel");
            let attempt = begin(&temp.0, &producer, 8);
            assert!(
                publish_with_hooks(&temp.0, &producer, attempt, b"module", &mut FailAt(point))
                    .is_err()
            );
            let retry = publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"module");
            if matches!(
                point,
                FaultPoint::RecordRenamed | FaultPoint::PublishedSynced
            ) {
                assert!(matches!(
                    retry,
                    Err(CompilerModuleHandoffErrorV1::AlreadyPublished)
                ));
            } else {
                retry.unwrap();
            }
            assert_eq!(
                consume_compiler_module_handoff_v1(&temp.0, &producer, attempt)
                    .unwrap()
                    .bytes(),
                b"module"
            );
        }
    }

    #[test]
    fn poisoned_crash_residue_is_rejected_before_any_cleanup() {
        let temp = TestDirectory::new();
        let producer = producer("kernel");
        let attempt = begin(&temp.0, &producer, 14);
        assert!(
            publish_with_hooks(
                &temp.0,
                &producer,
                attempt,
                b"module",
                &mut FailAt(FaultPoint::PayloadRenamed),
            )
            .is_err()
        );
        let slot = slot_path(&temp.0, &producer, attempt);
        let outside = temp.0.join("outside-poison-target");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, slot.join(".tmp-poison")).unwrap();
        assert!(matches!(
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"module"),
            Err(CompilerModuleHandoffErrorV1::InvalidSlot { .. })
        ));
        assert_eq!(fs::read(slot.join(PAYLOAD_ENTRY)).unwrap(), b"module");
        assert_eq!(fs::read(outside).unwrap(), b"outside");
    }

    #[test]
    fn non_utf8_residue_is_statted_exactly_without_touching_its_utf8_alias() {
        let temp = TestDirectory::new();
        let producer = producer("non_utf8_residue");
        let attempt = begin(&temp.0, &producer, 16);
        assert!(
            publish_with_hooks(
                &temp.0,
                &producer,
                attempt,
                b"module",
                &mut FailAt(FaultPoint::PayloadRenamed),
            )
            .is_err()
        );
        let slot = slot_path(&temp.0, &producer, attempt);
        let raw_name = OsString::from_vec([TEMP_PREFIX.as_bytes(), b"alias-", &[0xff]].concat());
        let alias_name = format!("{TEMP_PREFIX}alias-\u{fffd}");
        assert_eq!(raw_name.to_string_lossy(), alias_name);
        let raw_path = slot.join(&raw_name);
        let alias_path = slot.join(&alias_name);
        symlink("missing-target", &raw_path).unwrap();
        write_private_test_file(&alias_path, b"valid UTF-8 alias");

        let error =
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"module").unwrap_err();

        assert!(matches!(
            error,
            CompilerModuleHandoffErrorV1::InvalidSlot { ref path, ref reason }
                if path == &raw_path
                    && reason == "recovery residue is not a private single-link regular file"
        ));
        assert!(
            fs::symlink_metadata(&raw_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read(&alias_path).unwrap(), b"valid UTF-8 alias");
        assert_eq!(fs::read(slot.join(PAYLOAD_ENTRY)).unwrap(), b"module");
    }

    #[test]
    fn stale_cleanup_unlinks_non_utf8_names_and_utf8_aliases_exactly() {
        let temp = TestDirectory::new();
        let producer = producer("non_utf8_stale_cleanup");
        let stale = begin(&temp.0, &producer, 17);
        publish_compiler_module_handoff_v1(&temp.0, &producer, stale, b"stale").unwrap();
        let stale_slot = slot_path(&temp.0, &producer, stale);
        let raw_entry = OsString::from_vec([TEMP_PREFIX.as_bytes(), b"stale-", &[0xff]].concat());
        let alias_entry = format!("{TEMP_PREFIX}stale-\u{fffd}");
        assert_eq!(raw_entry.to_string_lossy(), alias_entry);
        write_private_test_file(&stale_slot.join(&raw_entry), b"raw stale entry");
        write_private_test_file(&stale_slot.join(&alias_entry), b"UTF-8 stale alias");

        let parent = stale_slot.parent().unwrap();
        let raw_slot = OsString::from_vec([SLOT_PREFIX.as_bytes(), b"stale-", &[0xff]].concat());
        let alias_slot = format!("{SLOT_PREFIX}stale-\u{fffd}");
        assert_eq!(raw_slot.to_string_lossy(), alias_slot);
        let raw_slot_path = parent.join(&raw_slot);
        let alias_slot_path = parent.join(&alias_slot);
        create_private_test_directory(&raw_slot_path);
        create_private_test_directory(&alias_slot_path);

        let current = begin(&temp.0, &producer, 18);
        publish_compiler_module_handoff_v1(&temp.0, &producer, current, b"current").unwrap();

        assert!(!stale_slot.exists());
        assert!(!raw_slot_path.exists());
        assert!(!alias_slot_path.exists());
        assert_eq!(
            consume_compiler_module_handoff_v1(&temp.0, &producer, current)
                .unwrap()
                .bytes(),
            b"current"
        );
    }

    #[test]
    fn recovery_cleanup_enforces_the_exact_slot_bound_without_partial_mutation() {
        let exact = TestDirectory::new();
        let exact_producer = producer("exact_slot_bound");
        let exact_attempt = begin(&exact.0, &exact_producer, 19);
        assert!(
            publish_with_hooks(
                &exact.0,
                &exact_producer,
                exact_attempt,
                b"exact module",
                &mut FailAt(FaultPoint::PayloadRenamed),
            )
            .is_err()
        );
        let exact_slot = slot_path(&exact.0, &exact_producer, exact_attempt);
        for index in 0..MAX_SLOT_ENTRIES - 1 {
            write_private_test_file(
                &exact_slot.join(format!("{TEMP_PREFIX}exact-bound-{index}")),
                b"exact-bound residue",
            );
        }
        assert_eq!(fs::read_dir(&exact_slot).unwrap().count(), MAX_SLOT_ENTRIES);

        publish_compiler_module_handoff_v1(
            &exact.0,
            &exact_producer,
            exact_attempt,
            b"exact module",
        )
        .unwrap();
        assert_eq!(
            consume_compiler_module_handoff_v1(&exact.0, &exact_producer, exact_attempt)
                .unwrap()
                .bytes(),
            b"exact module"
        );

        let over = TestDirectory::new();
        let over_producer = producer("over_slot_bound");
        let over_attempt = begin(&over.0, &over_producer, 20);
        assert!(
            publish_with_hooks(
                &over.0,
                &over_producer,
                over_attempt,
                b"over module",
                &mut FailAt(FaultPoint::PayloadRenamed),
            )
            .is_err()
        );
        let over_slot = slot_path(&over.0, &over_producer, over_attempt);
        for index in 0..MAX_SLOT_ENTRIES {
            write_private_test_file(
                &over_slot.join(format!("{TEMP_PREFIX}over-bound-{index}")),
                b"over-bound residue",
            );
        }
        assert_eq!(
            fs::read_dir(&over_slot).unwrap().count(),
            MAX_SLOT_ENTRIES + 1
        );

        let error = publish_compiler_module_handoff_v1(
            &over.0,
            &over_producer,
            over_attempt,
            b"over module",
        )
        .unwrap_err();

        assert!(matches!(
            error,
            CompilerModuleHandoffErrorV1::InvalidSlot { ref reason, .. }
                if reason == "slot exceeds its entry bound"
        ));
        assert_eq!(
            fs::read(over_slot.join(PAYLOAD_ENTRY)).unwrap(),
            b"over module"
        );
        for index in 0..MAX_SLOT_ENTRIES {
            assert_eq!(
                fs::read(over_slot.join(format!("{TEMP_PREFIX}over-bound-{index}"))).unwrap(),
                b"over-bound residue"
            );
        }
    }

    #[test]
    fn stale_slot_overflow_fails_before_removing_any_slot() {
        let temp = TestDirectory::new();
        let producer = producer("stale_slot_overflow");
        let stale = begin(&temp.0, &producer, 21);
        publish_compiler_module_handoff_v1(&temp.0, &producer, stale, b"stale").unwrap();
        let stale_slot = slot_path(&temp.0, &producer, stale);
        let parent = stale_slot.parent().unwrap();
        for index in 0..MAX_STALE_SLOTS {
            fs::create_dir(parent.join(format!("{SLOT_PREFIX}overflow-{index}"))).unwrap();
        }
        assert_eq!(fs::read_dir(parent).unwrap().count(), MAX_STALE_SLOTS + 1);
        let current = begin(&temp.0, &producer, 22);

        let error = publish_compiler_module_handoff_v1(&temp.0, &producer, current, b"current")
            .unwrap_err();

        assert!(matches!(
            error,
            CompilerModuleHandoffErrorV1::InvalidSlot { ref reason, .. }
                if reason == "too many stale handoff slots"
        ));
        assert!(stale_slot.exists());
        assert_eq!(fs::read_dir(parent).unwrap().count(), MAX_STALE_SLOTS + 1);
        assert!(!slot_path(&temp.0, &producer, current).exists());
    }

    #[test]
    fn consumption_crash_after_tombstone_never_replays_bytes() {
        for point in [FaultPoint::ConsumedRenamed, FaultPoint::ConsumedSynced] {
            let temp = TestDirectory::new();
            let producer = producer("kernel");
            let attempt = begin(&temp.0, &producer, 9);
            publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"module").unwrap();
            assert!(consume_with_hooks(&temp.0, &producer, attempt, &mut FailAt(point)).is_err());
            assert!(matches!(
                consume_compiler_module_handoff_v1(&temp.0, &producer, attempt),
                Err(CompilerModuleHandoffErrorV1::AlreadyConsumed)
            ));
            assert!(
                !slot_path(&temp.0, &producer, attempt)
                    .join(PAYLOAD_ENTRY)
                    .exists()
            );
        }
    }

    #[test]
    fn consumption_before_tombstone_is_retryable() {
        let temp = TestDirectory::new();
        let producer = producer("kernel");
        let attempt = begin(&temp.0, &producer, 15);
        publish_compiler_module_handoff_v1(&temp.0, &producer, attempt, b"module").unwrap();
        assert!(
            consume_with_hooks(
                &temp.0,
                &producer,
                attempt,
                &mut FailAt(FaultPoint::PayloadValidated),
            )
            .is_err()
        );
        assert_eq!(
            consume_compiler_module_handoff_v1(&temp.0, &producer, attempt)
                .unwrap()
                .bytes(),
            b"module"
        );
    }

    #[test]
    fn newer_attempt_cleans_private_stale_slot() {
        let temp = TestDirectory::new();
        let producer = producer("kernel");
        let stale = begin(&temp.0, &producer, 10);
        publish_compiler_module_handoff_v1(&temp.0, &producer, stale, b"stale").unwrap();
        let stale_slot = slot_path(&temp.0, &producer, stale);
        let current = begin(&temp.0, &producer, 11);
        publish_compiler_module_handoff_v1(&temp.0, &producer, current, b"current").unwrap();
        assert!(!stale_slot.exists());
        assert_eq!(
            consume_compiler_module_handoff_v1(&temp.0, &producer, current)
                .unwrap()
                .bytes(),
            b"current"
        );
    }
}
