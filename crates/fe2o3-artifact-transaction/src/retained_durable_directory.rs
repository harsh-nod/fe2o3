//! Descriptor-only durable I/O for a supervisor-retained private service directory.
//!
//! This is a deliberately small filesystem mechanism shared by higher-level journals. It never
//! opens an authority path: every read, temporary write, sync, and rename is relative to the
//! retained directory file description supplied by the supervisor. Record replacement follows
//! the same synced-temp, durable-redo, rename, and directory-sync protocol used by the durable
//! link publication adapter.
//!
//! `AUTHORITY=none`: this mechanism validates local file shape and ordering only. It does not
//! authenticate record meaning, prevent rollback by a process that can mutate the directory,
//! coordinate multiple writers, or grant publication, execution, runtime, or GPU authority.

use crate::{EmitError, PinnedOutput};
use rustix::fd::OwnedFd;
use rustix::fs::{
    AtFlags, FileType, Mode, OFlags, RenameFlags, fstat, fsync, openat, renameat, renameat_with,
    statat,
};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_TEMP_ATTEMPTS: u64 = 128;
static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// Whether a deterministic fault occurs immediately before or after one durable operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedDurableFaultTimingV1 {
    Before,
    After,
}

/// Record operation exposed to bounded crash-injection tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedDurableRecordBoundaryV1 {
    CreateTemp,
    WriteTemp,
    SyncTemp,
    RenameTempToRedo,
    SyncRedoName,
    RenameRedoToCanonical,
    SyncCanonicalName,
}

/// Recovery operation exposed to bounded crash-injection tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedDurableRecoveryBoundaryV1 {
    /// Syncs a previously visible canonical record name before recovery may trust its durability.
    SyncDirectory,
}

/// Recovery-only namespace mutation exposed to bounded crash-injection tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RetainedDurableRecoveryMutationBoundaryV1 {
    /// Moves a visible canonical record to its distinct recovery name.
    RenameCanonicalToRecovery,
}

/// Private-artifact operation exposed to bounded crash-injection tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetainedDurableArtifactBoundaryV1 {
    CreateTemp,
    WriteTemp,
    SyncTemp,
    RenameTempToStaged,
    SyncStagedName,
    SetFinalMode,
    SyncFinalMode,
    RenameStagedToFinal,
    SyncFinalName,
    RenameFinalToStaged,
    SyncRecoveredStagedName,
}

/// Test-only boundary callbacks. Production callers use [`NoRetainedDurableDirectoryHooksV1`].
pub trait RetainedDurableDirectoryHooksV1 {
    /// Performs one directory sync. Tests may override this method to model the sync syscall
    /// itself returning an error; production hooks always execute the real `fsync`.
    fn sync_directory(&mut self, directory: &OwnedFd) -> io::Result<()> {
        fsync(directory).map_err(io::Error::from)
    }

    fn record(
        &mut self,
        _boundary: RetainedDurableRecordBoundaryV1,
        _timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        Ok(())
    }

    fn artifact(
        &mut self,
        _boundary: RetainedDurableArtifactBoundaryV1,
        _timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        Ok(())
    }

    /// Observes the recovery directory-sync boundary without bypassing the operation.
    fn recovery(
        &mut self,
        _boundary: RetainedDurableRecoveryBoundaryV1,
        _timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        Ok(())
    }

    /// Observes a recovery-only namespace mutation without bypassing the operation.
    fn recovery_mutation(
        &mut self,
        _boundary: RetainedDurableRecoveryMutationBoundaryV1,
        _timing: RetainedDurableFaultTimingV1,
    ) -> io::Result<()> {
        Ok(())
    }
}

/// Production no-fault durable-I/O hooks.
pub struct NoRetainedDurableDirectoryHooksV1;

impl RetainedDurableDirectoryHooksV1 for NoRetainedDurableDirectoryHooksV1 {}

/// Descriptor-relative durable-I/O failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum RetainedDurableDirectoryErrorV1 {
    Filesystem(EmitError),
    UnsafeEntry { entry: String, reason: String },
    InvalidName { entry: String },
    Size { actual: usize, maximum: usize },
    ContentMismatch { entry: String },
    ExistingEntry { entry: String },
    MissingEntry { entry: String },
    DirectoryIdentityChanged,
}

impl fmt::Display for RetainedDurableDirectoryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Filesystem(error) => write!(formatter, "durable directory I/O failed: {error}"),
            Self::UnsafeEntry { entry, reason } => {
                write!(formatter, "unsafe durable entry {entry}: {reason}")
            }
            Self::InvalidName { entry } => {
                write!(formatter, "invalid durable entry name {entry:?}")
            }
            Self::Size { actual, maximum } => {
                write!(
                    formatter,
                    "durable entry is {actual} bytes, maximum is {maximum}"
                )
            }
            Self::ContentMismatch { entry } => {
                write!(
                    formatter,
                    "durable entry {entry} does not contain the expected bytes"
                )
            }
            Self::ExistingEntry { entry } => {
                write!(formatter, "durable entry {entry} already exists")
            }
            Self::MissingEntry { entry } => write!(formatter, "durable entry {entry} is missing"),
            Self::DirectoryIdentityChanged => {
                formatter.write_str("retained durable directory identity or metadata changed")
            }
        }
    }
}

impl std::error::Error for RetainedDurableDirectoryErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Filesystem(error) => Some(error),
            _ => None,
        }
    }
}

impl From<EmitError> for RetainedDurableDirectoryErrorV1 {
    fn from(error: EmitError) -> Self {
        Self::Filesystem(error)
    }
}

impl From<io::Error> for RetainedDurableDirectoryErrorV1 {
    fn from(error: io::Error) -> Self {
        Self::Filesystem(EmitError::Io(error))
    }
}

/// Opaque retained descriptor for one private service-owned durability root.
///
/// The handle is move-only and has no path or raw-descriptor accessor. Admission requires the
/// exact directory to be owned by the current effective UID, mode `0700`, linked, and opened with
/// `FD_CLOEXEC`. Every operation revalidates those properties and the original device/inode.
pub struct RetainedDurableDirectoryV1 {
    output: PinnedOutput,
    service_uid: u32,
}

impl fmt::Debug for RetainedDurableDirectoryV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RetainedDurableDirectoryV1")
            .field("authority", &"none")
            .field("device", &self.output.device)
            .field("inode", &self.output.inode)
            .finish_non_exhaustive()
    }
}

