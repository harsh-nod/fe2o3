//! Persistent funding for the existing cleanup pool, never a second reaper.

use super::{DeferredReaperV1, EMPTY, ReapSlotV1, ReaperMode, deferred_reaper};
use crate::MAX_PROTECTED_ISSUER_PROCESSES_V1 as CAPACITY;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of, sync::atomic::Ordering};

const CONTROL_WORK: usize = 8;
const SHUTDOWN_WORK: usize = CONTROL_WORK + CAPACITY;

pub(super) struct NativeAccount {
    ledger: Account,
    cursor: usize,
    leased: bool,
    admission_open: bool,
}

/// Fixed failure categories for native cleanup funding and custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtectedIssuerCleanupErrorV2 {
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
}
use ProtectedIssuerCleanupErrorV2 as Failure;

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::State => f.write_str("cleanup service mode or controller is unavailable"),
            Self::Resource(error) => error.fmt(f),
            Self::InvalidTurn => f.write_str("cleanup turn exceeds the fixed slot bound"),
            Self::Capacity => f.write_str("cleanup pool is full"),
            Self::AdmissionStopped => f.write_str("cleanup service stopped new admission"),
            Self::Busy => f.write_str("cleanup service retains occupied slots"),
        }
    }
}
impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
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
pub struct ProtectedIssuerCleanupAdmissionErrorV2 {
    error: Failure,
    account: Account,
}
impl ProtectedIssuerCleanupAdmissionErrorV2 {
    /// Recovers the refusal and original owned account, including denial history.
    pub fn into_parts(self) -> (Failure, Account) {
        (self.error, self.account)
    }
}
impl fmt::Debug for ProtectedIssuerCleanupAdmissionErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtectedIssuerCleanupAdmissionErrorV2")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}
impl fmt::Display for ProtectedIssuerCleanupAdmissionErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl Error for ProtectedIssuerCleanupAdmissionErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

/// Inert cumulative accounting snapshot, not execution or successful-reaping evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedIssuerCleanupReportV2 {
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
/// This is cleanup funding only. Native consuming launch and service deployment
/// are not yet connected. Logical quotas do not bound syscall or mutex latency.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerCleanupServiceV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedIssuerCleanupServiceV2>();
/// ```
pub struct ProtectedIssuerCleanupServiceV2 {
    reaper: &'static DeferredReaperV1,
    active: bool,
    shutdown_paid: bool,
}
use ProtectedIssuerCleanupServiceV2 as Service;

impl Service {
    /// Fixed pool/controller, outstanding reservation headers and logical pidfd charges.
    ///
    /// Embedded records, mutexes, phase state, lease and account metadata are
    /// included exactly once by `size_of`; this is logical retention, not RSS.
    pub const STORAGE: usize = size_of::<DeferredReaperV1>()
        + size_of::<Self>()
        + CAPACITY
            * (size_of::<ProtectedIssuerCleanupReservationV2>()
                + size_of::<std::os::fd::OwnedFd>());
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
    pub fn admit(account: Account) -> Result<Self, ProtectedIssuerCleanupAdmissionErrorV2> {
        Self::admit_at(deferred_reaper(), account)
    }

    fn admit_at(
        reaper: &'static DeferredReaperV1,
        mut account: Account,
    ) -> Result<Self, ProtectedIssuerCleanupAdmissionErrorV2> {
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
            return Err(ProtectedIssuerCleanupAdmissionErrorV2 { error, account });
        }
        let admission_open = NativeAccount::can_fund_turn(&account);
        *mode = ReaperMode::Native(NativeAccount {
            ledger: account,
            cursor: 0,
            leased: true,
            admission_open,
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
    pub fn pump(&mut self, visits: usize) -> Result<ProtectedIssuerCleanupReportV2, Failure> {
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

    /// Reserves charged capacity and prepays request emergency cleanup before clone.
    ///
    /// No child is created. The reservation cannot produce execution authority.
    /// Dropping it returns unused capacity without borrowing the request ledger.
    /// Admission requires enough service work for at least a one-cell cleanup turn;
    /// this is not a guarantee that every retained child can eventually be reaped.
    pub fn reserve_launch(
        &mut self,
        budget: &mut Budget<'_>,
    ) -> Result<ProtectedIssuerCleanupReservationV2, Failure> {
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
        Ok(ProtectedIssuerCleanupReservationV2 { slot: Some(slot) })
    }

    /// Reads fixed account counters; does not scan cells or perform cleanup I/O.
    pub fn report(&self) -> Result<ProtectedIssuerCleanupReportV2, Failure> {
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

    fn report(&self) -> ProtectedIssuerCleanupReportV2 {
        ProtectedIssuerCleanupReportV2 {
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
/// use fe2o3_compiler_execution_supervisor::ProtectedIssuerCleanupReservationV2;
/// fn clone<T: Clone>() {}
/// clone::<ProtectedIssuerCleanupReservationV2>();
/// ```
pub struct ProtectedIssuerCleanupReservationV2 {
    slot: Option<ReapSlotV1<'static>>,
}
impl Drop for ProtectedIssuerCleanupReservationV2 {
    fn drop(&mut self) {
        drop(self.slot.take());
    }
}

#[cfg(test)]
#[path = "process_reaper_native_tests.rs"]
mod tests;
