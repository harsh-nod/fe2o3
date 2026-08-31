//! Single-writer durable state and transition engine for the fe2o3 external anchor.
//!
//! The service owns one private directory, one Ed25519 signing key, and one monotonic
//! `(sequence, hash-chain head)` pair. An advance is signed as `Proposed` only after the new
//! state file and its atomic directory rename have both been synced. Recovery challenges never
//! mutate state. Exact retries of an already committed advance are idempotent.
//!
//! The descriptor-only entrypoint admits the exact locked process profile, sealed deployment and
//! key capabilities, existing durable root, and connected peer before entering this engine. Root
//! provisioning and distinct-UID supervisor handoff remain separate deployment responsibilities.

use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::OwnedFd;

use ed25519_dalek::{Signer, SigningKey};
use fe2o3_external_anchor_protocol::{
    ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorChallengeV1, AnchorPositionV1, AnchorProtocolErrorV1,
    ChallengeKindV1, HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
};
use rustix::fs::{
    AtFlags, FileType, FlockOperation, Mode, OFlags, flock, fstat, fsync, openat, renameat,
    unlinkat,
};
use rustix::process::geteuid;
use sha2::{Digest, Sha256};

#[allow(unsafe_code)]
mod entrypoint;
#[allow(unsafe_code)]
mod service;

pub use entrypoint::{
    EXTERNAL_ANCHOR_SERVICE_LIFECYCLE_FD_V1, EXTERNAL_ANCHOR_SERVICE_PEER_FD_V1,
    EXTERNAL_ANCHOR_SERVICE_ROOT_FD_V1, ExternalAnchorEntrypointErrorV1,
    ExternalAnchorExecutableErrorV1, run_inherited_external_anchor_service_v1,
};
pub use service::{
    ExternalAnchorDaemonErrorV1, ExternalAnchorServiceReportV1, serve_connected_peer_v1,
};

#[cfg(test)]
static ENTRYPOINT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const STATE_MAGIC: [u8; 8] = *b"F2ARST1\0";
const STATE_VERSION_V1: u16 = 1;
const STATE_PREFIX_BYTES: usize = 88;
const STATE_SEQUENCE_OFFSET: usize = 16;
const STATE_HEAD_OFFSET: usize = 24;
const STATE_KEY_IDENTITY_OFFSET: usize = 56;
const STATE_CHECKSUM_OFFSET: usize = STATE_PREFIX_BYTES;
const STATE_FILE: &str = "anchor-state-v1";
const NEXT_STATE_FILE: &str = ".anchor-state-v1.next";
const STATE_CHECKSUM_DOMAIN: &[u8] = b"FE2O3/EXTERNAL-MONOTONIC-ANCHOR/DURABLE-STATE/V1\0";

/// Exact canonical byte length of the durable V1 anchor state.
pub const EXTERNAL_ANCHOR_STATE_BYTES_V1: usize = 120;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DurableAnchorStateV1 {
    sequence: u64,
    head: HashChainHeadV1,
}

impl DurableAnchorStateV1 {
    const fn genesis() -> Self {
        Self {
            sequence: 0,
            head: HashChainHeadV1::from_bytes([0_u8; 32]),
        }
    }

    fn encode(self, key: &PinnedAnchorKeyV1) -> [u8; EXTERNAL_ANCHOR_STATE_BYTES_V1] {
        let mut bytes = [0_u8; EXTERNAL_ANCHOR_STATE_BYTES_V1];
        bytes[..8].copy_from_slice(&STATE_MAGIC);
        bytes[8..10].copy_from_slice(&STATE_VERSION_V1.to_le_bytes());
        bytes[STATE_SEQUENCE_OFFSET..STATE_SEQUENCE_OFFSET + 8]
            .copy_from_slice(&self.sequence.to_le_bytes());
        bytes[STATE_HEAD_OFFSET..STATE_HEAD_OFFSET + 32].copy_from_slice(&self.head.to_bytes());
        bytes[STATE_KEY_IDENTITY_OFFSET..STATE_KEY_IDENTITY_OFFSET + 32]
            .copy_from_slice(&key.identity().to_bytes());
        let checksum = state_checksum(&bytes[..STATE_PREFIX_BYTES]);
        bytes[STATE_CHECKSUM_OFFSET..].copy_from_slice(&checksum);
        bytes
    }

