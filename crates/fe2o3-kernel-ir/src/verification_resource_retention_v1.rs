//! Original-account exit obligations for privately retained borrowed owners.
use super::*;
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

pub(super) struct State {
    loans: AtomicUsize,
}
impl State {
    pub(super) const fn new() -> Self {
        Self {
            loans: AtomicUsize::new(0),
        }
    }
}

pub(super) struct Exit<'a>(&'a window::WindowState);
impl<'a> Exit<'a> {
    pub(super) fn new(window: &'a window::WindowState) -> Self {
        Self(window)
    }
}
impl Drop for Exit<'_> {
    fn drop(&mut self) {
        if self
            .0
            .retention
            .as_ref()
            .is_some_and(|state| state.loans.load(Ordering::Relaxed) != 0)
        {
            // This guard is owned by the hidden account callback frame, not by
            // any caller-visible future or loan that can be forgotten.
            std::process::abort();
        }
    }
}

/// Move-only, same-thread retention of the original owned account's active view.
/// This is a logical lifetime obligation, not native or proof authority. Dropping
/// it ends only this token's retention; it asserts no execution settlement and
/// refunds no storage. Execution owners must retain it privately behind their
/// actual settlement guards, never expose it as a caller-completable permit.
///
/// Forgetting a token makes the original account callback fail-stop on normal
/// return or unwind, before its Work/storage window can move or be destroyed.
/// Forgetting the entire consuming account future instead retains that account.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::CanonicalKernelIrOriginalAccountRetentionLoanV1 as Loan;
/// fn duplicate(loan: Loan<'_>) { let _ = loan.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::CanonicalKernelIrOriginalAccountRetentionLoanV1 as Loan;
/// fn requires_send<T: Send>() {}
/// requires_send::<Loan<'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
/// fn escape(account: &mut Owned) {
///     let loan = account.with_budget(|budget| budget.retain_original_account_v1().unwrap());
/// }
/// ```
pub struct CanonicalKernelIrOriginalAccountRetentionLoanV1<'work> {
    state: &'work State,
    same_thread: PhantomData<Rc<()>>,
}

impl CanonicalKernelIrOriginalAccountRetentionLoanV1<'_> {
    /// Logical token header reserved during acquisition; never refunded by Drop.
    pub const RETAINED_STORAGE: usize = size_of::<Self>();
}

impl Drop for CanonicalKernelIrOriginalAccountRetentionLoanV1<'_> {
    fn drop(&mut self) {
        if self.state.loans.fetch_sub(1, Ordering::Relaxed) == 0 {
            std::process::abort();
        }
    }
}

impl<'work> CanonicalKernelIrVerificationResourceBudgetV1<'work> {
    /// Finite same-account retention bound, independent of native submission limits.
    pub const MAX_ORIGINAL_ACCOUNT_RETENTION_LOANS_V1: usize = 4096;

    /// Retains this original owned window until the returned private owner drops.
    /// Inline/replacement views and bounded scratch windows refuse. The original
    /// hidden callback guard survives view replacement and detects forgotten loans.
    /// Work and token-header storage are admitted before incrementing the counter;
    /// one compare-exchange attempt refuses interference rather than retrying.
    /// Drop never refunds the header: after safely dropping the loan, its enclosing
    /// owner may explicitly release that exact original-account reservation.
    pub fn retain_original_account_v1(
        &mut self,
    ) -> Result<
        CanonicalKernelIrOriginalAccountRetentionLoanV1<'work>,
        CanonicalKernelIrVerificationResourceErrorV1,
    > {
        use CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        self.charge_work(8)?;
        let state = self
            .window
            .and_then(|window| window.retention.as_ref())
            .ok_or(Resource::Accounting)?;
        if !matches!(self.storage, Storage::Borrowed(_)) {
            return Err(Resource::Accounting);
        }
        let current = state.loans.load(Ordering::Relaxed);
        let next = current
            .checked_add(1)
            .filter(|&n| n <= Self::MAX_ORIGINAL_ACCOUNT_RETENTION_LOANS_V1)
            .ok_or(Resource::Accounting)?;
        self.reserve_storage(CanonicalKernelIrOriginalAccountRetentionLoanV1::RETAINED_STORAGE)?;
        state
            .loans
            .compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
            .map_err(|_| Resource::Accounting)?;
        Ok(CanonicalKernelIrOriginalAccountRetentionLoanV1 {
            state,
            same_thread: PhantomData,
        })
    }
}

#[cfg(test)]
#[path = "verification_resource_retention_v1_tests.rs"]
mod tests;
