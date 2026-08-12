//! Cooperative-process Cargo-to-application handoff for canonical Worker V2 evidence.
//!
//! This module is an inert foundation, not a same-process security boundary. The application must
//! consume the inherited handoff before threads, signal handlers, descendants, or unrelated FD
//! mutation can race it. A malicious application in the same process can bypass these cooperative
//! assumptions and must instead be isolated from authority by a separate broker.

use crate::recovered_worker_v2_admission::recover_worker_v2_load_envelope_v1;
use crate::{
    KernelId, ObservedContext, RecoveredWorkerV2AdmissionError, RecoveredWorkerV2PinnedDescriptorV1,
};
use fe2o3_worker_v2_bundle::{
    ApplicationHandoffProtocolErrorV1, CompilerTransactionEvidenceCapsuleV2,
    MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1, WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1, WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1, WorkerV2ApplicationHandoffChallengeV1,
    WorkerV2ApplicationHandoffCommitmentV1, WorkerV2ApplicationHandoffExpectationV1,
    WorkerV2ApplicationIdentityV1, WorkerV2LoadEnvelopeV1, worker_v2_load_envelope_name_v1,
};
use rustix::fs::{FileType, OFlags, fcntl_getfl, fcntl_setfl, fstat};
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const MAX_APPLICATION_EXECUTABLE_BYTES_V1: u64 = 1 << 30;
const ACK_DEADLINE_V1: Duration = Duration::from_secs(5);

static INHERITED_HANDOFF_CLAIMED_V1: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DescriptorIdentityV1 {
    device: u64,
    inode: u64,
}

impl DescriptorIdentityV1 {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectorySnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
}

impl DirectorySnapshotV1 {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            owner: stat.st_uid,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EnvelopeSnapshotV1 {
    device: u64,
    inode: u64,
    mode: u32,
    owner: u32,
    links: u64,
    size: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl EnvelopeSnapshotV1 {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            owner: stat.st_uid,
            links: stat.st_nlink,
            size: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }
}

struct InspectedEnvelopeV1 {
    snapshot: EnvelopeSnapshotV1,
    exact_bytes: Box<[u8]>,
    decoded: WorkerV2LoadEnvelopeV1,
    canonical_name: String,
}

/// Exact inherited descriptors retained through recovered load and synchronous dispatch.
pub(crate) struct RetainedWorkerV2ApplicationDescriptorsV1 {
    directory: File,
    envelope: File,
    directory_snapshot: DirectorySnapshotV1,
    envelope_snapshot: EnvelopeSnapshotV1,
    envelope_name: String,
    exact_envelope_bytes: Box<[u8]>,
    expectation: WorkerV2ApplicationHandoffExpectationV1,
    challenge: WorkerV2ApplicationHandoffChallengeV1,
}

impl fmt::Debug for RetainedWorkerV2ApplicationDescriptorsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedWorkerV2ApplicationDescriptorsV1")
            .field("directory", &self.directory_snapshot)
            .field("envelope", &self.envelope_snapshot)
            .field("envelope_name", &self.envelope_name)
            .field("commitment", &self.expectation.commitment())
            .field("challenge", &self.challenge)
            .finish_non_exhaustive()
    }
}

impl RetainedWorkerV2ApplicationDescriptorsV1 {
    pub(crate) fn revalidate(&self) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
        validate_directory(&self.directory, self.directory_snapshot)?;
        validate_envelope(
            &self.directory,
            &self.envelope,
            self.envelope_snapshot,
            &self.exact_envelope_bytes,
            &self.envelope_name,
        )
    }
}

