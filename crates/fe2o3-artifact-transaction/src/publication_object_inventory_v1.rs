//! Descriptor-only custody for one pre-sidecar strict V3 publication.
//!
//! The exporter is the only path-based half of this protocol. It runs as the artifact-store
//! owner, acquires both existing publication locks, and opens the fixed object roster. Import and
//! every later revalidation operate only on those already-open descriptions with `fstat`,
//! `getdents64`, `pread`, and nonblocking lock probes. The resulting owner is inert custody: it
//! cannot publish, consume, load, link, or launch anything.

use crate::attempt::{AttemptPhase, AttemptRegistry, MAX_ATTEMPT_BYTES};
use crate::compiler_module_handoff::semantic_v3::{
    PublicationObjectHandoffValidationV1, publication_object_names_v1,
    publication_object_ready_bytes_v1, validate_publication_object_handoff_v1,
};
use crate::compiler_module_handoff::{
    MAX_SLOT_ENTRIES, MAX_STALE_SLOTS, PAYLOAD_ENTRY, READY_ENTRY,
};
use crate::{
    ATTEMPT_FILE, BuildAttempt, BuildSession, CompilerModuleHandoffErrorV3,
    CompilerModuleHandoffReceiptV3, EmitError, LOCK_FILE, MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
    MAX_OUTPUT_ENTRIES, PinnedOutput, ProducerIdentity, acquire_linux_descriptor_flock,
    acquire_linux_ofd_exclusive_lock,
};
use rustix::fd::{AsRawFd, OwnedFd};
use rustix::fs::{FileType, Mode, OFlags, Stat, fstat, openat};
use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::Path;

/// Exact number of descriptors in one V1 publication-object transfer.
pub const COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1: usize = 9;

const ROOT_SCAN_INDEX: usize = 0;
const ROOT_FLOCK_HOLDER_INDEX: usize = 1;
const ARTIFACT_LOCK_HOLDER_INDEX: usize = 2;
const ARTIFACT_LOCK_PROBE_INDEX: usize = 3;
const ATTEMPTS_INDEX: usize = 4;
const PRODUCER_INDEX: usize = 5;
const SLOT_INDEX: usize = 6;
const READY_INDEX: usize = 7;
const MODULE_INDEX: usize = 8;
const MAX_PRODUCER_ENTRIES_V1: usize = MAX_STALE_SLOTS + 1;
const DIRECTORY_SCAN_BUFFER_BYTES_V1: usize = 16 * 1024;
const FILE_COMPARE_BUFFER_BYTES_V1: usize = 16 * 1024;
const LINUX_DIRENT64_FIXED_BYTES: usize = 19;

/// Fixed semantic role of one descriptor in the transfer order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum PublicationObjectRoleV1 {
    /// Read-only output-root description used for bounded scans and flock probes.
    OutputRootScan = 0,
    /// Independent read-only output-root description retaining the exclusive flock.
    OutputRootFlockHolder = 1,
    /// Read-write named artifact-lock description retaining the exclusive OFD lock.
    ArtifactLockHolder = 2,
    /// Independent read-write named artifact-lock description used for conflict probes.
    ArtifactLockProbe = 3,
    /// Read-only `.fe2o3-attempts-v1` description.
    AttemptRegistry = 4,
    /// Read-only exact V3 producer-directory description.
    ProducerDirectory = 5,
    /// Read-only exact Production-slot directory description.
    ProductionSlotDirectory = 6,
    /// Read-only exact `ready` record description.
    ReadyRecord = 7,
    /// Read-only exact `module` payload description.
    ModulePayload = 8,
}

impl PublicationObjectRoleV1 {
    const ALL: [Self; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1] = [
        Self::OutputRootScan,
        Self::OutputRootFlockHolder,
        Self::ArtifactLockHolder,
        Self::ArtifactLockProbe,
        Self::AttemptRegistry,
        Self::ProducerDirectory,
        Self::ProductionSlotDirectory,
        Self::ReadyRecord,
        Self::ModulePayload,
    ];

    const fn index(self) -> usize {
        self as usize
    }
}

impl fmt::Display for PublicationObjectRoleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::OutputRootScan => "output-root scan",
            Self::OutputRootFlockHolder => "output-root flock holder",
            Self::ArtifactLockHolder => "artifact-lock holder",
            Self::ArtifactLockProbe => "artifact-lock probe",
            Self::AttemptRegistry => "attempt registry",
            Self::ProducerDirectory => "V3 producer directory",
            Self::ProductionSlotDirectory => "Production slot directory",
            Self::ReadyRecord => "ready record",
            Self::ModulePayload => "module payload",
        })
    }
}

/// Identity of the separately observed client artifact-root descriptor.
///
/// The service derives this snapshot from its independently retained view of remote FD 197. The
/// imported root descriptions must name the same inode with the exact observed owner, directory
/// mode, and link count. Directory size, blocks, and timestamps are intentionally excluded because
/// their representation and update behavior are filesystem-dependent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PublicationObjectRootIdentityV1 {
    device: u64,
    inode: u64,
    owner_uid: u32,
    owner_gid: u32,
    mode: u32,
    link_count: u64,
}

impl PublicationObjectRootIdentityV1 {
    /// Constructs an identity from the security-relevant `fstat` fields observed on remote FD 197.
    pub const fn new(
        device: u64,
        inode: u64,
        owner_uid: u32,
        owner_gid: u32,
        mode: u32,
        link_count: u64,
    ) -> Self {
        Self {
            device,
            inode,
            owner_uid,
            owner_gid,
            mode,
            link_count,
        }
    }

    /// Returns the filesystem device identity.
    pub const fn device(self) -> u64 {
        self.device
    }

    /// Returns the inode identity.
    pub const fn inode(self) -> u64 {
        self.inode
    }

    /// Returns the exact owner UID observed on remote FD 197.
    pub const fn owner_uid(self) -> u32 {
        self.owner_uid
    }

