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
    read_attempt_registry,
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

#[derive(Clone, Copy)]
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

struct HandoffRecord {
    slot: [u8; 32],
    attempt: BuildAttempt,
    producer: [u8; 32],
    identity: CompilerModuleHandoffIdentityV1,
    length: usize,
    file: FileIdentity,
}

impl HandoffRecord {
    fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(RECORD_BYTES);
        bytes.extend_from_slice(RECORD_MAGIC);
        bytes.extend_from_slice(&RECORD_VERSION.to_le_bytes());
        bytes.extend_from_slice(&self.slot);
        bytes.extend_from_slice(&self.attempt.generation().to_le_bytes());
        bytes.extend_from_slice(self.attempt.session().as_bytes());
        bytes.extend_from_slice(self.attempt.invocation().as_bytes());
        bytes.extend_from_slice(&self.producer);
        bytes.extend_from_slice(self.identity.as_bytes());
        bytes.extend_from_slice(&(self.length as u64).to_le_bytes());
        bytes.extend_from_slice(&self.file.device.to_le_bytes());
        bytes.extend_from_slice(&self.file.inode.to_le_bytes());
        bytes.extend_from_slice(&self.file.modified_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.file.modified_nanoseconds.to_le_bytes());
        bytes.extend_from_slice(&self.file.changed_seconds.to_le_bytes());
        bytes.extend_from_slice(&self.file.changed_nanoseconds.to_le_bytes());
        bytes.extend_from_slice(&(self.file.length as u64).to_le_bytes());
        let checksum = sha256_parts(&[RECORD_DOMAIN, &bytes]);
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), RECORD_BYTES);
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != RECORD_BYTES {
            return Err("record has a noncanonical length");
        }
        let (body, checksum) = bytes.split_at(bytes.len() - 32);
        if sha256_parts(&[RECORD_DOMAIN, body]).as_slice() != checksum {
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
        let session = super::BuildSession::from_bytes(decoder.array()?);
        let invocation = super::BuildInvocation::from_bytes(decoder.array()?);
        let attempt = BuildAttempt::from_env_value(&format!(
            "{generation}:{}:{}",
            session.to_hex(),
            invocation.to_hex()
        ))
        .map_err(|_| "record contains an invalid attempt")?;
        let producer = decoder.array()?;
        let identity = CompilerModuleHandoffIdentityV1(decoder.array()?);
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
        if !decoder.finished() || length == 0 || length > MAX_COMPILER_MODULE_HANDOFF_BYTES {
            return Err("record contains an invalid module length");
        }
        Ok(Self {
            slot,
            attempt,
            producer,
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
    name: String,
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
    validate_handoff_size(handoff_bytes.len())?;
    let output = PinnedOutput::open(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize(&output, producer, attempt)?;
    let producer_id = producer_identity(producer);
    let slot_id = slot_identity(producer_id, attempt, handoff_slot);
    let parent = open_or_create_private_directory(
        &output.fd,
        &output.display_path,
        &format!("{PARENT_PREFIX}{}", hex(&producer_id)),
        hooks,
    )?;
    cleanup_stale_slots(&parent, producer_id, attempt)?;
    let slot = open_or_create_private_directory(
        &parent.fd,
        &parent.path,
        &format!("{SLOT_PREFIX}{}", hex(&slot_id)),
        hooks,
    )?;
    recover_slot(&slot)?;
    if entry_exists(&slot, CONSUMED_ENTRY)? {
        read_bound_record(&slot, CONSUMED_ENTRY, producer_id, slot_id, attempt)?;
        cleanup_consumed_payload(&slot);
        return Err(CompilerModuleHandoffErrorV1::AlreadyConsumed);
    }
    if entry_exists(&slot, READY_ENTRY)? {
        let committed = read_bound_record(&slot, READY_ENTRY, producer_id, slot_id, attempt)?;
        let committed_bytes = read_payload(&slot, &committed)?;
        return if committed_bytes == handoff_bytes {
            Err(CompilerModuleHandoffErrorV1::AlreadyPublished)
        } else {
            Err(CompilerModuleHandoffErrorV1::ConflictingPublication)
        };
    }

    let identity = CompilerModuleHandoffIdentityV1(sha256(handoff_bytes));
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
        ));
    }
    let record = HandoffRecord {
        slot: slot_id,
        attempt,
        producer: producer_id,
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
    validate_named_record(&slot, READY_ENTRY, &record_bytes)?;
    Ok(CompilerModuleHandoffReceiptV1 {
        attempt,
        slot: handoff_slot,
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
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize(&output, producer, attempt)?;
    let producer_id = producer_identity(producer);
    let slot_id = slot_identity(producer_id, attempt, handoff_slot);
    let parent = open_private_directory(
        &output.fd,
        &output.display_path,
        &format!("{PARENT_PREFIX}{}", hex(&producer_id)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    cleanup_stale_slots(&parent, producer_id, attempt)?;
    let slot = open_private_directory(
        &parent.fd,
        &parent.path,
        &format!("{SLOT_PREFIX}{}", hex(&slot_id)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    recover_slot(&slot)?;
    if entry_exists(&slot, CONSUMED_ENTRY)? {
        read_bound_record(&slot, CONSUMED_ENTRY, producer_id, slot_id, attempt)?;
        cleanup_consumed_payload(&slot);
        return Err(CompilerModuleHandoffErrorV1::AlreadyConsumed);
    }
    let record = read_bound_record(&slot, READY_ENTRY, producer_id, slot_id, attempt)?;
    let bytes = read_payload(&slot, &record)?;
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
    Ok(ConsumedCompilerModuleHandoffV1 {
        attempt,
        slot: handoff_slot,
        identity: record.identity,
        bytes: Arc::from(bytes),
    })
}

fn authorize(
    output: &PinnedOutput,
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
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
    if record.phase != AttemptPhase::Building || record.backend_receipt.is_some() {
        return Err(attempt_error(
            "build attempt is not in the claimable building phase",
        ));
    }
    Ok(())
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

fn slot_identity(
    producer: [u8; 32],
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV1,
) -> [u8; 32] {
    let generation = attempt.generation().to_le_bytes();
    if slot == CompilerModuleHandoffSlotV1::Default {
        return sha256_parts(&[
            SLOT_DOMAIN,
            &producer,
            &generation,
            attempt.session().as_bytes(),
            attempt.invocation().as_bytes(),
        ]);
    }
    sha256_parts(&[
        NAMED_SLOT_DOMAIN,
        &producer,
        &generation,
        attempt.session().as_bytes(),
        attempt.invocation().as_bytes(),
        &[slot as u8],
    ])
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
    name: &str,
) -> Result<Option<PinnedDirectory>, CompilerModuleHandoffErrorV1> {
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
        name: name.to_string(),
        path: parent_path.join(name),
        device: stat.st_dev,
        inode: stat.st_ino,
    };
    directory.verify()?;
    Ok(Some(directory))
}

fn cleanup_stale_slots(
    parent: &PinnedDirectory,
    producer: [u8; 32],
    attempt: BuildAttempt,
) -> Result<(), CompilerModuleHandoffErrorV1> {
    let current = [
        CompilerModuleHandoffSlotV1::Default,
        CompilerModuleHandoffSlotV1::GeneralGemmReference,
        CompilerModuleHandoffSlotV1::GeneralGemmVectorizedAOnly,
    ]
    .map(|slot| {
        format!(
            "{SLOT_PREFIX}{}",
            hex(&slot_identity(producer, attempt, slot))
        )
    });
    let scan = rustix::io::fcntl_dupfd_cloexec(&parent.fd, 0).map_err(std::io::Error::from)?;
    let mut entries = Dir::read_from(&scan).map_err(std::io::Error::from)?;
    let mut stale = Vec::new();
    for entry in &mut entries {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_string_lossy();
        if name == "." || name == ".." || current.iter().any(|current| name == current.as_str()) {
            continue;
        }
        if !name.starts_with(SLOT_PREFIX) {
            return Err(invalid_slot(
                &parent.path.join(name.as_ref()),
                "unexpected producer handoff entry",
            ));
        }
        if stale.len() == MAX_STALE_SLOTS {
            return Err(invalid_slot(&parent.path, "too many stale handoff slots"));
        }
        stale.push(name.into_owned());
    }
    for name in stale {
        remove_slot_entry(parent, &name)?;
    }
    if parent_entry_count(parent)? > current.len() {
        return Err(invalid_slot(
            &parent.path,
            "handoff producer directory exceeds its entry bound",
        ));
    }
    parent.verify()?;
    Ok(())
}

fn remove_slot_entry(
    parent: &PinnedDirectory,
    name: &str,
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
        let name = entry.file_name().to_string_lossy();
        if name == "." || name == ".." {
            continue;
        }
        if names.len() == MAX_SLOT_ENTRIES {
            return Err(invalid_slot(&slot.path, "slot exceeds its entry bound"));
        }
        let stat = statat(&slot.fd, name.as_ref(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            return Err(invalid_slot(
                &slot.path.join(name.as_ref()),
                "nested directories are forbidden",
            ));
        }
        names.push(name.into_owned());
    }
    for name in names {
        unlinkat(&slot.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
    }
    fsync(&slot.fd).map_err(std::io::Error::from)?;
    Ok(())
}

fn recover_slot(slot: &PinnedDirectory) -> Result<(), CompilerModuleHandoffErrorV1> {
    let names = slot_entries(slot)?;
    for name in &names {
        if !matches!(name.as_str(), PAYLOAD_ENTRY | READY_ENTRY | CONSUMED_ENTRY)
            && !name.starts_with(TEMP_PREFIX)
        {
            return Err(invalid_slot(&slot.path.join(name), "unexpected slot entry"));
        }
    }
    if names.iter().any(|name| name == READY_ENTRY)
        && names.iter().any(|name| name == CONSUMED_ENTRY)
    {
        return Err(invalid_slot(
            &slot.path,
            "ready and consumed records coexist",
        ));
    }
    let committed = names
        .iter()
        .any(|name| name == READY_ENTRY || name == CONSUMED_ENTRY);
    let residue = names
        .into_iter()
        .filter(|name| name.starts_with(TEMP_PREFIX) || (!committed && name == PAYLOAD_ENTRY))
        .collect::<Vec<_>>();
    for name in &residue {
        reject_nonregular_before_cleanup(slot, name)?;
    }
    for name in residue {
        if name.starts_with(TEMP_PREFIX) || name == PAYLOAD_ENTRY {
            unlinkat(&slot.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        }
    }
    if !committed {
        fsync(&slot.fd).map_err(std::io::Error::from)?;
    }
    slot.verify()?;
    Ok(())
}

fn slot_entries(slot: &PinnedDirectory) -> Result<Vec<String>, CompilerModuleHandoffErrorV1> {
    let scan = rustix::io::fcntl_dupfd_cloexec(&slot.fd, 0).map_err(std::io::Error::from)?;
    let mut directory = Dir::read_from(&scan).map_err(std::io::Error::from)?;
    let mut names = Vec::new();
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_string_lossy();
        if name == "." || name == ".." {
            continue;
        }
        if names.len() == MAX_SLOT_ENTRIES {
            return Err(invalid_slot(&slot.path, "slot exceeds its entry bound"));
        }
        names.push(name.into_owned());
    }
    Ok(names)
}

fn parent_entry_count(parent: &PinnedDirectory) -> Result<usize, CompilerModuleHandoffErrorV1> {
    let scan = rustix::io::fcntl_dupfd_cloexec(&parent.fd, 0).map_err(std::io::Error::from)?;
    let mut directory = Dir::read_from(&scan).map_err(std::io::Error::from)?;
    let mut count = 0usize;
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            count = count.saturating_add(1);
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

fn read_payload(
    slot: &PinnedDirectory,
    record: &HandoffRecord,
) -> Result<Vec<u8>, CompilerModuleHandoffErrorV1> {
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
        ));
    }
    let mut bytes = Vec::with_capacity(record.length);
    Read::by_ref(&mut file)
        .take((MAX_COMPILER_MODULE_HANDOFF_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    let after = fstat(&file).map_err(std::io::Error::from)?;
    let still_named =
        statat(&slot.fd, PAYLOAD_ENTRY, AtFlags::SYMLINK_NOFOLLOW).map_err(std::io::Error::from)?;
    if bytes.len() != record.length
        || !record.file.matches(&after)
        || !record.file.matches(&still_named)
    {
        return Err(invalid_slot(
            &slot.path.join(PAYLOAD_ENTRY),
            "payload changed while its descriptor was read",
        ));
    }
    if sha256(&bytes) != *record.identity.as_bytes() {
        return Err(CompilerModuleHandoffErrorV1::DigestMismatch);
    }
    Ok(bytes)
}

fn read_bound_record(
    slot: &PinnedDirectory,
    entry: &str,
    producer: [u8; 32],
    slot_identity: [u8; 32],
    attempt: BuildAttempt,
) -> Result<HandoffRecord, CompilerModuleHandoffErrorV1> {
    let bytes = read_private_file(slot, entry, RECORD_BYTES)?
        .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    let record = HandoffRecord::decode(&bytes)
        .map_err(|reason| invalid_slot(&slot.path.join(entry), reason))?;
    if record.slot != slot_identity || record.producer != producer || record.attempt != attempt {
        return Err(invalid_slot(
            &slot.path.join(entry),
            "record binding does not match the requested attempt and producer",
        ));
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

fn validate_named_record(
    slot: &PinnedDirectory,
    entry: &str,
    expected: &[u8],
) -> Result<(), CompilerModuleHandoffErrorV1> {
    let actual = read_private_file(slot, entry, RECORD_BYTES)?
        .ok_or_else(|| invalid_slot(&slot.path.join(entry), "record disappeared after commit"))?;
    if actual != expected {
        return Err(invalid_slot(
            &slot.path.join(entry),
            "record changed after commit",
        ));
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
    entry: &str,
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

fn validate_handoff_size(length: usize) -> Result<(), CompilerModuleHandoffErrorV1> {
    if length == 0 || length > MAX_COMPILER_MODULE_HANDOFF_BYTES {
        return Err(CompilerModuleHandoffErrorV1::InvalidHandoffSize {
            actual: length,
            maximum: MAX_COMPILER_MODULE_HANDOFF_BYTES,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildInvocation, BuildSession, begin_build_attempt};
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