    fn decode(bytes: &[u8], key: &PinnedAnchorKeyV1) -> Result<Self, ExternalAnchorServiceErrorV1> {
        if bytes.len() != EXTERNAL_ANCHOR_STATE_BYTES_V1 {
            return Err(ExternalAnchorServiceErrorV1::InvalidStateLength {
                actual: bytes.len(),
            });
        }
        if bytes[..8] != STATE_MAGIC {
            return Err(ExternalAnchorServiceErrorV1::InvalidStateMagic);
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != STATE_VERSION_V1 {
            return Err(ExternalAnchorServiceErrorV1::UnsupportedStateVersion { actual: version });
        }
        if bytes[10..STATE_SEQUENCE_OFFSET]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err(ExternalAnchorServiceErrorV1::NonzeroStateReserved);
        }
        if bytes[STATE_KEY_IDENTITY_OFFSET..STATE_KEY_IDENTITY_OFFSET + 32]
            != key.identity().to_bytes()
        {
            return Err(ExternalAnchorServiceErrorV1::StateKeyIdentityMismatch);
        }
        let expected = state_checksum(&bytes[..STATE_PREFIX_BYTES]);
        if bytes[STATE_CHECKSUM_OFFSET..] != expected {
            return Err(ExternalAnchorServiceErrorV1::StateChecksumMismatch);
        }
        Ok(Self {
            sequence: u64::from_le_bytes(
                bytes[STATE_SEQUENCE_OFFSET..STATE_SEQUENCE_OFFSET + 8]
                    .try_into()
                    .expect("fixed state sequence is in bounds"),
            ),
            head: HashChainHeadV1::from_bytes(
                bytes[STATE_HEAD_OFFSET..STATE_HEAD_OFFSET + 32]
                    .try_into()
                    .expect("fixed state head is in bounds"),
            ),
        })
    }
}

/// Durable, single-writer external-anchor state machine.
///
/// Construction takes ownership of the exact directory descriptor and retains an exclusive
/// advisory lock for the lifetime of the service. The directory must be owned by the effective
/// service UID and have mode `0700`.
pub struct DurableExternalAnchorV1 {
    root: OwnedFd,
    signing_key: SigningKey,
    pinned_key: PinnedAnchorKeyV1,
    state: DurableAnchorStateV1,
    poisoned: bool,
}

/// Whether atomic durable-state admission opened existing state or created genesis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DurableExternalAnchorOpenDispositionV1 {
    /// A canonical state file already existed and was strictly admitted.
    Existing,
    /// No state file existed and canonical genesis was durably created.
    Initialized,
}

impl fmt::Debug for DurableExternalAnchorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableExternalAnchorV1")
            .field("sequence", &self.state.sequence)
            .field("head", &self.state.head)
            .field("key_identity", &self.pinned_key.identity())
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl DurableExternalAnchorV1 {
    /// Creates the canonical genesis state in an empty, private service directory.
    pub fn initialize(
        root: OwnedFd,
        signing_key: SigningKey,
    ) -> Result<Self, ExternalAnchorServiceErrorV1> {
        let pinned_key = pinned_key(&signing_key)?;
        admit_and_lock_root(&root)?;
        remove_leftover_next(&root)?;
        let state = DurableAnchorStateV1::genesis();
        create_initial_state(&root, &state.encode(&pinned_key))?;
        Ok(Self {
            root,
            signing_key,
            pinned_key,
            state,
            poisoned: false,
        })
    }

    /// Opens and strictly validates an existing canonical state file.
    pub fn open(
        root: OwnedFd,
        signing_key: SigningKey,
    ) -> Result<Self, ExternalAnchorServiceErrorV1> {
        let pinned_key = pinned_key(&signing_key)?;
        admit_and_lock_root(&root)?;
        remove_leftover_next(&root)?;
        let state = read_state(&root, &pinned_key)?;
        Ok(Self {
            root,
            signing_key,
            pinned_key,
            state,
            poisoned: false,
        })
    }

    /// Atomically opens existing state or creates genesis only when the state file is absent.
    ///
    /// Root admission, exclusive locking, abandoned-next cleanup, the exact absence check, and
    /// genesis creation occur under one retained directory lock. Malformed, inaccessible,
    /// key-substituted, or otherwise invalid existing state is never reset.
    pub fn open_or_initialize(
        root: OwnedFd,
        signing_key: SigningKey,
    ) -> Result<(Self, DurableExternalAnchorOpenDispositionV1), ExternalAnchorServiceErrorV1> {
        let pinned_key = pinned_key(&signing_key)?;
        admit_and_lock_root(&root)?;
        remove_leftover_next(&root)?;
        let (state, disposition) = match read_state(&root, &pinned_key) {
            Ok(state) => (state, DurableExternalAnchorOpenDispositionV1::Existing),
            Err(error) if error.is_missing_state_file() => {
                let state = DurableAnchorStateV1::genesis();
                create_initial_state(&root, &state.encode(&pinned_key))?;
                (state, DurableExternalAnchorOpenDispositionV1::Initialized)
            }
            Err(error) => return Err(error),
        };
        Ok((
            Self {
                root,
                signing_key,
                pinned_key,
                state,
                poisoned: false,
            },
            disposition,
        ))
    }

    pub const fn sequence(&self) -> u64 {
        self.state.sequence
    }

    pub const fn head(&self) -> HashChainHeadV1 {
        self.state.head
    }

    pub fn verifying_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Applies one exact challenge and returns a signed observation.
    ///
    /// An `Advance` from the current prior position durably commits the proposed state before
    /// signing it. An exact retry at the proposed position returns the same semantic observation
    /// without another write. A `Recover` only observes the exact prior or proposed position.
    pub fn exchange(
        &mut self,
        challenge_bytes: &[u8],
    ) -> Result<[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1], ExternalAnchorServiceErrorV1> {
        self.exchange_with_persistence_hooks(challenge_bytes, &mut NoopPersistenceHooksV1)
    }

    fn exchange_with_persistence_hooks<H: PersistenceHooksV1>(
        &mut self,
        challenge_bytes: &[u8],
        hooks: &mut H,
    ) -> Result<[u8; ANCHOR_OBSERVATION_WIRE_LEN_V1], ExternalAnchorServiceErrorV1> {
        if self.poisoned {
            return Err(ExternalAnchorServiceErrorV1::Poisoned);
        }
        let challenge = AnchorChallengeV1::decode(challenge_bytes)?;
        if challenge.anchor_key_identity() != self.pinned_key.identity() {
            return Err(ExternalAnchorServiceErrorV1::ChallengeKeyIdentityMismatch);
        }

        let at_prior = self.state.sequence.checked_add(1) == Some(challenge.expected_sequence())
            && self.state.head == challenge.prior_head();
        let at_proposed = self.state.sequence == challenge.expected_sequence()
            && self.state.head == challenge.proposed_head();
        if !at_prior && !at_proposed {
            return Err(ExternalAnchorServiceErrorV1::ChallengeStateMismatch);
        }

        let position = match (challenge.kind(), at_prior, at_proposed) {
            (ChallengeKindV1::Advance, true, false) => {
                let next = DurableAnchorStateV1 {
                    sequence: challenge.expected_sequence(),
                    head: challenge.proposed_head(),
                };
                if let Err(error) =
                    replace_state_with_hooks(&self.root, &next.encode(&self.pinned_key), hooks)
                {
                    self.poisoned = true;
                    return Err(error);
                }
                self.state = next;
                AnchorPositionV1::Proposed
            }
            (ChallengeKindV1::Advance | ChallengeKindV1::Recover, false, true) => {
                AnchorPositionV1::Proposed
            }
            (ChallengeKindV1::Recover, true, false) => AnchorPositionV1::Prior,
            _ => return Err(ExternalAnchorServiceErrorV1::ChallengeStateMismatch),
        };

        let unsigned = UnsignedAnchorObservationV1::from_challenge(&challenge, position);
        let signature = self.signing_key.sign(&unsigned.signing_bytes()).to_bytes();
        Ok(unsigned.attach_signature(signature))
    }
}