    /// Returns the exact owner GID observed on remote FD 197.
    pub const fn owner_gid(self) -> u32 {
        self.owner_gid
    }

    /// Returns the exact directory type and permission mode observed on remote FD 197.
    pub const fn mode(self) -> u32 {
        self.mode
    }

    /// Returns the exact link count observed on remote FD 197.
    pub const fn link_count(self) -> u64 {
        self.link_count
    }

    fn from_stat(stat: &Stat) -> Self {
        Self::new(
            stat.st_dev,
            stat.st_ino,
            stat.st_uid,
            stat.st_gid,
            stat.st_mode,
            stat.st_nlink,
        )
    }

    fn matches(self, stat: &Stat) -> bool {
        self.device == stat.st_dev
            && self.inode == stat.st_ino
            && self.owner_uid == stat.st_uid
            && self.owner_gid == stat.st_gid
            && self.mode == stat.st_mode
            && self.link_count == stat.st_nlink
    }
}

/// Failure to export, import, or revalidate one fixed publication-object inventory.
#[derive(Debug)]
pub enum CompilerModuleHandoffPublicationObjectErrorV1 {
    /// The received SCM_RIGHTS vector did not contain exactly nine descriptors.
    DescriptorCount { actual: usize },
    /// Existing path-based artifact admission or locking failed during export.
    Artifact(EmitError),
    /// Existing strict V3 record or payload authentication failed.
    Handoff(CompilerModuleHandoffErrorV3),
    /// A descriptor role did not satisfy its fixed structural contract.
    InvalidDescriptor {
        role: PublicationObjectRoleV1,
        reason: String,
    },
    /// Two roles unexpectedly named the same inode, or an intentional pair did not.
    InvalidAlias {
        left: PublicationObjectRoleV1,
        right: PublicationObjectRoleV1,
    },
    /// The received output root disagreed with the independently observed FD 197 identity.
    RootIdentityMismatch,
    /// A bounded descriptor-only directory scan was malformed or had the wrong roster.
    InvalidRoster {
        role: PublicationObjectRoleV1,
        reason: String,
    },
    /// The attempt registry did not authorize the exact frontend-owned building generation.
    Attempt { reason: String },
    /// A retained holder did not conflict with its independent lock probe.
    LockNotHeld { role: PublicationObjectRoleV1 },
    /// A descriptor-only system operation failed.
    Io(io::Error),
}

impl fmt::Display for CompilerModuleHandoffPublicationObjectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescriptorCount { actual } => write!(
                formatter,
                "publication-object transfer contained {actual} descriptors instead of exactly {COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1}"
            ),
            Self::Artifact(error) => write!(formatter, "artifact export failed: {error}"),
            Self::Handoff(error) => write!(formatter, "strict V3 handoff is invalid: {error}"),
            Self::InvalidDescriptor { role, reason } => {
                write!(formatter, "invalid {role} descriptor: {reason}")
            }
            Self::InvalidAlias { left, right } => {
                write!(
                    formatter,
                    "invalid descriptor alias between {left} and {right}"
                )
            }
            Self::RootIdentityMismatch => formatter.write_str(
                "publication-object output root does not match the separately observed client root",
            ),
            Self::InvalidRoster { role, reason } => {
                write!(formatter, "invalid {role} roster: {reason}")
            }
            Self::Attempt { reason } => write!(formatter, "invalid publication attempt: {reason}"),
            Self::LockNotHeld { role } => {
                write!(
                    formatter,
                    "the {role} does not retain its required exclusive lock"
                )
            }
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for CompilerModuleHandoffPublicationObjectErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Artifact(error) => Some(error),
            Self::Handoff(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for CompilerModuleHandoffPublicationObjectErrorV1 {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<EmitError> for CompilerModuleHandoffPublicationObjectErrorV1 {
    fn from(error: EmitError) -> Self {
        Self::Artifact(error)
    }
}

impl From<CompilerModuleHandoffErrorV3> for CompilerModuleHandoffPublicationObjectErrorV1 {
    fn from(error: CompilerModuleHandoffErrorV3) -> Self {
        Self::Handoff(error)
    }
}

/// Move-only export custody for exactly nine already-open publication objects.
///
/// The descriptor vector returned by [`Self::into_descriptors`] is in
/// [`PublicationObjectRoleV1`] discriminant order. This type exposes no borrowed descriptor API,
/// so the only public operation that reveals descriptors transfers ownership of all of them.
pub struct CompilerModuleHandoffPublicationObjectExportV1 {
    expected_root: PublicationObjectRootIdentityV1,
    descriptors: [OwnedFd; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
}

impl fmt::Debug for CompilerModuleHandoffPublicationObjectExportV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleHandoffPublicationObjectExportV1")
            .field("expected_root", &self.expected_root)
            .field("descriptor_count", &self.descriptors.len())
            .finish_non_exhaustive()
    }
}

impl CompilerModuleHandoffPublicationObjectExportV1 {
    /// Returns the exact output-root identity the receiver must compare with remote FD 197.
    pub const fn expected_root(&self) -> PublicationObjectRootIdentityV1 {
        self.expected_root
    }

    /// Moves every descriptor into fixed-order SCM_RIGHTS transport custody.
    pub fn into_descriptors(self) -> Vec<OwnedFd> {
        Vec::from(self.descriptors)
    }
}

/// Move-only validated custody of one exact pre-sidecar strict V3 publication.
///
/// The owner deliberately implements neither `Clone` nor `AsFd` and has no operation that moves
/// out the strict handoff or any descriptor. Its only state transition is fail-closed,
/// descriptor-only revalidation.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::CompilerModuleHandoffPublicationObjectInventoryV1;
///
/// fn cannot_duplicate(inventory: CompilerModuleHandoffPublicationObjectInventoryV1) {
///     let _duplicate = inventory.clone();
/// }
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
///
/// use fe2o3_artifact_transaction::CompilerModuleHandoffPublicationObjectInventoryV1;
///
/// fn cannot_borrow_descriptor(inventory: &CompilerModuleHandoffPublicationObjectInventoryV1) {
///     let _descriptor = inventory.as_fd();
/// }
/// ```
pub struct CompilerModuleHandoffPublicationObjectInventoryV1 {
    expected_root: PublicationObjectRootIdentityV1,
    producer: ProducerIdentity,
    receipt: CompilerModuleHandoffReceiptV3,
    handoff: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    descriptors: [OwnedFd; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
}

impl fmt::Debug for CompilerModuleHandoffPublicationObjectInventoryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerModuleHandoffPublicationObjectInventoryV1")
            .field("expected_root", &self.expected_root)
            .field("attempt", &self.receipt.attempt())
            .field("slot", &self.receipt.slot())
            .field("transaction_identity", &self.receipt.transaction_identity())
            .field("handoff_identity", &self.receipt.handoff_identity())
            .finish_non_exhaustive()
    }
}