impl RetainedDurableDirectoryV1 {
    /// Admits one supervisor-retained directory descriptor without consulting any path.
    pub fn admit_service_owned(
        descriptor: OwnedFd,
    ) -> Result<Self, RetainedDurableDirectoryErrorV1> {
        let flags = rustix::io::fcntl_getfd(&descriptor).map_err(io::Error::from)?;
        if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Err(Self::unsafe_entry(
                "<service-root-fd>",
                "retained directory descriptor lacks FD_CLOEXEC",
            ));
        }
        let stat = fstat(&descriptor).map_err(io::Error::from)?;
        let service_uid = rustix::process::geteuid().as_raw();
        validate_root_stat(&stat, service_uid)?;
        Ok(Self {
            output: PinnedOutput {
                fd: descriptor,
                display_path: PathBuf::from("<service-owned-root-fd>"),
                device: stat.st_dev,
                inode: stat.st_ino,
                path_guard: None,
            },
            service_uid,
        })
    }

    /// Returns the fixed non-authority marker.
    pub const fn authority(&self) -> &'static str {
        "none"
    }

    /// Tests whether another retained descriptor names this exact durability root.
    pub fn matches_descriptor(
        &self,
        descriptor: &OwnedFd,
    ) -> Result<bool, RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        let flags = rustix::io::fcntl_getfd(descriptor).map_err(io::Error::from)?;
        if !flags.contains(rustix::io::FdFlags::CLOEXEC) {
            return Ok(false);
        }
        let stat = fstat(descriptor).map_err(io::Error::from)?;
        if validate_root_stat(&stat, self.service_uid).is_err() {
            return Ok(false);
        }
        Ok(stat.st_dev == self.output.device && stat.st_ino == self.output.inode)
    }

    /// Reads one bounded private single-link regular file relative to the retained root.
    pub fn read_private(
        &self,
        entry: &str,
        maximum_bytes: usize,
    ) -> Result<Option<Vec<u8>>, RetainedDurableDirectoryErrorV1> {
        self.read_managed(entry, maximum_bytes, ManagedMode::Private)
    }

    /// Reads one exact-mode final file relative to the retained root.
    pub fn read_published(
        &self,
        entry: &str,
        maximum_bytes: usize,
        mode: u32,
    ) -> Result<Option<Vec<u8>>, RetainedDurableDirectoryErrorV1> {
        validate_final_mode(mode)?;
        self.read_managed(entry, maximum_bytes, ManagedMode::Exact(mode))
    }

    /// Reads a staged file before or after its durable final-mode transition.
    pub fn read_staged(
        &self,
        entry: &str,
        maximum_bytes: usize,
        final_mode: u32,
    ) -> Result<Option<Vec<u8>>, RetainedDurableDirectoryErrorV1> {
        validate_final_mode(final_mode)?;
        self.read_managed(
            entry,
            maximum_bytes,
            ManagedMode::PrivateOrExact(final_mode),
        )
    }

    fn read_managed(
        &self,
        entry: &str,
        maximum_bytes: usize,
        mode: ManagedMode,
    ) -> Result<Option<Vec<u8>>, RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        validate_name(entry)?;
        let fd = match openat(
            &self.output.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(error) if error == rustix::io::Errno::NOENT => return Ok(None),
            Err(error) => return Err(io::Error::from(error).into()),
        };
        validate_managed_file(&fd, entry, self.service_uid, mode)?;
        let mut bytes = Vec::new();
        fs::File::from(fd)
            .take((maximum_bytes.saturating_add(1)) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > maximum_bytes {
            return Err(RetainedDurableDirectoryErrorV1::Size {
                actual: bytes.len(),
                maximum: maximum_bytes,
            });
        }
        Ok(Some(bytes))
    }

    /// Commits one canonical record through a durable redo name.
    pub fn commit_record(
        &self,
        canonical: &str,
        redo: &str,
        bytes: &[u8],
        maximum_bytes: usize,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        validate_name(canonical)?;
        validate_name(redo)?;
        if canonical == redo {
            return Err(RetainedDurableDirectoryErrorV1::InvalidName {
                entry: canonical.to_owned(),
            });
        }
        let expected_canonical = self.read_private(canonical, maximum_bytes)?;
        self.stage_record_redo(redo, bytes, maximum_bytes, hooks)?;
        self.promote_validated_redo(
            canonical,
            redo,
            expected_canonical.as_deref(),
            bytes,
            maximum_bytes,
            hooks,
        )
    }

    /// Durably publishes exact record bytes under a private redo name without promoting it.
    ///
    /// Callers may inspect resource policy after this returns and before invoking
    /// [`Self::promote_validated_redo`]. The redo is recoverable state and must either be promoted
    /// or durably removed by the caller's serialized protocol.
    pub fn stage_record_redo(
        &self,
        redo: &str,
        bytes: &[u8],
        maximum_bytes: usize,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        validate_name(redo)?;
        if bytes.is_empty() || bytes.len() > maximum_bytes {
            return Err(RetainedDurableDirectoryErrorV1::Size {
                actual: bytes.len(),
                maximum: maximum_bytes,
            });
        }
        require_absent(&self.output.fd, redo)?;
        let (temporary_name, mut temporary) =
            self.create_temp(redo, |boundary, timing| hooks.record(boundary, timing))?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::WriteTemp,
            RetainedDurableFaultTimingV1::Before,
        )?;
        temporary.write_all(bytes)?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::WriteTemp,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncTemp,
            RetainedDurableFaultTimingV1::Before,
        )?;
        temporary.sync_all()?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncTemp,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::RenameTempToRedo,
            RetainedDurableFaultTimingV1::Before,
        )?;
        renameat_with(
            &self.output.fd,
            &temporary_name,
            &self.output.fd,
            redo,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::RenameTempToRedo,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncRedoName,
            RetainedDurableFaultTimingV1::Before,
        )?;
        hooks.sync_directory(&self.output.fd)?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncRedoName,
            RetainedDurableFaultTimingV1::After,
        )?;
        self.verify()?;
        Ok(())
    }

    /// Promotes an already validated redo after rechecking both observed byte strings.
    pub fn promote_validated_redo(
        &self,
        canonical: &str,
        redo: &str,
        expected_canonical: Option<&[u8]>,
        expected_redo: &[u8],
        maximum_bytes: usize,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        let canonical_now = self.read_private(canonical, maximum_bytes)?;
        let redo_now = self.read_private(redo, maximum_bytes)?;
        if canonical_now.as_deref() != expected_canonical
            || redo_now.as_deref() != Some(expected_redo)
        {
            return Err(RetainedDurableDirectoryErrorV1::ContentMismatch {
                entry: redo.to_owned(),
            });
        }
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
            RetainedDurableFaultTimingV1::Before,
        )?;
        renameat(&self.output.fd, redo, &self.output.fd, canonical).map_err(io::Error::from)?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncCanonicalName,
            RetainedDurableFaultTimingV1::Before,
        )?;
        hooks.sync_directory(&self.output.fd)?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncCanonicalName,
            RetainedDurableFaultTimingV1::After,
        )?;
        self.verify()
    }

    /// Recommits a recovered canonical record through a fresh durable redo transaction.
    ///
    /// A canonical name can be visible after a redo rename even when the directory sync from the
    /// original commit failed or had an ambiguous result. Recovery must not treat that visibility
    /// as durability. A later successful sync alone cannot repair a writeback error already
    /// reported for the original transaction. This operation therefore validates the exact visible
    /// record and redo absence, renames canonical to redo and syncs that mutation, promotes redo
    /// back to canonical and syncs again, then repeats both byte checks. The rename cycle preserves
    /// the managed-entry count, including at an exact persistent-entry quota.
    pub fn establish_recovered_record_durability(
        &self,
        canonical: &str,
        redo: &str,
        expected_canonical: &[u8],
        maximum_bytes: usize,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<Vec<u8>, RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        validate_name(canonical)?;
        validate_name(redo)?;
        if canonical == redo {
            return Err(RetainedDurableDirectoryErrorV1::InvalidName {
                entry: canonical.to_owned(),
            });
        }
        if expected_canonical.is_empty() || expected_canonical.len() > maximum_bytes {
            return Err(RetainedDurableDirectoryErrorV1::Size {
                actual: expected_canonical.len(),
                maximum: maximum_bytes,
            });
        }
        require_exact_private(self, canonical, expected_canonical, maximum_bytes)?;
        require_missing_private(self, redo, maximum_bytes)?;
        hit_recovery_mutation(
            hooks,
            RetainedDurableRecoveryMutationBoundaryV1::RenameCanonicalToRecovery,
            RetainedDurableFaultTimingV1::Before,
        )?;
        renameat_with(
            &self.output.fd,
            canonical,
            &self.output.fd,
            redo,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)?;
        hit_recovery_mutation(
            hooks,
            RetainedDurableRecoveryMutationBoundaryV1::RenameCanonicalToRecovery,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncRedoName,
            RetainedDurableFaultTimingV1::Before,
        )?;
        hooks.sync_directory(&self.output.fd)?;
        hit_record(
            hooks,
            RetainedDurableRecordBoundaryV1::SyncRedoName,
            RetainedDurableFaultTimingV1::After,
        )?;
        self.verify()?;
        require_missing_private(self, canonical, maximum_bytes)?;
        require_exact_private(self, redo, expected_canonical, maximum_bytes)?;
        self.promote_validated_redo(
            canonical,
            redo,
            None,
            expected_canonical,
            maximum_bytes,
            hooks,
        )?;
        hit_recovery(
            hooks,
            RetainedDurableRecoveryBoundaryV1::SyncDirectory,
            RetainedDurableFaultTimingV1::Before,
        )?;
        hooks.sync_directory(&self.output.fd)?;
        hit_recovery(
            hooks,
            RetainedDurableRecoveryBoundaryV1::SyncDirectory,
            RetainedDurableFaultTimingV1::After,
        )?;
        let canonical_after =
            require_exact_private(self, canonical, expected_canonical, maximum_bytes)?;
        require_missing_private(self, redo, maximum_bytes)?;
        self.verify()?;
        Ok(canonical_after)
    }

    /// Durably stages exact artifact bytes under one private, non-public name.
    pub fn stage_artifact(
        &self,
        staged: &str,
        bytes: &[u8],
        maximum_bytes: usize,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        validate_name(staged)?;
        if bytes.is_empty() || bytes.len() > maximum_bytes {
            return Err(RetainedDurableDirectoryErrorV1::Size {
                actual: bytes.len(),
                maximum: maximum_bytes,
            });
        }
        require_absent(&self.output.fd, staged)?;
        let (temporary_name, mut temporary) = self.create_artifact_temp(staged, hooks)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::WriteTemp,
            RetainedDurableFaultTimingV1::Before,
        )?;
        temporary.write_all(bytes)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::WriteTemp,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncTemp,
            RetainedDurableFaultTimingV1::Before,
        )?;
        temporary.sync_all()?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncTemp,
            RetainedDurableFaultTimingV1::After,
        )?;
        validate_private_file(&temporary, &temporary_name, self.service_uid)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::RenameTempToStaged,
            RetainedDurableFaultTimingV1::Before,
        )?;
        renameat_with(
            &self.output.fd,
            &temporary_name,
            &self.output.fd,
            staged,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::RenameTempToStaged,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncStagedName,
            RetainedDurableFaultTimingV1::Before,
        )?;
        hooks.sync_directory(&self.output.fd)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncStagedName,
            RetainedDurableFaultTimingV1::After,
        )?;
        self.verify_exact(staged, bytes)
    }

    /// Recommits an exact visible artifact through a fresh staged-to-final transaction.
    ///
    /// A final name may be visible even though the directory sync that followed its rename failed.
    /// A later sync alone cannot establish that the original rename was durable after a writeback
    /// error. This operation therefore moves the validated final object back to its distinct staged
    /// name, syncs that namespace mutation, and publishes it again through a fresh final rename and
    /// directory sync. If an earlier cycle already reached the staged name, this operation resumes
    /// that exact staged object. The cycle preserves the managed-entry count at exact quotas.
    pub fn establish_recovered_artifact_durability(
        &self,
        staged: &str,
        final_entry: &str,
        expected_bytes: &[u8],
        final_mode: u32,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        validate_name(staged)?;
        validate_name(final_entry)?;
        validate_final_mode(final_mode)?;
        if staged == final_entry {
            return Err(RetainedDurableDirectoryErrorV1::InvalidName {
                entry: staged.to_owned(),
            });
        }
        let staged_exists = self.read_managed(
            staged,
            expected_bytes.len(),
            ManagedMode::PrivateOrExact(final_mode),
        )?;
        let final_exists = self.read_published(final_entry, expected_bytes.len(), final_mode)?;
        match (staged_exists, final_exists) {
            (Some(staged_bytes), None) if staged_bytes == expected_bytes => {
                return self.publish_validated_staged(
                    staged,
                    final_entry,
                    expected_bytes,
                    final_mode,
                    hooks,
                );
            }
            (None, Some(final_bytes)) if final_bytes == expected_bytes => {}
            (None, None) => {
                return Err(RetainedDurableDirectoryErrorV1::MissingEntry {
                    entry: final_entry.to_owned(),
                });
            }
            _ => {
                return Err(RetainedDurableDirectoryErrorV1::ContentMismatch {
                    entry: final_entry.to_owned(),
                });
            }
        }
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::RenameFinalToStaged,
            RetainedDurableFaultTimingV1::Before,
        )?;
        renameat_with(
            &self.output.fd,
            final_entry,
            &self.output.fd,
            staged,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::RenameFinalToStaged,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncRecoveredStagedName,
            RetainedDurableFaultTimingV1::Before,
        )?;
        hooks.sync_directory(&self.output.fd)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncRecoveredStagedName,
            RetainedDurableFaultTimingV1::After,
        )?;
        self.verify()?;
        let staged_after = self.read_managed(
            staged,
            expected_bytes.len(),
            ManagedMode::PrivateOrExact(final_mode),
        )?;
        if self
            .read_published(final_entry, expected_bytes.len(), final_mode)?
            .is_some()
            || staged_after.as_deref() != Some(expected_bytes)
        {
            return Err(RetainedDurableDirectoryErrorV1::ContentMismatch {
                entry: staged.to_owned(),
            });
        }
        self.publish_validated_staged(staged, final_entry, expected_bytes, final_mode, hooks)
    }

    /// Atomically makes one exact staged artifact visible at its bound final component.
    pub fn publish_staged(
        &self,
        staged: &str,
        final_entry: &str,
        expected_bytes: &[u8],
        final_mode: u32,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        self.verify()?;
        validate_name(staged)?;
        validate_name(final_entry)?;
        validate_final_mode(final_mode)?;
        if staged == final_entry {
            return Err(RetainedDurableDirectoryErrorV1::InvalidName {
                entry: staged.to_owned(),
            });
        }
        let staged_exists = self.read_managed(
            staged,
            expected_bytes.len(),
            ManagedMode::PrivateOrExact(final_mode),
        )?;
        let final_exists = self.read_published(final_entry, expected_bytes.len(), final_mode)?;
        match (staged_exists, final_exists) {
            (Some(staged_bytes), None) if staged_bytes == expected_bytes => {}
            (None, Some(final_bytes)) if final_bytes == expected_bytes => {
                return self.establish_recovered_artifact_durability(
                    staged,
                    final_entry,
                    &final_bytes,
                    final_mode,
                    hooks,
                );
            }
            (None, None) => {
                return Err(RetainedDurableDirectoryErrorV1::MissingEntry {
                    entry: staged.to_owned(),
                });
            }
            _ => {
                return Err(RetainedDurableDirectoryErrorV1::ContentMismatch {
                    entry: final_entry.to_owned(),
                });
            }
        }
        self.publish_validated_staged(staged, final_entry, expected_bytes, final_mode, hooks)
    }

    fn publish_validated_staged(
        &self,
        staged: &str,
        final_entry: &str,
        expected_bytes: &[u8],
        final_mode: u32,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        let staged_fd = openat(
            &self.output.fd,
            staged,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        validate_managed_file(
            &staged_fd,
            staged,
            self.service_uid,
            ManagedMode::PrivateOrExact(final_mode),
        )?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SetFinalMode,
            RetainedDurableFaultTimingV1::Before,
        )?;
        rustix::fs::fchmod(&staged_fd, Mode::from_raw_mode(final_mode)).map_err(io::Error::from)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SetFinalMode,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncFinalMode,
            RetainedDurableFaultTimingV1::Before,
        )?;
        fsync(&staged_fd).map_err(io::Error::from)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncFinalMode,
            RetainedDurableFaultTimingV1::After,
        )?;
        let prepared = validate_managed_file(
            &staged_fd,
            staged,
            self.service_uid,
            ManagedMode::Exact(final_mode),
        )?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::RenameStagedToFinal,
            RetainedDurableFaultTimingV1::Before,
        )?;
        let prepared_after_hook = validate_managed_file(
            &staged_fd,
            staged,
            self.service_uid,
            ManagedMode::Exact(final_mode),
        )?;
        let named_after_hook =
            statat(&self.output.fd, staged, AtFlags::SYMLINK_NOFOLLOW).map_err(io::Error::from)?;
        require_unchanged_managed_name(&prepared, &prepared_after_hook, &named_after_hook, staged)?;
        renameat_with(
            &self.output.fd,
            staged,
            &self.output.fd,
            final_entry,
            RenameFlags::NOREPLACE,
        )
        .map_err(io::Error::from)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::RenameStagedToFinal,
            RetainedDurableFaultTimingV1::After,
        )?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncFinalName,
            RetainedDurableFaultTimingV1::Before,
        )?;
        hooks.sync_directory(&self.output.fd)?;
        hit_artifact(
            hooks,
            RetainedDurableArtifactBoundaryV1::SyncFinalName,
            RetainedDurableFaultTimingV1::After,
        )?;
        self.verify_published_exact(final_entry, expected_bytes, final_mode)
    }

    /// Revalidates one exact public file without consulting a path outside the retained root.
    pub fn verify_exact(
        &self,
        entry: &str,
        expected_bytes: &[u8],
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        let Some(actual) = self.read_private(entry, expected_bytes.len())? else {
            return Err(RetainedDurableDirectoryErrorV1::MissingEntry {
                entry: entry.to_owned(),
            });
        };
        if actual != expected_bytes {
            return Err(RetainedDurableDirectoryErrorV1::ContentMismatch {
                entry: entry.to_owned(),
            });
        }
        Ok(())
    }

    /// Revalidates one exact-mode final file.
    pub fn verify_published_exact(
        &self,
        entry: &str,
        expected_bytes: &[u8],
        mode: u32,
    ) -> Result<(), RetainedDurableDirectoryErrorV1> {
        let Some(actual) = self.read_published(entry, expected_bytes.len(), mode)? else {
            return Err(RetainedDurableDirectoryErrorV1::MissingEntry {
                entry: entry.to_owned(),
            });
        };
        if actual != expected_bytes {
            return Err(RetainedDurableDirectoryErrorV1::ContentMismatch {
                entry: entry.to_owned(),
            });
        }
        Ok(())
    }

    /// Returns SHA-256 over one exact retained file after private-file validation.
    pub fn file_sha256(
        &self,
        entry: &str,
        maximum_bytes: usize,
    ) -> Result<Option<([u8; 32], usize)>, RetainedDurableDirectoryErrorV1> {
        Ok(self.read_private(entry, maximum_bytes)?.map(|bytes| {
            let digest = Sha256::digest(&bytes).into();
            (digest, bytes.len())
        }))
    }

    /// Returns SHA-256 over one exact-mode final file.
    pub fn published_file_sha256(
        &self,
        entry: &str,
        maximum_bytes: usize,
        mode: u32,
    ) -> Result<Option<([u8; 32], usize)>, RetainedDurableDirectoryErrorV1> {
        Ok(self
            .read_published(entry, maximum_bytes, mode)?
            .map(|bytes| {
                let digest = Sha256::digest(&bytes).into();
                (digest, bytes.len())
            }))
    }

    /// Returns SHA-256 over a staged file before or after final-mode transition.
    pub fn staged_file_sha256(
        &self,
        entry: &str,
        maximum_bytes: usize,
        final_mode: u32,
    ) -> Result<Option<([u8; 32], usize)>, RetainedDurableDirectoryErrorV1> {
        Ok(self
            .read_staged(entry, maximum_bytes, final_mode)?
            .map(|bytes| {
                let digest = Sha256::digest(&bytes).into();
                (digest, bytes.len())
            }))
    }

    fn verify(&self) -> Result<(), RetainedDurableDirectoryErrorV1> {
        let stat = fstat(&self.output.fd).map_err(io::Error::from)?;
        validate_root_stat(&stat, self.service_uid)?;
        if stat.st_dev != self.output.device || stat.st_ino != self.output.inode {
            return Err(RetainedDurableDirectoryErrorV1::DirectoryIdentityChanged);
        }
        Ok(())
    }

    fn create_temp<F>(
        &self,
        base: &str,
        mut hook: F,
    ) -> Result<(String, fs::File), RetainedDurableDirectoryErrorV1>
    where
        F: FnMut(RetainedDurableRecordBoundaryV1, RetainedDurableFaultTimingV1) -> io::Result<()>,
    {
        let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
        for offset in 0..MAX_TEMP_ATTEMPTS {
            let candidate = format!(
                "{base}.tmp-{}-{}",
                std::process::id(),
                start.wrapping_add(offset)
            );
            hook(
                RetainedDurableRecordBoundaryV1::CreateTemp,
                RetainedDurableFaultTimingV1::Before,
            )?;
            match openat(
                &self.output.fd,
                &candidate,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(fd) => {
                    hook(
                        RetainedDurableRecordBoundaryV1::CreateTemp,
                        RetainedDurableFaultTimingV1::After,
                    )?;
                    return Ok((candidate, fs::File::from(fd)));
                }
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(error) => return Err(io::Error::from(error).into()),
            }
        }
        Err(Self::unsafe_entry(
            base,
            "private temporary-name space exhausted",
        ))
    }

    fn create_artifact_temp(
        &self,
        base: &str,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
    ) -> Result<(String, fs::File), RetainedDurableDirectoryErrorV1> {
        let start = NEXT_TEMP_ID.fetch_add(MAX_TEMP_ATTEMPTS, Ordering::Relaxed);
        for offset in 0..MAX_TEMP_ATTEMPTS {
            let candidate = format!(
                "{base}.tmp-{}-{}",
                std::process::id(),
                start.wrapping_add(offset)
            );
            hit_artifact(
                hooks,
                RetainedDurableArtifactBoundaryV1::CreateTemp,
                RetainedDurableFaultTimingV1::Before,
            )?;
            match openat(
                &self.output.fd,
                &candidate,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(fd) => {
                    hit_artifact(
                        hooks,
                        RetainedDurableArtifactBoundaryV1::CreateTemp,
                        RetainedDurableFaultTimingV1::After,
                    )?;
                    return Ok((candidate, fs::File::from(fd)));
                }
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(error) => return Err(io::Error::from(error).into()),
            }
        }
        Err(Self::unsafe_entry(
            base,
            "private temporary-name space exhausted",
        ))
    }

    fn unsafe_entry(entry: &str, reason: &str) -> RetainedDurableDirectoryErrorV1 {
        RetainedDurableDirectoryErrorV1::UnsafeEntry {
            entry: entry.to_owned(),
            reason: reason.to_owned(),
        }
    }
}

