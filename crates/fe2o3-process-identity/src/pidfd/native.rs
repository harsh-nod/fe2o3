//! Finite, original-account pidfd observation for the native application route.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as StorageAccount,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use std::{marker::PhantomData, mem::size_of};

type Result<T> = std::result::Result<T, NativeReceivedProcessPidfdErrorV1>;
const SCRATCH: usize = 16 * 1024;
const RECORD_BYTES: usize = 4096;

#[derive(Debug)]
pub enum NativeReceivedProcessPidfdErrorV1 {
    Resource(Resource),
    Observation(PidfdObservationErrorV1),
}
impl From<Resource> for NativeReceivedProcessPidfdErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<PidfdObservationErrorV1> for NativeReceivedProcessPidfdErrorV1 {
    fn from(error: PidfdObservationErrorV1) -> Self {
        Self::Observation(error)
    }
}
impl fmt::Display for NativeReceivedProcessPidfdErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Observation(error) => error.fmt(f),
        }
    }
}
impl Error for NativeReceivedProcessPidfdErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Observation(error) => Some(error),
        }
    }
}

/// Unreserved full owner charge; callers retain it on the original resource account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeReceivedProcessPidfdStorageV1(usize);
impl NativeReceivedProcessPidfdStorageV1 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

/// Original received descriptor, observed without hidden EINTR retries.
///
/// All syscall attempts and bounded parsing are charged to the original Work
/// and storage account. Every syscall is single-attempt; EINTR fails closed.
/// Procfs records use one bounded pread and one EOF probe, never std retrying
/// readers. Failures retain operation scratch; success releases only scratch.
/// This is point-in-time observation, not sender authentication or authority.
/// Compatible kernel procfs remains a trusted prerequisite.
///
/// No raw descriptor export, duplication, wait, signal or legacy promotion exists.
///
/// ```compile_fail
/// use fe2o3_process_identity::pidfd::NativeReceivedProcessPidfdV1 as Native;
/// fn duplicate(value: Native) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_process_identity::pidfd::NativeReceivedProcessPidfdV1 as Native;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Native>();
/// ```
/// ```compile_fail
/// use fe2o3_process_identity::pidfd::{NativeReceivedProcessPidfdV1 as Native,
///     ReceivedProcessPidfdV1 as Legacy};
/// fn promote<'work>(value: Legacy) -> Native<'work> { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_process_identity::pidfd::NativeReceivedProcessPidfdV1 as Native;
/// use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn escape(fd: std::os::fd::OwnedFd) -> Native<'static> {
///     let mut work = Work::new(usize::MAX);
///     let mut budget = Budget::new(&mut work, 1_000_000);
///     Native::admit_received(fd, std::process::id(), &mut budget).unwrap().0
/// }
/// ```
pub struct NativeReceivedProcessPidfdV1<'work> {
    pidfd: OwnedFd,
    target: PidfdTargetObservationV1,
    object: PidfdObjectV1,
    start_time_ticks: u64,
    ledger: Ledger,
    storage_account: Option<StorageAccount>,
    work: PhantomData<&'work Work>,
}
impl fmt::Debug for NativeReceivedProcessPidfdV1<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NativeReceivedProcessPidfdV1")
            .field("pid", &self.target.pid)
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}
impl<'work> NativeReceivedProcessPidfdV1<'work> {
    pub fn admit_received(
        pidfd: OwnedFd,
        expected_pid: u32,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, NativeReceivedProcessPidfdStorageV1)> {
        budget.charge_work(1)?;
        let floor = budget.storage();
        budget.reserve_storage(SCRATCH)?;
        if expected_pid == 0 {
            return Err(PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::ExpectedPid,
                "expected native received process PID must be nonzero",
            )
            .into());
        }
        let object = inspect_object(&pidfd, budget)?;
        probe(budget, || require_process_pidfd_mode(&pidfd))?;
        let target = inspect_target(&pidfd, budget)?;
        require_pidfd_target(target.pid, expected_pid)?;
        probe(budget, || require_pidfd_not_pollable(&pidfd))?;
        let value = Self {
            start_time_ticks: inspect_start_time(expected_pid, budget)?,
            pidfd,
            target,
            object,
            ledger: budget.work_ledger_identity_v1(),
            storage_account: budget.storage_account_identity_v1(),
            work: PhantomData,
        };
        value.revalidate_inner(budget)?;
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        Ok((
            value,
            NativeReceivedProcessPidfdStorageV1(Self::retained_storage()),
        ))
    }

    pub const fn retained_storage() -> usize {
        size_of::<(Self, NativeReceivedProcessPidfdStorageV1)>()
    }

    pub fn revalidate(&self, budget: &mut Budget<'work>) -> Result<()> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.storage_account != budget.storage_account_identity_v1()
            || budget.storage() < Self::retained_storage()
        {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(1)?;
        budget.reserve_storage(SCRATCH)?;
        self.revalidate_inner(budget)?;
        budget.release_storage(SCRATCH)?;
        Ok(())
    }

    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> Result<()> {
        for _ in 0..2 {
            probe(budget, || require_pidfd_not_pollable(&self.pidfd))?;
            probe(budget, || require_process_pidfd_mode(&self.pidfd))?;
            if inspect_object(&self.pidfd, budget)? != self.object
                || inspect_target(&self.pidfd, budget)? != self.target
            {
                return Err(PidfdObservationErrorV1::new(
                    PidfdObservationErrorKindV1::IdentityChanged,
                    "native received pidfd object, target or observation source changed",
                )
                .into());
            }
            require_client_start_time(
                inspect_start_time(self.target.pid, budget)?,
                self.start_time_ticks,
            )?;
        }
        probe(budget, || require_pidfd_not_pollable(&self.pidfd))
    }

    pub const fn pid(&self) -> u32 {
        self.target.pid
    }
    pub const fn start_time_ticks(&self) -> u64 {
        self.start_time_ticks
    }
}