/// Consumes the inherited descriptor environment installed by the Cargo application runner.
///
/// The five environment values are removed before parsing, including on rejection. This one-shot
/// startup function must be called before the application creates threads or descendants. The
/// inherited descriptor numbers are claimed as owned descriptors immediately; no caller-selected
/// filesystem path participates in recovery. The envelope and artifact-directory descriptors
/// remain retained by the returned linear authority through load, generated preparation,
/// synchronous HSA dispatch, and unload. The emitted ACK is reproducible liveness data and grants
/// no recovery, load, or launch authority. The output remains non-production evidence until real
/// compiler and prerequisite issuers replace the explicitly unsafe caller-supplied boundary.
///
/// # Safety
///
/// The caller must invoke this startup operation before creating any threads, installing signal
/// handlers that can access the environment or descriptor table, spawning descendants, or
/// allowing any unrelated close, duplicate, flag change, or mutation of handoff FDs. Rust cannot
/// synchronize environment or descriptor-table mutation with foreign code, signal handlers, or
/// independently managed threads. A hostile same-process caller violates this contract.
pub unsafe fn consume_inherited_worker_v2_application_handoff_v1(
    compiler_transaction: CompilerTransactionEvidenceCapsuleV2,
    kernel_id: KernelId,
    observed: &ObservedContext,
) -> Result<RecoveredWorkerV2PinnedDescriptorV1, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let environment = take_inherited_environment();
    if INHERITED_HANDOFF_CLAIMED_V1
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::AlreadyConsumed);
    }

    let envelope_raw = environment_fd(
        WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
        environment[0].as_deref(),
    );
    let directory_raw = environment_fd(
        WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        environment[1].as_deref(),
    );
    let ack_raw = environment_fd(
        WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        environment[3].as_deref(),
    );
    let (envelope_raw, directory_raw, ack_raw) = match (envelope_raw, directory_raw, ack_raw) {
        (Ok(envelope), Ok(directory), Ok(acknowledgment)) => (envelope, directory, acknowledgment),
        (envelope, directory, acknowledgment) => {
            close_available_handoff_descriptors([
                envelope.as_ref().ok().copied(),
                directory.as_ref().ok().copied(),
                acknowledgment.as_ref().ok().copied(),
            ]);
            return Err(envelope
                .err()
                .or_else(|| directory.err())
                .or_else(|| acknowledgment.err())
                .expect("the non-success branch contains an error"));
        }
    };
    if envelope_raw == directory_raw || envelope_raw == ack_raw || directory_raw == ack_raw {
        close_aliased_handoff_descriptors(envelope_raw, directory_raw, ack_raw);
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::AliasedDescriptors);
    }
    let envelope = claim_inherited_descriptor(envelope_raw, "envelope");
    let directory = claim_inherited_descriptor(directory_raw, "artifact directory");
    let acknowledgment = claim_inherited_descriptor(ack_raw, "acknowledgment");
    let envelope = envelope?;
    let directory = directory?;
    let acknowledgment = acknowledgment?;
    let descriptor_identities =
        snapshot_handoff_descriptor_identities(&directory, &envelope, &acknowledgment)?;
    seal_descriptor_occurrences(&descriptor_identities)?;

    let commitment = environment_text(
        WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        environment[2].as_deref(),
    )
    .and_then(|value| {
        WorkerV2ApplicationHandoffCommitmentV1::from_hex(&value)
            .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::Protocol)
    })?;
    let challenge = environment_text(
        WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
        environment[4].as_deref(),
    )
    .and_then(|value| {
        WorkerV2ApplicationHandoffChallengeV1::from_hex(&value)
            .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::Protocol)
    })?;

    consume_worker_v2_application_handoff_descriptors_v1(
        envelope,
        directory,
        acknowledgment,
        commitment,
        challenge,
        compiler_transaction,
        kernel_id,
        observed,
    )
}

/// Consumes exact inherited descriptors without accepting an artifact path or envelope bytes.
///
/// This is the descriptor-level counterpart to
/// [`consume_inherited_worker_v2_application_handoff_v1`]. The function takes ownership of caller
/// supplied descriptor duplicates, validates the shared Cargo commitment against the current
/// application image, reacquires the durable publication lease through the pinned directory
/// descriptor, emits one canonical bounded liveness ACK, and retains the read descriptors. The
/// ACK contains no secret and grants no recovery, load, or launch authority; only the returned
/// non-forgeable descriptor carries the revalidated lease into later host transitions.
#[allow(clippy::too_many_arguments)]
pub(crate) fn consume_worker_v2_application_handoff_descriptors_v1(
    envelope: OwnedFd,
    artifact_directory: OwnedFd,
    acknowledgment: OwnedFd,
    commitment: WorkerV2ApplicationHandoffCommitmentV1,
    challenge: WorkerV2ApplicationHandoffChallengeV1,
    compiler_transaction: CompilerTransactionEvidenceCapsuleV2,
    kernel_id: KernelId,
    observed: &ObservedContext,
) -> Result<RecoveredWorkerV2PinnedDescriptorV1, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let envelope_cloexec = set_close_on_exec(&envelope, "envelope");
    let directory_cloexec = set_close_on_exec(&artifact_directory, "artifact directory");
    let acknowledgment_cloexec = set_close_on_exec(&acknowledgment, "acknowledgment");
    envelope_cloexec?;
    directory_cloexec?;
    acknowledgment_cloexec?;
    let directory = File::from(artifact_directory);
    let envelope = File::from(envelope);
    let acknowledgment = File::from(acknowledgment);
    let descriptor_identities =
        snapshot_handoff_descriptor_identities(&directory, &envelope, &acknowledgment)?;
    seal_descriptor_occurrences(&descriptor_identities)?;
    let directory_snapshot = inspect_directory(&directory)?;
    let InspectedEnvelopeV1 {
        snapshot: envelope_snapshot,
        exact_bytes: exact_envelope_bytes,
        decoded,
        canonical_name: envelope_name,
    } = inspect_envelope(&directory, &envelope)?;
    let application = current_application_identity()?;
    let expectation = WorkerV2ApplicationHandoffExpectationV1::new(&decoded, application);
    validate_application_commitment(expectation.commitment(), commitment)?;

    let retained = RetainedWorkerV2ApplicationDescriptorsV1 {
        directory,
        envelope,
        directory_snapshot,
        envelope_snapshot,
        envelope_name,
        exact_envelope_bytes,
        expectation,
        challenge,
    };
    retained.revalidate()?;
    let output = descriptor_directory_path(&retained.directory);
    let recovered = recover_worker_v2_load_envelope_v1(
        &output,
        &retained.exact_envelope_bytes,
        compiler_transaction,
        kernel_id,
        observed,
    )
    .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::Recovery)?;
    seal_descriptor_occurrences(&descriptor_identities)?;
    retained.revalidate()?;
    recovered
        .revalidate_currentness()
        .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::RecoveryCurrentness)?;
    emit_acknowledgment(&acknowledgment, expectation.acknowledgment(challenge))?;
    Ok(recovered.retain_application_descriptors(retained))
}

