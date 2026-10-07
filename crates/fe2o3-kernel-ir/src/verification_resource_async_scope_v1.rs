//! Lending asynchronous access to the original owned logical resource account.
use super::*;

impl CanonicalKernelIrOwnedVerificationResourceBudgetV1 {
    /// Consumes this original account for one lending asynchronous callback,
    /// returning the same account and callback result after the borrow ends.
    /// Work, storage, peak and first-denial state are neither copied nor reset.
    /// No allocation, resource replenishment or execution authority is introduced.
    ///
    /// The view is created on first poll, inside the pinned future, and keeps the
    /// original Work/storage window at stable locations across suspension. A
    /// returned account may move only after all callback borrows have ended;
    /// identity tokens are meaningful only within that original active borrow.
    ///
    /// Dropping the future first drops the callback's borrowed owners, then its
    /// view and account. There is no automatic storage refund on cancellation.
    /// Forgetting the outer future retains the original account with its callback,
    /// rather than ending a borrow of an externally movable account. Execution
    /// owners must still enforce their own settlement, cancellation and nested
    /// forgotten-future guards; this inert ledger is not an execution scope.
    /// The hidden original-account exit guard fails stop if any explicit
    /// retention loan remains when this callback returns or its future drops.
    /// This also covers a forgotten nested future or replaced budget view.
    ///
    /// The borrowed view cannot escape through the callback result:
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    /// async fn escape(account: Owned) {
    ///     let (_, view) = account.into_budget_scope_async_v1(async |view| view).await;
    /// }
    /// ```
    ///
    /// A moved-out original view cannot escape either:
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{
    ///     CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    ///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    ///     CanonicalKernelIrWorkBudgetV1 as Work,
    /// };
    /// async fn escape(account: Owned, replacement: &'static mut Work) {
    ///     let (_, view) = account.into_budget_scope_async_v1(async move |view| {
    ///         std::mem::replace(view, Budget::new(replacement, 0))
    ///     }).await;
    /// }
    /// ```
    ///
    /// The caller cannot reuse the consumed account while its future is live:
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    /// fn duplicate(mut account: Owned) {
    ///     let future = account.into_budget_scope_async_v1(async |_| ());
    ///     account.with_budget(|view| view.charge_work(1).unwrap());
    ///     drop(future);
    /// }
    /// ```
    pub async fn into_budget_scope_async_v1<T>(
        mut self,
        operation: impl for<'work> AsyncFnOnce(
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
        ) -> T,
    ) -> (Self, T) {
        let result = {
            let _retained = retention::Exit::new(&self.window);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1 {
                work: &mut self.work,
                storage: Storage::Borrowed(&mut self.storage),
                window: Some(&self.window),
            };
            operation(&mut budget).await
        };
        (self, result)
    }
}

#[cfg(test)]
#[path = "verification_resource_async_scope_v1_tests.rs"]
mod tests;
