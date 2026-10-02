//! Same-account scratch queries with additional work and storage ceilings.
use super::*;
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

impl CanonicalKernelIrVerificationResourceBudgetV1<'_> {
    /// Fixed entry work, included in a bounded scratch scope's work allowance.
    pub const BOUNDED_SCRATCH_WORK_V1: usize = 8;
    /// Logical control-frame charge, included in the additional storage allowance.
    pub const BOUNDED_SCRATCH_STORAGE_V1: usize =
        4 * size_of::<Self>() + 8 * size_of::<usize>() + 1024;

    /// Runs a scratch-only query on the original work and storage account.
    ///
    /// Allowances include this scope's control work and frame. Every nested
    /// charge is admitted before use against both the caller's limits and these
    /// additional ceilings. The incoming storage and control frame cannot be
    /// released by the callback. Existing denial history does not prohibit a
    /// smaller query; accepted work, peaks, and first denials are never reset.
    ///
    /// Success, failure, unwind, and view replacement restore the original work
    /// limit and release only this account's new scratch. Replacing the view is
    /// rejected without charging or releasing the replacement account. Returned
    /// values have no retained reservation; callers must reserve their charge.
    /// The scoped view cannot escape or move the original account out:
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{
    ///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    /// };
    /// fn escape<'a>(budget: &mut Budget<'a>) {
    ///     let _view = budget.with_bounded_scratch_v1::<_, Resource>(
    ///         100, 4096, |view| Ok(view));
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{
    ///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    ///     CanonicalKernelIrWorkBudgetV1 as Work,
    /// };
    /// fn move_out(budget: &mut Budget<'_>, replacement_work: &'static mut Work) {
    ///     let _view = budget.with_bounded_scratch_v1::<_, Resource>(
    ///         100, 4096, |view| {
    ///             Ok(std::mem::replace(view, Budget::new(replacement_work, 0)))
    ///         });
    /// }
    /// ```
    pub fn with_bounded_scratch_v1<T, E: From<CanonicalKernelIrVerificationResourceErrorV1>>(
        &mut self,
        work_allowance: usize,
        storage_allowance: usize,
        operation: impl for<'scope> FnOnce(
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'scope>,
        ) -> Result<T, E>,
    ) -> Result<T, E> {
        use CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        let floor = self.storage();
        let work_limit = self
            .work()
            .checked_add(work_allowance)
            .ok_or(Resource::Arithmetic)?
            .min(self.work.limit());
        let ceiling = floor
            .checked_add(storage_allowance)
            .ok_or(Resource::Arithmetic)?
            .min(self.storage_limit())
            .min(self.window.map_or(usize::MAX, window::WindowState::ceiling));
        if floor > ceiling || floor < self.window.map_or(0, window::WindowState::floor) {
            return Err(Resource::Accounting.into());
        }
        let window = window::WindowState::bounded(floor, ceiling);
        let ledger = self.work_ledger_identity_v1();
        let original_limit = self.work.replace_limit(work_limit);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let mut scoped = CanonicalKernelIrVerificationResourceBudgetV1 {
                work: &mut *self.work,
                storage: Storage::Borrowed(self.storage.state_mut()),
                window: Some(&window),
            };
            let account = scoped.storage_account_identity_v1();
            scoped.charge_work(Self::BOUNDED_SCRATCH_WORK_V1)?;
            scoped.reserve_storage(Self::BOUNDED_SCRATCH_STORAGE_V1)?;
            window.protect_floor(scoped.storage());
            let result = operation(&mut scoped);
            if scoped.work_ledger_identity_v1() != ledger
                || scoped.storage_account_identity_v1() != account
                || !scoped
                    .window
                    .is_some_and(|current| std::ptr::eq(current, &window))
                || scoped.storage() < window.floor()
                || scoped.storage() > ceiling
            {
                return Err(Resource::Accounting.into());
            }
            result
        }));
        self.work.replace_limit(original_limit);
        let cleanup = self
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|scratch| self.release_storage(scratch));
        match outcome {
            Ok(result) => {
                cleanup?;
                result
            }
            Err(payload) => resume_unwind(payload),
        }
    }
}

#[cfg(test)]
#[path = "verification_resource_scope_v1_tests.rs"]
mod tests;