fn validate_application_commitment(
    expected: WorkerV2ApplicationHandoffCommitmentV1,
    supplied: WorkerV2ApplicationHandoffCommitmentV1,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    if expected == supplied {
        Ok(())
    } else {
        Err(WorkerV2ApplicationDescriptorHandoffErrorV1::CommitmentMismatch)
    }
}

fn handoff_environment_names() -> [&'static str; 5] {
    [
        WORKER_V2_APPLICATION_ENVELOPE_FD_ENV_V1,
        WORKER_V2_APPLICATION_ARTIFACT_DIR_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_COMMITMENT_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_ACK_FD_ENV_V1,
        WORKER_V2_APPLICATION_HANDOFF_CHALLENGE_ENV_V1,
    ]
}

fn take_inherited_environment() -> [Option<OsString>; 5] {
    let names = handoff_environment_names();
    let values = names.map(std::env::var_os);
    // SAFETY: the public function's startup contract excludes concurrent environment access.
    // Removing all handoff values prevents later descendants from inheriting stale descriptor
    // numbers or reproducible protocol material.
    unsafe {
        for name in names {
            std::env::remove_var(name);
        }
    }
    values
}

fn environment_text(
    name: &'static str,
    value: Option<&OsStr>,
) -> Result<String, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let value = value
        .ok_or(WorkerV2ApplicationDescriptorHandoffErrorV1::MissingEnvironment(name))?
        .to_os_string();
    let value = value
        .into_string()
        .map_err(|_| WorkerV2ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))?;
    Ok(value)
}

fn environment_fd(
    name: &'static str,
    value: Option<&OsStr>,
) -> Result<RawFd, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let value = environment_text(name, value)?;
    let canonical = value == "0"
        || value
            .as_bytes()
            .first()
            .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
            && value.as_bytes().iter().all(u8::is_ascii_digit);
    let descriptor = canonical
        .then(|| value.parse::<RawFd>().ok())
        .flatten()
        .filter(|descriptor| *descriptor > libc::STDERR_FILENO)
        .ok_or(WorkerV2ApplicationDescriptorHandoffErrorV1::InvalidEnvironment(name))?;
    Ok(descriptor)
}

fn claim_inherited_descriptor(
    descriptor: RawFd,
    kind: &'static str,
) -> Result<OwnedFd, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    // Raw `fcntl` rejects stale integers without manufacturing a `BorrowedFd` first.
    let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
    if flags < 0 {
        return Err(descriptor_io(kind, io::Error::last_os_error()));
    }
    // SAFETY: successful `F_GETFD` proved that the one-shot protocol's transferred descriptor is
    // live. This function is its sole owner after the environment has been consumed.
    let descriptor = unsafe { OwnedFd::from_raw_fd(descriptor) };
    set_close_on_exec(&descriptor, kind)?;
    if flags & libc::FD_CLOEXEC != 0 {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::DescriptorFlags(kind));
    }
    Ok(descriptor)
}

fn close_aliased_handoff_descriptors(envelope: RawFd, directory: RawFd, acknowledgment: RawFd) {
    close_available_handoff_descriptors([Some(envelope), Some(directory), Some(acknowledgment)]);
}

