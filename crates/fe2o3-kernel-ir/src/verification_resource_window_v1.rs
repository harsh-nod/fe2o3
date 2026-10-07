//! Allocation-free additional-storage windows on the original owned account.
use super::*;
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Equality-only identity of the original owned storage state during its borrow.
/// Not a reservation, persistent identifier, or authority. Inline budgets have
/// no independently anchored window state and cannot create these windows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrStorageAccountIdentityV1(usize);

pub(super) struct WindowState {
    floor: AtomicUsize,
    ceiling: AtomicUsize,
    pub(super) retention: Option<retention::State>,
}
impl WindowState {
    pub(super) const fn new() -> Self {
        Self {
            floor: AtomicUsize::new(0),
            ceiling: AtomicUsize::new(usize::MAX),
            retention: Some(retention::State::new()),
        }
    }
    pub(super) fn floor(&self) -> usize {
        self.floor.load(Ordering::Relaxed)
    }
    pub(super) fn ceiling(&self) -> usize {
        self.ceiling.load(Ordering::Relaxed)
    }

    pub(super) const fn bounded(floor: usize, ceiling: usize) -> Self {
        Self {
            floor: AtomicUsize::new(floor),
            ceiling: AtomicUsize::new(ceiling),
            retention: None,
        }
    }

    pub(super) fn protect_floor(&self, floor: usize) {
        self.floor.store(floor, Ordering::Relaxed);
    }
}

struct Restore<'a> {
    state: &'a WindowState,
    floor: usize,
    ceiling: usize,
}
impl Drop for Restore<'_> {
    fn drop(&mut self) {
        self.state.floor.store(self.floor, Ordering::Relaxed);
        self.state.ceiling.store(self.ceiling, Ordering::Relaxed);
    }
}

impl CanonicalKernelIrVerificationResourceBudgetV1<'_> {
    /// Fixed window bookkeeping; nested operation work is additional.
    pub const STORAGE_WINDOW_WORK_V1: usize = 8;
    /// Control-frame allowance for enclosing callers, not an allocation/RSS bound.
    pub const STORAGE_WINDOW_SCRATCH_V1: usize =
        4 * size_of::<Restore<'static>>() + size_of::<[usize; 8]>() + 1024;

    /// Identifies owned storage independently of the Budget slot and Work pointer.
    /// Meaningful only while this account's original borrow remains alive.
    pub fn storage_account_identity_v1(&self) -> Option<CanonicalKernelIrStorageAccountIdentityV1> {
        self.window.map(|_| {
            CanonicalKernelIrStorageAccountIdentityV1(
                self.storage.state() as *const StorageState as usize
            )
        })
    }

    /// Restricts additional coexisting storage on this SAME Budget and account.
    /// The base is the actual entry storage, never a caller-selected exclusion.
    /// Original total `storage_limit()` is unchanged. Every nested reservation
    /// obeys both ceilings; releases cannot cross the entry floor. A nested
    /// window cannot widen its parent. This operation never refunds storage.
    ///
    /// Only views from Owned::with_budget are supported. Window control is
    /// anchored separately in that owned account, so errors, unwind and view
    /// replacement restore ONLY its previous window without touching a foreign
    /// account. Replacement refuses; storage/work/peak/denials are never copied,
    /// reset or restored. Returned owners keep their reservations. The caller
    /// prepays STORAGE_WINDOW_SCRATCH_V1 in its enclosing control-frame quote.
    pub fn with_additional_storage_window_v1<
        T,
        E: From<CanonicalKernelIrVerificationResourceErrorV1>,
    >(
        &mut self,
        allowance: usize,
        operation: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        use CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        self.charge_work(Self::STORAGE_WINDOW_WORK_V1)?;
        let state = self.window.ok_or(Resource::Accounting)?;
        let identity = self.storage_account_identity_v1();
        let ledger = self.work_ledger_identity_v1();
        let address = self as *const Self as usize;
        let floor = self.storage();
        let total = self.storage_limit();
        if floor > total || floor < state.floor() || floor > state.ceiling() {
            return Err(Resource::Accounting.into());
        }
        let ceiling = floor.checked_add(allowance).ok_or(Resource::Arithmetic)?;
        let restore = Restore {
            state,
            floor: state.floor(),
            ceiling: state.ceiling(),
        };
        state.floor.store(floor, Ordering::Relaxed);
        state
            .ceiling
            .store(ceiling.min(total).min(restore.ceiling), Ordering::Relaxed);
        let result = catch_unwind(AssertUnwindSafe(|| operation(self)));
        let unchanged = self.storage_account_identity_v1() == identity
            && self.work_ledger_identity_v1() == ledger
            && self as *const Self as usize == address
            && self
                .window
                .is_some_and(|current| std::ptr::eq(current, state))
            && self.storage_limit() == total
            && self.storage() >= floor
            && self.storage() <= state.ceiling();
        drop(restore);
        match result {
            Ok(result) if unchanged => result,
            Ok(_) => Err(Resource::Accounting.into()),
            Err(panic) => resume_unwind(panic),
        }
    }
}

#[cfg(test)]
#[path = "verification_resource_window_v1_tests.rs"]
mod tests;