fn pinned_key(signing_key: &SigningKey) -> Result<PinnedAnchorKeyV1, ExternalAnchorServiceErrorV1> {
    PinnedAnchorKeyV1::from_bytes(signing_key.verifying_key().to_bytes()).map_err(Into::into)
}

fn admit_and_lock_root(root: &OwnedFd) -> Result<(), ExternalAnchorServiceErrorV1> {
    let stat = fstat(root).map_err(|source| io_error("inspect anchor root", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return Err(ExternalAnchorServiceErrorV1::RootNotDirectory);
    }
    let expected_uid = geteuid().as_raw();
    if stat.st_uid != expected_uid {
        return Err(ExternalAnchorServiceErrorV1::RootOwnerMismatch {
            expected: expected_uid,
            actual: stat.st_uid,
        });
    }
    if stat.st_mode & 0o7777 != 0o700 {
        return Err(ExternalAnchorServiceErrorV1::RootModeMismatch {
            actual: stat.st_mode & 0o7777,
        });
    }
    flock(root, FlockOperation::NonBlockingLockExclusive).map_err(|source| {
        if source == rustix::io::Errno::WOULDBLOCK {
            ExternalAnchorServiceErrorV1::StoreBusy
        } else {
            io_error("lock anchor root", source)
        }
    })
}

fn remove_leftover_next(root: &OwnedFd) -> Result<(), ExternalAnchorServiceErrorV1> {
    match unlinkat(root, NEXT_STATE_FILE, AtFlags::empty()) {
        Ok(()) => fsync(root).map_err(|source| io_error("sync anchor root cleanup", source)),
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(source) => Err(io_error("remove leftover anchor state", source)),
    }
}

fn create_initial_state(
    root: &OwnedFd,
    bytes: &[u8; EXTERNAL_ANCHOR_STATE_BYTES_V1],
) -> Result<(), ExternalAnchorServiceErrorV1> {
    let fd = openat(
        root,
        STATE_FILE,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|source| io_error("create initial anchor state", source))?;
    write_and_sync(fd, bytes, "write initial anchor state")?;
    fsync(root).map_err(|source| io_error("sync initial anchor state directory", source))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PersistenceBoundaryV1 {
    BeforeCleanup,
    AfterCleanup,
    BeforeCreate,
    AfterCreate,
    BeforeWrite,
    AfterWrite,
    BeforeFileSync,
    AfterFileSync,
    BeforeRename,
    AfterRename,
    BeforeDirectorySync,
    AfterDirectorySync,
}

impl PersistenceBoundaryV1 {
    #[cfg(test)]
    const ALL: [Self; 12] = [
        Self::BeforeCleanup,
        Self::AfterCleanup,
        Self::BeforeCreate,
        Self::AfterCreate,
        Self::BeforeWrite,
        Self::AfterWrite,
        Self::BeforeFileSync,
        Self::AfterFileSync,
        Self::BeforeRename,
        Self::AfterRename,
        Self::BeforeDirectorySync,
        Self::AfterDirectorySync,
    ];

    const fn operation(self) -> &'static str {
        match self {
            Self::BeforeCleanup => "before cleaning next anchor state",
            Self::AfterCleanup => "after cleaning next anchor state",
            Self::BeforeCreate => "before creating next anchor state",
            Self::AfterCreate => "after creating next anchor state",
            Self::BeforeWrite => "before writing next anchor state",
            Self::AfterWrite => "after writing next anchor state",
            Self::BeforeFileSync => "before syncing next anchor state",
            Self::AfterFileSync => "after syncing next anchor state",
            Self::BeforeRename => "before publishing next anchor state",
            Self::AfterRename => "after publishing next anchor state",
            Self::BeforeDirectorySync => "before syncing published anchor state",
            Self::AfterDirectorySync => "after syncing published anchor state",
        }
    }
}

trait PersistenceHooksV1 {
    fn checkpoint(&mut self, boundary: PersistenceBoundaryV1) -> io::Result<()>;
}

struct NoopPersistenceHooksV1;

impl PersistenceHooksV1 for NoopPersistenceHooksV1 {
    fn checkpoint(&mut self, _boundary: PersistenceBoundaryV1) -> io::Result<()> {
        Ok(())
    }
}

fn replace_state_with_hooks<H: PersistenceHooksV1>(
    root: &OwnedFd,
    bytes: &[u8; EXTERNAL_ANCHOR_STATE_BYTES_V1],
    hooks: &mut H,
) -> Result<(), ExternalAnchorServiceErrorV1> {
    checkpoint(hooks, PersistenceBoundaryV1::BeforeCleanup)?;
    remove_leftover_next(root)?;
    checkpoint(hooks, PersistenceBoundaryV1::AfterCleanup)?;
    checkpoint(hooks, PersistenceBoundaryV1::BeforeCreate)?;
    let fd = openat(
        root,
        NEXT_STATE_FILE,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(|source| io_error("create next anchor state", source))?;
    checkpoint(hooks, PersistenceBoundaryV1::AfterCreate)?;
    let mut file = File::from(fd);
    checkpoint(hooks, PersistenceBoundaryV1::BeforeWrite)?;
    file.write_all(bytes)
        .map_err(|source| ExternalAnchorServiceErrorV1::Io {
            operation: "write next anchor state",
            source,
        })?;
    checkpoint(hooks, PersistenceBoundaryV1::AfterWrite)?;
    checkpoint(hooks, PersistenceBoundaryV1::BeforeFileSync)?;
    file.sync_all()
        .map_err(|source| ExternalAnchorServiceErrorV1::Io {
            operation: "sync next anchor state",
            source,
        })?;
    checkpoint(hooks, PersistenceBoundaryV1::AfterFileSync)?;
    drop(file);
    checkpoint(hooks, PersistenceBoundaryV1::BeforeRename)?;
    renameat(root, NEXT_STATE_FILE, root, STATE_FILE)
        .map_err(|source| io_error("publish next anchor state", source))?;
    checkpoint(hooks, PersistenceBoundaryV1::AfterRename)?;
    checkpoint(hooks, PersistenceBoundaryV1::BeforeDirectorySync)?;
    fsync(root).map_err(|source| io_error("sync published anchor state", source))?;
    checkpoint(hooks, PersistenceBoundaryV1::AfterDirectorySync)
}

fn checkpoint<H: PersistenceHooksV1>(
    hooks: &mut H,
    boundary: PersistenceBoundaryV1,
) -> Result<(), ExternalAnchorServiceErrorV1> {
    hooks
        .checkpoint(boundary)
        .map_err(|source| ExternalAnchorServiceErrorV1::Io {
            operation: boundary.operation(),
            source,
        })
}

fn write_and_sync(
    fd: OwnedFd,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), ExternalAnchorServiceErrorV1> {
    let mut file = File::from(fd);
    file.write_all(bytes)
        .map_err(|source| ExternalAnchorServiceErrorV1::Io { operation, source })?;
    file.sync_all()
        .map_err(|source| ExternalAnchorServiceErrorV1::Io { operation, source })
}

fn read_state(
    root: &OwnedFd,
    key: &PinnedAnchorKeyV1,
) -> Result<DurableAnchorStateV1, ExternalAnchorServiceErrorV1> {
    let fd = openat(
        root,
        STATE_FILE,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|source| io_error("open anchor state", source))?;
    let stat = fstat(&fd).map_err(|source| io_error("inspect anchor state", source))?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_nlink != 1
        || stat.st_uid != geteuid().as_raw()
        || stat.st_mode & 0o7777 != 0o600
    {
        return Err(ExternalAnchorServiceErrorV1::InvalidStateFileMetadata);
    }
    let mut file = File::from(fd);
    let mut bytes = [0_u8; EXTERNAL_ANCHOR_STATE_BYTES_V1];
    file.read_exact(&mut bytes)
        .map_err(|source| ExternalAnchorServiceErrorV1::Io {
            operation: "read anchor state",
            source,
        })?;
    let mut trailing = [0_u8; 1];
    if file
        .read(&mut trailing)
        .map_err(|source| ExternalAnchorServiceErrorV1::Io {
            operation: "check anchor state length",
            source,
        })?
        != 0
    {
        return Err(ExternalAnchorServiceErrorV1::InvalidStateLength {
            actual: EXTERNAL_ANCHOR_STATE_BYTES_V1 + 1,
        });
    }
    DurableAnchorStateV1::decode(&bytes, key)
}

fn state_checksum(prefix: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(STATE_CHECKSUM_DOMAIN);
    hasher.update(prefix);
    hasher.finalize().into()
}

fn io_error(operation: &'static str, source: rustix::io::Errno) -> ExternalAnchorServiceErrorV1 {
    ExternalAnchorServiceErrorV1::Io {
        operation,
        source: io::Error::from(source),
    }
}

#[derive(Debug)]
pub enum ExternalAnchorServiceErrorV1 {
    Protocol(AnchorProtocolErrorV1),
    RootNotDirectory,
    RootOwnerMismatch {
        expected: u32,
        actual: u32,
    },
    RootModeMismatch {
        actual: u32,
    },
    StoreBusy,
    InvalidStateFileMetadata,
    InvalidStateLength {
        actual: usize,
    },
    InvalidStateMagic,
    UnsupportedStateVersion {
        actual: u16,
    },
    NonzeroStateReserved,
    StateKeyIdentityMismatch,
    StateChecksumMismatch,
    ChallengeKeyIdentityMismatch,
    ChallengeStateMismatch,
    Poisoned,
    Io {
        operation: &'static str,
        source: io::Error,
    },
}

impl ExternalAnchorServiceErrorV1 {
    fn is_missing_state_file(&self) -> bool {
        matches!(
            self,
            Self::Io { operation: "open anchor state", source }
                if source.raw_os_error() == Some(libc::ENOENT)
        )
    }
}

impl fmt::Display for ExternalAnchorServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => {
                write!(formatter, "external-anchor protocol rejected: {error}")
            }
            Self::RootNotDirectory => {
                formatter.write_str("external-anchor root is not a directory")
            }
            Self::RootOwnerMismatch { expected, actual } => write!(
                formatter,
                "external-anchor root UID mismatch: expected {expected}, got {actual}"
            ),
            Self::RootModeMismatch { actual } => write!(
                formatter,
                "external-anchor root mode must be 0700, got {actual:04o}"
            ),
            Self::StoreBusy => formatter.write_str("external-anchor store already has a writer"),
            Self::InvalidStateFileMetadata => formatter
                .write_str("external-anchor state must be a private, single-link regular file"),
            Self::InvalidStateLength { actual } => write!(
                formatter,
                "external-anchor state length must be {EXTERNAL_ANCHOR_STATE_BYTES_V1}, got {actual}"
            ),
            Self::InvalidStateMagic => formatter.write_str("invalid external-anchor state magic"),
            Self::UnsupportedStateVersion { actual } => {
                write!(
                    formatter,
                    "unsupported external-anchor state version {actual}"
                )
            }
            Self::NonzeroStateReserved => {
                formatter.write_str("external-anchor state reserved bytes must be zero")
            }
            Self::StateKeyIdentityMismatch => {
                formatter.write_str("external-anchor state belongs to another signing key")
            }
            Self::StateChecksumMismatch => {
                formatter.write_str("external-anchor state checksum mismatch")
            }
            Self::ChallengeKeyIdentityMismatch => {
                formatter.write_str("challenge names another external-anchor key")
            }
            Self::ChallengeStateMismatch => formatter.write_str(
                "challenge names neither the current nor immediately proposed anchor state",
            ),
            Self::Poisoned => formatter
                .write_str("external-anchor persistence is uncertain; the service must restart"),
            Self::Io { operation, source } => write!(formatter, "{operation}: {source}"),
        }
    }
}