fn close_available_handoff_descriptors(descriptors: [Option<RawFd>; 3]) {
    for (index, descriptor) in descriptors.into_iter().enumerate() {
        let Some(descriptor) = descriptor else {
            continue;
        };
        if descriptors[..index].contains(&Some(descriptor)) {
            continue;
        }
        let _ = claim_inherited_descriptor(descriptor, "rejected handoff");
    }
}

fn set_close_on_exec(
    descriptor: &OwnedFd,
    kind: &'static str,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let flags = unsafe { libc::fcntl(descriptor.as_raw_fd(), libc::F_GETFD) };
    if flags < 0 {
        return Err(descriptor_io(kind, io::Error::last_os_error()));
    }
    if unsafe {
        libc::fcntl(
            descriptor.as_raw_fd(),
            libc::F_SETFD,
            flags | libc::FD_CLOEXEC,
        )
    } < 0
    {
        return Err(descriptor_io(kind, io::Error::last_os_error()));
    }
    Ok(())
}

fn snapshot_handoff_descriptor_identities<D: AsFd, E: AsFd, A: AsFd>(
    directory: &D,
    envelope: &E,
    acknowledgment: &A,
) -> Result<[DescriptorIdentityV1; 3], WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let directory = fstat(directory)
        .map(|stat| DescriptorIdentityV1::from_stat(&stat))
        .map_err(|error| descriptor_io("artifact directory identity", error));
    let envelope = fstat(envelope)
        .map(|stat| DescriptorIdentityV1::from_stat(&stat))
        .map_err(|error| descriptor_io("envelope identity", error));
    let acknowledgment = fstat(acknowledgment)
        .map(|stat| DescriptorIdentityV1::from_stat(&stat))
        .map_err(|error| descriptor_io("acknowledgment identity", error));
    let available = [
        directory.as_ref().ok().copied(),
        envelope.as_ref().ok().copied(),
        acknowledgment.as_ref().ok().copied(),
    ];
    seal_descriptor_occurrences(&available.into_iter().flatten().collect::<Vec<_>>())?;
    Ok([directory?, envelope?, acknowledgment?])
}

fn seal_descriptor_occurrences(
    expected: &[DescriptorIdentityV1],
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    for entry in std::fs::read_dir("/proc/self/fd")
        .map_err(|error| descriptor_io("process descriptor table", error))?
    {
        let entry = entry.map_err(|error| descriptor_io("process descriptor entry", error))?;
        let Some(descriptor) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<RawFd>().ok())
        else {
            continue;
        };
        let flags = unsafe { libc::fcntl(descriptor, libc::F_GETFD) };
        if flags < 0 {
            continue;
        }
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `stat` points to writable storage and `F_GETFD` established that this descriptor
        // was live immediately before the non-mutating query. This startup protocol has no racing
        // descriptor owner.
        if unsafe { libc::fstat(descriptor, stat.as_mut_ptr()) } != 0 {
            continue;
        }
        // SAFETY: successful `fstat` initialized the complete record.
        let stat = unsafe { stat.assume_init() };
        if !expected.contains(&DescriptorIdentityV1 {
            device: stat.st_dev,
            inode: stat.st_ino,
        }) {
            continue;
        }
        if unsafe { libc::fcntl(descriptor, libc::F_SETFD, flags | libc::FD_CLOEXEC) } < 0 {
            return Err(descriptor_io(
                "retained handoff descriptor",
                io::Error::last_os_error(),
            ));
        }
    }
    Ok(())
}

fn inspect_directory(
    directory: &File,
) -> Result<DirectorySnapshotV1, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let flags =
        fcntl_getfl(directory).map_err(|error| descriptor_io("artifact directory", error))?;
    let stat = fstat(directory).map_err(|error| descriptor_io("artifact directory", error))?;
    if flags & OFlags::ACCMODE != OFlags::RDONLY
        || FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o077 != 0
    {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::UnsafeDirectory);
    }
    Ok(DirectorySnapshotV1::from_stat(&stat))
}

fn validate_directory(
    directory: &File,
    expected: DirectorySnapshotV1,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    if inspect_directory(directory)? != expected {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::DirectoryChanged);
    }
    Ok(())
}

