//! Fresh bounded client pidfd custody over the shared process-continuity schedule.
use super::{
    AdmissionErrorKindV1, ExpectedClientProcessIdentityV1, LiveClientPidfdIdentityV1,
    checks::{self, CheckError},
    continuity::{InspectionIo, Native},
    native,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    fmt,
    mem::size_of,
    os::fd::{AsFd, OwnedFd},
};

/// Unreserved growth over a consumed pidfd, or the full returned transfer charge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveClientPidfdStorageV2(usize);
impl LiveClientPidfdStorageV2 {
    /// Keep input reservations and immediately reserve this returned charge.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use LiveClientPidfdStorageV2 as Storage;

/// Allocation-free native refusal. Rendering the diagnostic is a caller operation.
#[derive(Debug)]
pub struct LiveClientPidfdErrorV2(Failure);
#[derive(Debug)]
enum Failure {
    Resource(Resource),
    Inspection(CheckError),
}
type Error = LiveClientPidfdErrorV2;
type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub const fn kind(&self) -> Option<AdmissionErrorKindV1> {
        match &self.0 {
            Failure::Inspection(error) => Some(error.kind()),
            Failure::Resource(_) => None,
        }
    }
    pub const fn errno(&self) -> Option<i32> {
        match &self.0 {
            Failure::Inspection(error) => error.errno(),
            Failure::Resource(_) => None,
        }
    }
    pub const fn resource(&self) -> Option<Resource> {
        match self.0 {
            Failure::Resource(error) => Some(error),
            Failure::Inspection(_) => None,
        }
    }
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self(Failure::Resource(error))
    }
}
impl From<CheckError> for Error {
    fn from(error: CheckError) -> Self {
        Self(Failure::Inspection(error))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Failure::Resource(error) => error.fmt(formatter),
            Failure::Inspection(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0 {
            Failure::Resource(error) => Some(error),
            Failure::Inspection(error) => Some(error),
        }
    }
}

/// Conservative allowance for the current-process stat leaf, using the shared
/// one-target/start-time envelope. No target inspection is actually performed.
pub const CURRENT_PROCESS_START_TIME_WORK_V2: usize = native::operation_work(1);
/// Fixed logical scratch for procfs I/O, the scalar result and scope envelopes.
/// Not a generated-stack, kernel-memory or RSS bound.
pub const CURRENT_PROCESS_START_TIME_IO_STORAGE_V2: usize =
    native::operation_storage(size_of::<u64>(), 0);

/// Reads the current process's start-time ticks using finite native procfs I/O.
///
/// Uses the caller's ledger and restores its entry storage without refunding
/// work or clearing denials. No retained input or result allocation is charged.
/// A compatible procfs mount is trusted; this scalar alone grants no authority.
pub fn current_process_start_time_ticks_v2(budget: &mut Budget<'_>) -> Result<u64> {
    budget.with_prepaid_scope(
        0,
        native::ENTRY_WORK,
        CURRENT_PROCESS_START_TIME_WORK_V2,
        CURRENT_PROCESS_START_TIME_IO_STORAGE_V2,
        |_| Ok(Native::start_time(std::process::id())?),
    )
}

/// Move-only native custody of one freshly admitted, exact-target process pidfd.
///
/// Admission and revalidation use the shared target, descriptor, start-time and
/// non-reaping liveness checks with bounded native I/O, never legacy admission
/// or a V1-owner conversion. A compatible procfs mount remains a trusted input.
/// Success is point-in-time liveness, not authority or perpetual liveness.
///
/// The expected UID/GID are caller-supplied connection-time claims, not credentials
/// independently proven by the pidfd. The consumer must join them to its trusted
/// peer-credential observation. This owner supplies no socket admission, process
/// launch, signal, wait, reap or retained-descriptor access. Transfer returns a
/// separately charged duplicate, never a second descriptor retained by this owner.
///
/// Every call restores entry storage on success, refusal and unwind without
/// refunding accepted work. A consuming refusal closes the input even when entry
/// admission fails. Keep input charges until drop/transfer; reserve returned
/// growth immediately and retire the full retained charge only after owner drop.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::LiveClientPidfdIdentityV2;
/// fn clone<T: Clone>() {}
/// clone::<LiveClientPidfdIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::LiveClientPidfdIdentityV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<LiveClientPidfdIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::LiveClientPidfdIdentityV2;
/// fn raw<T: std::os::fd::IntoRawFd>() {}
/// raw::<LiveClientPidfdIdentityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{LiveClientPidfdIdentityV1, LiveClientPidfdIdentityV2};
/// fn upgrade(old: LiveClientPidfdIdentityV1) -> LiveClientPidfdIdentityV2 { old.into() }
/// ```
pub struct LiveClientPidfdIdentityV2 {
    // Only fresh Native admission populates this policy-neutral mechanical state.
    state: LiveClientPidfdIdentityV1,
}
type Client = LiveClientPidfdIdentityV2;

impl Client {
    pub(super) const RETAINED: usize = size_of::<(Self, Storage)>();
    /// Full logical charge for the consumed pidfd owner and receipt padding.
    /// This is not a bound on kernel object allocation or memory usage.
    pub const FD_STORAGE: usize = size_of::<(OwnedFd, Storage)>();
    /// Three target/start-time pairs, including admission's final liveness check.
    pub const ADMISSION_WORK: usize = native::operation_work(3);
    /// Two target/start-time pairs around the non-reaping liveness observation.
    pub const REVALIDATION_WORK: usize = native::operation_work(2);
    /// Original, duplicate and original again: six target/start-time pairs.
    pub const CLONE_TRANSFER_WORK: usize = native::operation_work(6);
    /// The same schedule on a temporary duplicate of a borrowed transfer pidfd.
    pub const VALIDATE_TRANSFER_WORK: usize = native::operation_work(6);
    /// Fixed logical staging, record buffers and error/control envelopes.
    /// Nested I/O never creates another ledger. Not generated-stack or RSS bounds.
    pub const IO_STORAGE: usize = native::operation_storage(Self::RETAINED, Self::FD_STORAGE);