fn validate_root_stat(
    stat: &rustix::fs::Stat,
    service_uid: u32,
) -> Result<(), RetainedDurableDirectoryErrorV1> {
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
        || stat.st_uid != service_uid
        || stat.st_mode & 0o777 != 0o700
        || stat.st_nlink == 0
    {
        return Err(RetainedDurableDirectoryV1::unsafe_entry(
            "<service-root-fd>",
            "root must remain a linked service-owned 0700 directory",
        ));
    }
    Ok(())
}

fn validate_private_file(
    descriptor: &impl rustix::fd::AsFd,
    entry: &str,
    service_uid: u32,
) -> Result<rustix::fs::Stat, RetainedDurableDirectoryErrorV1> {
    let stat = fstat(descriptor).map_err(io::Error::from)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != service_uid
        || stat.st_mode & 0o077 != 0
        || stat.st_nlink != 1
    {
        return Err(RetainedDurableDirectoryV1::unsafe_entry(
            entry,
            "entry must be a service-owned private single-link regular file",
        ));
    }
    Ok(stat)
}

#[derive(Clone, Copy)]
enum ManagedMode {
    Private,
    Exact(u32),
    PrivateOrExact(u32),
}

fn validate_managed_file(
    descriptor: &impl rustix::fd::AsFd,
    entry: &str,
    service_uid: u32,
    mode: ManagedMode,
) -> Result<rustix::fs::Stat, RetainedDurableDirectoryErrorV1> {
    let stat = fstat(descriptor).map_err(io::Error::from)?;
    let permissions = stat.st_mode & 0o7777;
    let valid_mode = match mode {
        ManagedMode::Private => permissions & 0o077 == 0,
        ManagedMode::Exact(expected) => permissions == expected,
        ManagedMode::PrivateOrExact(expected) => {
            permissions & 0o077 == 0 || permissions == expected
        }
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != service_uid
        || stat.st_nlink != 1
        || !valid_mode
    {
        return Err(RetainedDurableDirectoryV1::unsafe_entry(
            entry,
            "entry has unsafe type, owner, mode, or link count",
        ));
    }
    Ok(stat)
}

fn validate_final_mode(mode: u32) -> Result<(), RetainedDurableDirectoryErrorV1> {
    if mode == 0 || mode & !0o777 != 0 {
        return Err(RetainedDurableDirectoryErrorV1::UnsafeEntry {
            entry: "<final-mode>".to_owned(),
            reason: "final mode must contain only nonzero rwx permission bits".to_owned(),
        });
    }
    Ok(())
}

fn require_unchanged_managed_name(
    before: &rustix::fs::Stat,
    after: &rustix::fs::Stat,
    named: &rustix::fs::Stat,
    entry: &str,
) -> Result<(), RetainedDurableDirectoryErrorV1> {
    let unchanged = before.st_dev == after.st_dev
        && before.st_ino == after.st_ino
        && before.st_mode == after.st_mode
        && before.st_uid == after.st_uid
        && before.st_nlink == after.st_nlink
        && before.st_size == after.st_size
        && before.st_mtime == after.st_mtime
        && before.st_mtime_nsec == after.st_mtime_nsec
        && before.st_ctime == after.st_ctime
        && before.st_ctime_nsec == after.st_ctime_nsec;
    let still_named = after.st_dev == named.st_dev
        && after.st_ino == named.st_ino
        && after.st_mode == named.st_mode
        && after.st_uid == named.st_uid
        && after.st_nlink == named.st_nlink
        && after.st_size == named.st_size;
    if !unchanged || !still_named {
        return Err(RetainedDurableDirectoryV1::unsafe_entry(
            entry,
            "prepared staged entry changed before its final rename",
        ));
    }
    Ok(())
}

fn validate_name(entry: &str) -> Result<(), RetainedDurableDirectoryErrorV1> {
    let valid = !entry.is_empty()
        && entry.len() <= 240
        && entry != "."
        && entry != ".."
        && entry
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(RetainedDurableDirectoryErrorV1::InvalidName {
            entry: entry.to_owned(),
        })
    }
}