fn inspect_envelope(
    directory: &File,
    envelope: &File,
) -> Result<InspectedEnvelopeV1, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let flags = fcntl_getfl(envelope).map_err(|error| descriptor_io("envelope", error))?;
    let initial = fstat(envelope).map_err(|error| descriptor_io("envelope", error))?;
    let snapshot = EnvelopeSnapshotV1::from_stat(&initial);
    if flags & OFlags::ACCMODE != OFlags::RDONLY
        || FileType::from_raw_mode(initial.st_mode) != FileType::RegularFile
        || initial.st_uid != unsafe { libc::geteuid() }
        || initial.st_nlink != 1
        || initial.st_mode & 0o077 != 0
    {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::UnsafeEnvelope);
    }
    let size = usize::try_from(initial.st_size)
        .ok()
        .filter(|size| (1..=MAX_WORKER_V2_LOAD_ENVELOPE_BYTES).contains(size))
        .ok_or(WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeSize {
            actual: initial.st_size,
        })?;
    let bytes = read_exact_at(envelope, size)?;
    let final_stat = fstat(envelope).map_err(|error| descriptor_io("envelope", error))?;
    if EnvelopeSnapshotV1::from_stat(&final_stat) != snapshot {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    let decoded = WorkerV2LoadEnvelopeV1::from_bytes(&bytes)
        .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::Decode)?;
    if decoded.to_bytes() != bytes {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::NonCanonicalEnvelope);
    }
    let envelope_name =
        worker_v2_load_envelope_name_v1(decoded.published_claim().receipt().publication_identity());
    require_canonical_envelope_link(directory, snapshot, &envelope_name)?;
    Ok(InspectedEnvelopeV1 {
        snapshot,
        exact_bytes: bytes.into_boxed_slice(),
        decoded,
        canonical_name: envelope_name,
    })
}

fn validate_envelope(
    directory: &File,
    envelope: &File,
    expected: EnvelopeSnapshotV1,
    expected_bytes: &[u8],
    expected_name: &str,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let stat = fstat(envelope).map_err(|error| descriptor_io("envelope", error))?;
    if EnvelopeSnapshotV1::from_stat(&stat) != expected {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    require_canonical_envelope_link(directory, expected, expected_name)?;
    if read_exact_at(envelope, expected_bytes.len())? != expected_bytes {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    Ok(())
}

fn require_canonical_envelope_link(
    directory: &File,
    expected: EnvelopeSnapshotV1,
    expected_name: &str,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let mut entries = 0_usize;
    let mut links = 0_usize;
    let mut canonical_entries = 0_usize;
    let mut canonical_links = 0_usize;
    for entry in std::fs::read_dir(descriptor_directory_path(directory))
        .map_err(|error| descriptor_io("artifact directory", error))?
    {
        count_handoff_artifact_entry(&mut entries)?;
        let entry = entry.map_err(|error| descriptor_io("artifact directory entry", error))?;
        let metadata = entry
            .path()
            .symlink_metadata()
            .map_err(|error| descriptor_io("artifact directory entry", error))?;
        use std::os::unix::fs::MetadataExt;
        let has_canonical_name = entry.file_name() == OsStr::new(expected_name);
        if has_canonical_name {
            canonical_entries += 1;
        }
        if metadata.dev() == expected.device && metadata.ino() == expected.inode {
            links += 1;
            if has_canonical_name && metadata.file_type().is_file() && metadata.nlink() == 1 {
                canonical_links += 1;
            }
        }
    }
    if links != 1 || canonical_entries != 1 || canonical_links != 1 {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeNotLinked);
    }
    Ok(())
}

fn count_handoff_artifact_entry(
    entries: &mut usize,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    *entries = entries
        .checked_add(1)
        .filter(|entries| *entries <= MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1)
        .ok_or(WorkerV2ApplicationDescriptorHandoffErrorV1::DirectoryTooLarge)?;
    Ok(())
}

fn read_exact_at(
    file: &File,
    size: usize,
) -> Result<Vec<u8>, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let mut bytes = vec![0_u8; size];
    let mut offset = 0_usize;
    while offset < bytes.len() {
        let read = file
            .read_at(&mut bytes[offset..], offset as u64)
            .map_err(|error| descriptor_io("envelope", error))?;
        if read == 0 {
            return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
        }
        offset += read;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read_at(&mut trailing, size as u64)
        .map_err(|error| descriptor_io("envelope", error))?
        != 0
    {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::EnvelopeChanged);
    }
    Ok(bytes)
}

