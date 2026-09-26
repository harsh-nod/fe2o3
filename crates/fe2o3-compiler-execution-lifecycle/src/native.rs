//! Metered, non-exportable custody over the same lifecycle validation and locks.

use crate::{
    CompilerExecutionServiceLifecycleLeaseV1 as Lease, LifecycleLeaseErrorV1 as LeaseError,
    ROOT_ID_V1, io_error, lifecycle_name, open_parent, retain_parent_at, validate_parent,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::fs::{Mode, OFlags, fstat, openat};
use std::{
    error::Error,
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{AsFd, RawFd},
};

const ENTRY_WORK: usize = 8;
type Result<T> = std::result::Result<T, LifecycleLeaseErrorV2>;
use self::LifecycleLeaseStorageV2 as Storage;

/// Additional unreserved logical storage returned with a native lifecycle owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleLeaseStorageV2(usize);

impl LifecycleLeaseStorageV2 {
    /// Preserve consumed reservations and reserve this delta before retaining the owner.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Native lifecycle accounting or shared descriptor-policy failure.
#[derive(Debug)]
#[non_exhaustive]
pub enum LifecycleLeaseErrorV2 {
    /// Work, storage, an incoming floor, or checked accounting was refused.
    Resource(Resource),
    /// The shared root-owned lifecycle-file contract or a finite syscall failed.
    Lease(LeaseError),
    /// The retained parent descriptor no longer denotes its admitted device/inode.
    ParentChanged,
}

impl From<Resource> for LifecycleLeaseErrorV2 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<LeaseError> for LifecycleLeaseErrorV2 {
    fn from(error: LeaseError) -> Self {
        Self::Lease(error)
    }
}
impl fmt::Display for LifecycleLeaseErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Lease(error) => error.fmt(f),
            Self::ParentChanged => f.write_str("lifecycle parent identity changed"),
        }
    }
}
impl Error for LifecycleLeaseErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Lease(error) => Some(error),
            Self::ParentChanged => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Owner {
    Root,
    #[cfg(any(test, feature = "test-support"))]
    SameOwnerTest,
}
impl Owner {
    fn ids(self) -> (u32, u32) {
        match self {
            Self::Root => (ROOT_ID_V1, ROOT_ID_V1),
            #[cfg(any(test, feature = "test-support"))]
            Self::SameOwnerTest => (
                rustix::process::geteuid().as_raw(),
                rustix::process::getegid().as_raw(),
            ),
        }
    }
}

/// Move-only, metered shared lifecycle custody for a protected service.
///
/// Production admission requires the unchanged root-owned 0:0 parent/file policy.
/// The file, canonical pathname, and retained parent identity are revalidated using
/// the shared V1 primitives. This is lifecycle custody, not provisioning, peer,
/// signing, process-profile, or compiler-execution authority.
///
/// Admission consumes a prepaid [`Self::FILE_STORAGE`] File and borrows a separately
/// prepaid [`Self::STATE_ROOT_STORAGE`] descriptor. A containing root owner's full
/// charge must also remain live. Open borrows only the root descriptor. Admission
/// returns growth over the consumed File reservation; open returns the full owner
/// charge. Reserve that returned charge before retaining the owner. Consuming errors
/// close the File but never retire its old reservation automatically. Drop the owner
/// before retiring its full [`Self::retained_storage`] charge.
///
/// Every operation prepays its fixed work and scratch on the original ledger and
/// restores entry storage on success, error and unwind, preserving accepted work,
/// peak and first-denial history. All shared syscalls are single attempts; EINTR is
/// terminal and flock is nonblocking. These are logical quotas, not syscall latency,
/// allocator, generated-stack or RSS bounds. Drop only closes retained descriptors;
/// it never calls LOCK_UN, including when admission fails after acquiring a lock.
/// There is no public legacy upgrade, descriptor exposure or extraction.
///
/// ```
/// use fe2o3_compiler_execution_lifecycle::{CompilerExecutionServiceLifecycleLeaseV2 as Lease,
///     LifecycleLeaseErrorV2 as Error, LifecycleLeaseStorageV2 as Storage};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn admit(file: std::fs::File, root: &std::fs::File, b: &mut Budget<'_>)
///     -> Result<(Lease, Storage), Error> { Lease::admit(file, root, b) }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
/// fn duplicate(lease: Lease) { let _ = lease.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
/// fn copy<T: Copy>() {} copy::<Lease>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
/// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<Lease>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::{CompilerExecutionServiceLifecycleLeaseV1 as Old,
///     CompilerExecutionServiceLifecycleLeaseV2 as Lease};
/// fn upgrade(old: Old) -> Lease { old.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::{CompilerExecutionServiceLifecycleLeaseV1 as Old,
///     CompilerExecutionServiceLifecycleLeaseV2 as Lease};
/// fn extract(lease: Lease) -> Old { lease.inner }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
/// fn extract(lease: Lease) -> std::fs::File { lease.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
/// fn unmetered(lease: &Lease) { let _ = lease.revalidate(); }
/// ```
pub struct CompilerExecutionServiceLifecycleLeaseV2 {
    inner: Lease,
    parent_identity: (u64, u64),
}