fn probe<T>(
    budget: &mut Budget<'_>,
    operation: impl FnOnce() -> std::result::Result<T, PidfdObservationErrorV1>,
) -> Result<T> {
    budget.charge_work(1)?;
    operation().map_err(Into::into)
}

fn syscall<T>(
    budget: &mut Budget<'_>,
    kind: PidfdObservationErrorKindV1,
    operation: impl FnOnce() -> rustix::io::Result<T>,
) -> Result<T> {
    probe(budget, || {
        operation().map_err(|error| {
            PidfdObservationErrorV1::io(
                kind,
                "native pidfd observation syscall failed",
                error.into(),
            )
        })
    })
}

fn inspect_object(fd: &OwnedFd, budget: &mut Budget<'_>) -> Result<PidfdObjectV1> {
    // The shared observer contains exactly two non-retrying rustix calls.
    budget.charge_work(2)?;
    PidfdObjectV1::inspect(fd).map_err(Into::into)
}

fn inspect_target(fd: &OwnedFd, budget: &mut Budget<'_>) -> Result<PidfdTargetObservationV1> {
    budget.charge_work(1)?;
    // SAFETY: integer-only zeroed v0 record with the exact ioctl encoded length.
    let mut info = unsafe { MaybeUninit::<PidfdInfoV0>::zeroed().assume_init() };
    info.mask = PIDFD_INFO_PID_V0;
    // SAFETY: original descriptor borrowed; writable record has exact v0 size.
    if unsafe { libc::ioctl(fd.as_raw_fd(), PIDFD_GET_INFO_V0, &mut info) } == 0 {
        if info.mask & PIDFD_INFO_PID_V0 == 0 || info.pid == 0 || info.pid != info.tgid {
            return Err(PidfdObservationErrorV1::new(
                PidfdObservationErrorKindV1::InspectPidfd,
                "native PIDFD_GET_INFO omitted a usable process-leader target PID",
            )
            .into());
        }
        return Ok(PidfdTargetObservationV1 {
            pid: info.pid,
            source: PidfdIdentitySourceV1::KernelIoctl,
        });
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ENOTTY | libc::EINVAL) => inspect_fdinfo(fd, budget),
        Some(libc::ESRCH) => Err(PidfdObservationErrorV1::new(
            PidfdObservationErrorKindV1::AlreadyDead,
            "native process pidfd target exited before identity inspection",
        )
        .into()),
        _ => Err(PidfdObservationErrorV1::io(
            PidfdObservationErrorKindV1::InspectPidfd,
            "native PIDFD_GET_INFO failed",
            error,
        )
        .into()),
    }
}

