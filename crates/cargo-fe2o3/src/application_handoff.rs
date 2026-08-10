//! Descriptor-only Cargo-to-application handoff for canonical Worker V2 evidence.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use fe2o3_artifact_transaction::reacquire_current_hsaco_publication_lease_v1;
use fe2o3_worker_v2_bundle::{MAX_WORKER_V2_LOAD_ENVELOPE_BYTES, WorkerV2LoadEnvelopeV1};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, ResolveFlags, fstat, openat2, statat};
use sha2::{Digest, Sha256};

use crate::generation;
use crate::project::PinnedDirectory;

pub(crate) const RUNNER_CONTEXT_VERSION: &str = "2";
pub(crate) const ENVELOPE_FD_ENV: &str = "FE2O3_APPLICATION_ENVELOPE_FD_V1";
pub(crate) const COMMITMENT_ENV: &str = "FE2O3_APPLICATION_ENVELOPE_COMMITMENT_V1";
pub(crate) const APPLICATION_PROTOCOL_MARKER: &[u8] =
    b"FE2O3-APPLICATION-WORKER-V2-ENVELOPE-FD-V1\0";

const ENVELOPE_PREFIX: &[u8] = b".fe2o3-worker-v2-load-envelope-v1-";
const ENVELOPE_SUFFIX: &[u8] = b".envelope";
const ENVELOPE_NAME_BYTES: usize = ENVELOPE_PREFIX.len() + 64 + ENVELOPE_SUFFIX.len();
const MAX_ENVELOPE_CANDIDATES: usize = 256;
const COMMITMENT_DOMAIN: &[u8] = b"FE2O3/APPLICATION-WORKER-V2-HANDOFF/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    size: i64,
    modified_seconds: i64,
    modified_nanoseconds: u64,
    changed_seconds: i64,
    changed_nanoseconds: u64,
}

impl FileSnapshot {
    const fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            device: stat.st_dev,
            inode: stat.st_ino,
            mode: stat.st_mode,
            links: stat.st_nlink,
            size: stat.st_size,
            modified_seconds: stat.st_mtime,
            modified_nanoseconds: stat.st_mtime_nsec,
            changed_seconds: stat.st_ctime,
            changed_nanoseconds: stat.st_ctime_nsec,
        }
    }
}

