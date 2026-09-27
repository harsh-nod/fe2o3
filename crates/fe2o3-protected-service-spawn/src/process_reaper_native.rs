//! Persistent funding for the existing cleanup pool, never a second reaper.

use super::{DeferredReaperV1, EMPTY, ReapSlotV1, ReaperMode, deferred_reaper};
use crate::MAX_PROTECTED_SERVICE_PROCESSES_V2 as CAPACITY;
use crate::native_spawn::ProtectedServiceSpawnStorageV2 as Storage;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use rustix::io::Errno;
use std::{error::Error, fmt, fs::File, mem::size_of, sync::atomic::Ordering};

const CONTROL_WORK: usize = 8;
const SHUTDOWN_WORK: usize = CONTROL_WORK + CAPACITY;

pub(super) struct NativeAccount {
    ledger: Account,
    cursor: usize,
    leased: bool,
    admission_open: bool,
    deployment_guard: Option<File>,
}

/// Fixed failure categories for native cleanup funding and custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedServiceCleanupErrorV2 {
    /// A different mode, active controller, or closed service prevents this operation.
    State,
    /// Persistent service or request accounting refused the operation before its work.
    Resource(Resource),
    /// A pump must visit between one and the fixed pool capacity inclusive.
    InvalidTurn,
    /// All slots are occupied, including reservations and quarantined records.
    Capacity,
    /// Draining or insufficient cleanup work stopped new admissions; custody remains charged.
    AdmissionStopped,
    /// Orderly shutdown requires every slot to be empty.
    Busy,
    /// The single attempt to duplicate a retained deployment guard failed.
    GuardIo(Errno),
}
use ProtectedServiceCleanupErrorV2 as Failure;

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State => f.write_str("cleanup service mode or controller is unavailable"),
            Self::Resource(error) => error.fmt(f),
            Self::InvalidTurn => f.write_str("cleanup turn exceeds the fixed slot bound"),
            Self::Capacity => f.write_str("cleanup pool is full"),
            Self::AdmissionStopped => f.write_str("cleanup service stopped new admission"),
            Self::Busy => f.write_str("cleanup service retains occupied slots"),
            Self::GuardIo(error) => write!(f, "clone cleanup deployment guard: {error}"),
        }
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::GuardIo(error) => Some(error),
            _ => None,
        }
    }
}
impl From<Resource> for Failure {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

/// Admission refusal returns the original ledger with all reached charges preserved.
pub struct ProtectedServiceCleanupAdmissionErrorV2 {
    error: Failure,
    account: Account,
}
impl ProtectedServiceCleanupAdmissionErrorV2 {
    /// Recovers the refusal and original owned account, including denial history.
    pub fn into_parts(self) -> (Failure, Account) {
        (self.error, self.account)
    }
}
impl fmt::Debug for ProtectedServiceCleanupAdmissionErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedServiceCleanupAdmissionErrorV2")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ProtectedServiceCleanupAdmissionErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl Error for ProtectedServiceCleanupAdmissionErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

/// Inert cumulative accounting snapshot, not execution or successful-reaping evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedServiceCleanupReportV2 {
    /// Accepted service work, including the prefix supplied at admission.
    pub work: usize,
    /// Original cumulative work limit; recovery does not change it.
    pub work_limit: usize,
    /// First rejected cumulative work total, retained across smaller accepted turns.
    pub failed_work: Option<usize>,
    /// Current persistent storage charge, retained even with no active controller.
    pub storage: usize,
    /// Highest reached storage reservation on the original service account.
    pub peak_storage: usize,
    /// Whether new request reservations remain enabled.
    pub admission_open: bool,
    /// Index of the next cell visited by a funded turn.
    pub next_slot: usize,
}