    /// Consumes a prepaid pidfd; returns only growth over [`Self::FD_STORAGE`].
    /// Expected credentials must come from trusted provisioning or peer inspection.
    pub fn admit(
        pidfd: OwnedFd,
        expected_client: ExpectedClientProcessIdentityV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::scope(budget, Self::FD_STORAGE, Self::ADMISSION_WORK, |_| {
            let state = LiveClientPidfdIdentityV1::admit_with::<Native>(pidfd, expected_client)?;
            Ok((Self { state }, Storage(Self::RETAINED - Self::FD_STORAGE)))
        })
    }

    /// Rechecks exact retained descriptor, target, start time and current liveness.
    pub fn validate_liveness(&self, budget: &mut Budget<'_>) -> Result<()> {
        Self::scope(budget, Self::RETAINED, Self::REVALIDATION_WORK, |_| {
            Ok(self.state.validate_liveness_with::<Native>()?)
        })
    }

    /// Returns a separately charged CLOEXEC duplicate after checking the original,
    /// the duplicate against the admitted snapshot, and the original again.
    ///
    /// Prepay this owner's retained charge. Staging includes the output FD;
    /// reserve the returned full [`Self::FD_STORAGE`] immediately on success.
    /// Arbitrary subsequent FD operations are outside this continuity observation.
    pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(OwnedFd, Storage)> {
        Self::scope(budget, Self::RETAINED, Self::CLONE_TRANSFER_WORK, |_| {
            Ok((
                self.checked_duplicate_with::<Native>(&self.state.pidfd)?,
                Storage(Self::FD_STORAGE),
            ))
        })
    }

    /// Requires the prepaid owner plus [`Self::FD_STORAGE`] for the borrowed
    /// transfer. Checks its actual CLOEXEC flag on both sides of the shared exact
    /// object, target, start-time and non-reaping liveness schedule. The temporary
    /// duplicate is dropped before return, including on refusal or unwind.
    pub fn validate_transfer(&self, pidfd: &impl AsFd, budget: &mut Budget<'_>) -> Result<()> {
        Self::scope(
            budget,
            Self::RETAINED + Self::FD_STORAGE,
            Self::VALIDATE_TRANSFER_WORK,
            |_| {
                drop(self.checked_duplicate_with::<Native>(pidfd)?);
                Ok(())
            },
        )
    }