impl CompilerExecutionServiceLifecycleLeaseV2 {
    const RETAINED: usize = size_of::<(Self, Storage)>();

    /// Full logical incoming File charge, including its storage receipt and padding.
    pub const FILE_STORAGE: usize = size_of::<(File, Storage)>();
    /// Minimum borrowed state-root descriptor floor; containing owners remain prepaid.
    pub const STATE_ROOT_STORAGE: usize = Self::FILE_STORAGE;
    /// Complete work for open or admission, including exact-slot admission and entry.
    /// Covers at most 64 fixed descriptor/metadata/attribute operations and cleanup.
    pub const ADMISSION_WORK: usize = ENTRY_WORK + 64 * 1024;
    /// Complete work for retained-object revalidation, including entry and cleanup.
    pub const REVALIDATION_WORK: usize = ENTRY_WORK + 32 * 1024;
    /// Additional fixed peak scratch above the complete incoming reservation.
    pub const IO_STORAGE: usize = 4 * Self::RETAINED + 8 * size_of::<rustix::fs::Stat>() + 4096;

    /// Consumes a prepaid root-owned File and returns the retained-owner growth charge.
    pub fn admit(
        file: File,
        state_root: &impl AsFd,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_for_owner(file, state_root, None, Owner::Root, budget)
    }

    /// Also retains the parent at exactly one unused CLOEXEC descriptor slot.
    /// A busy slot is never overwritten; both newly owned descriptors close on refusal.
    pub fn admit_with_parent_at(
        file: File,
        state_root: &impl AsFd,
        private_parent_fd: RawFd,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_for_owner(
            file,
            state_root,
            Some(private_parent_fd),
            Owner::Root,
            budget,
        )
    }

    /// Opens an independent root-owned lease and returns its FULL retained charge.
    pub fn open(state_root: &impl AsFd, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::open_for_owner(state_root, Owner::Root, budget)
    }

    /// Admits a same-owner fixture on the original ledger, without production authority.
    #[cfg(feature = "test-support")]
    pub fn admit_non_authoritative_same_owner_test(
        file: File,
        state_root: &impl AsFd,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_for_owner(file, state_root, None, Owner::SameOwnerTest, budget)
    }

    /// Admits a same-owner fixture with one exact private parent descriptor slot.
    #[cfg(feature = "test-support")]
    pub fn admit_non_authoritative_same_owner_test_with_parent_at(
        file: File,
        state_root: &impl AsFd,
        private_parent_fd: RawFd,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_for_owner(
            file,
            state_root,
            Some(private_parent_fd),
            Owner::SameOwnerTest,
            budget,
        )
    }

