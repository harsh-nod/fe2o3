//! Atomic, inert publication of complete compiler-artifact generations.
//!
//! A generation is represented by immutable content-addressed blobs, one immutable canonical
//! manifest, and one scope record. The scope record is the only publication point: readers ignore
//! every blob and manifest that is not reachable through that record. Writers serialize through
//! the crate-wide persistent [`super::PinnedOutput`] lock and publish the record through a durable
//! redo name. Recovery replays only a redo whose predecessor is the currently committed manifest.
//! Admission pins the persistent lock inode. Lock-held maintenance scans every V1 scope, protects
//! canonical and legal redo/predecessor closures, and removes only metadata-validated stale temps
//! or unreachable immutable content. Configured quotas charge the greater of logical bytes and
//! `st_blocks * 512` for recognized V1 entries; unrelated protocol files are outside that byte
//! account but remain subject to the hard root-entry scan bound.
//!
//! The store assumes Linux `renameat2(RENAME_NOREPLACE)`, local-filesystem `fsync` and atomic
//! rename semantics, and a private directory used only by cooperating fe2o3 writers. It detects
//! symlinks, hardlinks, unsafe modes, owner changes, and observed inode substitution, but it is not
//! a security boundary against arbitrary same-UID code that ignores the lock. Returned leases are
//! inert snapshots and grant no compilation, verification, loading, or launch authority.

use super::{EmitError, LOCK_FILE, OutputLock, PinnedOutput, encode_hex};
use crate::{
    RetainedDurableArtifactBoundaryV1, RetainedDurableDirectoryErrorV1,
    RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1, RetainedDurableFaultTimingV1,
    RetainedDurableRecordBoundaryV1, RetainedDurableRecoveryBoundaryV1,
};
use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, fstat, fstatvfs, fsync, openat, statat, unlinkat,
};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Seek};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};

const MANIFEST_MAGIC: &[u8] = b"FE2O3-COMPILER-ARTIFACT-MANIFEST-V1\0";
const MANIFEST_VERSION: u16 = 1;
const MANIFEST_IDENTITY_DOMAIN: &[u8] =
    b"fe2o3.compiler-artifact-generation.manifest-identity.v1\0";
const SCOPE_RECORD_MAGIC: &[u8] = b"FE2O3-COMPILER-ARTIFACT-SCOPE-V1\0";
const SCOPE_RECORD_VERSION: u16 = 1;
const SCOPE_RECORD_CHECKSUM_DOMAIN: &[u8] =
    b"fe2o3.compiler-artifact-generation.scope-record-checksum.v1\0";
const SCOPE_NAME_DOMAIN: &[u8] = b"fe2o3.compiler-artifact-generation.scope-name.v1\0";
const BLOB_PREFIX: &str = ".fe2o3-compiler-generation-v1-blob-";
const MANIFEST_PREFIX: &str = ".fe2o3-compiler-generation-v1-manifest-";
const SCOPE_PREFIX: &str = ".fe2o3-compiler-generation-v1-scope-";
const CONTENT_SUFFIX: &str = ".bin";
const RECORD_SUFFIX: &str = ".record";
const REDO_SUFFIX: &str = ".redo";
const STAGED_SUFFIX: &str = ".staged";
const CONTENT_MODE: u32 = 0o400;
const MAX_DIRECTORY_ENTRIES: usize = 32_768;
const FINAL_ENTRY_HEADROOM: usize = 16;
const DEFAULT_MANAGED_ENTRIES: usize = 8_192;
const DEFAULT_MANAGED_BYTES: u64 = 512 * 1024 * 1024;
const HARD_MANAGED_BYTES: u64 = 1024 * 1024 * 1024;

/// Maximum canonical semantic-MIR payload in one generation.
pub const MAX_COMPILER_SEMANTIC_MIR_BYTES_V1: usize = 128 * 1024 * 1024;

/// Maximum canonical neutral-KIR payload in one generation.
pub const MAX_COMPILER_NEUTRAL_KIR_BYTES_V1: usize = 16 * 1024 * 1024;

/// Maximum canonical target-KIR payload in one generation.
pub const MAX_COMPILER_TARGET_KIR_BYTES_V1: usize = 16 * 1024 * 1024;

/// Maximum canonical lineage receipt in one generation.
pub const MAX_COMPILER_LINEAGE_BYTES_V1: usize = 4 * 1024 * 1024;

/// Maximum optional HSACO payload in one generation.
pub const MAX_COMPILER_HSACO_BYTES_V1: usize = 64 * 1024 * 1024;

/// Maximum aggregate payload bytes in one generation.
pub const MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1: usize = MAX_COMPILER_SEMANTIC_MIR_BYTES_V1
    + MAX_COMPILER_NEUTRAL_KIR_BYTES_V1
    + MAX_COMPILER_TARGET_KIR_BYTES_V1
    + MAX_COMPILER_LINEAGE_BYTES_V1
    + MAX_COMPILER_HSACO_BYTES_V1;

/// Maximum canonical manifest size.
pub const MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1: usize = 512;

/// Maximum canonical scope-record size.
pub const MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1: usize = 192;

/// Default persistent byte quota for the V1 generation namespace.
pub const DEFAULT_COMPILER_ARTIFACT_STORE_BYTES_V1: u64 = DEFAULT_MANAGED_BYTES;

/// Hard upper bound accepted for a configured V1 generation-store byte quota.
pub const HARD_MAX_COMPILER_ARTIFACT_STORE_BYTES_V1: u64 = HARD_MANAGED_BYTES;

/// Default persistent entry quota for the V1 generation namespace.
pub const DEFAULT_COMPILER_ARTIFACT_STORE_ENTRIES_V1: usize = DEFAULT_MANAGED_ENTRIES;

/// Hard upper bound on all directory entries inspected by the V1 store.
pub const HARD_MAX_COMPILER_ARTIFACT_STORE_ENTRIES_V1: usize = MAX_DIRECTORY_ENTRIES;

/// Stable publication scope selected by the caller.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct CompilerArtifactGenerationScopeV1([u8; 32]);

impl CompilerArtifactGenerationScopeV1 {
    /// Constructs a scope identity from exact canonical identity bytes.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact scope identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for CompilerArtifactGenerationScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerArtifactGenerationScopeV1")
            .field(&encode_hex(&self.0))
            .finish()
    }
}

/// Domain-separated identity of one canonical generation manifest.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct CompilerArtifactGenerationManifestIdentityV1([u8; 32]);

impl CompilerArtifactGenerationManifestIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for CompilerArtifactGenerationManifestIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerArtifactGenerationManifestIdentityV1")
            .field(&encode_hex(&self.0))
            .finish()
    }
}

/// Fixed role of one generation artifact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerArtifactRoleV1 {
    /// Exact caller-validated semantic-MIR bytes.
    SemanticMir = 1,
    /// Exact caller-validated target-neutral KIR bytes.
    NeutralKir = 2,
    /// Exact caller-validated target-specialized KIR bytes.
    TargetKir = 3,
    /// Exact caller-validated MIR-to-KIR lineage bytes.
    Lineage = 4,
    /// Optional finalized gfx942 code object.
    Hsaco = 5,
}

impl CompilerArtifactRoleV1 {
    const REQUIRED: [Self; 4] = [
        Self::SemanticMir,
        Self::NeutralKir,
        Self::TargetKir,
        Self::Lineage,
    ];

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::SemanticMir),
            2 => Some(Self::NeutralKir),
            3 => Some(Self::TargetKir),
            4 => Some(Self::Lineage),
            5 => Some(Self::Hsaco),
            _ => None,
        }
    }

    const fn maximum_bytes(self) -> usize {
        match self {
            Self::SemanticMir => MAX_COMPILER_SEMANTIC_MIR_BYTES_V1,
            Self::NeutralKir => MAX_COMPILER_NEUTRAL_KIR_BYTES_V1,
            Self::TargetKir => MAX_COMPILER_TARGET_KIR_BYTES_V1,
            Self::Lineage => MAX_COMPILER_LINEAGE_BYTES_V1,
            Self::Hsaco => MAX_COMPILER_HSACO_BYTES_V1,
        }
    }
}

/// Borrowed complete generation submitted for publication.
pub struct CompilerArtifactGenerationRequestV1<'a> {
    compiler_identity: [u8; 32],
    pipeline_identity: [u8; 32],
    target_identity: [u8; 32],
    semantic_mir: &'a [u8],
    neutral_kir: &'a [u8],
    target_kir: &'a [u8],
    lineage: &'a [u8],
    hsaco: Option<&'a [u8]>,
}

impl fmt::Debug for CompilerArtifactGenerationRequestV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerArtifactGenerationRequestV1")
            .field("compiler_identity", &encode_hex(&self.compiler_identity))
            .field("pipeline_identity", &encode_hex(&self.pipeline_identity))
            .field("target_identity", &encode_hex(&self.target_identity))
            .field("semantic_mir_bytes", &self.semantic_mir.len())
            .field("neutral_kir_bytes", &self.neutral_kir.len())
            .field("target_kir_bytes", &self.target_kir.len())
            .field("lineage_bytes", &self.lineage.len())
            .field("hsaco_bytes", &self.hsaco.map(<[u8]>::len))
            .finish()
    }
}

impl<'a> CompilerArtifactGenerationRequestV1<'a> {
    /// Constructs one fixed-role generation request.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        compiler_identity: [u8; 32],
        pipeline_identity: [u8; 32],
        target_identity: [u8; 32],
        semantic_mir: &'a [u8],
        neutral_kir: &'a [u8],
        target_kir: &'a [u8],
        lineage: &'a [u8],
        hsaco: Option<&'a [u8]>,
    ) -> Self {
        Self {
            compiler_identity,
            pipeline_identity,
            target_identity,
            semantic_mir,
            neutral_kir,
            target_kir,
            lineage,
            hsaco,
        }
    }

    /// Returns the compiler/toolchain closure identity.
    pub const fn compiler_identity(&self) -> [u8; 32] {
        self.compiler_identity
    }

    /// Returns the production-pipeline identity.
    pub const fn pipeline_identity(&self) -> [u8; 32] {
        self.pipeline_identity
    }

    /// Returns the exact target identity.
    pub const fn target_identity(&self) -> [u8; 32] {
        self.target_identity
    }

    /// Returns bytes for a fixed role, if present.
    pub const fn artifact(&self, role: CompilerArtifactRoleV1) -> Option<&'a [u8]> {
        match role {
            CompilerArtifactRoleV1::SemanticMir => Some(self.semantic_mir),
            CompilerArtifactRoleV1::NeutralKir => Some(self.neutral_kir),
            CompilerArtifactRoleV1::TargetKir => Some(self.target_kir),
            CompilerArtifactRoleV1::Lineage => Some(self.lineage),
            CompilerArtifactRoleV1::Hsaco => self.hsaco,
        }
    }
}

/// One fixed-role entry in a canonical generation manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerArtifactGenerationManifestEntryV1 {
    role: CompilerArtifactRoleV1,
    length: u64,
    sha256: [u8; 32],
}

impl CompilerArtifactGenerationManifestEntryV1 {
    /// Returns the fixed artifact role.
    pub const fn role(self) -> CompilerArtifactRoleV1 {
        self.role
    }

    /// Returns the exact canonical byte length.
    pub const fn length(self) -> u64 {
        self.length
    }

    /// Returns SHA-256 over the exact artifact bytes.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }
}

/// Canonical, inert description of one complete compiler-artifact generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerArtifactGenerationManifestV1 {
    scope: CompilerArtifactGenerationScopeV1,
    compiler_identity: [u8; 32],
    pipeline_identity: [u8; 32],
    target_identity: [u8; 32],
    entries: Vec<CompilerArtifactGenerationManifestEntryV1>,
    canonical_bytes: Vec<u8>,
    identity: CompilerArtifactGenerationManifestIdentityV1,
}

impl CompilerArtifactGenerationManifestV1 {
    /// Builds and validates the deterministic manifest for one request without publishing it.
    pub fn for_request(
        scope: CompilerArtifactGenerationScopeV1,
        request: &CompilerArtifactGenerationRequestV1<'_>,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(5)
            .map_err(|_| allocation_error(5))?;
        for role in CompilerArtifactRoleV1::REQUIRED {
            let bytes = request
                .artifact(role)
                .expect("fixed required generation role is present");
            entries.push(manifest_entry(role, bytes)?);
        }
        if let Some(bytes) = request.artifact(CompilerArtifactRoleV1::Hsaco) {
            entries.push(manifest_entry(CompilerArtifactRoleV1::Hsaco, bytes)?);
        }
        validate_entries(&entries)?;
        Self::from_parts(
            scope,
            request.compiler_identity,
            request.pipeline_identity,
            request.target_identity,
            entries,
        )
    }