impl Error for ExternalAnchorServiceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<AnchorProtocolErrorV1> for ExternalAnchorServiceErrorV1 {
    fn from(error: AnchorProtocolErrorV1) -> Self {
        Self::Protocol(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File, OpenOptions};
    use std::io;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    use ed25519_dalek::SigningKey;
    use fe2o3_external_anchor_protocol::{
        AnchorDecisionV1, AnchorPositionV1, AnchoredStateV1, CallerNonceV1, HashChainHeadV1,
        PinnedAnchorKeyV1, TransactionDigestV1,
    };
    use tempfile::TempDir;

    use super::{
        DurableAnchorStateV1, DurableExternalAnchorOpenDispositionV1, DurableExternalAnchorV1,
        EXTERNAL_ANCHOR_STATE_BYTES_V1, ExternalAnchorServiceErrorV1, NEXT_STATE_FILE,
        PersistenceBoundaryV1, PersistenceHooksV1, STATE_CHECKSUM_OFFSET, STATE_FILE,
    };

    struct CrashAtPersistenceBoundaryV1 {
        target: PersistenceBoundaryV1,
        fired: bool,
    }

    impl CrashAtPersistenceBoundaryV1 {
        const fn new(target: PersistenceBoundaryV1) -> Self {
            Self {
                target,
                fired: false,
            }
        }
    }

    impl PersistenceHooksV1 for CrashAtPersistenceBoundaryV1 {
        fn checkpoint(&mut self, boundary: PersistenceBoundaryV1) -> io::Result<()> {
            if !self.fired && boundary == self.target {
                self.fired = true;
                return Err(io::Error::other("injected external-anchor process crash"));
            }
            Ok(())
        }
    }

    fn root() -> (TempDir, File) {
        let directory = tempfile::tempdir().unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let root = File::open(directory.path()).unwrap();
        (directory, root)
    }

    fn keys(seed: u8) -> (SigningKey, PinnedAnchorKeyV1) {
        let signing = SigningKey::from_bytes(&[seed; 32]);
        let pinned = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
        (signing, pinned)
    }

    fn prepared(
        state: AnchoredStateV1,
        transaction: u8,
        pinned: &PinnedAnchorKeyV1,
    ) -> fe2o3_external_anchor_protocol::PreparedAnchorAdvanceV1 {
        state
            .prepare(TransactionDigestV1::from_bytes([transaction; 32]), pinned)
            .unwrap()
    }

    #[test]
    fn initialize_advance_reopen_and_recover_round_trip() {
        let (_directory, root) = root();
        let (signing, pinned) = keys(7);
        let mut service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        assert_eq!(service.sequence(), 0);
        assert_eq!(service.head(), HashChainHeadV1::from_bytes([0; 32]));

        let pending = prepared(
            AnchoredStateV1::from_local_state(service.sequence(), service.head()),
            31,
            &pinned,
        )
        .begin_advance(CallerNonceV1::from_bytes([9; 32]), &pinned)
        .unwrap();
        let response = service.exchange(pending.challenge().as_bytes()).unwrap();
        let decision = pending.verify(&response).unwrap();
        let committed = match decision {
            AnchorDecisionV1::Commit(commit) => commit,
            AnchorDecisionV1::Abort(_) => panic!("advance unexpectedly aborted"),
        };
        assert_eq!(service.sequence(), 1);
        assert_eq!(service.head(), committed.head());
        drop(service);

        let root = File::open(_directory.path()).unwrap();
        let (signing, _) = keys(7);
        let mut reopened = DurableExternalAnchorV1::open(root.into(), signing).unwrap();
        assert_eq!(reopened.sequence(), 1);
        let recovery = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            31,
            &pinned,
        )
        .begin_recovery(CallerNonceV1::from_bytes([10; 32]), &pinned)
        .unwrap();
        let response = reopened.exchange(recovery.challenge().as_bytes()).unwrap();
        assert!(matches!(
            recovery.verify(&response).unwrap(),
            AnchorDecisionV1::Commit(_)
        ));
    }