pub(crate) fn open_expected_generation(
    path: PathBuf,
    expected_device: u64,
    expected_inode: u64,
) -> Result<PinnedDirectory, String> {
    let directory = PinnedDirectory::open_existing(path, "Cargo application artifact directory")?;
    if !directory.matches_identity(expected_device, expected_inode) {
        return Err("Cargo application artifact directory identity was substituted".to_string());
    }
    generation::validate_owned_artifact(&directory)?;
    let stat = fstat(directory.file()).map_err(|error| {
        format!("failed to inspect Cargo application artifact directory: {error}")
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != unsafe { libc::geteuid() }
        || stat.st_mode & 0o022 != 0
    {
        return Err(
            "Cargo application artifact directory is not controlled by the current owner"
                .to_string(),
        );
    }
    directory.validate_path("Cargo application artifact directory")?;
    Ok(directory)
}

pub(crate) struct PinnedApplicationEnvelope<'directory> {
    directory: &'directory PinnedDirectory,
    name: String,
    file: File,
    snapshot: FileSnapshot,
    exact_bytes: Vec<u8>,
    envelope: WorkerV2LoadEnvelopeV1,
}

impl<'directory> PinnedApplicationEnvelope<'directory> {
    pub(crate) fn discover(directory: &'directory PinnedDirectory) -> Result<Option<Self>, String> {
        directory.validate_path("Cargo application artifact directory")?;
        let names = envelope_names(directory)?;
        if names.is_empty() {
            return Ok(None);
        }

        let mut current = None;
        let mut rejected = Vec::new();
        for name in names {
            let candidate = Self::open(directory, name)?;
            match candidate.reacquire_current() {
                Ok(()) if current.is_none() => current = Some(candidate),
                Ok(()) => {
                    return Err(
                        "multiple canonical Worker V2 envelopes claim the current publication"
                            .to_string(),
                    );
                }
                Err(error) => rejected.push(error),
            }
        }
        directory.validate_path("Cargo application artifact directory")?;
        current.map(Some).ok_or_else(|| {
            format!(
                "canonical Worker V2 envelopes exist but none is current: {}",
                rejected
                    .first()
                    .map(String::as_str)
                    .unwrap_or("no candidate admitted")
            )
        })
    }

    fn open(directory: &'directory PinnedDirectory, name: String) -> Result<Self, String> {
        let descriptor = openat2(
            directory.file(),
            Path::new(&name),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::BENEATH
                | ResolveFlags::NO_SYMLINKS
                | ResolveFlags::NO_MAGICLINKS
                | ResolveFlags::NO_XDEV,
        )
        .map_err(|error| format!("failed to open canonical Worker V2 envelope {name}: {error}"))?;
        let flags = rustix::io::fcntl_getfd(&descriptor)
            .map_err(|error| format!("failed to inspect envelope descriptor flags: {error}"))?;
        let status = rustix::fs::fcntl_getfl(&descriptor)
            .map_err(|error| format!("failed to inspect envelope access mode: {error}"))?;
        if !flags.contains(rustix::io::FdFlags::CLOEXEC)
            || status & OFlags::ACCMODE != OFlags::RDONLY
        {
            return Err("canonical Worker V2 envelope descriptor is not read-only CLOEXEC".into());
        }
        let initial = fstat(&descriptor)
            .map_err(|error| format!("failed to inspect canonical Worker V2 envelope: {error}"))?;
        validate_envelope_stat(directory, &name, &initial)?;
        let snapshot = FileSnapshot::from_stat(&initial);
        let size = usize::try_from(initial.st_size).map_err(|_| {
            "canonical Worker V2 envelope has a negative or unrepresentable size".to_string()
        })?;
        if size == 0 || size > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES {
            return Err(format!(
                "canonical Worker V2 envelope size {size} is outside 1..={MAX_WORKER_V2_LOAD_ENVELOPE_BYTES}"
            ));
        }
        let mut file = File::from(descriptor);
        let mut exact_bytes = Vec::with_capacity(size.saturating_add(1));
        Read::by_ref(&mut file)
            .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES + 1) as u64)
            .read_to_end(&mut exact_bytes)
            .map_err(|error| format!("failed to read canonical Worker V2 envelope: {error}"))?;
        let final_stat = fstat(&file).map_err(|error| {
            format!("failed to re-inspect canonical Worker V2 envelope: {error}")
        })?;
        if FileSnapshot::from_stat(&final_stat) != snapshot || exact_bytes.len() != size {
            return Err("canonical Worker V2 envelope changed while it was read".to_string());
        }
        let envelope = WorkerV2LoadEnvelopeV1::from_bytes(&exact_bytes)
            .map_err(|error| format!("invalid canonical Worker V2 envelope {name}: {error}"))?;
        if envelope.to_bytes() != exact_bytes {
            return Err("Worker V2 envelope encoding is not canonical".to_string());
        }
        let receipt = envelope.published_claim().receipt();
        if name != canonical_envelope_name(receipt.publication_identity()) {
            return Err("Worker V2 envelope filename does not bind its publication".to_string());
        }
        file.seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind canonical Worker V2 envelope: {error}"))?;
        Ok(Self {
            directory,
            name,
            file,
            snapshot,
            exact_bytes,
            envelope,
        })
    }

    fn reacquire_current(&self) -> Result<(), String> {
        let lease = reacquire_current_hsaco_publication_lease_v1(
            &self.directory.child_path(),
            self.envelope.published_claim(),
        )
        .map_err(|error| format!("{}: {error}", self.name))?;
        drop(lease);
        Ok(())
    }

    fn revalidate(&mut self) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| format!("failed to rewind inherited envelope: {error}"))?;
        let mut bytes = Vec::with_capacity(self.exact_bytes.len().saturating_add(1));
        Read::by_ref(&mut self.file)
            .take((MAX_WORKER_V2_LOAD_ENVELOPE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("failed to re-read inherited envelope: {error}"))?;
        let stat = fstat(&self.file)
            .map_err(|error| format!("failed to re-inspect inherited envelope: {error}"))?;
        validate_envelope_stat(self.directory, &self.name, &stat)?;
        if FileSnapshot::from_stat(&stat) != self.snapshot || bytes != self.exact_bytes {
            return Err("inherited Worker V2 envelope changed after validation".to_string());
        }
        let decoded = WorkerV2LoadEnvelopeV1::from_bytes(&bytes)
            .map_err(|error| format!("inherited Worker V2 envelope is no longer valid: {error}"))?;
        if decoded != self.envelope || decoded.to_bytes() != bytes {
            return Err("inherited Worker V2 envelope identity changed".to_string());
        }
        self.reacquire_current()?;
        self.directory
            .validate_path("Cargo application artifact directory")?;
        self.file
            .seek(SeekFrom::Start(0))
            .map(|_| ())
            .map_err(|error| format!("failed to rewind inherited envelope for child: {error}"))
    }

    pub(crate) fn configure_child(
        &mut self,
        command: &mut Command,
        child_sha256: [u8; 32],
    ) -> Result<[u8; 32], String> {
        self.revalidate()?;
        reject_inheritable_descriptors()?;
        let commitment = application_commitment(&self.envelope, child_sha256);
        let raw_fd = self.file.as_raw_fd();
        let expected = self.snapshot;
        command
            .env(ENVELOPE_FD_ENV, raw_fd.to_string())
            .env(COMMITMENT_ENV, hex(&commitment));
        // SAFETY: `self.file` remains alive through spawn. The callback only inspects that exact
        // inherited descriptor and clears CLOEXEC in the child-side descriptor table.
        unsafe {
            command.pre_exec(move || {
                let descriptor = BorrowedFd::borrow_raw(raw_fd);
                let flags = rustix::io::fcntl_getfd(descriptor).map_err(io::Error::from)?;
                let status = rustix::fs::fcntl_getfl(descriptor).map_err(io::Error::from)?;
                let stat = fstat(descriptor).map_err(io::Error::from)?;
                if !flags.contains(rustix::io::FdFlags::CLOEXEC)
                    || status & OFlags::ACCMODE != OFlags::RDONLY
                    || FileSnapshot::from_stat(&stat) != expected
                {
                    return Err(io::Error::from_raw_os_error(libc::ESTALE));
                }
                rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::empty())
                    .map_err(io::Error::from)
            });
        }
        Ok(commitment)
    }
}