fn require_absent(directory: &OwnedFd, entry: &str) -> Result<(), RetainedDurableDirectoryErrorV1> {
    match statat(directory, entry, AtFlags::SYMLINK_NOFOLLOW) {
        Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
        Ok(_) => Err(RetainedDurableDirectoryErrorV1::ExistingEntry {
            entry: entry.to_owned(),
        }),
        Err(error) => Err(io::Error::from(error).into()),
    }
}

fn require_exact_private(
    store: &RetainedDurableDirectoryV1,
    entry: &str,
    expected: &[u8],
    maximum_bytes: usize,
) -> Result<Vec<u8>, RetainedDurableDirectoryErrorV1> {
    let actual = store.read_private(entry, maximum_bytes)?.ok_or_else(|| {
        RetainedDurableDirectoryErrorV1::MissingEntry {
            entry: entry.to_owned(),
        }
    })?;
    if actual != expected {
        return Err(RetainedDurableDirectoryErrorV1::ContentMismatch {
            entry: entry.to_owned(),
        });
    }
    Ok(actual)
}

fn require_missing_private(
    store: &RetainedDurableDirectoryV1,
    entry: &str,
    maximum_bytes: usize,
) -> Result<(), RetainedDurableDirectoryErrorV1> {
    if store.read_private(entry, maximum_bytes)?.is_some() {
        return Err(RetainedDurableDirectoryErrorV1::ExistingEntry {
            entry: entry.to_owned(),
        });
    }
    Ok(())
}

