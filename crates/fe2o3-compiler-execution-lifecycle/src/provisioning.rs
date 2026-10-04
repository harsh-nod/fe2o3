//! Bounded exclusive custody of the fixed production lifecycle lock.

use crate::{
    COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1, LifecycleLeaseErrorV1 as LeaseError,
    LifecycleLeaseErrorV2 as Error, ObjectSnapshotV1, ROOT_ID_V1, io_error, lifecycle_name,
    validate_file, validate_named_file, validate_parent,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::{CWD, FlockOperation, Mode, OFlags, ResolveFlags, flock, fstat, openat, openat2};
use std::{fmt, fs::File, mem::size_of, path::Path};

const ENTRY_WORK: usize = 8;
type Result<T> = std::result::Result<T, Error>;
use ProvisioningLifecycleLeaseStorageV2 as Storage;

/// Full unreserved logical charge returned with an exclusive lifecycle owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisioningLifecycleLeaseStorageV2(usize);

impl ProvisioningLifecycleLeaseStorageV2 {
    /// Reserve this full charge before retaining the owner; drop before releasing it.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Move-only native exclusive provisioning lifecycle custody.
///
/// Opens only the existing production lifecycle pathname, with parent and file
/// owned by root (0:0). No shared lease is constructed or converted. The exact
/// parent and lock descriptors remain private; there is no clone, transfer,
/// descriptor exposure, extraction, or caller-selected production path/owner.
/// This lock alone grants no signing, publication, process or execution authority.
///
/// Each operation prepays fixed work and scratch on the original ledger before
/// filesystem access. Open has no borrowed input owner (input floor zero) and
/// returns the FULL owner/receipt charge. Revalidation requires that full charge
/// prepaid. All exits restore entry storage, preserving work, peak and first-denial
/// history. Drop closes descriptors only, never LOCK_UN. All syscalls are single
/// attempts, including nonblocking exclusive flock; EINTR is terminal.
///
/// Revalidation checks policy and exact device/inode identity for the retained
/// parent, its absolute canonical pathname, and the lock before and after flock.
/// Path resolution refuses symlinks in every parent component. Observations are
/// point-in-time checks, not protection against subsequent privileged renames.
/// Quotas bound logical work/storage, not syscall latency, allocator, stack or RSS.
///
/// ```
/// use fe2o3_compiler_execution_lifecycle::{
///     CompilerExecutionProvisioningLifecycleLeaseV2 as Lease,
///     LifecycleLeaseErrorV2 as Error, ProvisioningLifecycleLeaseStorageV2 as Storage,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn open(b: &mut Budget<'_>) -> Result<(Lease, Storage), Error> { Lease::open(b) }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// fn duplicate(lease: Lease) { let _ = lease.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// fn copy<T: Copy>() {} copy::<Lease>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<Lease>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// fn descriptor<T: std::os::fd::AsRawFd>() {} descriptor::<Lease>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// fn descriptor<T: std::os::fd::IntoRawFd>() {} descriptor::<Lease>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn transfer(lease: &Lease, b: &mut Budget<'_>) { lease.try_clone_for_transfer(b); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::{CompilerExecutionServiceLifecycleLeaseV1 as Shared,
///     CompilerExecutionProvisioningLifecycleLeaseV2 as Lease};
/// fn upgrade(shared: Shared) -> Lease { shared.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// fn extract(lease: Lease) -> std::fs::File { lease.file }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionProvisioningLifecycleLeaseV2 as Lease;
/// fn unmetered(lease: &Lease) { lease.revalidate(); }
/// ```
pub struct CompilerExecutionProvisioningLifecycleLeaseV2 {
    file: File,
    parent: File,
    snapshot: ObjectSnapshotV1,
    parent_identity: (u64, u64),
    expected_uid: u32,
    expected_gid: u32,
}

impl CompilerExecutionProvisioningLifecycleLeaseV2 {
    const RETAINED: usize = size_of::<(Self, Storage)>();

    /// Complete open work, including entry, two pathname checks, flock and cleanup.
    /// At most 66 filesystem calls; 96 weighted slots also cover fixed control work.
    pub const ADMISSION_WORK: usize = ENTRY_WORK + 96 * 1024;
    /// Complete revalidation work, including entry, two pathname checks and cleanup.
    /// Each check costs 24 calls, including the temporary parent's close, plus flock.
    pub const REVALIDATION_WORK: usize = ENTRY_WORK + 64 * 1024;
    /// Additional logical peak, including all owner/result staging, temporary parent,
    /// metadata, bounded path conversion, attribute bytes, errors and cleanup.
    pub const IO_STORAGE: usize = 4 * Self::RETAINED + 8 * size_of::<rustix::fs::Stat>() + 4096;

    /// Opens the fixed root-owned production lock and returns its full owner charge.
    /// The lock must already exist; this operation never creates or repairs it.
    pub fn open(budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::open_at(canonical_parent(), ROOT_ID_V1, ROOT_ID_V1, budget, |_| {
            Ok(())
        })
    }

    // Owner/path overrides and the post-lock observer are private fixture machinery.
    fn open_at(
        parent_path: &Path,
        uid: u32,
        gid: u32,
        budget: &mut Budget<'_>,
        after_lock: impl FnOnce(&Self) -> Result<()>,
    ) -> Result<(Self, Storage)> {
        budget.with_prepaid_scope(
            0,
            ENTRY_WORK,
            Self::ADMISSION_WORK,
            Self::IO_STORAGE,
            |_| {
                let parent = open_absolute_parent(parent_path)?;
                validate_parent(&parent, uid, gid)?;
                let parent_identity = parent_identity(&parent)?;
                // NONBLOCK prevents a substituted FIFO from blocking before type validation.
                let file = File::from(
                    openat(
                        &parent,
                        lifecycle_name()?,
                        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                        Mode::empty(),
                    )
                    .map_err(|e| io_error("open provisioning lifecycle lock", e))?,
                );
                let snapshot = validate_file(&file, uid, gid)?;
                let lease = Self {
                    file,
                    parent,
                    snapshot,
                    parent_identity,
                    expected_uid: uid,
                    expected_gid: gid,
                };
                lease.check_paths(parent_path)?;
                acquire_exclusive(&lease.file)?;
                after_lock(&lease)?;
                lease.check_paths(parent_path)?;
                Ok((lease, Storage(Self::RETAINED)))
            },
        )
    }

    /// Rechecks the retained objects, absolute production pathname and exclusive lock.
    /// Requires the complete owner charge prepaid and restores the entry storage floor.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.revalidate_at(canonical_parent(), budget)
    }

    fn revalidate_at(&self, parent_path: &Path, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(
            Self::RETAINED,
            ENTRY_WORK,
            Self::REVALIDATION_WORK,
            Self::IO_STORAGE,
            |_| {
                self.check_paths(parent_path)?;
                acquire_exclusive(&self.file)?;
                self.check_paths(parent_path)
            },
        )
    }

    fn check_paths(&self, parent_path: &Path) -> Result<()> {
        validate_parent(&self.parent, self.expected_uid, self.expected_gid)?;
        if parent_identity(&self.parent)? != self.parent_identity {
            return Err(Error::ParentChanged);
        }
        if validate_file(&self.file, self.expected_uid, self.expected_gid)? != self.snapshot {
            return Err(LeaseError::FileChanged.into());
        }
        validate_named_file(&self.parent, self.snapshot)?;
        let named_parent = open_absolute_parent(parent_path)?;
        validate_parent(&named_parent, self.expected_uid, self.expected_gid)?;
        if parent_identity(&named_parent)? != self.parent_identity {
            return Err(Error::ParentChanged);
        }
        validate_named_file(&named_parent, self.snapshot)?;
        Ok(())
    }

    /// Full owner/receipt charge; retire only after dropping the close-only owner.
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
}

fn canonical_parent() -> &'static Path {
    Path::new(COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1)
        .parent()
        .expect("fixed absolute lifecycle pathname has a parent")
}