fn envelope_names(directory: &PinnedDirectory) -> Result<Vec<String>, String> {
    let scan = directory.try_clone_for_transfer()?;
    let mut entries = rustix::fs::Dir::read_from(&scan)
        .map_err(|error| format!("failed to scan artifact directory: {error}"))?;
    let mut names = Vec::new();
    for entry in &mut entries {
        let entry = entry.map_err(|error| format!("failed to read artifact entry: {error}"))?;
        let bytes = entry.file_name().to_bytes();
        if !bytes.starts_with(ENVELOPE_PREFIX) {
            continue;
        }
        if !is_canonical_envelope_name(bytes) {
            return Err("malformed Worker V2 envelope publication name".to_string());
        }
        if names.len() == MAX_ENVELOPE_CANDIDATES {
            return Err(format!(
                "artifact directory exceeds {MAX_ENVELOPE_CANDIDATES} canonical envelope candidates"
            ));
        }
        names.push(
            std::str::from_utf8(bytes)
                .expect("canonical envelope names are ASCII")
                .to_string(),
        );
    }
    names.sort_unstable();
    Ok(names)
}

fn is_canonical_envelope_name(bytes: &[u8]) -> bool {
    bytes.len() == ENVELOPE_NAME_BYTES
        && bytes.starts_with(ENVELOPE_PREFIX)
        && bytes.ends_with(ENVELOPE_SUFFIX)
        && bytes[ENVELOPE_PREFIX.len()..ENVELOPE_PREFIX.len() + 64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn canonical_envelope_name(publication: [u8; 32]) -> String {
    format!(
        "{}{}{}",
        std::str::from_utf8(ENVELOPE_PREFIX).expect("ASCII prefix"),
        hex(&publication),
        std::str::from_utf8(ENVELOPE_SUFFIX).expect("ASCII suffix")
    )
}

fn validate_envelope_stat(
    directory: &PinnedDirectory,
    name: &str,
    opened: &rustix::fs::Stat,
) -> Result<(), String> {
    let linked = statat(directory.file(), name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("failed to inspect linked Worker V2 envelope {name}: {error}"))?;
    if FileType::from_raw_mode(opened.st_mode) != FileType::RegularFile
        || opened.st_dev != linked.st_dev
        || opened.st_ino != linked.st_ino
        || opened.st_nlink != 1
        || opened.st_uid != unsafe { libc::geteuid() }
        || opened.st_mode & 0o077 != 0
    {
        return Err(format!("refusing unsafe Worker V2 envelope {name}"));
    }
    Ok(())
}

fn application_commitment(envelope: &WorkerV2LoadEnvelopeV1, child_sha256: [u8; 32]) -> [u8; 32] {
    let claim = envelope.published_claim();
    let plan = claim.plan();
    let attempt = plan.attempt();
    let receipt = claim.receipt();
    let mut digest = Sha256::new();
    digest.update(COMMITMENT_DOMAIN);
    digest.update(plan.scope().package().as_bytes());
    digest.update(attempt.generation().to_le_bytes());
    digest.update(attempt.session().as_bytes());
    digest.update(attempt.invocation().as_bytes());
    for field in [
        receipt.attempt_identity(),
        receipt.producer_identity(),
        receipt.scope_identity(),
        receipt.plan_commitment(),
        receipt.upstream_evidence_identity(),
        receipt.finalized_output_identity(),
        receipt.publication_identity(),
    ] {
        digest.update(field);
    }
    digest.update(envelope.identity().as_bytes());
    digest.update(child_sha256);
    digest.finalize().into()
}

fn reject_inheritable_descriptors() -> Result<(), String> {
    let entries = std::fs::read_dir("/proc/self/fd")
        .map_err(|error| format!("failed to audit runner descriptors: {error}"))?;
    for entry in entries {
        let name = entry
            .map_err(|error| format!("failed to audit runner descriptor: {error}"))?
            .file_name();
        let Some(raw_fd) = name.to_str().and_then(|value| value.parse::<RawFd>().ok()) else {
            continue;
        };
        if raw_fd <= libc::STDERR_FILENO {
            continue;
        }
        // SAFETY: each number came from a live `/proc/self/fd` entry. A concurrent close simply
        // turns the fcntl into EBADF, which is harmless for this fail-closed audit.
        let descriptor = unsafe { BorrowedFd::borrow_raw(raw_fd) };
        match rustix::io::fcntl_getfd(descriptor) {
            Ok(flags) if flags.contains(rustix::io::FdFlags::CLOEXEC) => {}
            Ok(_) => {
                return Err(format!(
                    "runner refuses to pass inheritable descriptor {raw_fd} to the application"
                ));
            }
            Err(rustix::io::Errno::BADF) => {}
            Err(error) => {
                return Err(format!(
                    "failed to audit runner descriptor {raw_fd}: {error}"
                ));
            }
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}