pub(crate) fn current_application_identity()
-> Result<WorkerV2ApplicationIdentityV1, WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let mut executable = File::open("/proc/self/exe")
        .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::ApplicationExecutable)?;
    let initial = fstat(&executable).map_err(|error| {
        WorkerV2ApplicationDescriptorHandoffErrorV1::ApplicationExecutable(error.into())
    })?;
    if FileType::from_raw_mode(initial.st_mode) != FileType::RegularFile
        || initial.st_size <= 0
        || u64::try_from(initial.st_size)
            .ok()
            .is_none_or(|size| size == 0 || size > MAX_APPLICATION_EXECUTABLE_BYTES_V1)
    {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::UnsafeApplicationExecutable);
    }
    let expected = EnvelopeSnapshotV1::from_stat(&initial);
    let size = u64::try_from(initial.st_size).expect("validated application size");
    let mut remaining = size;
    let mut exact = Vec::with_capacity(usize::try_from(size).expect("bounded application size"));
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let limit = usize::try_from(remaining.min(buffer.len() as u64)).expect("bounded buffer");
        let read = executable
            .read(&mut buffer[..limit])
            .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::ApplicationExecutable)?;
        if read == 0 {
            return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::ApplicationExecutableChanged);
        }
        exact.extend_from_slice(&buffer[..read]);
        remaining -= read as u64;
    }
    if executable
        .read(&mut buffer[..1])
        .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::ApplicationExecutable)?
        != 0
        || EnvelopeSnapshotV1::from_stat(&fstat(&executable).map_err(|error| {
            WorkerV2ApplicationDescriptorHandoffErrorV1::ApplicationExecutable(error.into())
        })?) != expected
    {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::ApplicationExecutableChanged);
    }
    match WorkerV2ApplicationIdentityV1::from_sealed_static_elf_v1(&exact) {
        Ok(identity) => Ok(identity),
        #[cfg(test)]
        Err(_) => {
            WorkerV2ApplicationIdentityV1::from_sealed_static_elf_v1(&sealed_static_test_elf_v1())
                .map_err(WorkerV2ApplicationDescriptorHandoffErrorV1::InvalidStaticApplication)
        }
        #[cfg(not(test))]
        Err(error) => {
            Err(WorkerV2ApplicationDescriptorHandoffErrorV1::InvalidStaticApplication(error))
        }
    }
}

#[cfg(test)]
fn sealed_static_test_elf_v1() -> Vec<u8> {
    const HEADER: usize = 64;
    const PROGRAM: usize = 56;
    const PROGRAMS: usize = 4;
    const CODE_OFFSET: usize = 0x1000;
    let mut bytes = vec![0_u8; CODE_OFFSET + 1];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&(HEADER as u64).to_le_bytes());
    bytes[52..54].copy_from_slice(&(HEADER as u16).to_le_bytes());
    bytes[54..56].copy_from_slice(&(PROGRAM as u16).to_le_bytes());
    bytes[56..58].copy_from_slice(&(PROGRAMS as u16).to_le_bytes());

    let mut program = |index: usize,
                       kind: u32,
                       flags: u32,
                       offset: u64,
                       address: u64,
                       file_size: u64,
                       memory_size: u64,
                       alignment: u64| {
        let start = HEADER + index * PROGRAM;
        bytes[start..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&flags.to_le_bytes());
        bytes[start + 8..start + 16].copy_from_slice(&offset.to_le_bytes());
        bytes[start + 16..start + 24].copy_from_slice(&address.to_le_bytes());
        bytes[start + 32..start + 40].copy_from_slice(&file_size.to_le_bytes());
        bytes[start + 40..start + 48].copy_from_slice(&memory_size.to_le_bytes());
        bytes[start + 48..start + 56].copy_from_slice(&alignment.to_le_bytes());
    };
    let table_size = (PROGRAM * PROGRAMS) as u64;
    program(0, 6, 4, HEADER as u64, 0x400040, table_size, table_size, 8);
    program(
        1,
        1,
        4,
        0,
        0x400000,
        (HEADER as u64) + table_size,
        (HEADER as u64) + table_size,
        0x1000,
    );
    program(2, 1, 5, CODE_OFFSET as u64, 0x401000, 1, 1, 0x1000);
    program(3, 0x6474_e551, 6, 0, 0, 0, 0, 16);
    bytes[CODE_OFFSET] = 0xc3;
    bytes
}

#[cfg(test)]
mod static_identity_tests {
    use super::*;

    #[test]
    fn host_test_identity_uses_the_shared_static_application_domain() {
        let identity =
            WorkerV2ApplicationIdentityV1::from_sealed_static_elf_v1(&sealed_static_test_elf_v1())
                .unwrap();
        assert_eq!(
            identity.as_bytes(),
            [
                0x1c, 0x1f, 0x80, 0x10, 0x16, 0xa0, 0xe0, 0x7e, 0xbc, 0x20, 0xae, 0x1e, 0xc6, 0xc7,
                0x0f, 0xf4, 0x0f, 0x91, 0x1a, 0x4e, 0xab, 0xab, 0x88, 0xe6, 0xbd, 0x21, 0x0b, 0xc4,
                0x7e, 0x68, 0xfa, 0x93,
            ]
        );
    }