/// Exclusive controller for the process-global, persistently charged cleanup pool.
///
/// Admission irrevocably selects native mode; V1 cannot start its unmetered
/// worker in that process. Pumping is explicit and synchronous, without a
/// background thread. Drop returns control, not custody: the same account and
/// records remain in the pool for `recover`. Work limits are never renewed.
///
/// This is policy-neutral cleanup funding, not deployment authority. Native issuer
/// and root coordinator launches consume it through the same pool.
/// Logical quotas do not bound syscall or mutex latency.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedServiceCleanupServiceV2>();
/// ```
pub struct ProtectedServiceCleanupServiceV2 {
    reaper: &'static DeferredReaperV1,
    active: bool,
    shutdown_paid: bool,
}
use ProtectedServiceCleanupServiceV2 as Service;

impl Service {
    /// Fixed pool/controller, outstanding reservation headers and logical pidfd charges.
    ///
    /// Includes capacity for one deployment guard retained until empty shutdown.
    /// Embedded records, mutexes, phase state, lease and account metadata are
    /// included exactly once by `size_of`; this is logical retention, not RSS.
    pub const STORAGE: usize = size_of::<DeferredReaperV1>()
        + size_of::<Self>()
        + CAPACITY
            * (size_of::<ProtectedServiceCleanupReservationV2>()
                + size_of::<std::os::fd::OwnedFd>())
        + Self::GUARD_FILE_STORAGE;
    /// Full logical input charge for a deployment guard descriptor, excluding file data.
    pub const GUARD_FILE_STORAGE: usize = size_of::<(File, usize)>();
    /// Fixed request and service work (each) to transfer a deployment guard.
    pub const GUARD_WORK: usize = CONTROL_WORK + CAPACITY;
    /// Fixed work on EACH original ledger for a guard clone, including one
    /// duplication, close-only result cleanup and bounded controller checks.
    pub const GUARD_CLONE_WORK: usize = CONTROL_WORK + 2 * (1024 + 64) + 256;
    /// Request scratch above entry storage; the full output GUARD_FILE_STORAGE
    /// is also reserved before duplication. Persistent pool storage is unchanged.
    pub const GUARD_CLONE_SCRATCH: usize =
        4 * Self::GUARD_FILE_STORAGE + 4 * size_of::<Failure>() + 1024;
    /// Admission/rollback, controller Drop and one shutdown attempt prepaid before use.
    pub const ADMISSION_WORK: usize = CONTROL_WORK * 2 + SHUTDOWN_WORK;
    /// Reacquiring control prepays its Drop and one shutdown attempt on the same account.
    pub const RECOVERY_WORK: usize = CONTROL_WORK + SHUTDOWN_WORK;
    /// Fixed turn setup charge before inspecting any slot.
    pub const TURN_WORK: usize = CONTROL_WORK;
    /// Per-cell scan, lock, signal/wait, terminal retirement and descriptor/lease release.
    ///
    /// At most one signal and one nonblocking wait are performed per visited cell.
    pub const CELL_WORK: usize = 16;
    /// Request-local pre-clone scan plus emergency transfer and finalization allowance.
    ///
    /// Includes one signal, one wait, descriptor/lease retirement and slot publication;
    /// it does not fund later deferred turns or parent/child protocol execution.
    pub const RESERVATION_WORK: usize = CAPACITY + 32;

    /// Consumes an owned service account and reserves the entire pool before any launch.
    ///
    /// Current storage must be empty; a previously accepted work prefix is preserved.
    /// Refusal returns the account. This cannot replace a legacy or closed service.
    pub fn admit(account: Account) -> Result<Self, ProtectedServiceCleanupAdmissionErrorV2> {
        Self::admit_at(deferred_reaper(), account)
    }

    fn admit_at(
        reaper: &'static DeferredReaperV1,
        mut account: Account,
    ) -> Result<Self, ProtectedServiceCleanupAdmissionErrorV2> {
        let mut mode = reaper
            .mode
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let result = account.with_budget(|budget| {
            budget.charge_work(Self::ADMISSION_WORK)?;
            if !matches!(*mode, ReaperMode::Uninitialized) || budget.storage() != 0 {
                return Err(Failure::State);
            }
            budget.reserve_storage(Self::STORAGE)?;
            Ok(())
        });
        if let Err(error) = result {
            return Err(ProtectedServiceCleanupAdmissionErrorV2 { error, account });
        }
        let admission_open = NativeAccount::can_fund_turn(&account);
        *mode = ReaperMode::Native(NativeAccount {
            ledger: account,
            cursor: 0,
            leased: true,
            admission_open,
            deployment_guard: None,
        });
        Ok(Self {
            reaper,
            active: true,
            shutdown_paid: true,
        })
    }