impl CompilerModuleHandoffPublicationObjectInventoryV1 {
    /// Returns the inert strict V3 receipt bound to this exact descriptor graph.
    pub const fn receipt(&self) -> CompilerModuleHandoffReceiptV3 {
        self.receipt
    }

    /// Borrows the strictly decoded inert handoff while custody remains retained.
    pub const fn handoff(&self) -> &fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3 {
        &self.handoff
    }

    /// Returns the independently checked output-root identity.
    pub const fn expected_root(&self) -> PublicationObjectRootIdentityV1 {
        self.expected_root
    }

    /// Repeats complete descriptor-only metadata, roster, attempt, byte, digest, and strict decode
    /// authentication while both imported locks remain retained.
    pub fn revalidate(&self) -> Result<(), CompilerModuleHandoffPublicationObjectErrorV1> {
        let validation = validate_inventory(
            &self.descriptors,
            self.expected_root,
            &self.producer,
            self.receipt,
        )?;
        if validation.receipt != self.receipt
            || validation.handoff.identity() != self.handoff.identity()
            || validation.handoff.canonical_bytes() != self.handoff.canonical_bytes()
        {
            return Err(CompilerModuleHandoffPublicationObjectErrorV1::Handoff(
                CompilerModuleHandoffErrorV3::DigestMismatch,
            ));
        }
        Ok(())
    }
}

/// Opens and locks the fixed pre-sidecar object roster as the artifact-store owner.
///
/// This is the only API in the capability that performs pathname resolution. The returned export
/// keeps the output-root flock and named artifact OFD lock alive until all moved descriptions are
/// dropped by the eventual importer.
pub fn export_compiler_module_handoff_publication_objects_v1(
    output_dir: &Path,
    producer: &ProducerIdentity,
    receipt: CompilerModuleHandoffReceiptV3,
) -> Result<
    CompilerModuleHandoffPublicationObjectExportV1,
    CompilerModuleHandoffPublicationObjectErrorV1,