fn hit_record(
    hooks: &mut impl RetainedDurableDirectoryHooksV1,
    boundary: RetainedDurableRecordBoundaryV1,
    timing: RetainedDurableFaultTimingV1,
) -> Result<(), RetainedDurableDirectoryErrorV1> {
    hooks.record(boundary, timing).map_err(Into::into)
}

fn hit_artifact(
    hooks: &mut impl RetainedDurableDirectoryHooksV1,
    boundary: RetainedDurableArtifactBoundaryV1,
    timing: RetainedDurableFaultTimingV1,
) -> Result<(), RetainedDurableDirectoryErrorV1> {
    hooks.artifact(boundary, timing).map_err(Into::into)
}

fn hit_recovery(
    hooks: &mut impl RetainedDurableDirectoryHooksV1,
    boundary: RetainedDurableRecoveryBoundaryV1,
    timing: RetainedDurableFaultTimingV1,
) -> Result<(), RetainedDurableDirectoryErrorV1> {
    hooks.recovery(boundary, timing).map_err(Into::into)
}

fn hit_recovery_mutation(
    hooks: &mut impl RetainedDurableDirectoryHooksV1,
    boundary: RetainedDurableRecoveryMutationBoundaryV1,
    timing: RetainedDurableFaultTimingV1,
) -> Result<(), RetainedDurableDirectoryErrorV1> {
    hooks
        .recovery_mutation(boundary, timing)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::fs::File;
    use std::os::unix::fs::PermissionsExt;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(1);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-retained-durable-{}-{}",
                std::process::id(),
                NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self { path }
        }

        fn store(&self) -> RetainedDurableDirectoryV1 {
            RetainedDurableDirectoryV1::admit_service_owned(File::open(&self.path).unwrap().into())
                .unwrap()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }

    struct AmbiguousCommitHook;

    impl RetainedDurableDirectoryHooksV1 for AmbiguousCommitHook {
        fn record(
            &mut self,
            boundary: RetainedDurableRecordBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            if boundary == RetainedDurableRecordBoundaryV1::SyncCanonicalName
                && timing == RetainedDurableFaultTimingV1::Before
            {
                Err(io::Error::other("ambiguous canonical rename"))
            } else {
                Ok(())
            }
        }
    }

    #[derive(Default)]
    struct ArtifactRecoveryHook {
        fail_at: Option<(
            RetainedDurableArtifactBoundaryV1,
            RetainedDurableFaultTimingV1,
        )>,
        fail_sync_call: Option<usize>,
        sync_calls: usize,
        events: Vec<(
            RetainedDurableArtifactBoundaryV1,
            RetainedDurableFaultTimingV1,
        )>,
    }

    impl RetainedDurableDirectoryHooksV1 for ArtifactRecoveryHook {
        fn sync_directory(&mut self, directory: &OwnedFd) -> io::Result<()> {
            self.sync_calls += 1;
            if self.fail_sync_call == Some(self.sync_calls) {
                return Err(io::Error::from_raw_os_error(libc::EIO));
            }
            fsync(directory).map_err(io::Error::from)
        }

        fn artifact(
            &mut self,
            boundary: RetainedDurableArtifactBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            self.events.push((boundary, timing));
            if self.fail_at == Some((boundary, timing)) {
                Err(io::Error::other("simulated artifact crash"))
            } else {
                Ok(())
            }
        }
    }

    struct StagedSubstitutionHook {
        staged: PathBuf,
        displaced: PathBuf,
        replacement: Vec<u8>,
    }

    impl RetainedDurableDirectoryHooksV1 for StagedSubstitutionHook {
        fn artifact(
            &mut self,
            boundary: RetainedDurableArtifactBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            if boundary == RetainedDurableArtifactBoundaryV1::RenameStagedToFinal
                && timing == RetainedDurableFaultTimingV1::Before
            {
                fs::rename(&self.staged, &self.displaced)?;
                fs::write(&self.staged, &self.replacement)?;
                fs::set_permissions(&self.staged, fs::Permissions::from_mode(0o444))?;
            }
            Ok(())
        }
    }

    type RecoveryEvent = (
        RetainedDurableRecordBoundaryV1,
        RetainedDurableFaultTimingV1,
    );
    type RecoveryAfterEvent = (
        RetainedDurableRecordBoundaryV1,
        RetainedDurableFaultTimingV1,
        Box<dyn FnMut() -> io::Result<()>>,
    );

    struct RecoveryHook {
        fail_at: Option<RecoveryEvent>,
        fail_recovery_at: Option<RetainedDurableFaultTimingV1>,
        fail_recovery_mutation_at: Option<RetainedDurableFaultTimingV1>,
        after_event: Option<RecoveryAfterEvent>,
        fail_sync_call: Option<usize>,
        sync_calls: usize,
        events: Vec<RecoveryEvent>,
        recovery_events: Vec<RetainedDurableFaultTimingV1>,
        recovery_mutation_events: Vec<RetainedDurableFaultTimingV1>,
    }

    impl RecoveryHook {
        fn tracing() -> Self {
            Self {
                fail_at: None,
                fail_recovery_at: None,
                fail_recovery_mutation_at: None,
                after_event: None,
                fail_sync_call: None,
                sync_calls: 0,
                events: Vec::new(),
                recovery_events: Vec::new(),
                recovery_mutation_events: Vec::new(),
            }
        }
    }

    impl RetainedDurableDirectoryHooksV1 for RecoveryHook {
        fn sync_directory(&mut self, directory: &OwnedFd) -> io::Result<()> {
            self.sync_calls += 1;
            if self.fail_sync_call == Some(self.sync_calls) {
                return Err(io::Error::from_raw_os_error(libc::EIO));
            }
            fsync(directory).map_err(io::Error::from)
        }

        fn record(
            &mut self,
            boundary: RetainedDurableRecordBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            self.events.push((boundary, timing));
            if let Some((after_boundary, after_timing, callback)) = &mut self.after_event
                && (*after_boundary, *after_timing) == (boundary, timing)
            {
                callback()?;
            }
            if self.fail_at == Some((boundary, timing)) {
                Err(io::Error::other("injected recovery barrier failure"))
            } else {
                Ok(())
            }
        }

        fn recovery(
            &mut self,
            boundary: RetainedDurableRecoveryBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            assert_eq!(boundary, RetainedDurableRecoveryBoundaryV1::SyncDirectory);
            self.recovery_events.push(timing);
            if self.fail_recovery_at == Some(timing) {
                Err(io::Error::other("injected legacy recovery hook failure"))
            } else {
                Ok(())
            }
        }

        fn recovery_mutation(
            &mut self,
            boundary: RetainedDurableRecoveryMutationBoundaryV1,
            timing: RetainedDurableFaultTimingV1,
        ) -> io::Result<()> {
            assert_eq!(
                boundary,
                RetainedDurableRecoveryMutationBoundaryV1::RenameCanonicalToRecovery
            );
            self.recovery_mutation_events.push(timing);
            if self.fail_recovery_mutation_at == Some(timing) {
                Err(io::Error::other("injected recovery mutation failure"))
            } else {
                Ok(())
            }
        }
    }

    fn leave_ambiguous_canonical(
        store: &RetainedDurableDirectoryV1,
        canonical: &str,
        redo: &str,
        bytes: &[u8],
    ) {
        let result = store.commit_record(canonical, redo, bytes, 1024, &mut AmbiguousCommitHook);
        assert!(result.is_err());
        assert_eq!(
            store.read_private(canonical, 1024).unwrap(),
            Some(bytes.to_vec())
        );
        assert_eq!(store.read_private(redo, 1024).unwrap(), None);
    }

    fn write_private(path: &std::path::Path, bytes: &[u8]) -> io::Result<()> {
        fs::write(path, bytes)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
    }

    #[test]
    fn visible_artifact_after_directory_fsync_eio_requires_a_fresh_rename_commit() {
        const FINAL_MODE: u32 = 0o444;
        let directory = TestDirectory::new();
        let store = directory.store();
        let expected = b"content-addressed compiler object";
        let mut no_faults = NoRetainedDurableDirectoryHooksV1;
        store
            .stage_artifact("object.staged", expected, 1024, &mut no_faults)
            .unwrap();

        let mut failing = ArtifactRecoveryHook {
            fail_sync_call: Some(1),
            ..ArtifactRecoveryHook::default()
        };
        let error = store
            .publish_staged(
                "object.staged",
                "object.final",
                expected,
                FINAL_MODE,
                &mut failing,
            )
            .unwrap_err();
        assert!(
            error
                .source()
                .unwrap()
                .to_string()
                .contains("Input/output error")
        );
        store
            .verify_published_exact("object.final", expected, FINAL_MODE)
            .unwrap();
        assert!(store.read_private("object.staged", 1024).unwrap().is_none());

        let mut recovery = ArtifactRecoveryHook::default();
        store
            .publish_staged(
                "object.staged",
                "object.final",
                expected,
                FINAL_MODE,
                &mut recovery,
            )
            .unwrap();

        assert_eq!(
            recovery.events,
            [
                (
                    RetainedDurableArtifactBoundaryV1::RenameFinalToStaged,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::RenameFinalToStaged,
                    RetainedDurableFaultTimingV1::After,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SyncRecoveredStagedName,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SyncRecoveredStagedName,
                    RetainedDurableFaultTimingV1::After,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SetFinalMode,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SetFinalMode,
                    RetainedDurableFaultTimingV1::After,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SyncFinalMode,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SyncFinalMode,
                    RetainedDurableFaultTimingV1::After,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::RenameStagedToFinal,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::RenameStagedToFinal,
                    RetainedDurableFaultTimingV1::After,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SyncFinalName,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableArtifactBoundaryV1::SyncFinalName,
                    RetainedDurableFaultTimingV1::After,
                ),
            ]
        );
        assert_eq!(recovery.sync_calls, 2);
        store
            .verify_published_exact("object.final", expected, FINAL_MODE)
            .unwrap();
        assert!(store.read_private("object.staged", 1024).unwrap().is_none());
    }

    #[test]
    fn recommit_resumes_after_final_is_renamed_or_staged_name_is_synced() {
        const FINAL_MODE: u32 = 0o444;
        for boundary in [
            RetainedDurableArtifactBoundaryV1::RenameFinalToStaged,
            RetainedDurableArtifactBoundaryV1::SyncRecoveredStagedName,
        ] {
            let directory = TestDirectory::new();
            let store = directory.store();
            let expected = b"restart-safe content-addressed object";
            let mut no_faults = NoRetainedDurableDirectoryHooksV1;
            store
                .stage_artifact("object.staged", expected, 1024, &mut no_faults)
                .unwrap();
            store
                .publish_staged(
                    "object.staged",
                    "object.final",
                    expected,
                    FINAL_MODE,
                    &mut no_faults,
                )
                .unwrap();

            let mut crash = ArtifactRecoveryHook {
                fail_at: Some((boundary, RetainedDurableFaultTimingV1::After)),
                ..ArtifactRecoveryHook::default()
            };
            assert!(
                store
                    .establish_recovered_artifact_durability(
                        "object.staged",
                        "object.final",
                        expected,
                        FINAL_MODE,
                        &mut crash,
                    )
                    .is_err(),
                "{boundary:?}"
            );
            assert!(
                store
                    .read_published("object.final", 1024, FINAL_MODE)
                    .unwrap()
                    .is_none(),
                "{boundary:?}"
            );
            assert_eq!(
                store
                    .read_staged("object.staged", 1024, FINAL_MODE)
                    .unwrap()
                    .as_deref(),
                Some(expected.as_slice()),
                "{boundary:?}"
            );

            store
                .establish_recovered_artifact_durability(
                    "object.staged",
                    "object.final",
                    expected,
                    FINAL_MODE,
                    &mut no_faults,
                )
                .unwrap();
            store
                .verify_published_exact("object.final", expected, FINAL_MODE)
                .unwrap();
            assert!(store.read_private("object.staged", 1024).unwrap().is_none());
        }
    }

    #[test]
    fn staged_name_substitution_before_final_rename_is_rejected() {
        const FINAL_MODE: u32 = 0o444;
        let directory = TestDirectory::new();
        let store = directory.store();
        let expected = b"validated staged artifact";
        let mut no_faults = NoRetainedDurableDirectoryHooksV1;
        store
            .stage_artifact("object.staged", expected, 1024, &mut no_faults)
            .unwrap();
        let mut substitution = StagedSubstitutionHook {
            staged: directory.path.join("object.staged"),
            displaced: directory.path.join("validated-staged-object"),
            replacement: b"substitute staged artifact".to_vec(),
        };

        assert!(
            store
                .publish_staged(
                    "object.staged",
                    "object.final",
                    expected,
                    FINAL_MODE,
                    &mut substitution,
                )
                .is_err()
        );
        assert!(
            store
                .read_published("object.final", 1024, FINAL_MODE)
                .unwrap()
                .is_none()
        );
        assert_eq!(fs::read(substitution.displaced).unwrap(), expected);
    }

    #[test]
    fn recovery_recommits_and_rereads_an_ambiguous_canonical_name() {
        let directory = TestDirectory::new();
        let store = directory.store();
        let expected = b"canonical prepared record";
        leave_ambiguous_canonical(&store, "record", "redo", expected);

        let mut hook = RecoveryHook::tracing();
        let recovered = store
            .establish_recovered_record_durability("record", "redo", expected, 1024, &mut hook)
            .unwrap();

        assert_eq!(recovered, expected);
        assert_eq!(
            hook.events,
            [
                (
                    RetainedDurableRecordBoundaryV1::SyncRedoName,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableRecordBoundaryV1::SyncRedoName,
                    RetainedDurableFaultTimingV1::After,
                ),
                (
                    RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
                    RetainedDurableFaultTimingV1::After,
                ),
                (
                    RetainedDurableRecordBoundaryV1::SyncCanonicalName,
                    RetainedDurableFaultTimingV1::Before,
                ),
                (
                    RetainedDurableRecordBoundaryV1::SyncCanonicalName,
                    RetainedDurableFaultTimingV1::After,
                ),
            ]
        );
        assert_eq!(
            hook.recovery_mutation_events,
            [
                RetainedDurableFaultTimingV1::Before,
                RetainedDurableFaultTimingV1::After,
            ]
        );
        assert_eq!(
            hook.recovery_events,
            [
                RetainedDurableFaultTimingV1::Before,
                RetainedDurableFaultTimingV1::After,
            ]
        );
        assert_eq!(hook.sync_calls, 3);
    }

    #[test]
    fn recovery_barrier_hook_failures_return_no_revalidated_bytes() {
        for boundary in [
            RetainedDurableRecordBoundaryV1::SyncRedoName,
            RetainedDurableRecordBoundaryV1::RenameRedoToCanonical,
            RetainedDurableRecordBoundaryV1::SyncCanonicalName,
        ] {
            for timing in [
                RetainedDurableFaultTimingV1::Before,
                RetainedDurableFaultTimingV1::After,
            ] {
                let directory = TestDirectory::new();
                let store = directory.store();
                let expected = b"canonical prepared record";
                leave_ambiguous_canonical(&store, "record", "redo", expected);
                let mut hook = RecoveryHook {
                    fail_at: Some((boundary, timing)),
                    ..RecoveryHook::tracing()
                };

                assert!(
                    store
                        .establish_recovered_record_durability(
                            "record", "redo", expected, 1024, &mut hook,
                        )
                        .is_err()
                );
                assert_eq!(hook.events.last(), Some(&(boundary, timing)));
            }
        }
    }

    #[test]
    fn recovery_specific_and_legacy_hook_failures_return_no_revalidated_bytes() {
        for timing in [
            RetainedDurableFaultTimingV1::Before,
            RetainedDurableFaultTimingV1::After,
        ] {
            let mutation_directory = TestDirectory::new();
            let mutation_store = mutation_directory.store();
            let expected = b"canonical prepared record";
            leave_ambiguous_canonical(&mutation_store, "record", "recovery", expected);
            let mut mutation_hook = RecoveryHook {
                fail_recovery_mutation_at: Some(timing),
                ..RecoveryHook::tracing()
            };
            assert!(
                mutation_store
                    .establish_recovered_record_durability(
                        "record",
                        "recovery",
                        expected,
                        1024,
                        &mut mutation_hook,
                    )
                    .is_err()
            );

            let legacy_directory = TestDirectory::new();
            let legacy_store = legacy_directory.store();
            leave_ambiguous_canonical(&legacy_store, "record", "recovery", expected);
            let mut legacy_hook = RecoveryHook {
                fail_recovery_at: Some(timing),
                ..RecoveryHook::tracing()
            };
            assert!(
                legacy_store
                    .establish_recovered_record_durability(
                        "record",
                        "recovery",
                        expected,
                        1024,
                        &mut legacy_hook,
                    )
                    .is_err()
            );
        }
    }

    #[test]
    fn recovery_returns_no_bytes_when_any_directory_fsync_returns_eio() {
        for fail_sync_call in [1, 2, 3] {
            let directory = TestDirectory::new();
            let store = directory.store();
            let expected = b"canonical prepared record";
            leave_ambiguous_canonical(&store, "record", "redo", expected);
            let mut hook = RecoveryHook {
                fail_sync_call: Some(fail_sync_call),
                ..RecoveryHook::tracing()
            };

            let error = store
                .establish_recovered_record_durability("record", "redo", expected, 1024, &mut hook)
                .unwrap_err();
            let source = error.source().unwrap();
            assert!(source.to_string().contains("Input/output error"));

            let mut no_faults = NoRetainedDurableDirectoryHooksV1;
            if store.read_private("redo", 1024).unwrap().is_some() {
                store
                    .promote_validated_redo("record", "redo", None, expected, 1024, &mut no_faults)
                    .unwrap();
            }
            assert_eq!(
                store
                    .establish_recovered_record_durability(
                        "record",
                        "redo",
                        expected,
                        1024,
                        &mut no_faults,
                    )
                    .unwrap(),
                expected,
                "recovery after fsync call {fail_sync_call} must perform a fresh transaction",
            );
        }
    }

    #[test]
    fn recovery_barrier_rejects_post_sync_canonical_or_redo_substitution() {
        for replace_canonical in [true, false] {
            let directory = TestDirectory::new();
            let store = directory.store();
            let expected = b"canonical prepared record";
            leave_ambiguous_canonical(&store, "record", "redo", expected);
            let target = directory
                .path
                .join(if replace_canonical { "record" } else { "redo" });
            let mut hook = RecoveryHook {
                after_event: Some((
                    RetainedDurableRecordBoundaryV1::SyncCanonicalName,
                    RetainedDurableFaultTimingV1::After,
                    Box::new(move || write_private(&target, b"hostile replacement")),
                )),
                ..RecoveryHook::tracing()
            };

            assert!(
                store
                    .establish_recovered_record_durability(
                        "record", "redo", expected, 1024, &mut hook,
                    )
                    .is_err()
            );
            assert_eq!(
                hook.events.last(),
                Some(&(
                    RetainedDurableRecordBoundaryV1::SyncCanonicalName,
                    RetainedDurableFaultTimingV1::After,
                ))
            );
        }
    }
}
