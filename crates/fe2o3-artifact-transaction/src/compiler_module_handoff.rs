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

trait HandoffSchema: Sized {
    type Slot: Copy + Eq;
    type Binding: Copy + Eq;

    const PARENT_PREFIX: &'static str;
    const SLOT_PREFIX: &'static str;
    const RECORD_MAGIC: &'static [u8];
    const RECORD_VERSION: u16;
    const PRODUCER_DOMAIN: &'static [u8];
    const SLOT_DOMAIN: &'static [u8];
    const NAMED_SLOT_DOMAIN: &'static [u8];
    const RECORD_DOMAIN: &'static [u8];
    const RECORD_BYTES: usize;
    const VALIDATE_RECORD_DURING_RECOVERY: bool;
    const ALL_SLOTS: [Self::Slot; 3];

    fn default_slot() -> Self::Slot;
    fn slot_tag(slot: Self::Slot) -> u8;
    fn encode_binding(binding: Self::Binding, bytes: &mut Vec<u8>);
    fn decode_binding(decoder: &mut Decoder<'_>) -> Result<Self::Binding, &'static str>;
    fn derive_identity(
        producer: [u8; 32],
        slot: [u8; 32],
        attempt: BuildAttempt,
        binding: Self::Binding,
        handoff_bytes: &[u8],
    ) -> [u8; 32];
}

struct HandoffV1Schema;

impl HandoffSchema for HandoffV1Schema {
    type Slot = CompilerModuleHandoffSlotV1;
    type Binding = ();

    const PARENT_PREFIX: &'static str = PARENT_PREFIX;
    const SLOT_PREFIX: &'static str = SLOT_PREFIX;
    const RECORD_MAGIC: &'static [u8] = RECORD_MAGIC;
    const RECORD_VERSION: u16 = RECORD_VERSION;
    const PRODUCER_DOMAIN: &'static [u8] = PRODUCER_DOMAIN;
    const SLOT_DOMAIN: &'static [u8] = SLOT_DOMAIN;
    const NAMED_SLOT_DOMAIN: &'static [u8] = NAMED_SLOT_DOMAIN;
    const RECORD_DOMAIN: &'static [u8] = RECORD_DOMAIN;
    const RECORD_BYTES: usize = RECORD_BYTES;
    const VALIDATE_RECORD_DURING_RECOVERY: bool = false;
    const ALL_SLOTS: [Self::Slot; 3] = [
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

enum HandoffEngineError {
    Common(CompilerModuleHandoffErrorV1),
    WrongBinding,
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
    bytes: Arc<[u8]>,
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
        if !decoder.finished() || length == 0 || length > MAX_COMPILER_MODULE_HANDOFF_BYTES {
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
    validate_handoff_size(handoff_bytes.len())?;
    let output = PinnedOutput::open(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize(&output, producer, attempt)?;
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
        let committed_bytes = read_payload::<S>(&slot, &committed)?;
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
        bytes: consumed.bytes,
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
    let output = PinnedOutput::open_existing(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    authorize(&output, producer, attempt)?;
    let producer_id = producer_identity_for::<S>(producer);
    let slot_id = slot_identity_for::<S>(producer_id, attempt, handoff_slot);
    let parent = open_private_directory(
        &output.fd,
        &output.display_path,
        &format!("{}{}", S::PARENT_PREFIX, hex(&producer_id)),
    )?
    .ok_or(CompilerModuleHandoffErrorV1::NotPublished)?;
    cleanup_stale_slots::<S>(&parent, producer_id, attempt)?;
    let slot = open_private_directory(
        &parent.fd,
        &parent.path,
        &format!("{}{}", S::SLOT_PREFIX, hex(&slot_id)),
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
    let bytes = read_payload::<S>(&slot, &record)?;
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

fn cleanup_stale_slots<S: HandoffSchema>(
    parent: &PinnedDirectory,
    producer: [u8; 32],
    attempt: BuildAttempt,
) -> Result<(), HandoffEngineError> {
    let current = S::ALL_SLOTS.map(|slot| {
        format!(
            "{}{}",
            S::SLOT_PREFIX,
            hex(&slot_identity_for::<S>(producer, attempt, slot))
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
        if !name.starts_with(S::SLOT_PREFIX) {
            return Err(invalid_slot(
                &parent.path.join(name.as_ref()),
                "unexpected producer handoff entry",
            )
            .into());
        }
        if stale.len() == MAX_STALE_SLOTS {
            return Err(invalid_slot(&parent.path, "too many stale handoff slots").into());
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
        )
        .into());
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

fn recover_slot<S: HandoffSchema>(slot: &PinnedDirectory) -> Result<(), HandoffEngineError> {
    let names = slot_entries(slot)?;
    for name in &names {
        if !matches!(name.as_str(), PAYLOAD_ENTRY | READY_ENTRY | CONSUMED_ENTRY)
            && !name.starts_with(TEMP_PREFIX)
        {
            return Err(invalid_slot(&slot.path.join(name), "unexpected slot entry").into());
        }
    }
    if names.iter().any(|name| name == READY_ENTRY)
        && names.iter().any(|name| name == CONSUMED_ENTRY)
    {
        return Err(invalid_slot(&slot.path, "ready and consumed records coexist").into());
    }
    let committed_entry = names.iter().find_map(|name| match name.as_str() {
        READY_ENTRY => Some(READY_ENTRY),
        CONSUMED_ENTRY => Some(CONSUMED_ENTRY),
        _ => None,
    });
    let residue = names
        .into_iter()
        .filter(|name| {
            name.starts_with(TEMP_PREFIX) || (committed_entry.is_none() && name == PAYLOAD_ENTRY)
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
        if name.starts_with(TEMP_PREFIX) || name == PAYLOAD_ENTRY {
            unlinkat(&slot.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        }
    }
    if committed_entry.is_none() {
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

fn read_payload<S: HandoffSchema>(
    slot: &PinnedDirectory,
    record: &HandoffRecord<S>,
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

    struct HandoffV2Schema;

    impl HandoffSchema for HandoffV2Schema {
        type Slot = CompilerModuleHandoffSlotV2;
        type Binding = fe2o3_build_authority::CompilerClosureV2;

        const PARENT_PREFIX: &'static str = PARENT_PREFIX_V2;
        const SLOT_PREFIX: &'static str = SLOT_PREFIX_V2;
        const RECORD_MAGIC: &'static [u8] = RECORD_MAGIC_V2;
        const RECORD_VERSION: u16 = RECORD_VERSION_V2;
        const PRODUCER_DOMAIN: &'static [u8] = PRODUCER_DOMAIN_V2;
        const SLOT_DOMAIN: &'static [u8] = SLOT_DOMAIN_V2;
        const NAMED_SLOT_DOMAIN: &'static [u8] = NAMED_SLOT_DOMAIN_V2;
        const RECORD_DOMAIN: &'static [u8] = RECORD_DOMAIN_V2;
        const RECORD_BYTES: usize = RECORD_BYTES_V2;
        const VALIDATE_RECORD_DURING_RECOVERY: bool = true;
        const ALL_SLOTS: [Self::Slot; 3] = [
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
            bytes: consumed.bytes,
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
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{BuildInvocation, BuildSession, begin_build_attempt};
        use std::os::unix::fs::PermissionsExt;
        use std::sync::{Arc, Barrier};
        use std::thread;

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
