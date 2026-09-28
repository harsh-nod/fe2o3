//! Metered lifecycle custody with explicit, controlled descriptor transfers.

use crate::{
    CompilerExecutionServiceLifecycleLeaseV1 as Lease, LifecycleLeaseErrorV1 as LeaseError,
    ROOT_ID_V1, acquire_shared, io_error, lifecycle_name, open_parent, retain_parent_at,
    validate_file, validate_named_file, validate_parent,
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
    /// The retained or root-derived parent does not denote the admitted device/inode.
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
/// There is no public legacy upgrade, borrowed descriptor exposure or extraction.
/// Explicit metered File transfers are for trusted controlled staging only: an
/// exported alias can change shared status flags or explicitly unlock the retained
/// open file description. This type does not isolate custody from such a holder.
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
/// ```
/// use fe2o3_compiler_execution_lifecycle::{CompilerExecutionServiceLifecycleLeaseV2 as Lease,
///     LifecycleLeaseErrorV2 as Error, LifecycleLeaseStorageV2 as Storage};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn transfer(lease: &Lease, b: &mut Budget<'_>)
///     -> Result<(std::fs::File, Storage), Error> { lease.try_clone_for_transfer(b) }
/// fn validate(lease: &Lease, file: &std::fs::File, b: &mut Budget<'_>)
///     -> Result<(), Error> { lease.validate_transfer(file, b) }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
/// fn unmetered(lease: &Lease) { let _ = lease.try_clone_for_transfer(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
/// fn unmetered(lease: &Lease, file: &std::fs::File) { let _ = lease.validate_transfer(file); }
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
    /// Complete root-binding work, including entry and two retained checks.
    /// Each retained check costs 23 calls. Deriving the actual root's parent,
    /// its six policy calls, identity and canonical sibling checks, and close
    /// add ten calls. All 56 calls are single attempts; 64 weighted slots also
    /// cover fixed comparisons and control. This is not a syscall latency bound.
    pub const ROOT_BINDING_WORK: usize = ENTRY_WORK + 64 * 1024;
    /// Fixed check scratch plus the full temporary root-derived parent owner.
    pub const ROOT_BINDING_SCRATCH: usize = Self::IO_STORAGE + Self::FILE_STORAGE;
    /// Complete clone or transfer-validation work, including entry and cleanup.
    /// Two owner checks cost 46 fixed calls, the candidate check 15, and cloning
    /// adds one duplication and at most one close on refusal. No syscall retries.
    pub const TRANSFER_WORK: usize = ENTRY_WORK + 64 * 1024;
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
            |_| self.check(),
        )
    }

    /// Rechecks this lease against the actual state root's canonical sibling lock.
    /// Requires the full retained lease plus STATE_ROOT_STORAGE prepaid on the
    /// original ledger; a containing root owner's full charge stays live too.
    /// Derives a fresh parent descriptor from the borrowed root, validates its
    /// security policy and exact admitted identity, and checks its canonical
    /// lifecycle pathname against the retained file. A valid unrelated parent
    /// is refused. No caller-supplied identity substitutes for these observations.
    ///
    /// This binds the root's parent, not the root directory's own identity or
    /// policy: distinct roots under the same canonical parent share this lock.
    /// The enclosing root owner must validate its own exact directory custody.
    /// Checks are point-in-time observations, not protection against later moves.
    /// No retained storage is returned. All exits close the temporary parent and
    /// restore entry storage, preserving work, peak and first-denial history.
    /// Neither borrowed input is closed or explicitly unlocked on refusal.
    ///
    /// ```
    /// use fe2o3_compiler_execution_lifecycle::{CompilerExecutionServiceLifecycleLeaseV2 as Lease,
    ///     LifecycleLeaseErrorV2 as Error};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn check(lease: &Lease, root: &std::fs::File, b: &mut Budget<'_>) -> Result<(), Error> {
    ///     lease.revalidate_for_root(root, b)
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
    /// fn unmetered(lease: &Lease, root: &std::fs::File) { lease.revalidate_for_root(root); }
    /// ```
    /// ```compile_fail
    /// use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn identity_only(lease: &Lease, identity: (u64, u64), b: &mut Budget<'_>) {
    ///     lease.revalidate_for_root(&identity, b);
    /// }
    /// ```
    pub fn revalidate_for_root(
        &self,
        state_root: &impl AsFd,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.revalidate_for_root_with(state_root, budget, |_| Ok(()))
    }

    // Private observer exercises refusal/unwind with the temporary parent live.
    fn revalidate_for_root_with(
        &self,
        state_root: &impl AsFd,
        budget: &mut Budget<'_>,
        after_parent: impl FnOnce(&File) -> Result<()>,
    ) -> Result<()> {
        let floor = Self::RETAINED
            .checked_add(Self::STATE_ROOT_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(
            floor,
            ENTRY_WORK,
            Self::ROOT_BINDING_WORK,
            Self::ROOT_BINDING_SCRATCH,
            |_| {
                self.check()?;
                let parent = open_parent(state_root)?;
                after_parent(&parent)?;
                validate_parent(&parent, self.inner.expected_uid, self.inner.expected_gid)?;
                if parent_identity(&parent)? != self.parent_identity {
                    return Err(LifecycleLeaseErrorV2::ParentChanged);
                }
                validate_named_file(&parent, self.inner.snapshot)?;
                self.check()
            },
        )
    }

    /// Returns a distinct CLOEXEC File sharing this lease's open file description
    /// by construction, using one F_DUPFD_CLOEXEC attempt after revalidation.
    /// The owner remains prepaid; reserve the returned FULL FILE_STORAGE before
    /// retaining the File. Failure/unwind closes only the new duplicate, never
    /// explicitly unlocks, and leaves the owner's reservation untouched.
    ///
    /// Keep every staged duplicate charged and validate the final staged File
    /// before controlled exec. Duplication, not reopening a pathname, preserves
    /// the same open file description and its shared flock across exec.
    pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
        self.clone_for_transfer_with(budget, |_| Ok(()))
    }

    // Private observer exercises cleanup after a real duplicate; production is a no-op.
    fn clone_for_transfer_with(
        &self,
        budget: &mut Budget<'_>,
        after_duplicate: impl FnOnce(&File) -> Result<()>,
    ) -> Result<(File, Storage)> {
        budget.with_prepaid_scope(
            Self::RETAINED,
            ENTRY_WORK,
            Self::TRANSFER_WORK,
            Self::IO_STORAGE,
            |_| {
                self.check()?;
                let file = File::from(
                    rustix::io::fcntl_dupfd_cloexec(&self.inner.file, 3)
                        .map_err(|e| io_error("duplicate lifecycle lease for transfer", e))?,
                );
                after_duplicate(&file)?;
                self.check_transfer(&file)?;
                self.check()?;
                Ok((file, Storage(Self::FILE_STORAGE)))
            },
        )
    }

    /// Checks the exact canonical inode and metadata, retained parent/pathname,
    /// and CLOEXEC/read-only descriptor policy, then acquires LOCK_SH|LOCK_NB on
    /// the candidate and rechecks it. Both the owner and full File charge must
    /// remain prepaid; this operation returns no new retained storage.
    ///
    /// This does NOT prove arbitrary open-file-description equality. An independent
    /// reopen of the same canonical inode may pass, acquiring its own shared lock.
    /// It neither proves past uninterrupted locking nor authenticates provenance.
    /// Failure/unwind never closes or unlocks either borrowed input; a late failure
    /// may leave the candidate shared-locked until its caller closes it. Never use
    /// LOCK_UN on controlled aliases, including during error cleanup.
    pub fn validate_transfer(&self, file: &File, budget: &mut Budget<'_>) -> Result<()> {
        let floor = Self::RETAINED
            .checked_add(Self::FILE_STORAGE)
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(
            floor,
            ENTRY_WORK,
            Self::TRANSFER_WORK,
            Self::IO_STORAGE,
            |_| {
                self.check()?;
                self.check_transfer(file)?;
                self.check()
            },
        )
    }

    fn check(&self) -> Result<()> {
        self.check_parent_identity()?;
        self.inner.revalidate()?;
        self.check_parent_identity()
    }

    fn check_transfer(&self, file: &File) -> Result<()> {
        if validate_file(file, self.inner.expected_uid, self.inner.expected_gid)?
            != self.inner.snapshot
        {
            return Err(LeaseError::FileChanged.into());
        }
        validate_named_file(&self.inner.parent, self.inner.snapshot)?;
        acquire_shared(file)?;
        if validate_file(file, self.inner.expected_uid, self.inner.expected_gid)?
            != self.inner.snapshot
        {
            return Err(LeaseError::FileChanged.into());
        }
        validate_named_file(&self.inner.parent, self.inner.snapshot)?;
        Ok(())
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

#[cfg(test)]
#[path = "native_transfer_tests.rs"]
mod transfer_tests;

#[cfg(test)]
#[path = "native_root_binding_tests.rs"]
mod root_binding_tests;