    /// Decodes an inert manifest and requires byte-for-byte canonical encoding.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        if bytes.len() > MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1 {
            return Err(codec_error("manifest exceeds its canonical byte bound"));
        }
        let mut decoder = Decoder::new(bytes);
        if decoder.take(MANIFEST_MAGIC.len())? != MANIFEST_MAGIC {
            return Err(codec_error("bad manifest magic"));
        }
        if decoder.u16()? != MANIFEST_VERSION {
            return Err(codec_error("unsupported manifest version"));
        }
        let scope = CompilerArtifactGenerationScopeV1(decoder.array()?);
        let compiler_identity = decoder.array()?;
        let pipeline_identity = decoder.array()?;
        let target_identity = decoder.array()?;
        let count = usize::from(decoder.u8()?);
        if !(4..=5).contains(&count) {
            return Err(codec_error(
                "manifest must contain four or five fixed roles",
            ));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| allocation_error(count))?;
        for _ in 0..count {
            let role = CompilerArtifactRoleV1::from_tag(decoder.u8()?)
                .ok_or_else(|| codec_error("manifest contains an unknown role"))?;
            let length = decoder.u64()?;
            let sha256 = decoder.array()?;
            entries.push(CompilerArtifactGenerationManifestEntryV1 {
                role,
                length,
                sha256,
            });
        }
        if !decoder.finished() {
            return Err(codec_error("manifest contains trailing bytes"));
        }
        validate_entries(&entries)?;
        let manifest = Self::from_parts(
            scope,
            compiler_identity,
            pipeline_identity,
            target_identity,
            entries,
        )?;
        if manifest.canonical_bytes != bytes {
            return Err(codec_error("manifest encoding is not canonical"));
        }
        Ok(manifest)
    }

    fn from_parts(
        scope: CompilerArtifactGenerationScopeV1,
        compiler_identity: [u8; 32],
        pipeline_identity: [u8; 32],
        target_identity: [u8; 32],
        entries: Vec<CompilerArtifactGenerationManifestEntryV1>,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        validate_entries(&entries)?;
        let mut canonical_bytes = Vec::new();
        canonical_bytes
            .try_reserve_exact(MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1)
            .map_err(|_| allocation_error(MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1))?;
        canonical_bytes.extend_from_slice(MANIFEST_MAGIC);
        canonical_bytes.extend_from_slice(&MANIFEST_VERSION.to_le_bytes());
        canonical_bytes.extend_from_slice(&scope.0);
        canonical_bytes.extend_from_slice(&compiler_identity);
        canonical_bytes.extend_from_slice(&pipeline_identity);
        canonical_bytes.extend_from_slice(&target_identity);
        canonical_bytes.push(
            u8::try_from(entries.len()).map_err(|_| codec_error("manifest role count overflow"))?,
        );
        for entry in &entries {
            canonical_bytes.push(entry.role as u8);
            canonical_bytes.extend_from_slice(&entry.length.to_le_bytes());
            canonical_bytes.extend_from_slice(&entry.sha256);
        }
        if canonical_bytes.len() > MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1 {
            return Err(codec_error("manifest exceeds its canonical byte bound"));
        }
        let identity = manifest_identity(&canonical_bytes);
        Ok(Self {
            scope,
            compiler_identity,
            pipeline_identity,
            target_identity,
            entries,
            canonical_bytes,
            identity,
        })
    }

    /// Returns the exact publication scope.
    pub const fn scope(&self) -> CompilerArtifactGenerationScopeV1 {
        self.scope
    }

    /// Returns the compiler/toolchain closure identity.
    pub const fn compiler_identity(&self) -> [u8; 32] {
        self.compiler_identity
    }

    /// Returns the production-pipeline identity.
    pub const fn pipeline_identity(&self) -> [u8; 32] {
        self.pipeline_identity
    }

    /// Returns the exact target identity.
    pub const fn target_identity(&self) -> [u8; 32] {
        self.target_identity
    }

    /// Returns the fixed ordered artifact entries.
    pub fn entries(&self) -> &[CompilerArtifactGenerationManifestEntryV1] {
        &self.entries
    }

    /// Returns the canonical manifest bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the domain-separated canonical manifest identity.
    pub const fn identity(&self) -> CompilerArtifactGenerationManifestIdentityV1 {
        self.identity
    }
}

/// Object-publication operation exposed to deterministic fault tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerArtifactGenerationObjectV1 {
    /// A fixed-role artifact blob.
    Artifact(CompilerArtifactRoleV1),
    /// The canonical generation manifest.
    Manifest,
}

/// Durable object boundary exposed to deterministic fault tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerArtifactGenerationObjectBoundaryV1 {
    CreateTemp,
    WriteTemp,
    SyncTemp,
    RenameTempToStaged,
    SyncStagedName,
    SetFinalMode,
    SyncFinalMode,
    RenameStagedToFinal,
    SyncFinalName,
}

/// Scope-record boundary exposed to deterministic fault tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerArtifactGenerationRecordBoundaryV1 {
    CreateTemp,
    WriteTemp,
    SyncTemp,
    RenameTempToRedo,
    SyncRedoName,
    RenameRedoToCanonical,
    SyncCanonicalName,
}

/// Scope-record operation associated with a deterministic fault.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerArtifactGenerationRecordOperationV1 {
    /// Initial publication of a new scope record.
    Commit,
    /// Recovery promotion of one predecessor-bound redo.
    Recover,
}

/// Whether a deterministic fault occurs before or after one operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerArtifactGenerationFaultTimingV1 {
    Before,
    After,
}

/// Deterministic filesystem fault point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerArtifactGenerationFaultPointV1 {
    /// Content-addressed object publication boundary.
    Object {
        object: CompilerArtifactGenerationObjectV1,
        boundary: CompilerArtifactGenerationObjectBoundaryV1,
        timing: CompilerArtifactGenerationFaultTimingV1,
    },
    /// Scope-record commit or recovery boundary.
    ScopeRecord {
        operation: CompilerArtifactGenerationRecordOperationV1,
        boundary: CompilerArtifactGenerationRecordBoundaryV1,
        timing: CompilerArtifactGenerationFaultTimingV1,
    },
    /// Complete-generation open of one immutable object.
    Open {
        object: CompilerArtifactGenerationObjectV1,
    },
    /// Bounded root-directory scan before publication.
    DirectoryScan,
}

/// Persistent namespace quotas enforced under the shared writer lock.
///
/// Byte accounting charges the greater of logical and allocated bytes for every recognized V1
/// entry. Unrelated files are excluded from the byte quota, but every root entry is covered by the
/// hard directory-entry bound. A replacement publication may temporarily require both the active
/// and incoming generation to fit while the scope-record commit remains atomic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerArtifactGenerationQuotaV1 {
    maximum_bytes: u64,
    maximum_entries: usize,
}

impl CompilerArtifactGenerationQuotaV1 {
    /// Constructs explicit persistent byte and managed-entry quotas.
    pub fn new(
        maximum_bytes: u64,
        maximum_entries: usize,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        if maximum_bytes == 0
            || maximum_bytes > HARD_MAX_COMPILER_ARTIFACT_STORE_BYTES_V1
            || maximum_entries == 0
            || maximum_entries > HARD_MAX_COMPILER_ARTIFACT_STORE_ENTRIES_V1
        {
            return Err(CompilerArtifactGenerationErrorV1::InvalidQuota {
                maximum_bytes,
                maximum_entries,
            });
        }
        Ok(Self {
            maximum_bytes,
            maximum_entries,
        })
    }

    /// Returns the configured persistent-byte limit.
    pub const fn maximum_bytes(self) -> u64 {
        self.maximum_bytes
    }

    /// Returns the configured managed-entry limit.
    pub const fn maximum_entries(self) -> usize {
        self.maximum_entries
    }
}

impl Default for CompilerArtifactGenerationQuotaV1 {
    fn default() -> Self {
        Self {
            maximum_bytes: DEFAULT_COMPILER_ARTIFACT_STORE_BYTES_V1,
            maximum_entries: DEFAULT_COMPILER_ARTIFACT_STORE_ENTRIES_V1,
        }
    }
}

/// Result of one trusted, lock-serialized namespace reclamation pass.
#[derive(Debug)]
pub struct CompilerArtifactGenerationReclamationV1 {
    removed_entries: usize,
    removed_bytes: u64,
    retained_entries: usize,
    retained_bytes: u64,
}

impl CompilerArtifactGenerationReclamationV1 {
    /// Returns the number of unreachable or stale managed entries removed.
    pub const fn removed_entries(&self) -> usize {
        self.removed_entries
    }

    /// Returns charged persistent bytes removed by this pass.
    pub const fn removed_bytes(&self) -> u64 {
        self.removed_bytes
    }

    /// Returns the number of protected managed entries retained.
    pub const fn retained_entries(&self) -> usize {
        self.retained_entries
    }

    /// Returns charged persistent bytes retained after this pass.
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }
}

/// A deterministic pause at one filesystem boundary for concurrency and substitution tests.
#[derive(Debug)]
pub struct CompilerArtifactGenerationObservationV1 {
    point: CompilerArtifactGenerationFaultPointV1,
    state: Mutex<ObservationState>,
    changed: Condvar,
}

#[derive(Debug, Default)]
struct ObservationState {
    reached: bool,
    released: bool,
}

impl CompilerArtifactGenerationObservationV1 {
    /// Selects one operation boundary to pause.
    pub fn new(point: CompilerArtifactGenerationFaultPointV1) -> Self {
        Self {
            point,
            state: Mutex::new(ObservationState::default()),
            changed: Condvar::new(),
        }
    }