fn open_self(budget: &mut Budget<'_>) -> Result<File> {
    let kind = PidfdObservationErrorKindV1::InspectPidfd;
    let flags =
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC;
    let current = File::from(syscall(budget, kind, || {
        rustix::fs::open("/proc/self", flags, rustix::fs::Mode::empty())
    })?);
    let numeric = File::from(syscall(budget, kind, || {
        rustix::fs::open(
            format!("/proc/{}", std::process::id()),
            flags | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )
    })?);
    probe(budget, || require_procfs(&current, "native /proc/self"))?;
    probe(budget, || {
        require_procfs(&numeric, "native numeric /proc self entry")
    })?;
    let left = syscall(budget, kind, || rustix::fs::fstat(&current))?;
    let right = syscall(budget, kind, || rustix::fs::fstat(&numeric))?;
    if (left.st_dev, left.st_ino) != (right.st_dev, right.st_ino) {
        return Err(PidfdObservationErrorV1::new(
            kind,
            "native procfs numeric self identity mismatch",
        )
        .into());
    }
    Ok(current)
}

fn inspect_fdinfo(fd: &OwnedFd, budget: &mut Budget<'_>) -> Result<PidfdTargetObservationV1> {
    let current = open_self(budget)?;
    let kind = PidfdObservationErrorKindV1::InspectPidfd;
    let flags =
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC;
    let directory = File::from(syscall(budget, kind, || {
        rustix::fs::openat(
            &current,
            "fdinfo",
            flags | rustix::fs::OFlags::DIRECTORY,
            rustix::fs::Mode::empty(),
        )
    })?);
    probe(budget, || {
        require_procfs(&directory, "native procfs fdinfo directory")
    })?;
    let record = File::from(syscall(budget, kind, || {
        rustix::fs::openat(
            &directory,
            fd.as_raw_fd().to_string(),
            flags,
            rustix::fs::Mode::empty(),
        )
    })?);
    probe(budget, || {
        require_procfs(&record, "native process pidfd identity record")
    })?;
    let mut bytes = [0; RECORD_BYTES + 1];
    let length = read_record(&record, &mut bytes, kind, budget)?;
    budget.charge_work(length)?;
    let text = std::str::from_utf8(&bytes[..length])
        .map_err(|_| PidfdObservationErrorV1::new(kind, "native procfs fdinfo is not UTF-8"))?;
    Ok(PidfdTargetObservationV1 {
        pid: parse_pidfd_fdinfo(text)?,
        source: PidfdIdentitySourceV1::ProcfsFdinfo,
    })
}

fn inspect_start_time(pid: u32, budget: &mut Budget<'_>) -> Result<u64> {
    let _validated_self = open_self(budget)?;
    let kind = PidfdObservationErrorKindV1::InspectStartTime;
    let record = File::from(syscall(budget, kind, || {
        rustix::fs::open(
            format!("/proc/{pid}/stat"),
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
    })?);
    probe(budget, || {
        require_procfs(&record, "native process stat identity")
    })?;
    let mut bytes = [0; RECORD_BYTES + 1];
    let length = read_record(&record, &mut bytes, kind, budget)?;
    budget.charge_work(length)?;
    parse_process_start_time_ticks(&bytes[..length], pid).map_err(Into::into)
}

fn read_record(
    file: &File,
    bytes: &mut [u8; RECORD_BYTES + 1],
    kind: PidfdObservationErrorKindV1,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    read_record_with(bytes, kind, budget, |buffer, offset| {
        rustix::io::pread(file, buffer, offset)
    })
}

fn read_record_with(
    bytes: &mut [u8; RECORD_BYTES + 1],
    kind: PidfdObservationErrorKindV1,
    budget: &mut Budget<'_>,
    mut read: impl FnMut(&mut [u8], u64) -> rustix::io::Result<usize>,
) -> Result<usize> {
    budget.charge_work(bytes.len())?;
    let length = syscall(budget, kind, || read(bytes, 0))?;
    let mut trailing = [0; 1];
    if length == 0
        || length > RECORD_BYTES
        || syscall(budget, kind, || read(&mut trailing, length as u64))? != 0
    {
        return Err(PidfdObservationErrorV1::new(
            kind,
            "native procfs record is empty, oversized or incomplete",
        )
        .into());
    }
    Ok(length)
}

#[cfg(test)]
mod tests;