    /// Reclaims a dropped controller without replacing its ledger, limits or records.
    ///
    /// Refuses while another controller is active. Exhaustion can prevent recovery;
    /// even an empty pool cannot then be closed through a new controller. It never
    /// authorizes an unmetered fallback or release of retained custody.
    pub fn recover() -> Result<Self, Failure> {
        Self::recover_at(deferred_reaper())
    }

    fn recover_at(reaper: &'static DeferredReaperV1) -> Result<Self, Failure> {
        let mut mode = reaper
            .mode
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let ReaperMode::Native(native) = &mut *mode else {
            return Err(Failure::State);
        };
        if native.leased {
            return Err(Failure::State);
        }
        native.charge(Self::RECOVERY_WORK)?;
        native.leased = true;
        Ok(Self {
            reaper,
            active: true,
            shutdown_paid: true,
        })
    }

    /// Visits a finite rotating prefix, after debiting its entire allowance.
    ///
    /// Failed charges do not advance the cursor or touch a child. Smaller later
    /// turns may drain remaining work; any service denial stops new admissions.
    pub fn pump(&mut self, visits: usize) -> Result<ProtectedServiceCleanupReportV2, Failure> {
        if visits == 0 || visits > CAPACITY {
            return Err(Failure::InvalidTurn);
        }
        let mut mode = self
            .reaper
            .mode
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let native = self.native(&mut mode)?;
        native.charge(Self::TURN_WORK + visits * Self::CELL_WORK)?;
        for _ in 0..visits {
            DeferredReaperV1::pump_cell(&self.reaper.cells[native.cursor]);
            native.cursor = (native.cursor + 1) % CAPACITY;
        }
        Ok(native.report())
    }

    /// Retains one close-only deployment guard in this persistently funded pool.
    ///
    /// The caller must authenticate and lock the descriptor before transfer. This
    /// method grants no authority, checks no pathname, and never unlocks the file.
    /// Only an empty, admission-open pool without a guard accepts it. There is no
    /// replacement or detach operation: even controller Drop, unwind, quarantine,
    /// and work exhaustion retain the descriptor until successful empty shutdown.
    ///
    /// Prepay GUARD_FILE_STORAGE on the request ledger; this consumes the File on
    /// either outcome and restores entry storage. Retire that input charge after
    /// return. Success transfers custody to the capacity prepaid by STORAGE, with
    /// GUARD_WORK debited on both original ledgers before any custody transfer.
    pub fn retain_deployment_guard(
        &mut self,
        guard: File,
        budget: &mut Budget<'_>,
    ) -> Result<(), Failure> {
        budget.with_prepaid_scope(Self::GUARD_FILE_STORAGE, 0, Self::GUARD_WORK, 0, |_| {
            let mut mode = self
                .reaper
                .mode
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let native = self.native(&mut mode)?;
            native.charge(Self::GUARD_WORK)?;
            if !native.admission_open
                || native.deployment_guard.is_some()
                || self
                    .reaper
                    .cells
                    .iter()
                    .any(|cell| cell.state.load(Ordering::Acquire) != EMPTY)
            {
                return Err(Failure::State);
            }
            native.deployment_guard = Some(guard);
            Ok(())
        })
    }