    #[test]
    fn atomic_open_or_initialize_creates_once_and_never_resets_invalid_state() {
        let (directory, root) = root();
        let (signing, _) = keys(19);
        let (service, disposition) =
            DurableExternalAnchorV1::open_or_initialize(root.into(), signing).unwrap();
        assert_eq!(
            disposition,
            DurableExternalAnchorOpenDispositionV1::Initialized
        );
        assert_eq!(service.sequence(), 0);
        drop(service);

        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(19);
        let (service, disposition) =
            DurableExternalAnchorV1::open_or_initialize(root.into(), signing).unwrap();
        assert_eq!(
            disposition,
            DurableExternalAnchorOpenDispositionV1::Existing
        );
        drop(service);

        let state_path = directory.path().join(STATE_FILE);
        let mut bytes = fs::read(&state_path).unwrap();
        bytes[STATE_CHECKSUM_OFFSET] ^= 1;
        fs::write(&state_path, bytes).unwrap();
        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(19);
        assert!(matches!(
            DurableExternalAnchorV1::open_or_initialize(root.into(), signing),
            Err(ExternalAnchorServiceErrorV1::StateChecksumMismatch)
        ));
    }

    #[test]
    fn recovery_at_prior_observes_without_advancing() {
        let (_directory, root) = root();
        let (signing, pinned) = keys(8);
        let mut service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        let pending = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            44,
            &pinned,
        )
        .begin_recovery(CallerNonceV1::from_bytes([11; 32]), &pinned)
        .unwrap();
        let response = service.exchange(pending.challenge().as_bytes()).unwrap();
        assert!(matches!(
            pending.verify(&response).unwrap(),
            AnchorDecisionV1::Abort(_)
        ));
        assert_eq!(service.sequence(), 0);
    }

    #[test]
    fn exact_advance_retry_is_idempotent() {
        let (_directory, root) = root();
        let (signing, pinned) = keys(9);
        let mut service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        let first_prepared = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            45,
            &pinned,
        );
        let first = first_prepared
            .begin_advance(CallerNonceV1::from_bytes([12; 32]), &pinned)
            .unwrap();
        service.exchange(first.challenge().as_bytes()).unwrap();
        let retry = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            45,
            &pinned,
        )
        .begin_advance(CallerNonceV1::from_bytes([13; 32]), &pinned)
        .unwrap();
        let response = service.exchange(retry.challenge().as_bytes()).unwrap();
        assert!(matches!(
            retry.verify(&response).unwrap(),
            AnchorDecisionV1::Commit(_)
        ));
        assert_eq!(service.sequence(), 1);
    }

    #[test]
    fn every_persistence_boundary_restarts_at_prior_or_exact_proposed_state() {
        for boundary in PersistenceBoundaryV1::ALL {
            let (directory, root) = root();
            let (signing, pinned) = keys(21);
            let mut service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
            let prepared = prepared(
                AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
                61,
                &pinned,
            );
            let proposed_head = prepared.proposed_head();
            let pending = prepared
                .begin_advance(CallerNonceV1::from_bytes([22; 32]), &pinned)
                .unwrap();
            let challenge = pending.challenge().as_bytes().to_vec();
            let mut fault = CrashAtPersistenceBoundaryV1::new(boundary);

            assert!(matches!(
                service.exchange_with_persistence_hooks(&challenge, &mut fault),
                Err(ExternalAnchorServiceErrorV1::Io { .. })
            ));
            assert!(fault.fired, "fault did not fire at {boundary:?}");
            assert!(matches!(
                service.exchange(&challenge),
                Err(ExternalAnchorServiceErrorV1::Poisoned)
            ));
            drop(service);

            let root = File::open(directory.path()).unwrap();
            let (signing, _) = keys(21);
            let mut restarted = DurableExternalAnchorV1::open(root.into(), signing).unwrap();
            let persisted_proposed = matches!(
                boundary,
                PersistenceBoundaryV1::AfterRename
                    | PersistenceBoundaryV1::BeforeDirectorySync
                    | PersistenceBoundaryV1::AfterDirectorySync
            );
            assert_eq!(
                restarted.sequence(),
                u64::from(persisted_proposed),
                "unexpected recovered sequence at {boundary:?}"
            );
            assert_eq!(
                restarted.head(),
                if persisted_proposed {
                    proposed_head
                } else {
                    HashChainHeadV1::from_bytes([0; 32])
                },
                "unexpected recovered head at {boundary:?}"
            );

            let response = restarted.exchange(&challenge).unwrap();
            assert!(matches!(
                pending.verify(&response).unwrap(),
                AnchorDecisionV1::Commit(_)
            ));
            assert_eq!(restarted.sequence(), 1);
            assert_eq!(restarted.head(), proposed_head);
            assert!(
                !directory.path().join(NEXT_STATE_FILE).exists(),
                "temporary state survived recovery at {boundary:?}"
            );

            drop(restarted);
            let root = File::open(directory.path()).unwrap();
            let (signing, _) = keys(21);
            let reopened = DurableExternalAnchorV1::open(root.into(), signing).unwrap();
            assert_eq!(reopened.sequence(), 1);
            assert_eq!(reopened.head(), proposed_head);
        }
    }

    #[test]
    fn lost_request_and_response_replay_converge_without_double_advance() {
        let (directory, root) = root();
        let (signing, pinned) = keys(23);
        let service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        let prepared = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            62,
            &pinned,
        );
        let proposed_head = prepared.proposed_head();
        let pending = prepared
            .begin_advance(CallerNonceV1::from_bytes([24; 32]), &pinned)
            .unwrap();
        let challenge = pending.challenge().as_bytes().to_vec();

        // A process loss before receiving the request leaves the durable prior state untouched.
        drop(service);
        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(23);
        let mut restarted = DurableExternalAnchorV1::open(root.into(), signing).unwrap();
        let lost_response = restarted.exchange(&challenge).unwrap();
        assert_eq!(restarted.sequence(), 1);
        drop(restarted);

        // A process loss after commit but before response delivery replays byte-identical output.
        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(23);
        let mut restarted = DurableExternalAnchorV1::open(root.into(), signing).unwrap();
        let replayed = restarted.exchange(&challenge).unwrap();
        assert_eq!(replayed, lost_response);
        assert!(matches!(
            pending.verify(&replayed).unwrap(),
            AnchorDecisionV1::Commit(_)
        ));
        assert_eq!(restarted.sequence(), 1);
        assert_eq!(restarted.head(), proposed_head);
    }

    #[test]
    fn stale_future_and_wrong_key_challenges_fail_closed() {
        let (_directory, root) = root();
        let (signing, pinned) = keys(10);
        let mut service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        let first_prepared = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            46,
            &pinned,
        );
        let first_head = first_prepared.proposed_head();
        let first = first_prepared
            .begin_advance(CallerNonceV1::from_bytes([14; 32]), &pinned)
            .unwrap();
        service.exchange(first.challenge().as_bytes()).unwrap();

        let stale = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            47,
            &pinned,
        )
        .begin_advance(CallerNonceV1::from_bytes([15; 32]), &pinned)
        .unwrap();
        assert!(matches!(
            service.exchange(stale.challenge().as_bytes()),
            Err(ExternalAnchorServiceErrorV1::ChallengeStateMismatch)
        ));

        let future = prepared(
            AnchoredStateV1::from_local_state(9, HashChainHeadV1::from_bytes([99; 32])),
            48,
            &pinned,
        )
        .begin_advance(CallerNonceV1::from_bytes([16; 32]), &pinned)
        .unwrap();
        assert!(matches!(
            service.exchange(future.challenge().as_bytes()),
            Err(ExternalAnchorServiceErrorV1::ChallengeStateMismatch)
        ));

        let (_, wrong) = keys(11);
        let wrong_key = prepared(AnchoredStateV1::from_local_state(1, first_head), 49, &wrong)
            .begin_advance(CallerNonceV1::from_bytes([17; 32]), &wrong)
            .unwrap();
        assert!(matches!(
            service.exchange(wrong_key.challenge().as_bytes()),
            Err(ExternalAnchorServiceErrorV1::ChallengeKeyIdentityMismatch)
        ));
        assert_eq!(service.sequence(), 1);
    }

    #[test]
    fn durable_state_rejects_every_single_byte_mutation() {
        let (_, pinned) = keys(12);
        let canonical = DurableAnchorStateV1 {
            sequence: 17,
            head: HashChainHeadV1::from_bytes([0x55; 32]),
        }
        .encode(&pinned);
        assert_eq!(canonical.len(), EXTERNAL_ANCHOR_STATE_BYTES_V1);
        for index in 0..canonical.len() {
            let mut mutated = canonical;
            mutated[index] ^= 1;
            assert!(
                DurableAnchorStateV1::decode(&mutated, &pinned).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
    }

    #[test]
    fn root_and_state_metadata_are_strict() {
        let (directory, root) = root();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let (signing, _) = keys(13);
        assert!(matches!(
            DurableExternalAnchorV1::initialize(root.into(), signing),
            Err(ExternalAnchorServiceErrorV1::RootModeMismatch { .. })
        ));

        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(13);
        let service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        drop(service);
        fs::set_permissions(
            directory.path().join(STATE_FILE),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(13);
        assert!(matches!(
            DurableExternalAnchorV1::open(root.into(), signing),
            Err(ExternalAnchorServiceErrorV1::InvalidStateFileMetadata)
        ));
    }

    #[test]
    fn second_writer_and_key_substitution_are_rejected() {
        let (directory, root) = root();
        let (signing, _) = keys(14);
        let service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();

        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(14);
        assert!(matches!(
            DurableExternalAnchorV1::open(root.into(), signing),
            Err(ExternalAnchorServiceErrorV1::StoreBusy)
        ));
        drop(service);

        let root = File::open(directory.path()).unwrap();
        let (wrong, _) = keys(15);
        assert!(matches!(
            DurableExternalAnchorV1::open(root.into(), wrong),
            Err(ExternalAnchorServiceErrorV1::StateKeyIdentityMismatch)
        ));
    }

    #[test]
    fn abandoned_next_state_is_removed_before_open() {
        let (directory, root) = root();
        let (signing, _) = keys(16);
        let service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        drop(service);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.path().join(NEXT_STATE_FILE))
            .unwrap();
        let root = File::open(directory.path()).unwrap();
        let (signing, _) = keys(16);
        DurableExternalAnchorV1::open(root.into(), signing).unwrap();
        assert!(!directory.path().join(NEXT_STATE_FILE).exists());
    }

    #[test]
    fn observations_name_only_prior_or_proposed_positions() {
        assert_eq!(AnchorPositionV1::Prior as u8, 1);
        assert_eq!(AnchorPositionV1::Proposed as u8, 2);
    }

    #[test]
    fn persistence_failure_permanently_poisons_the_process_instance() {
        let (directory, root) = root();
        let (signing, pinned) = keys(17);
        let mut service = DurableExternalAnchorV1::initialize(root.into(), signing).unwrap();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o500)).unwrap();
        let pending = prepared(
            AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32])),
            50,
            &pinned,
        )
        .begin_advance(CallerNonceV1::from_bytes([18; 32]), &pinned)
        .unwrap();
        assert!(matches!(
            service.exchange(pending.challenge().as_bytes()),
            Err(ExternalAnchorServiceErrorV1::Io { .. })
        ));
        assert!(matches!(
            service.exchange(pending.challenge().as_bytes()),
            Err(ExternalAnchorServiceErrorV1::Poisoned)
        ));
        assert_eq!(service.sequence(), 0);
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
}
