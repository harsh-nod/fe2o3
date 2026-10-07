//! Finite, observation-only acquisition of real publication custody.
use crate::{
    BuildAttempt, DurableCurrentLinkPublicationTokenV1, DurableLinkPublicationError,
    DurablePublishedClaimReacquisitionErrorV3, EmitError, RetainedDurableDirectoryV1,
    WorkerV3LoadReadinessErrorV1, WorkerV3LoadReadinessResultV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use rustix::fd::{AsFd, AsRawFd, OwnedFd};
use std::{cell::Cell, error::Error, fmt, io, marker::PhantomData, mem::size_of};

type Result<T> = std::result::Result<T, NativeCurrentPublicationErrorV1>;

/// Inert caller-selected ceilings, checked before allocation or file reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCurrentPublicationLimitsV1 {
    readiness: usize,
    artifact: usize,
}
impl NativeCurrentPublicationLimitsV1 {
    pub fn new(readiness: usize, artifact: usize) -> Result<Self> {
        if readiness == 0
            || readiness > crate::MAX_WORKER_V3_LOAD_ENVELOPE_CUSTODY_BYTES_V2
            || artifact == 0
            || artifact > crate::MAX_DURABLE_FINALIZED_ARTIFACT_BYTES
        {
            return Err(NativeCurrentPublicationErrorV1::Binding);
        }
        Ok(Self {
            readiness,
            artifact,
        })
    }
    pub const fn readiness_bytes(self) -> usize {
        self.readiness
    }
    pub const fn artifact_bytes(self) -> usize {
        self.artifact
    }
    fn quota(self) -> std::result::Result<usize, Resource> {
        // At most six bounded registry decodes, twelve content scans, their
        // temporary owned representations, and fewer than 512 fixed syscalls.
        // This deliberately includes the complete 1-MiB registry ceiling.
        self.readiness
            .checked_add(self.artifact)
            .and_then(|n| n.checked_add(crate::MAX_ATTEMPT_BYTES + 64 * 1024))
            .and_then(|n| n.checked_mul(128))
            .ok_or(Resource::Arithmetic)
    }
}

#[derive(Debug)]
pub enum NativeCurrentPublicationErrorV1 {
    Resource(Resource),
    Filesystem(EmitError),
    Readiness(WorkerV3LoadReadinessErrorV1),
    Claim(DurablePublishedClaimReacquisitionErrorV3),
    Publication(DurableLinkPublicationError),
    Busy,
    Binding,
}
impl fmt::Display for NativeCurrentPublicationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native publication custody: {self:?}")
    }
}
impl Error for NativeCurrentPublicationErrorV1 {}
impl From<Resource> for NativeCurrentPublicationErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<EmitError> for NativeCurrentPublicationErrorV1 {
    fn from(value: EmitError) -> Self {
        Self::Filesystem(value)
    }
}

/// Full unreserved logical owner charge; original input charges remain live.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCurrentPublicationStorageV1(usize);
impl NativeCurrentPublicationStorageV1 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Original directory and file observations protected by the actual cooperative lock.
///
/// Acquires the existing named OFD lock and root-inode flock without waiting;
/// native syscalls do not retry EINTR or short reads. Recovery residue is refused,
/// never repaired. Unrelated temporary names are not consulted or modified.
/// Retains the original Work lifetime/account, and terminally poisons on failed
/// revalidation. Successful calls release only operation scratch. No lock, token,
/// directory, or descriptor export permits an unmetered revalidation bypass.
///
/// Finite syscall attempts and logical allocation accounting are not an I/O
/// latency, RSS, hostile-filesystem, or noncooperating-writer guarantee. The
/// shared process lock registry has a short mutex critical section, including
/// during destruction. This is publication custody, not compiler/semantic/load
/// or launch authority, and does not decode any readiness schema.
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::NativeCurrentPublicationV1;
/// fn duplicate(value: NativeCurrentPublicationV1<'_>) { let _ = value.clone(); }
/// ```
pub struct NativeCurrentPublicationV1<'work> {
    readiness: WorkerV3LoadReadinessResultV1,
    token: DurableCurrentLinkPublicationTokenV1,
    limits: NativeCurrentPublicationLimitsV1,
    retained: usize,
    ledger: Ledger,
    account: Option<Account>,
    failed: Cell<bool>,
    work: PhantomData<&'work Work>,
}
impl<'work> NativeCurrentPublicationV1<'work> {
    pub const DIRECTORY_STORAGE: usize = size_of::<RetainedDurableDirectoryV1>() + 64;