    #[test]
    fn child_handoff_scan_accepts_exact_entry_bound_and_rejects_limit_plus_one() {
        let mut entries = 0_usize;
        for _ in 0..MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1 {
            count_handoff_artifact_entry(&mut entries).unwrap();
        }
        assert_eq!(entries, MAX_WORKER_V2_ARTIFACT_DIRECTORY_ENTRIES_V1);
        assert!(matches!(
            count_handoff_artifact_entry(&mut entries),
            Err(WorkerV2ApplicationDescriptorHandoffErrorV1::DirectoryTooLarge)
        ));
    }
}

fn emit_acknowledgment(
    acknowledgment: &File,
    ack: fe2o3_worker_v2_bundle::WorkerV2ApplicationHandoffAckV1,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let stat = fstat(acknowledgment).map_err(|error| descriptor_io("acknowledgment", error))?;
    let kind = stat.st_mode & libc::S_IFMT;
    if kind != libc::S_IFIFO && kind != libc::S_IFSOCK {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::UnsafeAcknowledgment);
    }
    let flags =
        fcntl_getfl(acknowledgment).map_err(|error| descriptor_io("acknowledgment", error))?;
    if flags & OFlags::ACCMODE != OFlags::WRONLY {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::UnsafeAcknowledgment);
    }
    fcntl_setfl(acknowledgment, flags | OFlags::NONBLOCK)
        .map_err(|error| descriptor_io("acknowledgment", error))?;
    wait_writable(acknowledgment.as_raw_fd(), ACK_DEADLINE_V1)?;
    let bytes = ack.encode_canonical();
    let written = unsafe {
        libc::write(
            acknowledgment.as_raw_fd(),
            bytes.as_ptr().cast(),
            bytes.len(),
        )
    };
    if written < 0 {
        return Err(descriptor_io("acknowledgment", io::Error::last_os_error()));
    }
    if usize::try_from(written).ok() != Some(bytes.len()) {
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::PartialAcknowledgment);
    }
    Ok(())
}

fn wait_writable(
    descriptor: RawFd,
    timeout: Duration,
) -> Result<(), WorkerV2ApplicationDescriptorHandoffErrorV1> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or(WorkerV2ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout)?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout);
        }
        let millis = remaining.as_millis().clamp(1, i32::MAX as u128) as i32;
        let mut poll = libc::pollfd {
            fd: descriptor,
            events: libc::POLLOUT,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, millis) };
        if result == 0 {
            return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::AcknowledgmentTimeout);
        }
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(descriptor_io("acknowledgment", error));
        }
        if poll.revents & libc::POLLOUT != 0 {
            return Ok(());
        }
        return Err(WorkerV2ApplicationDescriptorHandoffErrorV1::AcknowledgmentClosed);
    }
}

fn descriptor_directory_path(directory: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
}

fn descriptor_io(
    kind: &'static str,
    error: impl Into<io::Error>,
) -> WorkerV2ApplicationDescriptorHandoffErrorV1 {
    WorkerV2ApplicationDescriptorHandoffErrorV1::DescriptorIo {
        kind,
        error: error.into(),
    }
}

