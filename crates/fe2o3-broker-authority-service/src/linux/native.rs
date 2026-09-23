//! Fresh bounded anchor transport custody; no legacy-owner conversion.
use super::{checks::CheckError, continuity::Native, *};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{mem::size_of, os::fd::BorrowedFd};

const ENTRY_WORK: usize = 8;
const RECORD_BYTES: usize = 4096;
const RECORD_ATTEMPTS: usize = RECORD_BYTES + 1;

/// Additional unreserved logical storage. Preserve consumed inputs' charges,
/// reserve this delta immediately, and retire full charges only after drop or transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedExternalAnchorServiceStorageV2(usize);
impl ProtectedExternalAnchorServiceStorageV2 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use ProtectedExternalAnchorServiceStorageV2 as Storage;

/// Allocation-free native refusal. Display formatting is a caller operation.
#[derive(Debug)]
pub struct ProtectedExternalAnchorServiceErrorV2(Failure);
#[derive(Debug)]
enum Failure {
    Resource(Resource),
    Inspection(CheckError),
}
type Error = ProtectedExternalAnchorServiceErrorV2;
type Result<T> = std::result::Result<T, Error>;
impl Error {
    pub const fn kind(&self) -> Option<AdmissionErrorKindV1> {
        match &self.0 {
            Failure::Inspection(e) => Some(e.kind()),
            _ => None,
        }
    }
    pub const fn errno(&self) -> Option<i32> {
        match &self.0 {
            Failure::Inspection(e) => e.errno(),
            _ => None,
        }
    }
    pub const fn resource(&self) -> Option<Resource> {
        match self.0 {
            Failure::Resource(e) => Some(e),
            _ => None,
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Failure::Resource(e) => e.fmt(f),
            Failure::Inspection(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.0 {
            Failure::Resource(e) => Some(e),
            Failure::Inspection(e) => Some(e),
        }
    }
}

/// Move-only anchor endpoint and pidfd custody, freshly admitted with bounded I/O.
///
/// Uses the same socket, credential, process-target, start-time and liveness
/// schedule as V1, but never its growable reads, retry loops or allocating errors.
/// Production admission always requires an anchor UID distinct from the issuer.
/// A successful check is a point-in-time observation, not perpetual liveness,
/// exclusive endpoint ownership, key custody, anti-rollback or service authority.
///
/// Every call restores entry storage on success, refusal and unwind; accepted
/// work is not refunded. Incoming owners remain prepaid until transferred or
/// dropped. Consuming refusal closes its inputs even if entry admission fails.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceAdmissionV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedExternalAnchorServiceAdmissionV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceAdmissionV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<ProtectedExternalAnchorServiceAdmissionV2>();
/// ```
/// ```compile_fail
/// use fe2o3_broker_authority_service::{ProtectedExternalAnchorServiceAdmissionV1, ProtectedExternalAnchorServiceAdmissionV2};
/// fn upgrade(old: ProtectedExternalAnchorServiceAdmissionV1) -> ProtectedExternalAnchorServiceAdmissionV2 {
///     old.into()
/// }
/// ```
pub struct ProtectedExternalAnchorServiceAdmissionV2 {
    // Policy-neutral mechanical state is shared; admission/checks select Native.
    state: ProtectedExternalAnchorServiceAdmissionV1,
    #[cfg(test)]
    same_uid_fixture: bool,
}
type Anchor = ProtectedExternalAnchorServiceAdmissionV2;
impl Anchor {
    const RETAINED: usize = size_of::<(Self, Storage)>();
    /// Full logical charge for two borrowed, consumed or returned FD owners.
    /// Includes receipts/padding, not socket queues or kernel object allocation.
    pub const PAIR_STORAGE: usize = 2 * size_of::<(OwnedFd, Storage)>();
    /// Fixed scratch for staged owners, transient descriptor pairs, procfs
    /// records, syscall structures and scope/error envelopes. Not stack/RSS bounds.
    pub const IO_STORAGE: usize =
        8 * Self::RETAINED + 4 * Self::PAIR_STORAGE + 2 * RECORD_ATTEMPTS + 8192;
    /// Full logical operation allowances, including fallback record scans.
    pub const ADMISSION_WORK: usize = operation_work(7);
    pub const REVALIDATION_WORK: usize = operation_work(4);
    pub const VALIDATE_TRANSFER_WORK: usize = operation_work(19);
    pub const CLONE_TRANSFER_WORK: usize = operation_work(27);

    /// Consumes a prepaid endpoint/pidfd pair, returning only owner growth.
    pub fn admit(
        peer: OwnedFd,
        pidfd: OwnedFd,
        service: CompilerExecutionExternalAnchorServiceIdentityV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::admit_inner::<true>(peer, pidfd, service, budget)
    }

    fn admit_inner<const DISTINCT: bool>(
        peer: OwnedFd,
        pidfd: OwnedFd,
        service: CompilerExecutionExternalAnchorServiceIdentityV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        // No public or feature-gated native constructor can select false.
        #[cfg(not(test))]
        const {
            assert!(DISTINCT);
        }
        Self::scope(budget, Self::PAIR_STORAGE, Self::ADMISSION_WORK, |_| {
            let state = ProtectedExternalAnchorServiceAdmissionV1::admit_with::<Native, DISTINCT>(
                peer, pidfd, service,
            )?;
            Ok((
                Self {
                    state,
                    #[cfg(test)]
                    same_uid_fixture: !DISTINCT,
                },
                Storage(Self::RETAINED - Self::PAIR_STORAGE),
            ))
        })
    }

    pub fn validate_continuity(&self, budget: &mut Budget<'_>) -> Result<()> {
        Self::scope(budget, Self::RETAINED, Self::REVALIDATION_WORK, |_| {
            #[cfg(test)]
            if self.same_uid_fixture {
                return Ok(self.state.validate_continuity_with::<Native, false>()?);
            }
            Ok(self.state.validate_continuity_with::<Native, true>()?)
        })
    }

    /// Returns a separately charged CLOEXEC pair. Subsequent arbitrary FD
    /// operations are outside this method's quota and continuity observation.
    pub fn try_clone_for_transfer(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(OwnedFd, OwnedFd, Storage)> {
        Self::scope(budget, Self::RETAINED, Self::CLONE_TRANSFER_WORK, |_| {
            #[cfg(test)]
            let pair = if self.same_uid_fixture {
                self.state.clone_transfer_with::<Native, false>()?
            } else {
                self.state.clone_transfer_with::<Native, true>()?
            };
            #[cfg(not(test))]
            let pair = self.state.clone_transfer_with::<Native, true>()?;
            Ok((pair.0, pair.1, Storage(Self::PAIR_STORAGE)))
        })
    }

    /// Borrows a prepaid pair and this owner, requiring the exact retained
    /// socket object, pidfd object, service process, start time and credentials.
    pub fn validate_transfer(
        &self,
        peer: BorrowedFd<'_>,
        pidfd: BorrowedFd<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        Self::scope(
            budget,
            Self::RETAINED + Self::PAIR_STORAGE,
            Self::VALIDATE_TRANSFER_WORK,
            |_| {
                #[cfg(test)]
                if self.same_uid_fixture {
                    return Ok(self
                        .state
                        .validate_transfer_with::<Native, false>(&peer, &pidfd)?);
                }
                Ok(self
                    .state
                    .validate_transfer_with::<Native, true>(&peer, &pidfd)?)
            },
        )
    }

    pub const fn service_identity(&self) -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        self.state.expected_service
    }
    pub const fn service_process_identity(&self) -> ExpectedClientProcessIdentityV1 {
        self.state.live_service.expected_client
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }

    fn scope<T>(
        budget: &mut Budget<'_>,
        floor: usize,
        work: usize,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        budget.with_prepaid_scope(floor, ENTRY_WORK, work, Self::IO_STORAGE, operation)
    }
}
impl fmt::Debug for Anchor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedExternalAnchorServiceAdmissionV2")
            .field("authority", &"none")
            .field("service", &self.service_identity())
            .field("process", &self.service_process_identity())
            .finish_non_exhaustive()
    }
}