> {
    if receipt.attempt().session() == BuildSession::DIRECT {
        return Err(CompilerModuleHandoffPublicationObjectErrorV1::Attempt {
            reason: "direct compiler attempts cannot enter protected service custody".to_owned(),
        });
    }
    let output = PinnedOutput::open_existing(output_dir)?;
    let mut output_lock = output.lock()?;
    let (producer_name, slot_name) = publication_object_names_v1(producer, receipt.attempt());

    let root_scan = openat(
        &output.fd,
        ".",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let artifact_lock_probe = openat(
        &output.fd,
        LOCK_FILE,
        OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let attempts = openat(
        &output.fd,
        ATTEMPT_FILE,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let producer_directory = openat(
        &output.fd,
        producer_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let slot_directory = openat(
        &producer_directory,
        slot_name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let ready = openat(
        &slot_directory,
        READY_ENTRY,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;
    let module = openat(
        &slot_directory,
        PAYLOAD_ENTRY,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io::Error::from)?;

    let artifact_lock_holder = output_lock.fd.take().ok_or_else(|| {
        CompilerModuleHandoffPublicationObjectErrorV1::InvalidDescriptor {
            role: PublicationObjectRoleV1::ArtifactLockHolder,
            reason: "artifact lock lost its holder descriptor".to_owned(),
        }
    })?;
    let root_flock_holder = output_lock.root_guard.take().ok_or_else(|| {
        CompilerModuleHandoffPublicationObjectErrorV1::InvalidDescriptor {
            role: PublicationObjectRoleV1::OutputRootFlockHolder,
            reason: "output root lost its flock holder descriptor".to_owned(),
        }
    })?;
    drop(output_lock);
    let root_stat = fstat(&root_scan).map_err(io::Error::from)?;
    let expected_root = PublicationObjectRootIdentityV1::from_stat(&root_stat);
    let descriptors = [
        root_scan,
        root_flock_holder,
        artifact_lock_holder,
        artifact_lock_probe,
        attempts,
        producer_directory,
        slot_directory,
        ready,
        module,
    ];
    drop(validate_inventory(
        &descriptors,
        expected_root,
        producer,
        receipt,
    )?);
    Ok(CompilerModuleHandoffPublicationObjectExportV1 {
        expected_root,
        descriptors,
    })
}

/// Imports exactly nine moved descriptions and validates the complete pre-sidecar publication.
///
/// Callers must reject ancillary truncation and non-SCM_RIGHTS control messages before invoking
/// this core. This function independently rejects every descriptor count other than nine and
/// consumes the vector on both success and failure.
pub fn import_compiler_module_handoff_publication_objects_v1(
    descriptors: Vec<OwnedFd>,
    expected_root: PublicationObjectRootIdentityV1,
    producer: &ProducerIdentity,
    receipt: CompilerModuleHandoffReceiptV3,
) -> Result<
    CompilerModuleHandoffPublicationObjectInventoryV1,
    CompilerModuleHandoffPublicationObjectErrorV1,
> {
    let actual = descriptors.len();
    let descriptors: [OwnedFd; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1] = descriptors
        .try_into()
        .map_err(|_| CompilerModuleHandoffPublicationObjectErrorV1::DescriptorCount { actual })?;
    let validation = validate_inventory(&descriptors, expected_root, producer, receipt)?;
    Ok(CompilerModuleHandoffPublicationObjectInventoryV1 {
        expected_root,
        producer: producer.clone(),
        receipt: validation.receipt,
        handoff: validation.handoff,
        descriptors,
    })
}

fn validate_inventory(
    descriptors: &[OwnedFd; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
    expected_root: PublicationObjectRootIdentityV1,
    producer: &ProducerIdentity,
    receipt: CompilerModuleHandoffReceiptV3,
) -> Result<PublicationObjectHandoffValidationV1, CompilerModuleHandoffPublicationObjectErrorV1> {
    if receipt.attempt().session() == BuildSession::DIRECT {
        return Err(CompilerModuleHandoffPublicationObjectErrorV1::Attempt {
            reason: "direct compiler attempts cannot enter protected service custody".to_owned(),
        });
    }
    let first = validate_descriptor_set(descriptors, expected_root, receipt.length())?;
    validate_aliases(&first)?;
    validate_locks(descriptors)?;
    validate_rosters(descriptors, &first, producer, receipt.attempt())?;

    let attempt_bytes = read_exact_file(
        &descriptors[ATTEMPTS_INDEX],
        usize::try_from(first[ATTEMPTS_INDEX].size).map_err(|_| {
            invalid_descriptor(
                PublicationObjectRoleV1::AttemptRegistry,
                "attempt registry has a negative or unrepresentable size",
            )
        })?,
    )?;
    validate_attempt(producer, receipt.attempt(), &attempt_bytes)?;
    let ready_bytes = read_exact_file(
        &descriptors[READY_INDEX],
        publication_object_ready_bytes_v1(),
    )?;
    let module_bytes = read_exact_file(&descriptors[MODULE_INDEX], receipt.length())?;

    let validation = validate_publication_object_handoff_v1(
        producer,
        receipt,
        &ready_bytes,
        &first[MODULE_INDEX].stat,
        module_bytes,
    )?;

    let second = validate_descriptor_set(descriptors, expected_root, receipt.length())?;
    if first != second {
        return Err(invalid_descriptor(
            PublicationObjectRoleV1::OutputRootScan,
            "publication object metadata changed during authentication",
        ));
    }
    validate_aliases(&second)?;
    validate_locks(descriptors)?;
    validate_rosters(descriptors, &second, producer, receipt.attempt())?;
    verify_exact_file(&descriptors[ATTEMPTS_INDEX], &attempt_bytes)?;
    validate_attempt(producer, receipt.attempt(), &attempt_bytes)?;
    verify_exact_file(&descriptors[READY_INDEX], &ready_bytes)?;
    verify_exact_file(
        &descriptors[MODULE_INDEX],
        validation.handoff.canonical_bytes(),
    )?;
    Ok(validation)
}

#[derive(Clone, Copy, Debug)]
struct ValidatedObjectV1 {
    stat: Stat,
    size: i64,
}

impl PartialEq for ValidatedObjectV1 {
    fn eq(&self, other: &Self) -> bool {
        self.stat.st_dev == other.stat.st_dev
            && self.stat.st_ino == other.stat.st_ino
            && self.stat.st_mode == other.stat.st_mode
            && self.stat.st_nlink == other.stat.st_nlink
            && self.stat.st_uid == other.stat.st_uid
            && self.stat.st_gid == other.stat.st_gid
            && self.stat.st_size == other.stat.st_size
            && self.stat.st_mtime == other.stat.st_mtime
            && self.stat.st_mtime_nsec == other.stat.st_mtime_nsec
            && self.stat.st_ctime == other.stat.st_ctime
            && self.stat.st_ctime_nsec == other.stat.st_ctime_nsec
            && self.size == other.size
    }
}

impl Eq for ValidatedObjectV1 {}

fn validate_descriptor_set(
    descriptors: &[OwnedFd; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
    expected_root: PublicationObjectRootIdentityV1,
    module_length: usize,
) -> Result<
    [ValidatedObjectV1; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
    CompilerModuleHandoffPublicationObjectErrorV1,
> {
    let root = validate_descriptor(
        &descriptors[ROOT_SCAN_INDEX],
        PublicationObjectRoleV1::OutputRootScan,
        ExpectedObjectV1::directory(0o700),
    )?;
    if !expected_root.matches(&root.stat) {
        return Err(CompilerModuleHandoffPublicationObjectErrorV1::RootIdentityMismatch);
    }
    if root.stat.st_nlink < 2 {
        return Err(invalid_descriptor(
            PublicationObjectRoleV1::OutputRootScan,
            "output root has an invalid link count",
        ));
    }
    let owner = (root.stat.st_uid, root.stat.st_gid);
    let objects = [
        root,
        validate_descriptor(
            &descriptors[ROOT_FLOCK_HOLDER_INDEX],
            PublicationObjectRoleV1::OutputRootFlockHolder,
            ExpectedObjectV1::directory(0o700),
        )?,
        validate_descriptor(
            &descriptors[ARTIFACT_LOCK_HOLDER_INDEX],
            PublicationObjectRoleV1::ArtifactLockHolder,
            ExpectedObjectV1::regular_read_write(0),
        )?,
        validate_descriptor(
            &descriptors[ARTIFACT_LOCK_PROBE_INDEX],
            PublicationObjectRoleV1::ArtifactLockProbe,
            ExpectedObjectV1::regular_read_write(0),
        )?,
        validate_descriptor(
            &descriptors[ATTEMPTS_INDEX],
            PublicationObjectRoleV1::AttemptRegistry,
            ExpectedObjectV1::bounded_regular(1, MAX_ATTEMPT_BYTES),
        )?,
        validate_descriptor(
            &descriptors[PRODUCER_INDEX],
            PublicationObjectRoleV1::ProducerDirectory,
            ExpectedObjectV1::directory(0o700),
        )?,
        validate_descriptor(
            &descriptors[SLOT_INDEX],
            PublicationObjectRoleV1::ProductionSlotDirectory,
            ExpectedObjectV1::directory(0o700),
        )?,
        validate_descriptor(
            &descriptors[READY_INDEX],
            PublicationObjectRoleV1::ReadyRecord,
            ExpectedObjectV1::exact_regular(publication_object_ready_bytes_v1()),
        )?,
        validate_descriptor(
            &descriptors[MODULE_INDEX],
            PublicationObjectRoleV1::ModulePayload,
            ExpectedObjectV1::exact_bounded_regular(
                module_length,
                MAX_COMPILER_MODULE_HANDOFF_BYTES_V3,
            ),
        )?,
    ];
    for role in PublicationObjectRoleV1::ALL {
        let object = &objects[role.index()];
        if object.stat.st_dev != root.stat.st_dev {
            return Err(invalid_descriptor(
                role,
                "object is on a different filesystem",
            ));
        }
        if (object.stat.st_uid, object.stat.st_gid) != owner {
            return Err(invalid_descriptor(
                role,
                "object owner does not match the output root owner",
            ));
        }
    }
    if !same_inode(&objects[ROOT_SCAN_INDEX], &objects[ROOT_FLOCK_HOLDER_INDEX])
        || objects[ROOT_SCAN_INDEX].stat.st_nlink != objects[ROOT_FLOCK_HOLDER_INDEX].stat.st_nlink
    {
        return Err(
            CompilerModuleHandoffPublicationObjectErrorV1::InvalidAlias {
                left: PublicationObjectRoleV1::OutputRootScan,
                right: PublicationObjectRoleV1::OutputRootFlockHolder,
            },
        );
    }
    if objects[PRODUCER_INDEX].stat.st_nlink != 3 {
        return Err(invalid_descriptor(
            PublicationObjectRoleV1::ProducerDirectory,
            "producer directory must contain exactly one child directory",
        ));
    }
    if objects[SLOT_INDEX].stat.st_nlink != 2 {
        return Err(invalid_descriptor(
            PublicationObjectRoleV1::ProductionSlotDirectory,
            "Production slot must not contain child directories",
        ));
    }
    Ok(objects)
}

#[derive(Clone, Copy)]
struct ExpectedObjectV1 {
    file_type: FileType,
    access: i32,
    mode: u32,
    minimum_size: usize,
    maximum_size: usize,
    exact_size: Option<usize>,
    single_link: bool,
}

impl ExpectedObjectV1 {
    const fn directory(mode: u32) -> Self {
        Self {
            file_type: FileType::Directory,
            access: libc::O_RDONLY,
            mode,
            minimum_size: 0,
            maximum_size: usize::MAX,
            exact_size: None,
            single_link: false,
        }
    }

    const fn regular_read_write(exact_size: usize) -> Self {
        Self {
            file_type: FileType::RegularFile,
            access: libc::O_RDWR,
            mode: 0o600,
            minimum_size: exact_size,
            maximum_size: exact_size,
            exact_size: Some(exact_size),
            single_link: true,
        }
    }

    const fn bounded_regular(minimum_size: usize, maximum_size: usize) -> Self {
        Self {
            file_type: FileType::RegularFile,
            access: libc::O_RDONLY,
            mode: 0o600,
            minimum_size,
            maximum_size,
            exact_size: None,
            single_link: true,
        }
    }

    const fn exact_regular(exact_size: usize) -> Self {
        Self::exact_bounded_regular(exact_size, exact_size)
    }

    const fn exact_bounded_regular(exact_size: usize, maximum_size: usize) -> Self {
        Self {
            file_type: FileType::RegularFile,
            access: libc::O_RDONLY,
            mode: 0o600,
            minimum_size: 1,
            maximum_size,
            exact_size: Some(exact_size),
            single_link: true,
        }
    }
}

fn validate_descriptor(
    descriptor: &OwnedFd,
    role: PublicationObjectRoleV1,
    expected: ExpectedObjectV1,
) -> Result<ValidatedObjectV1, CompilerModuleHandoffPublicationObjectErrorV1> {
    let descriptor_flags = fcntl_get(descriptor, libc::F_GETFD)?;
    if descriptor_flags & libc::FD_CLOEXEC == 0 {
        return Err(invalid_descriptor(role, "FD_CLOEXEC is not set"));
    }
    let status_flags = fcntl_get(descriptor, libc::F_GETFL)?;
    #[cfg(target_os = "linux")]
    if status_flags & libc::O_PATH != 0 {
        return Err(invalid_descriptor(role, "O_PATH is forbidden"));
    }
    if status_flags & libc::O_ACCMODE != expected.access {
        return Err(invalid_descriptor(role, "descriptor access mode is wrong"));
    }
    let stat = fstat(descriptor).map_err(io::Error::from)?;
    if FileType::from_raw_mode(stat.st_mode) != expected.file_type {
        return Err(invalid_descriptor(
            role,
            "descriptor has the wrong object type",
        ));
    }
    if stat.st_mode & 0o7777 != expected.mode {
        return Err(invalid_descriptor(
            role,
            "descriptor has the wrong permission mode",
        ));
    }
    if expected.single_link && stat.st_nlink != 1 {
        return Err(invalid_descriptor(
            role,
            "regular object must have exactly one link",
        ));
    }
    let size = usize::try_from(stat.st_size)
        .map_err(|_| invalid_descriptor(role, "object size is negative or unrepresentable"))?;
    if size < expected.minimum_size || size > expected.maximum_size {
        return Err(invalid_descriptor(
            role,
            "object size is outside its fixed bound",
        ));
    }
    if expected.exact_size.is_some_and(|exact| size != exact) {
        return Err(invalid_descriptor(role, "object size is not exact"));
    }
    Ok(ValidatedObjectV1 {
        stat,
        size: stat.st_size,
    })
}

fn validate_aliases(
    objects: &[ValidatedObjectV1; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
) -> Result<(), CompilerModuleHandoffPublicationObjectErrorV1> {
    for left in 0..objects.len() {
        for right in left + 1..objects.len() {
            if !same_inode(&objects[left], &objects[right]) {
                continue;
            }
            let intentional = (left == ROOT_SCAN_INDEX && right == ROOT_FLOCK_HOLDER_INDEX)
                || (left == ARTIFACT_LOCK_HOLDER_INDEX && right == ARTIFACT_LOCK_PROBE_INDEX);
            if !intentional {
                return Err(
                    CompilerModuleHandoffPublicationObjectErrorV1::InvalidAlias {
                        left: PublicationObjectRoleV1::ALL[left],
                        right: PublicationObjectRoleV1::ALL[right],
                    },
                );
            }
        }
    }
    if !same_inode(
        &objects[ARTIFACT_LOCK_HOLDER_INDEX],
        &objects[ARTIFACT_LOCK_PROBE_INDEX],
    ) {
        return Err(
            CompilerModuleHandoffPublicationObjectErrorV1::InvalidAlias {
                left: PublicationObjectRoleV1::ArtifactLockHolder,
                right: PublicationObjectRoleV1::ArtifactLockProbe,
            },
        );
    }
    Ok(())
}

fn same_inode(left: &ValidatedObjectV1, right: &ValidatedObjectV1) -> bool {
    left.stat.st_dev == right.stat.st_dev && left.stat.st_ino == right.stat.st_ino
}

fn validate_locks(
    descriptors: &[OwnedFd; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
) -> Result<(), CompilerModuleHandoffPublicationObjectErrorV1> {
    // Probe with the compatible weak mode first. Success proves that no exclusive lock exists, so
    // reject before an exclusive operation on a weak designated holder could upgrade its lock.
    match acquire_linux_descriptor_shared_flock(&descriptors[ROOT_SCAN_INDEX]) {
        Ok(false) => {}
        Ok(true) => {
            unlock_flock(&descriptors[ROOT_SCAN_INDEX])?;
            return Err(CompilerModuleHandoffPublicationObjectErrorV1::LockNotHeld {
                role: PublicationObjectRoleV1::OutputRootFlockHolder,
            });
        }
        Err(error) => return Err(error.into()),
    }
    // An exclusive operation is idempotent only on the designated holder. An exclusive lock held
    // by an external open description instead conflicts here and is rejected as a decoy.
    if !acquire_linux_descriptor_flock(&descriptors[ROOT_FLOCK_HOLDER_INDEX], true)? {
        return Err(CompilerModuleHandoffPublicationObjectErrorV1::LockNotHeld {
            role: PublicationObjectRoleV1::OutputRootFlockHolder,
        });
    }

    match acquire_linux_ofd_read_lock(&descriptors[ARTIFACT_LOCK_PROBE_INDEX]) {
        Ok(false) => {}
        Ok(true) => {
            unlock_ofd(&descriptors[ARTIFACT_LOCK_PROBE_INDEX])?;
            return Err(CompilerModuleHandoffPublicationObjectErrorV1::LockNotHeld {
                role: PublicationObjectRoleV1::ArtifactLockHolder,
            });
        }
        Err(error) => return Err(error.into()),
    }
    if !acquire_linux_ofd_exclusive_lock(&descriptors[ARTIFACT_LOCK_HOLDER_INDEX], true)? {
        return Err(CompilerModuleHandoffPublicationObjectErrorV1::LockNotHeld {
            role: PublicationObjectRoleV1::ArtifactLockHolder,
        });
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn acquire_linux_descriptor_shared_flock(descriptor: &OwnedFd) -> io::Result<bool> {
    loop {
        // SAFETY: the descriptor is live and LOCK_SH | LOCK_NB is a valid flock operation.
        if unsafe { libc::flock(descriptor.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(libc::EACCES) | Some(libc::EAGAIN) => return Ok(false),
            Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::EOPNOTSUPP) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "publication-object custody requires Linux descriptor-owned directory flock support",
                ));
            }
            _ => return Err(error),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn acquire_linux_descriptor_shared_flock(_descriptor: &OwnedFd) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "publication-object custody requires Linux descriptor-owned directory flock support",
    ))
}

#[cfg(target_os = "linux")]
fn acquire_linux_ofd_read_lock(descriptor: &OwnedFd) -> io::Result<bool> {
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_RDLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    loop {
        // SAFETY: the descriptor is live and `lock` is a fully initialized whole-file read lock.
        if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_OFD_SETLK, &lock) } == 0 {
            return Ok(true);
        }
        let error = io::Error::last_os_error();
        match error.raw_os_error() {
            Some(libc::EINTR) => continue,
            Some(libc::EACCES) | Some(libc::EAGAIN) => return Ok(false),
            Some(libc::EINVAL) | Some(libc::ENOSYS) | Some(libc::EOPNOTSUPP) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "publication-object custody requires Linux open-file-description lock support",
                ));
            }
            _ => return Err(error),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn acquire_linux_ofd_read_lock(_descriptor: &OwnedFd) -> io::Result<bool> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "publication-object custody requires Linux open-file-description lock support",
    ))
}

fn validate_rosters(
    descriptors: &[OwnedFd; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
    objects: &[ValidatedObjectV1; COMPILER_MODULE_HANDOFF_PUBLICATION_OBJECT_FDS_V1],
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
) -> Result<(), CompilerModuleHandoffPublicationObjectErrorV1> {
    let (producer_name, slot_name) = publication_object_names_v1(producer, attempt);
    let root_entries = scan_directory(
        &descriptors[ROOT_SCAN_INDEX],
        PublicationObjectRoleV1::OutputRootScan,
        MAX_OUTPUT_ENTRIES,
    )?;
    require_entry(
        &root_entries,
        LOCK_FILE.as_bytes(),
        &objects[ARTIFACT_LOCK_HOLDER_INDEX],
        libc::DT_REG,
        PublicationObjectRoleV1::OutputRootScan,
    )?;
    require_entry(
        &root_entries,
        ATTEMPT_FILE.as_bytes(),
        &objects[ATTEMPTS_INDEX],
        libc::DT_REG,
        PublicationObjectRoleV1::OutputRootScan,
    )?;
    require_entry(
        &root_entries,
        producer_name.as_bytes(),
        &objects[PRODUCER_INDEX],
        libc::DT_DIR,
        PublicationObjectRoleV1::OutputRootScan,
    )?;

    let producer_entries = scan_directory(
        &descriptors[PRODUCER_INDEX],
        PublicationObjectRoleV1::ProducerDirectory,
        MAX_PRODUCER_ENTRIES_V1,
    )?;
    if producer_entries.len() != 1 {
        return Err(invalid_roster(
            PublicationObjectRoleV1::ProducerDirectory,
            "producer directory must contain only the exact Production slot",
        ));
    }
    require_entry(
        &producer_entries,
        slot_name.as_bytes(),
        &objects[SLOT_INDEX],
        libc::DT_DIR,
        PublicationObjectRoleV1::ProducerDirectory,
    )?;

    let slot_entries = scan_directory(
        &descriptors[SLOT_INDEX],
        PublicationObjectRoleV1::ProductionSlotDirectory,
        MAX_SLOT_ENTRIES,
    )?;
    if slot_entries.len() != 2 {
        return Err(invalid_roster(
            PublicationObjectRoleV1::ProductionSlotDirectory,
            "pre-sidecar Production slot must contain exactly ready and module",
        ));
    }
    require_entry(
        &slot_entries,
        READY_ENTRY.as_bytes(),
        &objects[READY_INDEX],
        libc::DT_REG,
        PublicationObjectRoleV1::ProductionSlotDirectory,
    )?;
    require_entry(
        &slot_entries,
        PAYLOAD_ENTRY.as_bytes(),
        &objects[MODULE_INDEX],
        libc::DT_REG,
        PublicationObjectRoleV1::ProductionSlotDirectory,
    )?;
    Ok(())
}

#[derive(Clone, Copy)]
struct DirectoryEntryV1 {
    inode: u64,
    file_type: u8,
}

fn scan_directory(
    descriptor: &OwnedFd,
    role: PublicationObjectRoleV1,
    maximum_entries: usize,
) -> Result<BTreeMap<Vec<u8>, DirectoryEntryV1>, CompilerModuleHandoffPublicationObjectErrorV1> {
    seek_directory_start(descriptor)?;
    let mut entries = BTreeMap::new();
    let mut buffer = [0_u8; DIRECTORY_SCAN_BUFFER_BYTES_V1];
    loop {
        let count = getdents64(descriptor, &mut buffer)?;
        if count == 0 {
            break;
        }
        let mut offset = 0usize;
        while offset < count {
            if count - offset < LINUX_DIRENT64_FIXED_BYTES {
                return Err(invalid_roster(role, "truncated getdents64 record"));
            }
            let record = &buffer[offset..count];
            let inode = u64::from_ne_bytes(record[0..8].try_into().expect("fixed inode field"));
            let record_length = usize::from(u16::from_ne_bytes([record[16], record[17]]));
            if record_length < LINUX_DIRENT64_FIXED_BYTES || record_length > record.len() {
                return Err(invalid_roster(role, "invalid getdents64 record length"));
            }
            let file_type = record[18];
            let name_region = &record[LINUX_DIRENT64_FIXED_BYTES..record_length];
            let name_end = name_region
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| invalid_roster(role, "directory entry has no NUL terminator"))?;
            let name = &name_region[..name_end];
            if name.is_empty() {
                return Err(invalid_roster(role, "directory entry has an empty name"));
            }
            if name != b"." && name != b".." {
                if inode == 0 {
                    return Err(invalid_roster(role, "directory entry has a zero inode"));
                }
                if entries.len() == maximum_entries {
                    return Err(invalid_roster(role, "directory exceeds its entry bound"));
                }
                if entries
                    .insert(name.to_vec(), DirectoryEntryV1 { inode, file_type })
                    .is_some()
                {
                    return Err(invalid_roster(role, "directory contains a duplicate name"));
                }
            }
            offset = offset
                .checked_add(record_length)
                .ok_or_else(|| invalid_roster(role, "directory record offset overflowed"))?;
        }
    }
    Ok(entries)
}

fn require_entry(
    entries: &BTreeMap<Vec<u8>, DirectoryEntryV1>,
    name: &[u8],
    object: &ValidatedObjectV1,
    expected_type: u8,
    role: PublicationObjectRoleV1,
) -> Result<(), CompilerModuleHandoffPublicationObjectErrorV1> {
    let entry = entries
        .get(name)
        .ok_or_else(|| invalid_roster(role, "required entry is missing"))?;
    if entry.inode != object.stat.st_ino {
        return Err(invalid_roster(
            role,
            "required entry does not name the transferred inode",
        ));
    }
    if entry.file_type != libc::DT_UNKNOWN && entry.file_type != expected_type {
        return Err(invalid_roster(role, "required entry has the wrong type"));
    }
    Ok(())
}

fn validate_attempt(
    producer: &ProducerIdentity,
    attempt: BuildAttempt,
    bytes: &[u8],
) -> Result<(), CompilerModuleHandoffPublicationObjectErrorV1> {
    let registry = AttemptRegistry::decode(bytes).map_err(|error| {
        CompilerModuleHandoffPublicationObjectErrorV1::Attempt {
            reason: error.to_string(),
        }
    })?;
    let record = registry
        .record_exact(&producer.stable_source, attempt)
        .map_err(
            |error| CompilerModuleHandoffPublicationObjectErrorV1::Attempt {
                reason: error.to_string(),
            },
        )?;
    if record.crate_name != producer.crate_name
        || record.phase != AttemptPhase::Building
        || record.backend_receipt.is_some()
        || record.generation != attempt.generation()
        || record.session != attempt.session()
        || record.invocation != attempt.invocation()
    {
        return Err(CompilerModuleHandoffPublicationObjectErrorV1::Attempt {
            reason: "attempt is not the exact frontend-owned building generation without a backend receipt"
                .to_owned(),
        });
    }
    Ok(())
}

fn read_exact_file(
    descriptor: &OwnedFd,
    exact_length: usize,
) -> Result<Vec<u8>, CompilerModuleHandoffPublicationObjectErrorV1> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(exact_length).map_err(|_| {
        CompilerModuleHandoffPublicationObjectErrorV1::Io(io::Error::other(format!(
            "could not reserve {exact_length} bytes for descriptor-only validation"
        )))
    })?;
    bytes.resize(exact_length, 0);
    pread_complete(descriptor, &mut bytes, 0)?;
    let mut trailing = [0_u8; 1];
    if pread_once(descriptor, &mut trailing, exact_length as u64)? != 0 {
        return Err(io::Error::other("descriptor grew beyond its authenticated length").into());
    }
    Ok(bytes)
}

fn verify_exact_file(
    descriptor: &OwnedFd,
    expected: &[u8],
) -> Result<(), CompilerModuleHandoffPublicationObjectErrorV1> {
    let mut offset = 0usize;
    let mut buffer = [0_u8; FILE_COMPARE_BUFFER_BYTES_V1];
    while offset < expected.len() {
        let count = (expected.len() - offset).min(buffer.len());
        pread_complete(descriptor, &mut buffer[..count], offset as u64)?;
        if buffer[..count] != expected[offset..offset + count] {
            return Err(io::Error::other("descriptor bytes changed during authentication").into());
        }
        offset += count;
    }
    let mut trailing = [0_u8; 1];
    if pread_once(descriptor, &mut trailing, expected.len() as u64)? != 0 {
        return Err(io::Error::other("descriptor grew during authentication").into());
    }
    Ok(())
}

fn pread_complete(descriptor: &OwnedFd, bytes: &mut [u8], offset: u64) -> io::Result<()> {
    let mut completed = 0usize;
    while completed < bytes.len() {
        let completed_offset = offset
            .checked_add(completed as u64)
            .ok_or_else(|| io::Error::other("pread offset overflowed"))?;
        let read = pread_once(descriptor, &mut bytes[completed..], completed_offset)?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "descriptor ended before its authenticated length",
            ));
        }
        completed += read;
    }
    Ok(())
}

fn pread_once(descriptor: &OwnedFd, bytes: &mut [u8], offset: u64) -> io::Result<usize> {
    let offset = libc::off_t::try_from(offset)
        .map_err(|_| io::Error::other("pread offset is out of range"))?;
    loop {
        // SAFETY: the live descriptor is borrowed for the call and `bytes` is a writable slice.
        let result = unsafe {
            libc::pread(
                descriptor.as_raw_fd(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                offset,
            )
        };
        if result >= 0 {
            return Ok(result as usize);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn fcntl_get(descriptor: &OwnedFd, command: i32) -> io::Result<i32> {
    loop {
        // SAFETY: the command is one of the argument-free F_GETFD/F_GETFL operations.
        let result = unsafe { libc::fcntl(descriptor.as_raw_fd(), command) };
        if result >= 0 {
            return Ok(result);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

fn unlock_flock(descriptor: &OwnedFd) -> io::Result<()> {
    loop {
        // SAFETY: the descriptor is live and LOCK_UN is a valid flock operation.
        if unsafe { libc::flock(descriptor.as_raw_fd(), libc::LOCK_UN) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(target_os = "linux")]
fn unlock_ofd(descriptor: &OwnedFd) -> io::Result<()> {
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_UNLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    loop {
        // SAFETY: the descriptor is live and `lock` is a fully initialized whole-file unlock.
        if unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_OFD_SETLK, &lock) } == 0 {
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn unlock_ofd(_descriptor: &OwnedFd) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "publication-object OFD locks require Linux",
    ))
}

fn seek_directory_start(descriptor: &OwnedFd) -> io::Result<()> {
    loop {
        // SAFETY: the descriptor is live and SEEK_SET with offset zero rewinds a directory stream.
        let result = unsafe { libc::lseek(descriptor.as_raw_fd(), 0, libc::SEEK_SET) };
        if result == 0 {
            return Ok(());
        }
        if result > 0 {
            return Err(io::Error::other(
                "directory rewind returned a nonzero offset",
            ));
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(target_os = "linux")]
fn getdents64(descriptor: &OwnedFd, bytes: &mut [u8]) -> io::Result<usize> {
    loop {
        // SAFETY: the descriptor is a validated directory and `bytes` is writable for its length.
        let result = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                descriptor.as_raw_fd(),
                bytes.as_mut_ptr(),
                bytes.len(),
            )
        };
        if result >= 0 {
            return Ok(result as usize);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn getdents64(_descriptor: &OwnedFd, _bytes: &mut [u8]) -> io::Result<usize> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "publication-object directory scans require Linux getdents64",
    ))
}

fn invalid_descriptor(
    role: PublicationObjectRoleV1,
    reason: impl Into<String>,
) -> CompilerModuleHandoffPublicationObjectErrorV1 {
    CompilerModuleHandoffPublicationObjectErrorV1::InvalidDescriptor {
        role,
        reason: reason.into(),
    }
}

fn invalid_roster(
    role: PublicationObjectRoleV1,
    reason: impl Into<String>,
) -> CompilerModuleHandoffPublicationObjectErrorV1 {
    CompilerModuleHandoffPublicationObjectErrorV1::InvalidRoster {
        role,
        reason: reason.into(),
    }
}