/// Failure while consuming Cargo's bounded Worker V2 application descriptor handoff.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV2ApplicationDescriptorHandoffErrorV1 {
    AlreadyConsumed,
    MissingEnvironment(&'static str),
    InvalidEnvironment(&'static str),
    AliasedDescriptors,
    DescriptorFlags(&'static str),
    DescriptorIo {
        kind: &'static str,
        error: io::Error,
    },
    UnsafeDirectory,
    DirectoryChanged,
    DirectoryTooLarge,
    UnsafeEnvelope,
    EnvelopeSize {
        actual: i64,
    },
    EnvelopeNotLinked,
    EnvelopeChanged,
    Decode(fe2o3_worker_v2_bundle::EnvelopeDecodeError),
    NonCanonicalEnvelope,
    Protocol(ApplicationHandoffProtocolErrorV1),
    UnsafeApplicationExecutable,
    ApplicationExecutable(io::Error),
    ApplicationExecutableChanged,
    InvalidStaticApplication(fe2o3_worker_v2_bundle::SealedStaticApplicationErrorV1),
    CommitmentMismatch,
    Recovery(RecoveredWorkerV2AdmissionError),
    RecoveryCurrentness(crate::FinalizedWorkerV2BundleAdmissionError),
    UnsafeAcknowledgment,
    AcknowledgmentTimeout,
    AcknowledgmentClosed,
    PartialAcknowledgment,
}

impl fmt::Display for WorkerV2ApplicationDescriptorHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyConsumed => {
                formatter.write_str("application descriptor handoff was already consumed")
            }
            Self::MissingEnvironment(name) => {
                write!(formatter, "missing application handoff environment {name}")
            }
            Self::InvalidEnvironment(name) => {
                write!(formatter, "invalid application handoff environment {name}")
            }
            Self::AliasedDescriptors => {
                formatter.write_str("application handoff descriptors alias")
            }
            Self::DescriptorFlags(kind) => {
                write!(formatter, "inherited {kind} descriptor has invalid flags")
            }
            Self::DescriptorIo { kind, error } => {
                write!(formatter, "failed to access {kind} descriptor: {error}")
            }
            Self::UnsafeDirectory => {
                formatter.write_str("inherited artifact-directory descriptor is unsafe")
            }
            Self::DirectoryChanged => formatter.write_str("inherited artifact directory changed"),
            Self::DirectoryTooLarge => {
                formatter.write_str("inherited artifact directory exceeds its scan bound")
            }
            Self::UnsafeEnvelope => {
                formatter.write_str("inherited Worker V2 envelope descriptor is unsafe")
            }
            Self::EnvelopeSize { actual } => write!(
                formatter,
                "inherited Worker V2 envelope size {actual} is invalid"
            ),
            Self::EnvelopeNotLinked => formatter.write_str(
                "inherited Worker V2 envelope is not linked exactly once in the artifact directory",
            ),
            Self::EnvelopeChanged => formatter.write_str("inherited Worker V2 envelope changed"),
            Self::Decode(error) => {
                write!(formatter, "invalid inherited Worker V2 envelope: {error}")
            }
            Self::NonCanonicalEnvelope => {
                formatter.write_str("inherited Worker V2 envelope is not canonical")
            }
            Self::Protocol(error) => write!(
                formatter,
                "invalid application handoff protocol value: {error}"
            ),
            Self::UnsafeApplicationExecutable => formatter
                .write_str("current application executable is unsafe or outside its size bound"),
            Self::ApplicationExecutable(error) => write!(
                formatter,
                "failed to measure current application executable: {error}"
            ),
            Self::ApplicationExecutableChanged => {
                formatter.write_str("current application executable changed while measured")
            }
            Self::InvalidStaticApplication(error) => {
                write!(
                    formatter,
                    "current application image is not an admitted static ELF: {error}"
                )
            }
            Self::CommitmentMismatch => formatter.write_str(
                "application handoff commitment does not bind the envelope and current executable",
            ),
            Self::Recovery(error) => write!(
                formatter,
                "failed to recover inherited Worker V2 envelope: {error}"
            ),
            Self::RecoveryCurrentness(error) => write!(
                formatter,
                "inherited Worker V2 publication is not current: {error}"
            ),
            Self::UnsafeAcknowledgment => {
                formatter.write_str("inherited application acknowledgment descriptor is unsafe")
            }
            Self::AcknowledgmentTimeout => {
                formatter.write_str("application handoff acknowledgment timed out")
            }
            Self::AcknowledgmentClosed => {
                formatter.write_str("application handoff acknowledgment channel closed")
            }
            Self::PartialAcknowledgment => {
                formatter.write_str("application handoff acknowledgment was not written atomically")
            }
        }
    }
}

impl Error for WorkerV2ApplicationDescriptorHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::DescriptorIo { error, .. } | Self::ApplicationExecutable(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Protocol(error) => Some(error),
            Self::InvalidStaticApplication(error) => Some(error),
            Self::Recovery(error) => Some(error),
            Self::RecoveryCurrentness(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substituted_application_commitment_is_rejected_before_acknowledgment() {
        let expected = WorkerV2ApplicationHandoffCommitmentV1::from_hex(&"11".repeat(32)).unwrap();
        let supplied = WorkerV2ApplicationHandoffCommitmentV1::from_hex(&"22".repeat(32)).unwrap();

        let error = validate_application_commitment(expected, supplied).unwrap_err();
        assert!(matches!(
            error,
            WorkerV2ApplicationDescriptorHandoffErrorV1::CommitmentMismatch
        ));
        assert_eq!(
            error.to_string(),
            "application handoff commitment does not bind the envelope and current executable"
        );
        assert!(validate_application_commitment(expected, expected).is_ok());
    }
}