// Each target probe may read fdinfo; each matching start-time probe reads stat.
// Prepay every 4097-attempt record schedule, even when the ioctl avoids fdinfo.
// The 64 per-record and 512 outer call slots include metadata, open/dup/close,
// credentials and liveness. Units are weighted calls and byte visits, not cycles.
const fn operation_work(probes: usize) -> usize {
    ENTRY_WORK + 512 * 1024 + 2 * probes * ((RECORD_ATTEMPTS + 64) * 1024 + 16 * RECORD_ATTEMPTS)
}

const _: () = {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    const fn envelope<T>() -> usize {
        size_of::<Result<T>>().saturating_sub(size_of::<T>())
            + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
    }
    assert!(Anchor::RETAINED >= Anchor::PAIR_STORAGE);
    assert!(RECORD_ATTEMPTS == super::native_io::MAX_READ_CALLS);
    assert!(super::native_io::SCRATCH_BYTES <= 2 * RECORD_ATTEMPTS + 4096);
    assert!(Anchor::PAIR_STORAGE >= size_of::<(OwnedFd, OwnedFd, Storage)>());
    assert!(
        8 * size_of::<Error>()
            + 128 * size_of::<usize>()
            + size_of::<Ledger>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            + envelope::<(Anchor, Storage)>()
            + envelope::<(OwnedFd, OwnedFd, Storage)>()
            + envelope::<()>()
            <= 4096
    );
    assert!(
        8 * size_of::<rustix::fs::Stat>()
            + 4 * size_of::<rustix::fs::StatFs>()
            + 4 * size_of::<libc::sockaddr_un>()
            + 4 * size_of::<PidfdInfoV0>()
            + 4 * size_of::<libc::pollfd>()
            + 256
            <= 4096
    );
};

#[cfg(test)]
#[path = "native_tests.rs"]
mod tests;