    /// Clones the existing deployment guard for controlled native validation.
    ///
    /// The active controller and an installed guard are required. Occupied and
    /// draining pools are allowed: this admits no child and cannot reopen
    /// admission. There is no replacement, detach, pathname or lock admission.
    /// The caller must validate the returned File against its actual native
    /// lifecycle owner; possession alone proves no deployment authority.
    ///
    /// One F_DUPFD_CLOEXEC attempt returns a descriptor at least 256, sharing the
    /// guard's open file description by construction. Never unlock that alias,
    /// change its shared flags or contents, or pass it to uncontrolled code.
    /// Close-only Drop does not unlock the pool's remaining alias. Conversely,
    /// an outstanding clone retains the lock even after successful pool shutdown.
    ///
    /// The guard stays charged to the original service account, not the request.
    /// GUARD_CLONE_WORK is debited on both ledgers before duplication. The request
    /// reserves GUARD_CLONE_SCRATCH plus GUARD_FILE_STORAGE above its entry charge;
    /// the service keeps its full STORAGE reservation. Entry request storage is
    /// restored on success, error and unwind without refunding work or history.
    /// Reserve the returned FULL additional_storage before retaining the File,
    /// then retire that charge only after Drop. No existing reservation is retired.
    ///
    /// ```
    /// use fe2o3_protected_service_spawn::{ProtectedServiceCleanupServiceV2 as Cleanup,
    ///     ProtectedServiceCleanupErrorV2 as Error};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn observe(c: &mut Cleanup, b: &mut Budget<'_>) -> Result<(), Error> {
    ///     let (file, charge) = c.try_clone_deployment_guard(b)?;
    ///     b.reserve_storage(charge.additional_storage())?;
    ///     // Actual native lifecycle validation belongs here, on this same budget.
    ///     drop(file);
    ///     b.release_storage(charge.additional_storage())?;
    ///     Ok(())
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_protected_service_spawn::ProtectedServiceCleanupServiceV2 as Cleanup;
    /// fn unmetered(c: &mut Cleanup) { let _ = c.try_clone_deployment_guard(); }
    /// ```
    pub fn try_clone_deployment_guard(
        &mut self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, Storage), Failure> {
        self.try_clone_deployment_guard_with(budget, |guard| {
            rustix::io::fcntl_dupfd_cloexec(guard, 256).map(File::from)
        })
    }

    fn try_clone_deployment_guard_with(
        &mut self,
        budget: &mut Budget<'_>,
        duplicate: impl FnOnce(&File) -> Result<File, Errno>,
    ) -> Result<(File, Storage), Failure> {
        budget.with_prepaid_scope(
            0,
            0,
            Self::GUARD_CLONE_WORK,
            Self::GUARD_CLONE_SCRATCH,
            |b| {
                b.reserve_storage(Self::GUARD_FILE_STORAGE)?;
                let mut mode = self
                    .reaper
                    .mode
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let native = self.native(&mut mode)?;
                native.charge(Self::GUARD_CLONE_WORK)?;
                if native.ledger.storage() != Self::STORAGE {
                    return Err(Resource::Accounting.into());
                }
                let guard = native.deployment_guard.as_ref().ok_or(Failure::State)?;
                let file = duplicate(guard).map_err(Failure::GuardIo)?;
                Ok((file, Storage(Self::GUARD_FILE_STORAGE)))
            },
        )
    }

    /// Reserves charged capacity and prepays request emergency cleanup before clone.
    ///
    /// No child is created. The reservation cannot produce execution authority.
    /// Dropping it returns unused capacity without borrowing the request ledger.
    /// Admission requires enough service work for at least a one-cell cleanup turn;
    /// this is not a guarantee that every retained child can eventually be reaped.
    pub fn reserve_launch(
        &mut self,
        budget: &mut Budget<'_>,
    ) -> Result<ProtectedServiceCleanupReservationV2, Failure> {
        budget.charge_work(Self::RESERVATION_WORK)?;
        let mut mode = self
            .reaper
            .mode
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let native = self.native(&mut mode)?;
        if !native.admission_open {
            return Err(Failure::AdmissionStopped);
        }
        let slot = self.reaper.reserve_slot().map_err(|_| Failure::Capacity)?;
        Ok(ProtectedServiceCleanupReservationV2 { slot: Some(slot) })
    }

    /// Reads fixed account counters; does not scan cells or perform cleanup I/O.
    pub fn report(&self) -> Result<ProtectedServiceCleanupReportV2, Failure> {
        let mut mode = self
            .reaper
            .mode
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        Ok(self.native(&mut mode)?.report())
    }