    fn checked_duplicate_with<M: InspectionIo<Error = CheckError>>(
        &self,
        pidfd: &impl AsFd,
    ) -> std::result::Result<OwnedFd, CheckError> {
        self.state.validate_liveness_with::<M>()?;
        let pidfd = pidfd.as_fd();
        let check_flags = || {
            checks::require_close_on_exec(
                &pidfd,
                AdmissionErrorKindV1::ClientPidfdCloseOnExec,
                "transferred client pidfd",
            )
        };
        check_flags()?;
        let duplicate = rustix::io::fcntl_dupfd_cloexec(pidfd, M::MIN_DUP_FD).map_err(|error| {
            CheckError::io(
                AdmissionErrorKindV1::InspectClientPidfd,
                "cannot duplicate client pidfd for transfer validation",
                error,
            )
        })?;
        // Reuse the admitted snapshot, not fresh admission that could accept a
        // different object. The shared checker owns every process-continuity probe.
        let transferred = LiveClientPidfdIdentityV1 {
            pidfd: duplicate,
            expected_client: self.state.expected_client,
            descriptor_identity: self.state.descriptor_identity,
            identity_source: self.state.identity_source,
            start_time_ticks: self.state.start_time_ticks,
        };
        transferred.validate_liveness_with::<M>()?;
        self.state.validate_liveness_with::<M>()?;
        check_flags()?;
        Ok(transferred.pidfd)
    }

    /// Immutable expected facts. UID/GID are supplied connection-time claims;
    /// the pidfd alone does not independently prove them or bind a socket peer.
    pub const fn expected_client(&self) -> ExpectedClientProcessIdentityV1 {
        self.state.expected_client
    }

    /// Cached `(device, inode, mode)` for inert admission-snapshot/role comparisons.
    /// This is neither a descriptor nor current reinspection or a capability.
    /// Require [`Self::validate_liveness`] at every subsequent use boundary.
    pub const fn descriptor_identity(&self) -> (u64, u64, u32) {
        let snapshot = self.state.descriptor_identity;
        (snapshot.device, snapshot.inode, snapshot.mode)
    }

    // Broker-only joins. These do not re-admit, convert or release this owner.
    pub(super) const fn descriptor_snapshot(&self) -> super::ObjectIdentityV1 {
        self.state.descriptor_identity
    }

    pub(super) const fn process_identity(&self) -> (u32, u64) {
        (self.state.expected_client.pid, self.state.start_time_ticks)
    }

    pub(super) fn pidfd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.state.pidfd.as_fd()
    }

    /// Full logical owner charge, retired only after this owner is dropped.
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }

    fn scope<T>(
        budget: &mut Budget<'_>,
        floor: usize,
        work: usize,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        budget.with_prepaid_scope(floor, native::ENTRY_WORK, work, Self::IO_STORAGE, operation)
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LiveClientPidfdIdentityV2")
            .field("authority", &"none")
            .field("expected_client", &self.expected_client())
            .finish_non_exhaustive()
    }
}

const _: () = {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    const fn envelope<T>() -> usize {
        size_of::<Result<T>>().saturating_sub(size_of::<T>())
            + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
    }
    assert!(Client::RETAINED >= Client::FD_STORAGE);
    assert!(CURRENT_PROCESS_START_TIME_IO_STORAGE_V2 >= super::native_io::SCRATCH_BYTES + 4096);
    assert!(super::native_io::SCRATCH_BYTES <= 2 * super::native_io::MAX_READ_CALLS + 4096);
    assert!(
        8 * size_of::<Error>()
            + 128 * size_of::<usize>()
            + size_of::<Ledger>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            + envelope::<(Client, Storage)>()
            + envelope::<(OwnedFd, Storage)>()
            + envelope::<u64>()
            + envelope::<()>()
            <= 4096
    );
    assert!(
        8 * size_of::<rustix::fs::Stat>()
            + 4 * size_of::<rustix::fs::StatFs>()
            + 4 * size_of::<super::PidfdInfoV0>()
            + 4 * size_of::<libc::pollfd>()
            + 256
            <= 4096
    );
};

#[cfg(test)]
#[path = "client_v2_tests.rs"]
mod tests;