    pub fn recover(
        directory: &RetainedDurableDirectoryV1,
        attempt: BuildAttempt,
        limits: NativeCurrentPublicationLimitsV1,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, NativeCurrentPublicationStorageV1)> {
        if budget.storage() < Self::DIRECTORY_STORAGE {
            return Err(Resource::Accounting.into());
        }
        let quota = limits.quota()?;
        budget.charge_work(quota)?;
        let floor = budget.storage();
        budget.reserve_storage(quota)?;
        let mut output = directory.pinned_output()?.try_clone()?;
        output.observation_only = true;
        output.native_read_limits = Some(limits);
        let lock = output
            .try_lock()?
            .ok_or(NativeCurrentPublicationErrorV1::Busy)?;
        let readiness = crate::worker_v3_load_readiness::recover_worker_v3_load_readiness_for_attempt_locked_v1(&output, attempt)
            .map_err(NativeCurrentPublicationErrorV1::Readiness)?;
        let lease = crate::durable_published_claim::validate_current_hsaco_publication_locked_v3(
            &output,
            readiness.published_claim(),
        )
        .map_err(NativeCurrentPublicationErrorV1::Claim)?;
        let retained = readiness
            .retained_rust_storage()
            .and_then(|n| n.checked_add(lease.retained_rust_storage()?))
            .and_then(|n| {
                n.checked_add(size_of::<Self>() + size_of::<NativeCurrentPublicationStorageV1>())
            })
            .ok_or(Resource::Arithmetic)?;
        if retained > quota {
            return Err(Resource::Accounting.into());
        }
        let value = Self {
            readiness,
            token: lease.retain_native_lock(lock),
            limits,
            retained,
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
            failed: Cell::new(false),
            work: PhantomData,
        };
        value
            .token
            .revalidate_locked_currentness()
            .map_err(NativeCurrentPublicationErrorV1::Publication)?;
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        Ok((value, NativeCurrentPublicationStorageV1(retained)))
    }

    pub fn readiness(&self) -> &WorkerV3LoadReadinessResultV1 {
        &self.readiness
    }
    pub fn exact_artifact_bytes(&self) -> &[u8] {
        self.token.exact_artifact_bytes()
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        let result = (|| {
            if self.failed.get()
                || self.ledger != budget.work_ledger_identity_v1()
                || self.account != budget.storage_account_identity_v1()
                || budget.storage() < self.retained
            {
                return Err(Resource::Accounting.into());
            }
            let quota = self.limits.quota()?;
            budget.charge_work(quota)?;
            budget.reserve_storage(quota)?;
            self.token
                .revalidate_locked_currentness()
                .map_err(NativeCurrentPublicationErrorV1::Publication)?;
            let current = crate::worker_v3_load_readiness::recover_worker_v3_load_readiness_for_attempt_locked_v1(
                self.token.native_output(), self.readiness.published_claim().plan().attempt(),
            ).map_err(NativeCurrentPublicationErrorV1::Readiness)?;
            if current.receipt() != self.readiness.receipt()
                || current.exact_envelope_bytes() != self.readiness.exact_envelope_bytes()
            {
                return Err(NativeCurrentPublicationErrorV1::Binding);
            }
            budget.release_storage(quota)?;
            Ok(())
        })();
        if result.is_err() {
            self.failed.set(true);
        }
        result
    }
}

pub(crate) fn same_file(left: &rustix::fs::Stat, right: &rustix::fs::Stat) -> bool {
    left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && left.st_mode == right.st_mode
        && left.st_uid == right.st_uid
        && left.st_gid == right.st_gid
        && left.st_nlink == right.st_nlink
        && left.st_size == right.st_size
        && left.st_mtime == right.st_mtime
        && left.st_mtime_nsec == right.st_mtime_nsec
        && left.st_ctime == right.st_ctime
        && left.st_ctime_nsec == right.st_ctime_nsec
}

pub(crate) fn read_exact_file(fd: &impl AsFd, length: usize) -> io::Result<Vec<u8>> {
    read_exact_with(length, |bytes, offset| {
        rustix::io::pread(fd, bytes, offset).map_err(io::Error::from)
    })
}

fn read_exact_with(
    length: usize,
    mut read: impl FnMut(&mut [u8], u64) -> io::Result<usize>,
) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| io::Error::other("native file allocation"))?;
    bytes.resize(length, 0);
    for (index, chunk) in bytes.chunks_mut(64 * 1024).enumerate() {
        if read(chunk, (index * 64 * 1024) as u64)? != chunk.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "short native positional read",
            ));
        }
    }
    if read(&mut [0], length as u64)? != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "native file grew",
        ));
    }
    Ok(bytes)
}

#[cfg(target_os = "linux")]
pub(crate) fn try_ofd_lock(fd: &OwnedFd) -> io::Result<bool> {
    // SAFETY: fully initialized flock and original live descriptor, single attempt.
    let mut lock: libc::flock = unsafe { std::mem::zeroed() };
    lock.l_type = libc::F_WRLCK as _;
    lock.l_whence = libc::SEEK_SET as _;
    lock_result(unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_OFD_SETLK, &lock) })
}
#[cfg(target_os = "linux")]
pub(crate) fn try_directory_lock(fd: &OwnedFd) -> io::Result<bool> {
    // SAFETY: original live descriptor and valid nonblocking flock flags.
    lock_result(unsafe { libc::flock(fd.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) })
}
fn lock_result(result: i32) -> io::Result<bool> {
    if result == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::EACCES) | Some(libc::EAGAIN) => Ok(false),
        _ => Err(error),
    }
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn try_ofd_lock(_: &OwnedFd) -> io::Result<bool> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}
#[cfg(not(target_os = "linux"))]
pub(crate) fn try_directory_lock(_: &OwnedFd) -> io::Result<bool> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(test)]
mod tests;