    /// Retires an empty pool, returning its original account and closing it permanently.
    ///
    /// The first attempt per controller was prepaid at admission/recovery; later
    /// attempts debit another bounded scan. Busy/refused shutdown retains custody.
    /// An admitted attempt stops new reservations, including when the pool is busy.
    pub fn shutdown(&mut self) -> Result<Account, Failure> {
        let mut mode = self
            .reaper
            .mode
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let native = self.native(&mut mode)?;
        if self.shutdown_paid {
            self.shutdown_paid = false;
        } else {
            native.charge(SHUTDOWN_WORK)?;
        }
        native.admission_open = false;
        if self
            .reaper
            .cells
            .iter()
            .any(|cell| cell.state.load(Ordering::Acquire) != EMPTY)
        {
            return Err(Failure::Busy);
        }
        if native.ledger.storage() != Self::STORAGE {
            return Err(Failure::Resource(Resource::Accounting));
        }
        // Every slot is terminal before releasing deployment replacement exclusion.
        drop(native.deployment_guard.take());
        native
            .ledger
            .with_budget(|budget| budget.release_storage(Self::STORAGE))?;
        let ReaperMode::Native(native) = std::mem::replace(&mut *mode, ReaperMode::Closed) else {
            unreachable!("exclusive native mode changed during shutdown")
        };
        self.active = false;
        Ok(native.ledger)
    }

    fn native<'a>(&self, mode: &'a mut ReaperMode) -> Result<&'a mut NativeAccount, Failure> {
        match mode {
            ReaperMode::Native(native) if self.active && native.leased => Ok(native),
            _ => Err(Failure::State),
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        if self.active {
            let mut mode = self
                .reaper
                .mode
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let ReaperMode::Native(native) = &mut *mode {
                native.leased = false;
            }
        }
    }
}

impl NativeAccount {
    fn can_fund_turn(ledger: &Account) -> bool {
        ledger.work_limit().saturating_sub(ledger.work()) >= Service::TURN_WORK + Service::CELL_WORK
    }

    fn charge(&mut self, work: usize) -> Result<(), Failure> {
        let result = self
            .ledger
            .with_budget(|budget| budget.charge_work(work))
            .map_err(|error| {
                self.admission_open = false;
                Failure::Resource(error)
            });
        self.admission_open &= Self::can_fund_turn(&self.ledger);
        result
    }

    fn report(&self) -> ProtectedServiceCleanupReportV2 {
        ProtectedServiceCleanupReportV2 {
            work: self.ledger.work(),
            work_limit: self.ledger.work_limit(),
            failed_work: self.ledger.failed_work(),
            storage: self.ledger.storage(),
            peak_storage: self.ledger.peak_storage(),
            admission_open: self.admission_open,
            next_slot: self.cursor,
        }
    }
}

/// Move-only, prepaid cleanup slot; not a child, process handle or launch authority.
///
/// Its storage belongs to the persistent service pool, not request scratch.
/// Native consuming launch will transfer its private slot into the shared child
/// owner; that consuming integration is not enabled by this reservation alone.
///
/// ```compile_fail
/// use fe2o3_protected_service_spawn::ProtectedServiceCleanupReservationV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedServiceCleanupReservationV2>();
/// ```
pub struct ProtectedServiceCleanupReservationV2 {
    slot: Option<ReapSlotV1<'static>>,
}
impl ProtectedServiceCleanupReservationV2 {
    pub(crate) fn into_slot(mut self) -> ReapSlotV1<'static> {
        self.slot.take().expect("unused native cleanup reservation")
    }
}
impl Drop for ProtectedServiceCleanupReservationV2 {
    fn drop(&mut self) {
        drop(self.slot.take());
    }
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn isolated_cleanup(account: Account) -> Service {
    // A separate process-lifetime pool avoids selecting the production global
    // mode in rootless controller tests. It never contains real child records.
    Service::admit_at(Box::leak(Box::new(DeferredReaperV1::new())), account).unwrap()
}

#[cfg(test)]
#[path = "process_reaper_native_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "process_reaper_guard_tests.rs"]
mod guard_tests;