fn open_absolute_parent(path: &Path) -> Result<File> {
    openat2(
        CWD,
        path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|e| io_error("open canonical provisioning lifecycle parent", e).into())
}

fn parent_identity(parent: &File) -> Result<(u64, u64)> {
    let stat = fstat(parent).map_err(|e| io_error("inspect provisioning parent identity", e))?;
    Ok((stat.st_dev, stat.st_ino))
}

fn acquire_exclusive(file: &File) -> Result<()> {
    flock(file, FlockOperation::NonBlockingLockExclusive).map_err(|e| {
        if e == rustix::io::Errno::WOULDBLOCK {
            LeaseError::Busy.into()
        } else {
            io_error("acquire provisioning lifecycle lease", e).into()
        }
    })
}

impl fmt::Debug for CompilerExecutionProvisioningLifecycleLeaseV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionProvisioningLifecycleLeaseV2")
            .field("authority", &"exclusive-deployment-lifecycle-only")
            .finish_non_exhaustive()
    }
}

const _: () = assert!(
    8 * size_of::<Error>()
        + 4 * size_of::<ObjectSnapshotV1>()
        + 64 * size_of::<usize>()
        + COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1.len()
        <= 4096
);

#[cfg(test)]
#[path = "provisioning_tests.rs"]
mod tests;