    /// Opens an independent same-owner fixture lease, without production authority.
    #[cfg(feature = "test-support")]
    pub fn open_non_authoritative_same_owner_test(
        state_root: &impl AsFd,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::open_for_owner(state_root, Owner::SameOwnerTest, budget)
    }

    fn admit_for_owner(
        file: File,
        state_root: &impl AsFd,
        slot: Option<RawFd>,
        owner: Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let floor = Self::FILE_STORAGE
            .checked_add(Self::STATE_ROOT_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(
            floor,
            ENTRY_WORK,
            Self::ADMISSION_WORK,
            Self::IO_STORAGE,
            |_| {
                let (uid, gid) = owner.ids();
                let parent = open_parent(state_root)?;
                let parent = match slot {
                    Some(slot) => retain_parent_at(parent, slot)?,
                    None => parent,
                };
                let lease = Self::admit_with_parent(file, parent, uid, gid)?;
                let growth = Self::RETAINED
                    .checked_sub(Self::FILE_STORAGE)
                    .ok_or(Resource::Accounting)?;
                Ok((lease, Storage(growth)))
            },
        )
    }

    fn open_for_owner(
        state_root: &impl AsFd,
        owner: Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        budget.with_prepaid_scope(
            Self::STATE_ROOT_STORAGE,
            ENTRY_WORK,
            Self::ADMISSION_WORK,
            Self::IO_STORAGE,
            |_| {
                let (uid, gid) = owner.ids();
                let parent = open_parent(state_root)?;
                validate_parent(&parent, uid, gid)?;
                // NONBLOCK prevents a hostile FIFO from blocking before shared type validation.
                let file = File::from(
                    openat(
                        &parent,
                        lifecycle_name()?,
                        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                        Mode::empty(),
                    )
                    .map_err(|e| io_error("open canonical lifecycle file", e))?,
                );
                Ok((
                    Self::admit_with_parent(file, parent, uid, gid)?,
                    Storage(Self::RETAINED),
                ))
            },
        )
    }

    fn admit_with_parent(file: File, parent: File, uid: u32, gid: u32) -> Result<Self> {
        let parent_identity = parent_identity(&parent)?;
        let inner = Lease::admit_with_parent(file, parent, uid, gid)?;
        let lease = Self {
            inner,
            parent_identity,
        };
        lease.check_parent_identity()?;
        Ok(lease)
    }

    /// Rechecks exact file/parent identities, shared metadata policy, pathname and lock.
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        budget.with_prepaid_scope(
            Self::RETAINED,
            ENTRY_WORK,
            Self::REVALIDATION_WORK,
            Self::IO_STORAGE,
            |_| {
                self.check_parent_identity()?;
                self.inner.revalidate()?;
                self.check_parent_identity()
            },
        )
    }

    fn check_parent_identity(&self) -> Result<()> {
        if parent_identity(&self.inner.parent)? != self.parent_identity {
            return Err(LifecycleLeaseErrorV2::ParentChanged);
        }
        Ok(())
    }

    /// Full owner/receipt charge; retire only after dropping this close-only owner.
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }
}

fn parent_identity(parent: &File) -> Result<(u64, u64)> {
    let stat =
        fstat(parent).map_err(|e| io_error("inspect retained lifecycle parent identity", e))?;
    Ok((stat.st_dev, stat.st_ino))
}

impl fmt::Debug for CompilerExecutionServiceLifecycleLeaseV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionServiceLifecycleLeaseV2")
            .field("authority", &"deployment-lifecycle-only")
            .finish_non_exhaustive()
    }
}

const _: () = {
    assert!(
        CompilerExecutionServiceLifecycleLeaseV2::RETAINED
            >= CompilerExecutionServiceLifecycleLeaseV2::FILE_STORAGE
    );
    assert!(
        8 * size_of::<LifecycleLeaseErrorV2>()
            + 4 * size_of::<crate::ObjectSnapshotV1>()
            + 64 * size_of::<usize>()
            <= 4096
    );
};

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