    /// Blocks until the selected operation boundary has been reached.
    pub fn wait_until_reached(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        while !state.reached {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    /// Releases the operation paused at the selected boundary.
    pub fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.released = true;
        self.changed.notify_all();
    }

    fn observe(&self, point: CompilerArtifactGenerationFaultPointV1) {
        if point != self.point {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.reached = true;
        self.changed.notify_all();
        while !state.released {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}

/// Options for deterministic durability tests.
#[derive(Clone, Debug, Default)]
pub struct CompilerArtifactGenerationOptionsV1 {
    faults: [Option<CompilerArtifactGenerationFaultPointV1>; 2],
    observation: Option<Arc<CompilerArtifactGenerationObservationV1>>,
}

impl CompilerArtifactGenerationOptionsV1 {
    /// Injects one crash-like deterministic filesystem failure.
    pub const fn inject_fault(point: CompilerArtifactGenerationFaultPointV1) -> Self {
        Self {
            faults: [Some(point), None],
            observation: None,
        }
    }

    /// Injects two ordered crash-like failures, primarily to exercise post-error recovery.
    pub const fn inject_fault_sequence(
        first: CompilerArtifactGenerationFaultPointV1,
        second: CompilerArtifactGenerationFaultPointV1,
    ) -> Self {
        Self {
            faults: [Some(first), Some(second)],
            observation: None,
        }
    }

    /// Pauses at one deterministic filesystem boundary until the observer is released.
    pub fn observe(observation: Arc<CompilerArtifactGenerationObservationV1>) -> Self {
        Self {
            faults: [None, None],
            observation: Some(observation),
        }
    }
}

/// Failure to validate, publish, recover, or open a compiler-artifact generation.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerArtifactGenerationErrorV1 {
    /// Descriptor-relative filesystem operation failed.
    Filesystem(EmitError),
    /// Retained private-directory operation failed.
    DurableDirectory(RetainedDurableDirectoryErrorV1),
    /// Input or decoded payload violates a role or aggregate bound.
    Bounds {
        role: Option<CompilerArtifactRoleV1>,
        actual: u64,
        maximum: u64,
    },
    /// Canonical manifest or scope-record bytes are malformed.
    Codec { reason: String },
    /// A committed object is absent or has unsafe metadata or content.
    UnsafeEntry { entry: String, reason: String },
    /// A redo is not one exact transition from the current scope record.
    ConflictingRedo { reason: String },
    /// The admitted root no longer has its original private service-owned identity.
    UnsafeRoot { reason: String },
    /// A bounded allocation failed before publication.
    AllocationFailed { requested: usize },
    /// Configured quotas are zero or exceed the implementation hard bounds.
    InvalidQuota {
        maximum_bytes: u64,
        maximum_entries: usize,
    },
    /// Live protected data plus the requested transition exceeds the persistent byte quota.
    StorageQuotaExceeded { actual: u64, maximum: u64 },
    /// Live protected data plus the requested transition exceeds the managed-entry quota.
    ManagedEntryLimitExceeded { actual: usize, maximum: usize },
    /// The root has too many entries to preserve protocol headroom.
    DirectoryEntryLimitExceeded { maximum: usize },
    /// A rollback-like cleanup failed before durable absence could be established.
    CommitStateIndeterminate { reason: String },
}

impl fmt::Display for CompilerArtifactGenerationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filesystem(error) => write!(formatter, "generation filesystem failure: {error}"),
            Self::DurableDirectory(error) => {
                write!(formatter, "generation durable-directory failure: {error}")
            }
            Self::Bounds {
                role,
                actual,
                maximum,
            } => match role {
                Some(role) => write!(
                    formatter,
                    "{role:?} payload is {actual} bytes, maximum is {maximum}"
                ),
                None => write!(
                    formatter,
                    "generation payloads total {actual} bytes, maximum is {maximum}"
                ),
            },
            Self::Codec { reason } => {
                write!(formatter, "invalid canonical generation data: {reason}")
            }
            Self::UnsafeEntry { entry, reason } => {
                write!(formatter, "unsafe generation entry {entry}: {reason}")
            }
            Self::ConflictingRedo { reason } => {
                write!(formatter, "conflicting generation redo: {reason}")
            }
            Self::UnsafeRoot { reason } => write!(formatter, "unsafe generation root: {reason}"),
            Self::AllocationFailed { requested } => {
                write!(formatter, "could not reserve {requested} bytes or entries")
            }
            Self::InvalidQuota {
                maximum_bytes,
                maximum_entries,
            } => write!(
                formatter,
                "invalid generation-store quota: {maximum_bytes} bytes and {maximum_entries} entries"
            ),
            Self::StorageQuotaExceeded { actual, maximum } => write!(
                formatter,
                "generation store requires {actual} persistent bytes, maximum is {maximum}"
            ),
            Self::ManagedEntryLimitExceeded { actual, maximum } => write!(
                formatter,
                "generation store requires {actual} managed entries, maximum is {maximum}"
            ),
            Self::DirectoryEntryLimitExceeded { maximum } => {
                write!(formatter, "generation root exceeds {maximum} entries")
            }
            Self::CommitStateIndeterminate { reason } => {
                write!(
                    formatter,
                    "generation commit state is indeterminate: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for CompilerArtifactGenerationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem(error) => Some(error),
            Self::DurableDirectory(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EmitError> for CompilerArtifactGenerationErrorV1 {
    fn from(error: EmitError) -> Self {
        Self::Filesystem(error)
    }
}

impl From<io::Error> for CompilerArtifactGenerationErrorV1 {
    fn from(error: io::Error) -> Self {
        Self::Filesystem(EmitError::Io(error))
    }
}

impl From<RetainedDurableDirectoryErrorV1> for CompilerArtifactGenerationErrorV1 {
    fn from(error: RetainedDurableDirectoryErrorV1) -> Self {
        Self::DurableDirectory(error)
    }
}

struct LeasedObject {
    role: Option<CompilerArtifactRoleV1>,
    bytes: Box<[u8]>,
}

impl fmt::Debug for LeasedObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LeasedObject")
            .field("role", &self.role)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// Move-only, fully validated snapshot of one complete committed generation.
///
/// The store validates exact storage bytes and generation relationships, but intentionally does
/// not grant semantic authority over the stored compiler formats. A lease owns only immutable
/// bytes and identities; all filesystem descriptors used during validation are closed before the
/// lease is returned, so superseded generations cannot retain deleted-open filesystem blocks.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerArtifactGenerationLeaseV1;
///
/// fn consume(_: CompilerArtifactGenerationLeaseV1) {}
///
/// fn cannot_copy(lease: CompilerArtifactGenerationLeaseV1) {
///     consume(lease);
///     consume(lease);
/// }
/// ```
#[derive(Debug)]
pub struct CompilerArtifactGenerationLeaseV1 {
    manifest: CompilerArtifactGenerationManifestV1,
    artifacts: Vec<LeasedObject>,
}

impl CompilerArtifactGenerationLeaseV1 {
    /// Returns the complete canonical manifest.
    pub const fn manifest(&self) -> &CompilerArtifactGenerationManifestV1 {
        &self.manifest
    }

    /// Returns the exact immutable bytes for one role, if the optional role is present.
    pub fn artifact(&self, role: CompilerArtifactRoleV1) -> Option<&[u8]> {
        self.artifacts
            .iter()
            .find(|artifact| artifact.role == Some(role))
            .map(|artifact| artifact.bytes.as_ref())
    }

    /// This inert snapshot grants no compiler-verification authority.
    pub const fn grants_verification_authority(&self) -> bool {
        false
    }

    /// This inert snapshot grants no code-object loading authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// This inert snapshot grants no kernel-launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Classified result of one publication attempt.
#[derive(Debug)]
pub enum CompilerArtifactGenerationPublishOutcomeV1 {
    /// The exact complete generation is committed and returned as a move-only lease.
    Committed(CompilerArtifactGenerationLeaseV1),
    /// Lock-held inspection proved the requested manifest is neither canonical nor legally pending.
    NotCommitted(CompilerArtifactGenerationErrorV1),
    /// The requested manifest may be committed or recoverably pending, but no lease could be returned.
    CommitIndeterminate {
        expected_manifest: CompilerArtifactGenerationManifestIdentityV1,
        error: CompilerArtifactGenerationErrorV1,
    },
}

impl CompilerArtifactGenerationPublishOutcomeV1 {
    /// Moves a committed lease out of this outcome.
    pub fn into_committed(self) -> Option<CompilerArtifactGenerationLeaseV1> {
        match self {
            Self::Committed(lease) => Some(lease),
            Self::NotCommitted(_) | Self::CommitIndeterminate { .. } => None,
        }
    }
}

enum ExpectedGenerationState {
    Absent,
    Recoverable,
    Uncertain,
}

#[derive(Debug)]
struct Names {
    record: String,
    redo: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PersistentLockIdentity {
    device: u64,
    inode: u64,
}

impl PersistentLockIdentity {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
        }
    }
}

impl Names {
    fn new(scope: CompilerArtifactGenerationScopeV1) -> Self {
        let mut digest = Sha256::new();
        digest.update(SCOPE_NAME_DOMAIN);
        digest.update(scope.0);
        let identity: [u8; 32] = digest.finalize().into();
        let record = format!("{SCOPE_PREFIX}{}{RECORD_SUFFIX}", encode_hex(&identity));
        let redo = format!("{record}{REDO_SUFFIX}");
        Self { record, redo }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ManagedEntryKind {
    Blob,
    Manifest,
    ScopeRecord,
    ScopeRedo,
    Staged,
    Temporary,
}

#[derive(Clone, Debug)]
struct ManagedEntry {
    name: String,
    kind: ManagedEntryKind,
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    links: u64,
    length: u64,
    charged_bytes: u64,
}

impl ManagedEntry {
    fn from_stat(
        name: String,
        kind: ManagedEntryKind,
        stat: &rustix::fs::Stat,
        service_uid: u32,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        validate_managed_inventory_stat(&name, kind, stat, service_uid)?;
        let length = u64::try_from(stat.st_size)
            .map_err(|_| unsafe_entry(&name, "managed entry has a negative length"))?;
        let allocated = u64::try_from(stat.st_blocks)
            .ok()
            .and_then(|blocks| blocks.checked_mul(512))
            .ok_or_else(|| unsafe_entry(&name, "managed allocated-byte count overflowed"))?;
        Ok(Self {
            name,
            kind,
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            uid: stat.st_uid,
            links: stat.st_nlink,
            length,
            charged_bytes: length.max(allocated),
        })
    }

    fn matches_stat(&self, stat: &rustix::fs::Stat) -> bool {
        self.device == stat.st_dev
            && self.inode == stat.st_ino
            && self.mode == stat.st_mode
            && self.uid == stat.st_uid
            && self.links == stat.st_nlink
            && i64::try_from(self.length).ok() == Some(stat.st_size)
    }
}

struct StoreInventory {
    root_entries: usize,
    managed_entries: Vec<ManagedEntry>,
    managed_bytes: u64,
}

#[derive(Default)]
struct ScopeRecords {
    canonical: Option<ScopeRecordV1>,
    redo: Option<ScopeRecordV1>,
}

#[derive(Clone, Copy)]
enum CandidateMode {
    Published,
    Staged,
}

/// Descriptor-pinned compiler-artifact generation store for one scope.
pub struct CompilerArtifactGenerationStoreV1 {
    output: PinnedOutput,
    durable: RetainedDurableDirectoryV1,
    scope: CompilerArtifactGenerationScopeV1,
    names: Names,
    service_uid: u32,
    lock_identity: PersistentLockIdentity,
    _lock_pin: OwnedFd,
    quota: CompilerArtifactGenerationQuotaV1,
}

impl fmt::Debug for CompilerArtifactGenerationStoreV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerArtifactGenerationStoreV1")
            .field("scope", &self.scope)
            .field("root", &self.output.display_path)
            .field("quota", &self.quota)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl CompilerArtifactGenerationStoreV1 {
    /// Admits an existing owner-matched `0700` root and pins its exact directory descriptor.
    pub fn open(
        root: &Path,
        scope: CompilerArtifactGenerationScopeV1,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        Self::open_with_quota(root, scope, CompilerArtifactGenerationQuotaV1::default())
    }

    /// Admits a store with explicit persistent-byte and managed-entry quotas.
    pub fn open_with_quota(
        root: &Path,
        scope: CompilerArtifactGenerationScopeV1,
        quota: CompilerArtifactGenerationQuotaV1,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        let quota =
            CompilerArtifactGenerationQuotaV1::new(quota.maximum_bytes(), quota.maximum_entries())?;
        let output = PinnedOutput::open_existing(root)?;
        let service_uid = rustix::process::geteuid().as_raw();
        let root_stat = fstat(&output.fd).map_err(io::Error::from)?;
        validate_store_root_stat(&root_stat, service_uid)?;
        let descriptor = rustix::io::fcntl_dupfd_cloexec(&output.fd, 0).map_err(io::Error::from)?;
        let durable = RetainedDurableDirectoryV1::admit_service_owned(descriptor)?;
        if !durable.matches_descriptor(&output.fd)? {
            return Err(CompilerArtifactGenerationErrorV1::UnsafeRoot {
                reason: "retained and lock descriptors name different roots".to_owned(),
            });
        }
        output.verify_path_identity()?;
        let initial_lock = output.lock()?;
        let lock_stat = fstat(
            initial_lock
                .fd
                .as_ref()
                .ok_or_else(|| codec_error("persistent lock descriptor is absent"))?,
        )
        .map_err(io::Error::from)?;
        let lock_identity = PersistentLockIdentity::from_stat(&lock_stat);
        let lock_pin = openat(
            &output.fd,
            LOCK_FILE,
            OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        let lock_pin_stat = fstat(&lock_pin).map_err(io::Error::from)?;
        if PersistentLockIdentity::from_stat(&lock_pin_stat) != lock_identity {
            return Err(CompilerArtifactGenerationErrorV1::UnsafeRoot {
                reason: "persistent lock inode changed while it was being pinned".to_owned(),
            });
        }
        drop(initial_lock);
        let store = Self {
            output,
            durable,
            scope,
            names: Names::new(scope),
            service_uid,
            lock_identity,
            _lock_pin: lock_pin,
            quota,
        };
        {
            let _lock = store.lock_checked()?;
            let mut hooks = StoreHooks::new([None, None], None);
            if let Err(error) = store.recover_scope_locked(&mut hooks) {
                if matches!(
                    error,
                    CompilerArtifactGenerationErrorV1::StorageQuotaExceeded { .. }
                        | CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded { .. }
                ) {
                    store.recover_scope_locked(&mut hooks)?;
                } else {
                    return Err(error);
                }
            }
            store.reclaim_locked()?;
        }
        Ok(store)
    }

    /// Returns the fixed publication scope.
    pub const fn scope(&self) -> CompilerArtifactGenerationScopeV1 {
        self.scope
    }

    /// Publishes one complete generation with production no-fault behavior.
    pub fn publish_generation_v1(
        &self,
        request: &CompilerArtifactGenerationRequestV1<'_>,
    ) -> CompilerArtifactGenerationPublishOutcomeV1 {
        self.publish_generation_v1_with_options(
            request,
            CompilerArtifactGenerationOptionsV1::default(),
        )
    }

    /// Publishes one complete generation with deterministic test fault injection.
    pub fn publish_generation_v1_with_options(
        &self,
        request: &CompilerArtifactGenerationRequestV1<'_>,
        options: CompilerArtifactGenerationOptionsV1,
    ) -> CompilerArtifactGenerationPublishOutcomeV1 {
        let mut hooks = StoreHooks::new(options.faults, options.observation);
        let manifest = match CompilerArtifactGenerationManifestV1::for_request(self.scope, request)
        {
            Ok(manifest) => manifest,
            Err(error) => {
                let expected_manifest =
                    match request_manifest_identity_without_allocation(self.scope, request) {
                        Ok(identity) => identity,
                        Err(_) => {
                            return CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(error);
                        }
                    };
                let lock = match self.lock_checked() {
                    Ok(lock) => lock,
                    Err(lock_error) => {
                        return CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate {
                            expected_manifest,
                            error: lock_error,
                        };
                    }
                };
                let outcome =
                    self.classify_publish_error_locked(expected_manifest, error, &mut hooks);
                drop(lock);
                return outcome;
            }
        };
        let expected_manifest = manifest.identity();
        let lock = match self.lock_checked() {
            Ok(lock) => lock,
            Err(error) => {
                return CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate {
                    expected_manifest,
                    error,
                };
            }
        };
        let outcome = match self.publish_locked(request, manifest, &mut hooks) {
            Ok(lease) => CompilerArtifactGenerationPublishOutcomeV1::Committed(lease),
            Err(error) => self.classify_publish_error_locked(expected_manifest, error, &mut hooks),
        };
        drop(lock);
        outcome
    }

    /// Recovers a legal redo, then validates and opens the complete committed generation.
    pub fn recover_generation_v1(
        &self,
    ) -> Result<Option<CompilerArtifactGenerationLeaseV1>, CompilerArtifactGenerationErrorV1> {
        self.recover_generation_v1_with_options(CompilerArtifactGenerationOptionsV1::default())
    }

    /// Fault-injectable form of [`Self::recover_generation_v1`].
    pub fn recover_generation_v1_with_options(
        &self,
        options: CompilerArtifactGenerationOptionsV1,
    ) -> Result<Option<CompilerArtifactGenerationLeaseV1>, CompilerArtifactGenerationErrorV1> {
        let _lock = self.lock_checked()?;
        self.ensure_root()?;
        let mut hooks = StoreHooks::new(options.faults, options.observation);
        let lease = self.recover_scope_locked(&mut hooks)?;
        self.reclaim_locked()?;
        self.ensure_root()?;
        Ok(lease)
    }

    /// Recovers and establishes durability for the current generation before returning a lease.
    ///
    /// Merely visible canonical state is never sufficient to mint the committed lease type. This
    /// operation promotes a legal redo or syncs and rereads canonical-only state before returning.
    /// It is therefore intentionally mutating.
    pub fn open_generation_v1(
        &self,
    ) -> Result<Option<CompilerArtifactGenerationLeaseV1>, CompilerArtifactGenerationErrorV1> {
        let _lock = self.lock_checked()?;
        self.ensure_root()?;
        let mut hooks = StoreHooks::new([None, None], None);
        let lease = self.recover_scope_locked(&mut hooks)?;
        self.reclaim_locked()?;
        self.ensure_root()?;
        Ok(lease)
    }

    fn publish_locked(
        &self,
        request: &CompilerArtifactGenerationRequestV1<'_>,
        manifest: CompilerArtifactGenerationManifestV1,
        hooks: &mut StoreHooks,
    ) -> Result<CompilerArtifactGenerationLeaseV1, CompilerArtifactGenerationErrorV1> {
        self.ensure_root()?;
        let current = self.recover_scope_locked(hooks)?;
        if current
            .as_ref()
            .is_some_and(|lease| lease.manifest.identity() == manifest.identity())
        {
            return current.ok_or_else(|| codec_error("current generation disappeared"));
        }
        self.reclaim_locked()?;
        self.ensure_publication_quota(&manifest, hooks)?;

        for entry in manifest.entries() {
            let bytes = request.artifact(entry.role).ok_or_else(|| {
                codec_error("typed request omitted a role retained by its manifest")
            })?;
            self.publish_object(
                &blob_name(entry.sha256),
                bytes,
                CompilerArtifactGenerationObjectV1::Artifact(entry.role),
                hooks,
            )?;
        }
        self.publish_object(
            &manifest_name(manifest.identity()),
            manifest.canonical_bytes(),
            CompilerArtifactGenerationObjectV1::Manifest,
            hooks,
        )?;

        let previous = current.map(|lease| lease.manifest.identity());
        let record = ScopeRecordV1::new(
            self.scope,
            previous,
            manifest.identity(),
            manifest.canonical_bytes().len(),
        )?;
        hooks.record_operation = CompilerArtifactGenerationRecordOperationV1::Commit;
        let canonical_before = self.read_scope_record_bytes(&self.names.record)?;
        self.durable.stage_record_redo(
            &self.names.redo,
            &record.canonical_bytes,
            MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1,
            hooks,
        )?;
        if let Err(quota_error) = self.ensure_actual_quota_locked() {
            if let Err(error) = self
                .discard_validated_redo_locked(canonical_before.as_deref(), &record.canonical_bytes)
            {
                return Err(
                    CompilerArtifactGenerationErrorV1::CommitStateIndeterminate {
                        reason: format!("could not durably discard a quota-rejected redo: {error}"),
                    },
                );
            }
            self.reclaim_locked()?;
            return Err(quota_error);
        }
        self.durable.promote_validated_redo(
            &self.names.record,
            &self.names.redo,
            canonical_before.as_deref(),
            &record.canonical_bytes,
            MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1,
            hooks,
        )?;
        let lease = self
            .open_generation_locked(Some(manifest.identity()), hooks)?
            .ok_or_else(|| unsafe_entry(&self.names.record, "committed scope record is absent"))?;
        self.reclaim_locked()?;
        self.ensure_root()?;
        Ok(lease)
    }

    fn ensure_actual_quota_locked(&self) -> Result<(), CompilerArtifactGenerationErrorV1> {
        let inventory = self.inventory_locked(None)?;
        if inventory.managed_entries.len() > self.quota.maximum_entries {
            return Err(
                CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded {
                    actual: inventory.managed_entries.len(),
                    maximum: self.quota.maximum_entries,
                },
            );
        }
        if inventory.managed_bytes > self.quota.maximum_bytes {
            return Err(CompilerArtifactGenerationErrorV1::StorageQuotaExceeded {
                actual: inventory.managed_bytes,
                maximum: self.quota.maximum_bytes,
            });
        }
        Ok(())
    }

    fn discard_validated_redo_locked(
        &self,
        expected_canonical: Option<&[u8]>,
        expected_redo: &[u8],
    ) -> Result<(), CompilerArtifactGenerationErrorV1> {
        if self.read_scope_record_bytes(&self.names.record)?.as_deref() != expected_canonical
            || self.read_scope_record_bytes(&self.names.redo)?.as_deref() != Some(expected_redo)
        {
            return Err(unsafe_entry(
                &self.names.redo,
                "scope records changed before rejected redo cleanup",
            ));
        }
        unlinkat(&self.output.fd, &self.names.redo, AtFlags::empty()).map_err(io::Error::from)?;
        fsync(&self.output.fd).map_err(io::Error::from)?;
        if self.read_scope_record_bytes(&self.names.record)?.as_deref() != expected_canonical
            || self.read_scope_record_bytes(&self.names.redo)?.is_some()
        {
            return Err(unsafe_entry(
                &self.names.redo,
                "rejected redo cleanup did not preserve exact canonical-only state",
            ));
        }
        Ok(())
    }

    /// Reclaims stale V1 temporary entries and unreachable immutable V1 content.
    ///
    /// The pass is fail-closed and serialized by the persistent writer lock. It retains all valid
    /// canonical generations and legal redo/predecessor closures across every V1 scope in this
    /// root. Existing leases remain usable from owned validated bytes when a superseded content
    /// name is unlinked; leases retain no filesystem descriptors.
    pub fn reclaim_store_v1(
        &self,
    ) -> Result<CompilerArtifactGenerationReclamationV1, CompilerArtifactGenerationErrorV1> {
        let _lock = self.lock_checked()?;
        self.ensure_root()?;
        let result = self.reclaim_locked()?;
        self.ensure_root()?;
        Ok(result)
    }

    fn lock_checked(&self) -> Result<OutputLock, CompilerArtifactGenerationErrorV1> {
        self.ensure_root()?;
        let lock = self.output.lock()?;
        let descriptor = lock
            .fd
            .as_ref()
            .ok_or_else(|| codec_error("persistent lock descriptor is absent"))?;
        let identity =
            PersistentLockIdentity::from_stat(&fstat(descriptor).map_err(io::Error::from)?);
        if identity != self.lock_identity {
            return Err(CompilerArtifactGenerationErrorV1::UnsafeRoot {
                reason: "persistent lock inode changed after store admission".to_owned(),
            });
        }
        self.ensure_root()?;
        Ok(lock)
    }

    fn classify_expected_locked(
        &self,
        expected: CompilerArtifactGenerationManifestIdentityV1,
        hooks: &mut StoreHooks,
    ) -> ExpectedGenerationState {
        if self.ensure_root().is_err() {
            return ExpectedGenerationState::Uncertain;
        }
        let canonical_bytes = match self.read_scope_record_bytes(&self.names.record) {
            Ok(bytes) => bytes,
            Err(_) => return ExpectedGenerationState::Uncertain,
        };
        let redo_bytes = match self.read_scope_record_bytes(&self.names.redo) {
            Ok(bytes) => bytes,
            Err(_) => return ExpectedGenerationState::Uncertain,
        };
        let canonical = match canonical_bytes.as_deref() {
            Some(bytes) => match ScopeRecordV1::decode_canonical(bytes, self.scope) {
                Ok(record) => Some(record),
                Err(_) => return ExpectedGenerationState::Uncertain,
            },
            None => None,
        };
        let redo = match redo_bytes.as_deref() {
            Some(bytes) => match ScopeRecordV1::decode_canonical(bytes, self.scope) {
                Ok(record) => Some(record),
                Err(_) => return ExpectedGenerationState::Uncertain,
            },
            None => None,
        };
        if let Some(canonical) = &canonical
            && canonical.manifest == expected
        {
            let redo_is_legal = match &redo {
                Some(redo) if redo.manifest == canonical.manifest => {
                    redo.canonical_bytes == canonical.canonical_bytes
                }
                Some(redo) => redo.previous == Some(canonical.manifest),
                None => true,
            };
            if !redo_is_legal {
                return ExpectedGenerationState::Uncertain;
            }
            return match self.open_generation_for_record(canonical, hooks) {
                Ok(_) => ExpectedGenerationState::Recoverable,
                Err(_) => ExpectedGenerationState::Uncertain,
            };
        }
        let Some(redo) = redo else {
            return ExpectedGenerationState::Absent;
        };
        let legal = match &canonical {
            Some(current) if current.manifest == redo.manifest => {
                current.canonical_bytes == redo.canonical_bytes
            }
            Some(current) => redo.previous == Some(current.manifest),
            None => redo.previous.is_none(),
        };
        if redo.manifest != expected || !legal {
            return ExpectedGenerationState::Absent;
        }
        match self.open_generation_for_record(&redo, hooks) {
            Ok(_) => ExpectedGenerationState::Recoverable,
            Err(_) => ExpectedGenerationState::Uncertain,
        }
    }

    fn classify_publish_error_locked(
        &self,
        expected_manifest: CompilerArtifactGenerationManifestIdentityV1,
        error: CompilerArtifactGenerationErrorV1,
        hooks: &mut StoreHooks,
    ) -> CompilerArtifactGenerationPublishOutcomeV1 {
        if matches!(
            error,
            CompilerArtifactGenerationErrorV1::CommitStateIndeterminate { .. }
        ) {
            return CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate {
                expected_manifest,
                error,
            };
        }
        match self.classify_expected_locked(expected_manifest, hooks) {
            ExpectedGenerationState::Recoverable | ExpectedGenerationState::Uncertain => {
                CompilerArtifactGenerationPublishOutcomeV1::CommitIndeterminate {
                    expected_manifest,
                    error,
                }
            }
            ExpectedGenerationState::Absent => {
                CompilerArtifactGenerationPublishOutcomeV1::NotCommitted(error)
            }
        }
    }

    fn ensure_root(&self) -> Result<(), CompilerArtifactGenerationErrorV1> {
        let stat = fstat(&self.output.fd).map_err(io::Error::from)?;
        validate_store_root_stat(&stat, self.service_uid)?;
        if !self.durable.matches_descriptor(&self.output.fd)? {
            return Err(CompilerArtifactGenerationErrorV1::UnsafeRoot {
                reason: "root identity, owner, mode, or descriptor metadata changed".to_owned(),
            });
        }
        self.output.verify_path_identity()?;
        let lock_stat = statat(&self.output.fd, LOCK_FILE, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(io::Error::from)?;
        if FileType::from_raw_mode(lock_stat.st_mode) != FileType::RegularFile
            || lock_stat.st_uid != self.service_uid
            || lock_stat.st_mode & 0o7777 != 0o600
            || lock_stat.st_nlink != 1
            || PersistentLockIdentity::from_stat(&lock_stat) != self.lock_identity
        {
            return Err(CompilerArtifactGenerationErrorV1::UnsafeRoot {
                reason: "named persistent lock no longer matches the admitted private inode"
                    .to_owned(),
            });
        }
        Ok(())
    }

    fn read_scope_record_bytes(
        &self,
        entry: &str,
    ) -> Result<Option<Vec<u8>>, CompilerArtifactGenerationErrorV1> {
        let descriptor = match openat(
            &self.output.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
            Err(error) => return Err(io::Error::from(error).into()),
        };
        let before = fstat(&descriptor).map_err(io::Error::from)?;
        let expected_length = validate_scope_record_stat(&before, self.service_uid, entry)?;
        let named_before =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_same_content_file(&before, &named_before, expected_length, entry)?;

        let mut file = fs::File::from(descriptor);
        let bytes = read_exact_expected_length(&mut file, expected_length, entry)?;
        let after = fstat(&file).map_err(io::Error::from)?;
        let named_after =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_unchanged_content_file(&before, &after, expected_length, entry)?;
        require_same_content_file(&after, &named_after, expected_length, entry)?;
        Ok(Some(bytes))
    }

    fn inventory_locked(
        &self,
        mut hooks: Option<&mut StoreHooks>,
    ) -> Result<StoreInventory, CompilerArtifactGenerationErrorV1> {
        if let Some(hooks) = &mut hooks {
            hooks.hit_directory_scan()?;
        }
        // The scan hook is the last non-mutating publication boundary. Recheck the admitted lock
        // name after it so a deterministic replacement is rejected before object publication.
        self.ensure_root()?;
        let descriptor =
            rustix::io::fcntl_dupfd_cloexec(&self.output.fd, 0).map_err(io::Error::from)?;
        let mut directory = rustix::fs::Dir::read_from(&descriptor).map_err(io::Error::from)?;
        let mut root_entries = 0usize;
        let mut managed_entries = Vec::new();
        managed_entries
            .try_reserve(32)
            .map_err(|_| allocation_error(32))?;
        let mut managed_bytes = 0u64;
        for entry in &mut directory {
            let entry = entry.map_err(io::Error::from)?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            root_entries = root_entries.checked_add(1).ok_or(
                CompilerArtifactGenerationErrorV1::DirectoryEntryLimitExceeded {
                    maximum: MAX_DIRECTORY_ENTRIES,
                },
            )?;
            if root_entries > MAX_DIRECTORY_ENTRIES {
                return Err(
                    CompilerArtifactGenerationErrorV1::DirectoryEntryLimitExceeded {
                        maximum: MAX_DIRECTORY_ENTRIES,
                    },
                );
            }
            let kind = classify_managed_name(name)?;
            let Some(kind) = kind else {
                continue;
            };
            let name = std::str::from_utf8(name)
                .map_err(|_| unsafe_entry("<non-UTF8>", "managed name is not canonical ASCII"))?
                .to_owned();
            if managed_entries.len() == MAX_DIRECTORY_ENTRIES {
                return Err(
                    CompilerArtifactGenerationErrorV1::DirectoryEntryLimitExceeded {
                        maximum: MAX_DIRECTORY_ENTRIES,
                    },
                );
            }
            managed_entries
                .try_reserve(1)
                .map_err(|_| allocation_error(managed_entries.len().saturating_add(1)))?;
            let stat = statat(&self.output.fd, &name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(io::Error::from)?;
            let managed = ManagedEntry::from_stat(name, kind, &stat, self.service_uid)?;
            managed_bytes = managed_bytes
                .checked_add(managed.charged_bytes)
                .ok_or_else(|| unsafe_entry(&managed.name, "managed byte accounting overflowed"))?;
            managed_entries.push(managed);
        }
        Ok(StoreInventory {
            root_entries,
            managed_entries,
            managed_bytes,
        })
    }

    fn ensure_publication_quota(
        &self,
        manifest: &CompilerArtifactGenerationManifestV1,
        hooks: &mut StoreHooks,
    ) -> Result<(), CompilerArtifactGenerationErrorV1> {
        let inventory = self.inventory_locked(Some(hooks))?;
        let manifest_name = manifest_name(manifest.identity);
        let mut existing_names = HashSet::new();
        existing_names
            .try_reserve(inventory.managed_entries.len())
            .map_err(|_| allocation_error(inventory.managed_entries.len()))?;
        for entry in &inventory.managed_entries {
            existing_names.insert(entry.name.as_str());
        }

        let mut incoming_objects = HashMap::new();
        incoming_objects
            .try_reserve(manifest.entries.len().saturating_add(1))
            .map_err(|_| allocation_error(manifest.entries.len().saturating_add(1)))?;
        if !existing_names.contains(manifest_name.as_str()) {
            incoming_objects.insert(manifest_name, manifest.canonical_bytes.len());
        }
        for artifact in &manifest.entries {
            let name = blob_name(artifact.sha256);
            if existing_names.contains(name.as_str()) {
                continue;
            }
            let length = usize::try_from(artifact.length)
                .map_err(|_| codec_error("artifact length does not fit this host"))?;
            match incoming_objects.entry(name) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(length);
                }
                std::collections::hash_map::Entry::Occupied(entry) if *entry.get() != length => {
                    return Err(codec_error(
                        "one content address names incoming artifacts with different lengths",
                    ));
                }
                std::collections::hash_map::Entry::Occupied(_) => {}
            }
        }
        // One redo record is transiently present in addition to each unique immutable object.
        let incoming_files = incoming_objects
            .len()
            .checked_add(1)
            .ok_or_else(|| allocation_error(usize::MAX))?;
        let projected_entries = inventory
            .managed_entries
            .len()
            .checked_add(incoming_files)
            .ok_or(
                CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded {
                    actual: usize::MAX,
                    maximum: self.quota.maximum_entries,
                },
            )?;
        if projected_entries > self.quota.maximum_entries {
            return Err(
                CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded {
                    actual: projected_entries,
                    maximum: self.quota.maximum_entries,
                },
            );
        }
        let projected_root_entries = inventory.root_entries.saturating_add(incoming_files);
        if projected_root_entries > MAX_DIRECTORY_ENTRIES.saturating_sub(FINAL_ENTRY_HEADROOM) {
            return Err(
                CompilerArtifactGenerationErrorV1::DirectoryEntryLimitExceeded {
                    maximum: MAX_DIRECTORY_ENTRIES,
                },
            );
        }

        let filesystem = fstatvfs(&self.output.fd).map_err(io::Error::from)?;
        let allocation_unit = filesystem.f_frsize.max(filesystem.f_bsize).max(512);
        let mut incoming_bytes = estimated_file_charge(
            MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1,
            allocation_unit,
        )?;
        for length in incoming_objects.values().copied() {
            incoming_bytes = incoming_bytes
                .checked_add(estimated_file_charge(length, allocation_unit)?)
                .ok_or_else(|| codec_error("publication byte projection overflowed"))?;
        }
        let projected_bytes = inventory.managed_bytes.saturating_add(incoming_bytes);
        if projected_bytes > self.quota.maximum_bytes {
            return Err(CompilerArtifactGenerationErrorV1::StorageQuotaExceeded {
                actual: projected_bytes,
                maximum: self.quota.maximum_bytes,
            });
        }
        Ok(())
    }

    fn reclaim_locked(
        &self,
    ) -> Result<CompilerArtifactGenerationReclamationV1, CompilerArtifactGenerationErrorV1> {
        let inventory = self.inventory_locked(None)?;
        let mut by_name = HashMap::new();
        by_name
            .try_reserve(inventory.managed_entries.len())
            .map_err(|_| allocation_error(inventory.managed_entries.len()))?;
        for (index, entry) in inventory.managed_entries.iter().enumerate() {
            if by_name.insert(entry.name.clone(), index).is_some() {
                return Err(unsafe_entry(
                    &entry.name,
                    "duplicate managed directory name",
                ));
            }
        }

        let mut protected = HashSet::new();
        protected
            .try_reserve(inventory.managed_entries.len())
            .map_err(|_| allocation_error(inventory.managed_entries.len()))?;
        let mut scopes: HashMap<CompilerArtifactGenerationScopeV1, ScopeRecords> = HashMap::new();
        scopes
            .try_reserve(inventory.managed_entries.len().min(1024))
            .map_err(|_| allocation_error(inventory.managed_entries.len().min(1024)))?;
        for entry in &inventory.managed_entries {
            if !matches!(
                entry.kind,
                ManagedEntryKind::ScopeRecord | ManagedEntryKind::ScopeRedo
            ) {
                continue;
            }
            let bytes = self
                .read_scope_record_bytes(&entry.name)?
                .ok_or_else(|| unsafe_entry(&entry.name, "managed record disappeared"))?;
            let record = ScopeRecordV1::decode_canonical_any(&bytes)?;
            let names = Names::new(record.scope);
            let expected_name = match entry.kind {
                ManagedEntryKind::ScopeRecord => &names.record,
                ManagedEntryKind::ScopeRedo => &names.redo,
                _ => unreachable!(),
            };
            if &entry.name != expected_name {
                return Err(unsafe_entry(
                    &entry.name,
                    "scope record name does not match its canonical scope identity",
                ));
            }
            protected.insert(entry.name.clone());
            let slot = scopes.entry(record.scope).or_default();
            let destination = match entry.kind {
                ManagedEntryKind::ScopeRecord => &mut slot.canonical,
                ManagedEntryKind::ScopeRedo => &mut slot.redo,
                _ => unreachable!(),
            };
            if destination.replace(record).is_some() {
                return Err(unsafe_entry(&entry.name, "duplicate scope record role"));
            }
        }

        for records in scopes.values() {
            if let Some(redo) = &records.redo {
                let legal = match &records.canonical {
                    Some(current) if current.manifest == redo.manifest => {
                        current.canonical_bytes == redo.canonical_bytes
                    }
                    Some(current) => redo.previous == Some(current.manifest),
                    None => redo.previous.is_none(),
                };
                if !legal {
                    return Err(CompilerArtifactGenerationErrorV1::ConflictingRedo {
                        reason: "namespace maintenance found a non-successor redo".to_owned(),
                    });
                }
            }
            if let Some(canonical) = &records.canonical {
                self.protect_record_generation(
                    canonical,
                    &inventory.managed_entries,
                    &by_name,
                    &mut protected,
                )?;
            }
            if let Some(redo) = &records.redo {
                self.protect_record_generation(
                    redo,
                    &inventory.managed_entries,
                    &by_name,
                    &mut protected,
                )?;
            }
        }

        let mut removed_entries = 0usize;
        let mut removed_bytes = 0u64;
        for entry in &inventory.managed_entries {
            if protected.contains(&entry.name) {
                continue;
            }
            let current = statat(&self.output.fd, &entry.name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(io::Error::from)?;
            validate_managed_inventory_stat(&entry.name, entry.kind, &current, self.service_uid)?;
            if !entry.matches_stat(&current) {
                return Err(unsafe_entry(
                    &entry.name,
                    "managed entry changed during reclamation",
                ));
            }
            unlinkat(&self.output.fd, &entry.name, AtFlags::empty()).map_err(io::Error::from)?;
            removed_entries = removed_entries
                .checked_add(1)
                .ok_or_else(|| codec_error("removed-entry accounting overflowed"))?;
            removed_bytes = removed_bytes
                .checked_add(entry.charged_bytes)
                .ok_or_else(|| codec_error("removed-byte accounting overflowed"))?;
        }
        if removed_entries != 0 {
            fsync(&self.output.fd).map_err(io::Error::from)?;
        }
        let retained_entries = inventory
            .managed_entries
            .len()
            .checked_sub(removed_entries)
            .ok_or_else(|| codec_error("retained-entry accounting underflowed"))?;
        let retained_bytes = inventory
            .managed_bytes
            .checked_sub(removed_bytes)
            .ok_or_else(|| codec_error("retained-byte accounting underflowed"))?;
        if retained_entries > self.quota.maximum_entries {
            return Err(
                CompilerArtifactGenerationErrorV1::ManagedEntryLimitExceeded {
                    actual: retained_entries,
                    maximum: self.quota.maximum_entries,
                },
            );
        }
        if retained_bytes > self.quota.maximum_bytes {
            return Err(CompilerArtifactGenerationErrorV1::StorageQuotaExceeded {
                actual: retained_bytes,
                maximum: self.quota.maximum_bytes,
            });
        }
        Ok(CompilerArtifactGenerationReclamationV1 {
            removed_entries,
            removed_bytes,
            retained_entries,
            retained_bytes,
        })
    }

    fn protect_record_generation(
        &self,
        record: &ScopeRecordV1,
        inventory: &[ManagedEntry],
        by_name: &HashMap<String, usize>,
        protected: &mut HashSet<String>,
    ) -> Result<(), CompilerArtifactGenerationErrorV1> {
        let manifest_name = manifest_name(record.manifest);
        let manifest_entry = by_name
            .get(&manifest_name)
            .and_then(|index| inventory.get(*index))
            .ok_or_else(|| unsafe_entry(&manifest_name, "referenced manifest is absent"))?;
        if manifest_entry.kind != ManagedEntryKind::Manifest
            || usize::try_from(manifest_entry.length).ok() != Some(record.manifest_length)
        {
            return Err(unsafe_entry(
                &manifest_name,
                "referenced manifest inventory metadata is inconsistent",
            ));
        }
        let bytes = self
            .read_candidate_exact(
                &manifest_name,
                record.manifest_length,
                CandidateMode::Published,
            )?
            .ok_or_else(|| unsafe_entry(&manifest_name, "referenced manifest is absent"))?;
        if manifest_identity(&bytes) != record.manifest {
            return Err(unsafe_entry(
                &manifest_name,
                "referenced manifest does not match its content address",
            ));
        }
        let manifest = CompilerArtifactGenerationManifestV1::decode_canonical(&bytes)?;
        if manifest.scope != record.scope || manifest.identity != record.manifest {
            return Err(unsafe_entry(
                &manifest_name,
                "referenced manifest names a different scope or identity",
            ));
        }
        if protected.contains(&manifest_name) {
            return Ok(());
        }
        protected.insert(manifest_name);
        for artifact in manifest.entries {
            let name = blob_name(artifact.sha256);
            let entry = by_name
                .get(&name)
                .and_then(|index| inventory.get(*index))
                .ok_or_else(|| unsafe_entry(&name, "referenced artifact is absent"))?;
            if entry.kind != ManagedEntryKind::Blob || entry.length != artifact.length {
                return Err(unsafe_entry(
                    &name,
                    "referenced artifact inventory metadata is inconsistent",
                ));
            }
            let stat = statat(&self.output.fd, &name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(io::Error::from)?;
            let expected_length = usize::try_from(artifact.length)
                .map_err(|_| codec_error("artifact length does not fit this host"))?;
            validate_content_stat(&stat, self.service_uid, expected_length, &name)?;
            protected.insert(name);
        }
        Ok(())
    }

    fn publish_object(
        &self,
        final_name: &str,
        bytes: &[u8],
        object: CompilerArtifactGenerationObjectV1,
        hooks: &mut StoreHooks,
    ) -> Result<(), CompilerArtifactGenerationErrorV1> {
        if let Some(matches) =
            self.candidate_matches_exact(final_name, bytes, CandidateMode::Published)?
        {
            if matches {
                return Ok(());
            }
            return Err(unsafe_entry(
                final_name,
                "content-addressed object contains different bytes",
            ));
        }

        let staged_name = format!("{final_name}{STAGED_SUFFIX}");
        match self.candidate_matches_exact(&staged_name, bytes, CandidateMode::Staged)? {
            Some(true) => {}
            Some(false) => {
                return Err(unsafe_entry(
                    &staged_name,
                    "staged content-addressed object contains different bytes",
                ));
            }
            None => {
                hooks.object = object;
                self.durable
                    .stage_artifact(&staged_name, bytes, bytes.len(), hooks)?;
            }
        }
        hooks.object = object;
        self.durable
            .publish_staged(&staged_name, final_name, bytes, CONTENT_MODE, hooks)?;
        Ok(())
    }

    fn candidate_matches_exact(
        &self,
        entry: &str,
        expected: &[u8],
        mode: CandidateMode,
    ) -> Result<Option<bool>, CompilerArtifactGenerationErrorV1> {
        let descriptor = match openat(
            &self.output.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
            Err(error) => return Err(io::Error::from(error).into()),
        };
        let before = fstat(&descriptor).map_err(io::Error::from)?;
        validate_candidate_stat(&before, self.service_uid, expected.len(), entry, mode)?;
        let named_before =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_same_content_file(&before, &named_before, expected.len(), entry)?;
        let mut file = fs::File::from(descriptor);
        let matches = compare_exact_expected_bytes(&mut file, expected, entry)?;
        let after = fstat(&file).map_err(io::Error::from)?;
        validate_candidate_stat(&after, self.service_uid, expected.len(), entry, mode)?;
        let named_after =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_unchanged_content_file(&before, &after, expected.len(), entry)?;
        require_same_content_file(&after, &named_after, expected.len(), entry)?;
        Ok(Some(matches))
    }

    fn read_candidate_exact(
        &self,
        entry: &str,
        expected_length: usize,
        mode: CandidateMode,
    ) -> Result<Option<Vec<u8>>, CompilerArtifactGenerationErrorV1> {
        let descriptor = match openat(
            &self.output.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(descriptor) => descriptor,
            Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
            Err(error) => return Err(io::Error::from(error).into()),
        };
        let before = fstat(&descriptor).map_err(io::Error::from)?;
        validate_candidate_stat(&before, self.service_uid, expected_length, entry, mode)?;
        let named_before =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_same_content_file(&before, &named_before, expected_length, entry)?;
        let mut file = fs::File::from(descriptor);
        let bytes = read_exact_expected_length(&mut file, expected_length, entry)?;
        let after = fstat(&file).map_err(io::Error::from)?;
        validate_candidate_stat(&after, self.service_uid, expected_length, entry, mode)?;
        let named_after =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_unchanged_content_file(&before, &after, expected_length, entry)?;
        require_same_content_file(&after, &named_after, expected_length, entry)?;
        Ok(Some(bytes))
    }

    fn recover_scope_locked(
        &self,
        hooks: &mut StoreHooks,
    ) -> Result<Option<CompilerArtifactGenerationLeaseV1>, CompilerArtifactGenerationErrorV1> {
        let redo_bytes = self.read_scope_record_bytes(&self.names.redo)?;
        let canonical_bytes = self.read_scope_record_bytes(&self.names.record)?;
        let Some(redo_bytes) = redo_bytes else {
            let Some(canonical_bytes) = canonical_bytes else {
                return Ok(None);
            };
            hooks.record_operation = CompilerArtifactGenerationRecordOperationV1::Recover;
            let recovered_bytes = self.durable.establish_recovered_record_durability(
                &self.names.record,
                &self.names.redo,
                &canonical_bytes,
                MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1,
                hooks,
            )?;
            if recovered_bytes != canonical_bytes {
                return Err(unsafe_entry(
                    &self.names.record,
                    "canonical scope record changed across its recovery durability barrier",
                ));
            }
            let canonical = ScopeRecordV1::decode_canonical(&recovered_bytes, self.scope)?;
            return self.open_generation_for_record(&canonical, hooks).map(Some);
        };
        let redo = ScopeRecordV1::decode_canonical(&redo_bytes, self.scope)?;
        let canonical = canonical_bytes
            .as_deref()
            .map(|bytes| ScopeRecordV1::decode_canonical(bytes, self.scope))
            .transpose()?;

        let current = canonical
            .as_ref()
            .map(|record| self.open_generation_for_record(record, hooks))
            .transpose()?;
        let same_generation = canonical
            .as_ref()
            .is_some_and(|record| record.manifest == redo.manifest);
        let legal = match &canonical {
            Some(current) if current.manifest == redo.manifest => {
                current.canonical_bytes == redo.canonical_bytes
            }
            Some(current) => redo.previous == Some(current.manifest),
            None => redo.previous.is_none(),
        };
        if !legal {
            return Err(CompilerArtifactGenerationErrorV1::ConflictingRedo {
                reason: "redo predecessor does not equal the current committed manifest".to_owned(),
            });
        }
        let redo_generation = if same_generation {
            None
        } else {
            Some(self.open_generation_for_record(&redo, hooks)?)
        };

        hooks.record_operation = CompilerArtifactGenerationRecordOperationV1::Recover;
        if let Err(quota_error) = self.ensure_actual_quota_locked() {
            self.discard_validated_redo_locked(canonical_bytes.as_deref(), &redo_bytes)?;
            self.reclaim_locked()?;
            return Err(quota_error);
        }
        self.durable.promote_validated_redo(
            &self.names.record,
            &self.names.redo,
            canonical_bytes.as_deref(),
            &redo_bytes,
            MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1,
            hooks,
        )?;
        let canonical_after = self.read_scope_record_bytes(&self.names.record)?;
        if canonical_after.as_deref() != Some(redo_bytes.as_slice()) {
            return Err(unsafe_entry(
                &self.names.record,
                "recovered scope record differs from its validated redo",
            ));
        }
        self.ensure_root()?;
        Ok(if same_generation {
            current
        } else {
            redo_generation
        })
    }

    fn open_generation_locked(
        &self,
        expected: Option<CompilerArtifactGenerationManifestIdentityV1>,
        hooks: &mut StoreHooks,
    ) -> Result<Option<CompilerArtifactGenerationLeaseV1>, CompilerArtifactGenerationErrorV1> {
        let Some(record_bytes) = self.read_scope_record_bytes(&self.names.record)? else {
            if expected.is_some() {
                return Err(unsafe_entry(
                    &self.names.record,
                    "expected committed scope record is absent",
                ));
            }
            return Ok(None);
        };
        let record = ScopeRecordV1::decode_canonical(&record_bytes, self.scope)?;
        if expected.is_some_and(|expected| expected != record.manifest) {
            return Err(unsafe_entry(
                &self.names.record,
                "scope record does not name the expected manifest",
            ));
        }
        let lease = self.open_generation_for_record(&record, hooks)?;
        let record_after = self.read_scope_record_bytes(&self.names.record)?;
        if record_after.as_deref() != Some(record_bytes.as_slice()) {
            return Err(unsafe_entry(
                &self.names.record,
                "scope record changed while its generation was being opened",
            ));
        }
        Ok(Some(lease))
    }

    fn open_generation_for_record(
        &self,
        record: &ScopeRecordV1,
        hooks: &mut StoreHooks,
    ) -> Result<CompilerArtifactGenerationLeaseV1, CompilerArtifactGenerationErrorV1> {
        let manifest_object = self.open_leased_object(
            &manifest_name(record.manifest),
            record.manifest_length,
            MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1,
            ExpectedDigest::Manifest(record.manifest),
            CompilerArtifactGenerationObjectV1::Manifest,
            None,
            hooks,
        )?;
        let manifest =
            CompilerArtifactGenerationManifestV1::decode_canonical(&manifest_object.bytes)?;
        if manifest.scope != self.scope
            || manifest.identity != record.manifest
            || manifest.canonical_bytes.len() != record.manifest_length
        {
            return Err(unsafe_entry(
                manifest_name(record.manifest),
                "manifest identity, scope, or length differs from the scope record",
            ));
        }
        drop(manifest_object);

        let mut artifacts = Vec::new();
        artifacts
            .try_reserve_exact(manifest.entries.len())
            .map_err(|_| allocation_error(manifest.entries.len()))?;
        for entry in &manifest.entries {
            let length = usize::try_from(entry.length)
                .map_err(|_| codec_error("artifact length does not fit this host"))?;
            artifacts.push(self.open_leased_object(
                &blob_name(entry.sha256),
                length,
                entry.role.maximum_bytes(),
                ExpectedDigest::Blob(entry.sha256),
                CompilerArtifactGenerationObjectV1::Artifact(entry.role),
                Some(entry.role),
                hooks,
            )?);
        }
        self.ensure_root()?;
        Ok(CompilerArtifactGenerationLeaseV1 {
            manifest,
            artifacts,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn open_leased_object(
        &self,
        entry: &str,
        expected_length: usize,
        maximum_bytes: usize,
        expected_digest: ExpectedDigest,
        object: CompilerArtifactGenerationObjectV1,
        role: Option<CompilerArtifactRoleV1>,
        hooks: &mut StoreHooks,
    ) -> Result<LeasedObject, CompilerArtifactGenerationErrorV1> {
        if expected_length == 0 || expected_length > maximum_bytes {
            return Err(unsafe_entry(
                entry,
                "object length is outside its role bound",
            ));
        }
        hooks.hit_open(object)?;
        let descriptor = openat(
            &self.output.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        let before = fstat(&descriptor).map_err(io::Error::from)?;
        validate_content_stat(&before, self.service_uid, expected_length, entry)?;
        let named_before =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_same_content_file(&before, &named_before, expected_length, entry)?;

        let mut file = fs::File::from(descriptor);
        let streamed_digest = stream_exact_digest_without_allocation(
            &mut file,
            expected_length,
            expected_digest,
            entry,
        )?;
        if !expected_digest.matches_digest(streamed_digest) {
            return Err(unsafe_entry(
                entry,
                "object digest does not match its content address",
            ));
        }
        let streamed_after = fstat(&file).map_err(io::Error::from)?;
        let streamed_named_after =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_unchanged_content_file(&before, &streamed_after, expected_length, entry)?;
        require_same_content_file(
            &streamed_after,
            &streamed_named_after,
            expected_length,
            entry,
        )?;
        file.rewind()?;
        let bytes = read_exact_expected_length(&mut file, expected_length, entry)?;
        if !expected_digest.matches(&bytes) {
            return Err(unsafe_entry(
                entry,
                "object digest does not match its content address",
            ));
        }

        let after = fstat(&file).map_err(io::Error::from)?;
        let named_after =
            statat(&self.output.fd, entry, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_unchanged_content_file(&before, &after, expected_length, entry)?;
        require_same_content_file(&after, &named_after, expected_length, entry)?;
        drop(file);
        Ok(LeasedObject {
            role,
            bytes: bytes.into_boxed_slice(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopeRecordV1 {
    scope: CompilerArtifactGenerationScopeV1,
    previous: Option<CompilerArtifactGenerationManifestIdentityV1>,
    manifest: CompilerArtifactGenerationManifestIdentityV1,
    manifest_length: usize,
    canonical_bytes: Vec<u8>,
}

impl ScopeRecordV1 {
    fn new(
        scope: CompilerArtifactGenerationScopeV1,
        previous: Option<CompilerArtifactGenerationManifestIdentityV1>,
        manifest: CompilerArtifactGenerationManifestIdentityV1,
        manifest_length: usize,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        if manifest_length == 0
            || manifest_length > MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1
        {
            return Err(codec_error("scope record has an invalid manifest length"));
        }
        let mut canonical_bytes = Vec::new();
        canonical_bytes
            .try_reserve_exact(MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1)
            .map_err(|_| {
                allocation_error(MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1)
            })?;
        canonical_bytes.extend_from_slice(SCOPE_RECORD_MAGIC);
        canonical_bytes.extend_from_slice(&SCOPE_RECORD_VERSION.to_le_bytes());
        canonical_bytes.extend_from_slice(&scope.0);
        canonical_bytes.push(u8::from(previous.is_some()));
        canonical_bytes.extend_from_slice(
            &previous
                .map(CompilerArtifactGenerationManifestIdentityV1::as_bytes)
                .unwrap_or([0; 32]),
        );
        canonical_bytes.extend_from_slice(&manifest.0);
        canonical_bytes.extend_from_slice(
            &u32::try_from(manifest_length)
                .map_err(|_| codec_error("manifest length overflows scope record"))?
                .to_le_bytes(),
        );
        let checksum = sha256_parts(&[SCOPE_RECORD_CHECKSUM_DOMAIN, &canonical_bytes]);
        canonical_bytes.extend_from_slice(&checksum);
        if canonical_bytes.len() > MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1 {
            return Err(codec_error("scope record exceeds its canonical byte bound"));
        }
        Ok(Self {
            scope,
            previous,
            manifest,
            manifest_length,
            canonical_bytes,
        })
    }

    fn decode_canonical(
        bytes: &[u8],
        expected_scope: CompilerArtifactGenerationScopeV1,
    ) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        let record = Self::decode_canonical_any(bytes)?;
        if record.scope != expected_scope {
            return Err(codec_error("scope record names a different scope"));
        }
        Ok(record)
    }

    fn decode_canonical_any(bytes: &[u8]) -> Result<Self, CompilerArtifactGenerationErrorV1> {
        if bytes.len() > MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1 {
            return Err(codec_error("scope record exceeds its canonical byte bound"));
        }
        let checksum_offset = bytes
            .len()
            .checked_sub(32)
            .ok_or_else(|| codec_error("truncated scope record checksum"))?;
        let expected_checksum = sha256_parts(&[
            SCOPE_RECORD_CHECKSUM_DOMAIN,
            bytes
                .get(..checksum_offset)
                .ok_or_else(|| codec_error("truncated scope record"))?,
        ]);
        if bytes.get(checksum_offset..) != Some(expected_checksum.as_slice()) {
            return Err(codec_error("scope record checksum mismatch"));
        }
        let mut decoder = Decoder::new(
            bytes
                .get(..checksum_offset)
                .ok_or_else(|| codec_error("truncated scope record"))?,
        );
        if decoder.take(SCOPE_RECORD_MAGIC.len())? != SCOPE_RECORD_MAGIC {
            return Err(codec_error("bad scope record magic"));
        }
        if decoder.u16()? != SCOPE_RECORD_VERSION {
            return Err(codec_error("unsupported scope record version"));
        }
        let scope = CompilerArtifactGenerationScopeV1(decoder.array()?);
        let previous_tag = decoder.u8()?;
        let previous_bytes = decoder.array()?;
        let previous = match previous_tag {
            0 if previous_bytes == [0; 32] => None,
            1 => Some(CompilerArtifactGenerationManifestIdentityV1(previous_bytes)),
            _ => return Err(codec_error("invalid scope-record predecessor encoding")),
        };
        let manifest = CompilerArtifactGenerationManifestIdentityV1(decoder.array()?);
        let manifest_length = usize::try_from(decoder.u32()?)
            .map_err(|_| codec_error("manifest length does not fit this host"))?;
        if !decoder.finished() {
            return Err(codec_error("scope record contains trailing bytes"));
        }
        let record = Self::new(scope, previous, manifest, manifest_length)?;
        if record.canonical_bytes != bytes {
            return Err(codec_error("scope record encoding is not canonical"));
        }
        Ok(record)
    }
}

#[derive(Clone, Copy)]
enum ExpectedDigest {
    Blob([u8; 32]),
    Manifest(CompilerArtifactGenerationManifestIdentityV1),
}

impl ExpectedDigest {
    fn matches(self, bytes: &[u8]) -> bool {
        let mut digest = self.hasher();
        digest.update(bytes);
        self.matches_digest(digest.finalize().into())
    }

    fn hasher(self) -> Sha256 {
        let mut digest = Sha256::new();
        if matches!(self, Self::Manifest(_)) {
            digest.update(MANIFEST_IDENTITY_DOMAIN);
        }
        digest
    }

    fn matches_digest(self, actual: [u8; 32]) -> bool {
        match self {
            Self::Blob(expected) => actual == expected,
            Self::Manifest(expected) => actual == expected.0,
        }
    }
}

struct StoreHooks {
    faults: [Option<CompilerArtifactGenerationFaultPointV1>; 2],
    observation: Option<Arc<CompilerArtifactGenerationObservationV1>>,
    object: CompilerArtifactGenerationObjectV1,
    record_operation: CompilerArtifactGenerationRecordOperationV1,
}

impl StoreHooks {
    fn new(
        faults: [Option<CompilerArtifactGenerationFaultPointV1>; 2],
        observation: Option<Arc<CompilerArtifactGenerationObservationV1>>,
    ) -> Self {
        Self {
            faults,
            observation,
            object: CompilerArtifactGenerationObjectV1::Manifest,
            record_operation: CompilerArtifactGenerationRecordOperationV1::Recover,
        }
    }

    fn hit(&mut self, point: CompilerArtifactGenerationFaultPointV1) -> io::Result<()> {
        if let Some(observation) = &self.observation {
            observation.observe(point);
        }
        if self.faults[0] == Some(point) {
            self.faults[0] = self.faults[1].take();
            Err(io::Error::other(format!(
                "injected compiler-artifact generation fault at {point:?}"
            )))
        } else {
            Ok(())
        }
    }

    fn hit_open(
        &mut self,
        object: CompilerArtifactGenerationObjectV1,
    ) -> Result<(), CompilerArtifactGenerationErrorV1> {
        self.hit(CompilerArtifactGenerationFaultPointV1::Open { object })?;
        Ok(())
    }

    fn hit_directory_scan(&mut self) -> Result<(), CompilerArtifactGenerationErrorV1> {
        self.hit(CompilerArtifactGenerationFaultPointV1::DirectoryScan)?;
        Ok(())
    }
}

impl RetainedDurableDirectoryHooksV1 for StoreHooks {
    fn record(
        &mut self,
        boundary: RetainedDurableRecordBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        let boundary = map_record_boundary(boundary);
        let timing = map_timing(timing);
        self.hit(CompilerArtifactGenerationFaultPointV1::ScopeRecord {
            operation: self.record_operation,
            boundary,
            timing,
        })
    }

    fn artifact(
        &mut self,
        boundary: RetainedDurableArtifactBoundaryV1,
        timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        self.hit(CompilerArtifactGenerationFaultPointV1::Object {
            object: self.object,
            boundary: map_object_boundary(boundary),
            timing: map_timing(timing),
        })
    }

    fn recovery(
        &mut self,
        _boundary: RetainedDurableRecoveryBoundaryV1,
        _timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        Ok(())
    }
}

fn map_timing(timing: RetainedDurableFaultTimingV1) -> CompilerArtifactGenerationFaultTimingV1 {
    match timing {
        RetainedDurableFaultTimingV1::Before => CompilerArtifactGenerationFaultTimingV1::Before,
        RetainedDurableFaultTimingV1::After => CompilerArtifactGenerationFaultTimingV1::After,
    }
}

fn map_object_boundary(
    boundary: RetainedDurableArtifactBoundaryV1,
) -> CompilerArtifactGenerationObjectBoundaryV1 {
    match boundary {
        RetainedDurableArtifactBoundaryV1::CreateTemp => {
            CompilerArtifactGenerationObjectBoundaryV1::CreateTemp
        }
        RetainedDurableArtifactBoundaryV1::WriteTemp => {
            CompilerArtifactGenerationObjectBoundaryV1::WriteTemp
        }
        RetainedDurableArtifactBoundaryV1::SyncTemp => {
            CompilerArtifactGenerationObjectBoundaryV1::SyncTemp
        }
        RetainedDurableArtifactBoundaryV1::RenameTempToStaged => {
            CompilerArtifactGenerationObjectBoundaryV1::RenameTempToStaged
        }
        RetainedDurableArtifactBoundaryV1::SyncStagedName => {
            CompilerArtifactGenerationObjectBoundaryV1::SyncStagedName
        }
        RetainedDurableArtifactBoundaryV1::SetFinalMode => {
            CompilerArtifactGenerationObjectBoundaryV1::SetFinalMode
        }
        RetainedDurableArtifactBoundaryV1::SyncFinalMode => {
            CompilerArtifactGenerationObjectBoundaryV1::SyncFinalMode
        }
        RetainedDurableArtifactBoundaryV1::RenameStagedToFinal => {
            CompilerArtifactGenerationObjectBoundaryV1::RenameStagedToFinal
        }
        RetainedDurableArtifactBoundaryV1::SyncFinalName => {
            CompilerArtifactGenerationObjectBoundaryV1::SyncFinalName
        }
    }
}

fn map_record_boundary(
    boundary: RetainedDurableRecordBoundaryV1,
) -> CompilerArtifactGenerationRecordBoundaryV1 {
    match boundary {
        RetainedDurableRecordBoundaryV1::CreateTemp => {
            CompilerArtifactGenerationRecordBoundaryV1::CreateTemp
        }
        RetainedDurableRecordBoundaryV1::WriteTemp => {
            CompilerArtifactGenerationRecordBoundaryV1::WriteTemp
        }
        RetainedDurableRecordBoundaryV1::SyncTemp => {
            CompilerArtifactGenerationRecordBoundaryV1::SyncTemp
        }
        RetainedDurableRecordBoundaryV1::RenameTempToRedo => {
            CompilerArtifactGenerationRecordBoundaryV1::RenameTempToRedo
        }
        RetainedDurableRecordBoundaryV1::SyncRedoName => {
            CompilerArtifactGenerationRecordBoundaryV1::SyncRedoName
        }
        RetainedDurableRecordBoundaryV1::RenameRedoToCanonical => {
            CompilerArtifactGenerationRecordBoundaryV1::RenameRedoToCanonical
        }
        RetainedDurableRecordBoundaryV1::SyncCanonicalName => {
            CompilerArtifactGenerationRecordBoundaryV1::SyncCanonicalName
        }
    }
}

fn manifest_entry(
    role: CompilerArtifactRoleV1,
    bytes: &[u8],
) -> Result<CompilerArtifactGenerationManifestEntryV1, CompilerArtifactGenerationErrorV1> {
    validate_role_length(role, bytes.len())?;
    Ok(CompilerArtifactGenerationManifestEntryV1 {
        role,
        length: u64::try_from(bytes.len())
            .map_err(|_| codec_error("artifact length does not fit u64"))?,
        sha256: sha256(bytes),
    })
}

fn request_manifest_identity_without_allocation(
    scope: CompilerArtifactGenerationScopeV1,
    request: &CompilerArtifactGenerationRequestV1<'_>,
) -> Result<CompilerArtifactGenerationManifestIdentityV1, CompilerArtifactGenerationErrorV1> {
    let mut aggregate = 0u64;
    for role in CompilerArtifactRoleV1::REQUIRED {
        let bytes = request
            .artifact(role)
            .expect("fixed required generation role is present");
        validate_role_length(role, bytes.len())?;
        aggregate = aggregate.checked_add(bytes.len() as u64).ok_or(
            CompilerArtifactGenerationErrorV1::Bounds {
                role: None,
                actual: u64::MAX,
                maximum: MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
            },
        )?;
    }
    if let Some(bytes) = request.artifact(CompilerArtifactRoleV1::Hsaco) {
        validate_role_length(CompilerArtifactRoleV1::Hsaco, bytes.len())?;
        aggregate = aggregate.checked_add(bytes.len() as u64).ok_or(
            CompilerArtifactGenerationErrorV1::Bounds {
                role: None,
                actual: u64::MAX,
                maximum: MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
            },
        )?;
    }
    if aggregate > MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64 {
        return Err(CompilerArtifactGenerationErrorV1::Bounds {
            role: None,
            actual: aggregate,
            maximum: MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
        });
    }

    let mut digest = Sha256::new();
    digest.update(MANIFEST_IDENTITY_DOMAIN);
    digest.update(MANIFEST_MAGIC);
    digest.update(MANIFEST_VERSION.to_le_bytes());
    digest.update(scope.0);
    digest.update(request.compiler_identity);
    digest.update(request.pipeline_identity);
    digest.update(request.target_identity);
    digest.update([if request.hsaco.is_some() { 5 } else { 4 }]);
    for role in CompilerArtifactRoleV1::REQUIRED {
        let bytes = request
            .artifact(role)
            .expect("fixed required generation role is present");
        digest.update([role as u8]);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(sha256(bytes));
    }
    if let Some(bytes) = request.artifact(CompilerArtifactRoleV1::Hsaco) {
        digest.update([CompilerArtifactRoleV1::Hsaco as u8]);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(sha256(bytes));
    }
    Ok(CompilerArtifactGenerationManifestIdentityV1(
        digest.finalize().into(),
    ))
}

fn validate_entries(
    entries: &[CompilerArtifactGenerationManifestEntryV1],
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    if !(4..=5).contains(&entries.len()) {
        return Err(codec_error(
            "manifest must contain four or five fixed roles",
        ));
    }
    for (index, required) in CompilerArtifactRoleV1::REQUIRED.iter().enumerate() {
        if entries.get(index).map(|entry| entry.role) != Some(*required) {
            return Err(codec_error(
                "manifest required roles are missing or out of order",
            ));
        }
    }
    if entries.len() == 5
        && entries.get(4).map(|entry| entry.role) != Some(CompilerArtifactRoleV1::Hsaco)
    {
        return Err(codec_error(
            "optional HSACO role is missing or out of order",
        ));
    }
    let mut aggregate = 0u64;
    for entry in entries {
        let length = usize::try_from(entry.length)
            .map_err(|_| codec_error("artifact length does not fit this host"))?;
        validate_role_length(entry.role, length)?;
        aggregate = aggregate.checked_add(entry.length).ok_or(
            CompilerArtifactGenerationErrorV1::Bounds {
                role: None,
                actual: u64::MAX,
                maximum: MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
            },
        )?;
    }
    if aggregate > MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64 {
        return Err(CompilerArtifactGenerationErrorV1::Bounds {
            role: None,
            actual: aggregate,
            maximum: MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
        });
    }
    Ok(())
}

fn validate_role_length(
    role: CompilerArtifactRoleV1,
    length: usize,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    let maximum = role.maximum_bytes();
    if length == 0 || length > maximum {
        return Err(CompilerArtifactGenerationErrorV1::Bounds {
            role: Some(role),
            actual: length as u64,
            maximum: maximum as u64,
        });
    }
    Ok(())
}

fn validate_store_root_stat(
    stat: &rustix::fs::Stat,
    service_uid: u32,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != service_uid
        || stat.st_mode & 0o7777 != 0o700
        || stat.st_nlink == 0
    {
        return Err(CompilerArtifactGenerationErrorV1::UnsafeRoot {
            reason: "root must be a linked owner-matched directory with exact mode 0700".to_owned(),
        });
    }
    Ok(())
}

fn classify_managed_name(
    name: &[u8],
) -> Result<Option<ManagedEntryKind>, CompilerArtifactGenerationErrorV1> {
    if exact_hash_name(name, BLOB_PREFIX.as_bytes(), CONTENT_SUFFIX.as_bytes()) {
        return Ok(Some(ManagedEntryKind::Blob));
    }
    if exact_hash_name(name, MANIFEST_PREFIX.as_bytes(), CONTENT_SUFFIX.as_bytes()) {
        return Ok(Some(ManagedEntryKind::Manifest));
    }
    if exact_hash_name(name, SCOPE_PREFIX.as_bytes(), RECORD_SUFFIX.as_bytes()) {
        return Ok(Some(ManagedEntryKind::ScopeRecord));
    }
    let record_redo_suffix = format!("{RECORD_SUFFIX}{REDO_SUFFIX}");
    if exact_hash_name(name, SCOPE_PREFIX.as_bytes(), record_redo_suffix.as_bytes()) {
        return Ok(Some(ManagedEntryKind::ScopeRedo));
    }
    if let Some(base) = name.strip_suffix(STAGED_SUFFIX.as_bytes())
        && (exact_hash_name(base, BLOB_PREFIX.as_bytes(), CONTENT_SUFFIX.as_bytes())
            || exact_hash_name(base, MANIFEST_PREFIX.as_bytes(), CONTENT_SUFFIX.as_bytes()))
    {
        return Ok(Some(ManagedEntryKind::Staged));
    }
    if let Some(marker) = name.windows(5).rposition(|window| window == b".tmp-") {
        let (base, suffix) = name.split_at(marker);
        let suffix = suffix
            .get(5..)
            .ok_or_else(|| unsafe_entry("<managed-temp>", "temporary suffix is truncated"))?;
        let mut fields = suffix.split(|byte| *byte == b'-');
        let pid = fields.next().unwrap_or_default();
        let sequence = fields.next().unwrap_or_default();
        let canonical_suffix = !pid.is_empty()
            && !sequence.is_empty()
            && fields.next().is_none()
            && pid.iter().all(u8::is_ascii_digit)
            && sequence.iter().all(u8::is_ascii_digit);
        let staged_base = base
            .strip_suffix(STAGED_SUFFIX.as_bytes())
            .is_some_and(|final_name| {
                exact_hash_name(
                    final_name,
                    BLOB_PREFIX.as_bytes(),
                    CONTENT_SUFFIX.as_bytes(),
                ) || exact_hash_name(
                    final_name,
                    MANIFEST_PREFIX.as_bytes(),
                    CONTENT_SUFFIX.as_bytes(),
                )
            });
        let redo_base =
            exact_hash_name(base, SCOPE_PREFIX.as_bytes(), record_redo_suffix.as_bytes());
        if canonical_suffix && (staged_base || redo_base) {
            return Ok(Some(ManagedEntryKind::Temporary));
        }
    }
    if name.starts_with(BLOB_PREFIX.as_bytes())
        || name.starts_with(MANIFEST_PREFIX.as_bytes())
        || name.starts_with(SCOPE_PREFIX.as_bytes())
    {
        let entry = String::from_utf8_lossy(name).into_owned();
        return Err(unsafe_entry(
            &entry,
            "reserved generation-store name is not canonical",
        ));
    }
    Ok(None)
}

fn exact_hash_name(name: &[u8], prefix: &[u8], suffix: &[u8]) -> bool {
    name.strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
        .is_some_and(|digest| digest.len() == 64 && digest.iter().all(is_lower_hex))
}

const fn is_lower_hex(byte: &u8) -> bool {
    matches!(byte, b'0'..=b'9' | b'a'..=b'f')
}

fn validate_managed_inventory_stat(
    entry: &str,
    kind: ManagedEntryKind,
    stat: &rustix::fs::Stat,
    service_uid: u32,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    let length = u64::try_from(stat.st_size).ok();
    let (valid_mode, maximum, allow_empty) = match kind {
        ManagedEntryKind::Blob => (
            stat.st_mode & 0o7777 == CONTENT_MODE,
            MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
            false,
        ),
        ManagedEntryKind::Manifest => (
            stat.st_mode & 0o7777 == CONTENT_MODE,
            MAX_COMPILER_ARTIFACT_GENERATION_MANIFEST_BYTES_V1 as u64,
            false,
        ),
        ManagedEntryKind::ScopeRecord | ManagedEntryKind::ScopeRedo => (
            stat.st_mode & 0o7777 == 0o600,
            MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1 as u64,
            false,
        ),
        ManagedEntryKind::Staged => (
            matches!(stat.st_mode & 0o7777, CONTENT_MODE | 0o600),
            MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
            false,
        ),
        ManagedEntryKind::Temporary => (
            stat.st_mode & 0o7777 == 0o600,
            MAX_COMPILER_ARTIFACT_GENERATION_BYTES_V1 as u64,
            true,
        ),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != service_uid
        || stat.st_nlink != 1
        || !valid_mode
        || length.is_none_or(|length| (!allow_empty && length == 0) || length > maximum)
    {
        return Err(unsafe_entry(
            entry,
            "managed entry has an unsafe type, owner, mode, link count, or length",
        ));
    }
    Ok(())
}

fn validate_candidate_stat(
    stat: &rustix::fs::Stat,
    service_uid: u32,
    expected_length: usize,
    entry: &str,
    mode: CandidateMode,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    let valid_mode = match mode {
        CandidateMode::Published => stat.st_mode & 0o7777 == CONTENT_MODE,
        CandidateMode::Staged => matches!(stat.st_mode & 0o7777, CONTENT_MODE | 0o600),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != service_uid
        || stat.st_nlink != 1
        || !valid_mode
        || usize::try_from(stat.st_size).ok() != Some(expected_length)
    {
        return Err(unsafe_entry(
            entry,
            "candidate object does not have exact expected metadata and length",
        ));
    }
    Ok(())
}

fn read_exact_expected_length(
    file: &mut fs::File,
    expected_length: usize,
    entry: &str,
) -> Result<Vec<u8>, CompilerArtifactGenerationErrorV1> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_length)
        .map_err(|_| allocation_error(expected_length))?;
    bytes.resize(expected_length, 0);
    if let Err(error) = file.read_exact(&mut bytes) {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            return Err(unsafe_entry(
                entry,
                "object became shorter while it was read",
            ));
        }
        return Err(error.into());
    }
    let mut trailing = [0u8; 1];
    if file.read(&mut trailing)? != 0 {
        return Err(unsafe_entry(
            entry,
            "object became longer while it was read",
        ));
    }
    Ok(bytes)
}

fn compare_exact_expected_bytes(
    file: &mut fs::File,
    expected: &[u8],
    entry: &str,
) -> Result<bool, CompilerArtifactGenerationErrorV1> {
    const BUFFER_BYTES: usize = 16 * 1024;
    let mut buffer = [0u8; BUFFER_BYTES];
    let mut matches = true;
    for expected_chunk in expected.chunks(BUFFER_BYTES) {
        let actual = buffer
            .get_mut(..expected_chunk.len())
            .ok_or_else(|| codec_error("comparison chunk exceeds its fixed buffer"))?;
        read_exact_corruption_aware(file, actual, entry)?;
        matches &= actual == expected_chunk;
    }
    require_exact_end(file, entry)?;
    Ok(matches)
}

fn stream_exact_digest_without_allocation(
    file: &mut fs::File,
    expected_length: usize,
    expected_digest: ExpectedDigest,
    entry: &str,
) -> Result<[u8; 32], CompilerArtifactGenerationErrorV1> {
    const BUFFER_BYTES: usize = 16 * 1024;
    let mut buffer = [0u8; BUFFER_BYTES];
    let mut remaining = expected_length;
    let mut digest = expected_digest.hasher();
    while remaining != 0 {
        let chunk_length = remaining.min(BUFFER_BYTES);
        let chunk = buffer
            .get_mut(..chunk_length)
            .ok_or_else(|| codec_error("digest chunk exceeds its fixed buffer"))?;
        read_exact_corruption_aware(file, chunk, entry)?;
        digest.update(chunk);
        remaining -= chunk_length;
    }
    require_exact_end(file, entry)?;
    Ok(digest.finalize().into())
}

fn read_exact_corruption_aware(
    file: &mut fs::File,
    bytes: &mut [u8],
    entry: &str,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    if let Err(error) = file.read_exact(bytes) {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            return Err(unsafe_entry(
                entry,
                "object became shorter while it was read",
            ));
        }
        return Err(error.into());
    }
    Ok(())
}

fn require_exact_end(
    file: &mut fs::File,
    entry: &str,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    let mut trailing = [0u8; 1];
    if file.read(&mut trailing)? != 0 {
        return Err(unsafe_entry(
            entry,
            "object became longer while it was read",
        ));
    }
    Ok(())
}

fn estimated_file_charge(
    length: usize,
    allocation_unit: u64,
) -> Result<u64, CompilerArtifactGenerationErrorV1> {
    let length = u64::try_from(length).map_err(|_| codec_error("file length does not fit u64"))?;
    length
        .checked_add(allocation_unit - 1)
        .map(|value| value / allocation_unit * allocation_unit)
        .ok_or_else(|| codec_error("file charge overflowed"))
}

fn validate_scope_record_stat(
    stat: &rustix::fs::Stat,
    service_uid: u32,
    entry: &str,
) -> Result<usize, CompilerArtifactGenerationErrorV1> {
    let length = usize::try_from(stat.st_size).ok();
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != service_uid
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_nlink != 1
        || length.is_none_or(|length| {
            length == 0 || length > MAX_COMPILER_ARTIFACT_GENERATION_SCOPE_RECORD_BYTES_V1
        })
    {
        return Err(unsafe_entry(
            entry,
            "scope record must be an owner-matched 0600 single-link regular file within bounds",
        ));
    }
    length.ok_or_else(|| unsafe_entry(entry, "scope record has a negative or oversized length"))
}

fn validate_content_stat(
    stat: &rustix::fs::Stat,
    service_uid: u32,
    expected_length: usize,
    entry: &str,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != service_uid
        || stat.st_mode & 0o7777 != CONTENT_MODE
        || stat.st_nlink != 1
        || usize::try_from(stat.st_size).ok() != Some(expected_length)
    {
        return Err(unsafe_entry(
            entry,
            "object must be an owner-matched 0400 single-link regular file of exact length",
        ));
    }
    Ok(())
}

fn require_same_content_file(
    descriptor: &rustix::fs::Stat,
    named: &rustix::fs::Stat,
    expected_length: usize,
    entry: &str,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    if descriptor.st_dev != named.st_dev
        || descriptor.st_ino != named.st_ino
        || descriptor.st_mode != named.st_mode
        || descriptor.st_uid != named.st_uid
        || descriptor.st_nlink != named.st_nlink
        || descriptor.st_size != named.st_size
        || usize::try_from(named.st_size).ok() != Some(expected_length)
    {
        return Err(unsafe_entry(
            entry,
            "named object does not match its pinned descriptor",
        ));
    }
    Ok(())
}

fn require_unchanged_content_file(
    before: &rustix::fs::Stat,
    after: &rustix::fs::Stat,
    expected_length: usize,
    entry: &str,
) -> Result<(), CompilerArtifactGenerationErrorV1> {
    require_same_content_file(before, after, expected_length, entry)?;
    if before.st_mtime != after.st_mtime
        || before.st_mtime_nsec != after.st_mtime_nsec
        || before.st_ctime != after.st_ctime
        || before.st_ctime_nsec != after.st_ctime_nsec
    {
        return Err(unsafe_entry(entry, "object changed while it was read"));
    }
    Ok(())
}

fn blob_name(identity: [u8; 32]) -> String {
    format!("{BLOB_PREFIX}{}{CONTENT_SUFFIX}", encode_hex(&identity))
}

fn manifest_name(identity: CompilerArtifactGenerationManifestIdentityV1) -> String {
    format!(
        "{MANIFEST_PREFIX}{}{CONTENT_SUFFIX}",
        encode_hex(&identity.0)
    )
}

fn manifest_identity(bytes: &[u8]) -> CompilerArtifactGenerationManifestIdentityV1 {
    CompilerArtifactGenerationManifestIdentityV1(sha256_parts(&[MANIFEST_IDENTITY_DOMAIN, bytes]))
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

fn codec_error(reason: impl Into<String>) -> CompilerArtifactGenerationErrorV1 {
    CompilerArtifactGenerationErrorV1::Codec {
        reason: reason.into(),
    }
}

fn unsafe_entry(
    entry: impl Into<String>,
    reason: impl Into<String>,
) -> CompilerArtifactGenerationErrorV1 {
    CompilerArtifactGenerationErrorV1::UnsafeEntry {
        entry: entry.into(),
        reason: reason.into(),
    }
}

fn allocation_error(requested: usize) -> CompilerArtifactGenerationErrorV1 {
    CompilerArtifactGenerationErrorV1::AllocationFailed { requested }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CompilerArtifactGenerationErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| codec_error("decoder offset overflow"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| codec_error("truncated canonical generation data"))?;
        self.offset = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, CompilerArtifactGenerationErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, CompilerArtifactGenerationErrorV1> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("exact decoder width"),
        ))
    }

    fn u32(&mut self) -> Result<u32, CompilerArtifactGenerationErrorV1> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("exact decoder width"),
        ))
    }

    fn u64(&mut self) -> Result<u64, CompilerArtifactGenerationErrorV1> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("exact decoder width"),
        ))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], CompilerArtifactGenerationErrorV1> {
        Ok(self.take(N)?.try_into().expect("exact decoder width"))
    }

    const fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request<'a>(lineage: &'a [u8]) -> CompilerArtifactGenerationRequestV1<'a> {
        CompilerArtifactGenerationRequestV1::new(
            [1; 32],
            [2; 32],
            [3; 32],
            b"mir",
            b"neutral",
            b"target",
            lineage,
            Some(b"hsaco"),
        )
    }

    #[test]
    fn aggregate_length_overflow_is_rejected_without_wrapping() {
        let entries = vec![
            CompilerArtifactGenerationManifestEntryV1 {
                role: CompilerArtifactRoleV1::SemanticMir,
                length: MAX_COMPILER_SEMANTIC_MIR_BYTES_V1 as u64,
                sha256: [0; 32],
            },
            CompilerArtifactGenerationManifestEntryV1 {
                role: CompilerArtifactRoleV1::NeutralKir,
                length: MAX_COMPILER_NEUTRAL_KIR_BYTES_V1 as u64,
                sha256: [0; 32],
            },
            CompilerArtifactGenerationManifestEntryV1 {
                role: CompilerArtifactRoleV1::TargetKir,
                length: MAX_COMPILER_TARGET_KIR_BYTES_V1 as u64,
                sha256: [0; 32],
            },
            CompilerArtifactGenerationManifestEntryV1 {
                role: CompilerArtifactRoleV1::Lineage,
                length: MAX_COMPILER_LINEAGE_BYTES_V1 as u64,
                sha256: [0; 32],
            },
            CompilerArtifactGenerationManifestEntryV1 {
                role: CompilerArtifactRoleV1::Hsaco,
                length: MAX_COMPILER_HSACO_BYTES_V1 as u64,
                sha256: [0; 32],
            },
        ];
        validate_entries(&entries).unwrap();

        let oversized_bytes = vec![0; MAX_COMPILER_LINEAGE_BYTES_V1 + 1];
        let oversized = request(&oversized_bytes);
        assert!(matches!(
            CompilerArtifactGenerationManifestV1::for_request(
                CompilerArtifactGenerationScopeV1::from_bytes([9; 32]),
                &oversized
            ),
            Err(CompilerArtifactGenerationErrorV1::Bounds {
                role: Some(CompilerArtifactRoleV1::Lineage),
                ..
            })
        ));
    }

    #[test]
    fn allocation_failure_identity_path_matches_canonical_manifest_identity() {
        let scope = CompilerArtifactGenerationScopeV1::from_bytes([9; 32]);
        let request = request(b"lineage");
        let manifest = CompilerArtifactGenerationManifestV1::for_request(scope, &request).unwrap();
        assert_eq!(
            request_manifest_identity_without_allocation(scope, &request).unwrap(),
            manifest.identity()
        );
    }
}
