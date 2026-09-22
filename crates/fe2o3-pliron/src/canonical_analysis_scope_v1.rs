//! Lexically scoped analyses of one immutable, connected canonical owner.
//! Mutation requires ending this scope and deriving fresh output analyses.

use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, CanonicalKirMemorySsaErrorV1,
    CanonicalKirMemorySsaLimitsV1, CanonicalKirMemorySsaV1, CanonicalKirSparseErrorV1,
    CanonicalKirSparseLimitsV1, CanonicalKirSparseV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, VerifiedCanonicalKernelIrModuleV12,
};
use std::{
    cell::Cell,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Debug)]
pub enum CanonicalAnalysisScopeErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Sparse(CanonicalKirSparseErrorV1),
    MemorySsa(CanonicalKirMemorySsaErrorV1),
}

impl fmt::Display for CanonicalAnalysisScopeErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Sparse(error) => error.fmt(f),
            Self::MemorySsa(error) => error.fmt(f),
        }
    }
}

impl Error for CanonicalAnalysisScopeErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Resource(error) => error,
            Self::Inventory(error) => error,
            Self::Sparse(error) => error,
            Self::MemorySsa(error) => error,
        })
    }
}

/// Lazy sparse and MemorySSA caches borrowing one actual immutable inventory.
/// There is no public constructor, replacement-owner API, or hash-based reuse.
/// Algorithms and their payload receipts remain in `fe2o3-kernel-analysis`.
pub struct CanonicalAnalysisScopeV1<'inventory, 'graph, 'budget, 'work> {
    inventory: &'inventory CanonicalKirInventoryV1<'graph>,
    sparse: Option<CanonicalKirSparseV1<'inventory, 'graph>>,
    memory_ssa: Option<CanonicalKirMemorySsaV1<'inventory, 'graph>>,
    budget: &'budget mut Budget<'work>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    live_floor: &'budget Cell<usize>,
    poisoned: &'budget Cell<bool>,
}

impl<'inventory, 'graph, 'work> CanonicalAnalysisScopeV1<'inventory, 'graph, '_, 'work> {
    pub fn inventory(&self) -> &CanonicalKirInventoryV1<'graph> {
        self.inventory
    }

    /// Lends the existing inventory and original budget without deriving a cache.
    /// Each request prepays the same six entry/postflight checks as cache queries.
    /// Consumer traversal and output storage retain their own accounting.
    ///
    /// The inventory borrow cannot escape this callback. Immutable references to
    /// the original graph owner may retain its existing lifetime; neither kind of
    /// reference grants source, safety, artifact, or launch authority.
    /// Consumer errors do not poison the scope. Observed ledger/slot/live-floor
    /// violations do, even when caught. Scratch must drop before return/unwind;
    /// escaping output storage must be prepaid before the outer scope.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
    ///     CanonicalKernelIrVerificationResourceBudgetV1};
    /// use fe2o3_pliron::{with_canonical_analysis_scope_v1, CanonicalAnalysisScopeErrorV1};
    /// fn escape(owner: &VerifiedCanonicalKernelIrModuleV12,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     let _ = with_canonical_analysis_scope_v1(owner, budget, |scope| {
    ///         scope.with_inventory_v1(|inventory, _|
    ///             Ok::<_, CanonicalAnalysisScopeErrorV1>(inventory))
    ///     });
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
    ///     CanonicalKernelIrVerificationResourceBudgetV1};
    /// use fe2o3_pliron::{with_canonical_analysis_scope_v1, CanonicalAnalysisScopeErrorV1};
    /// fn escape_rows(owner: &VerifiedCanonicalKernelIrModuleV12,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     let _ = with_canonical_analysis_scope_v1(owner, budget, |scope| {
    ///         scope.with_inventory_v1(|inventory, _|
    ///             Ok::<_, CanonicalAnalysisScopeErrorV1>(inventory.functions()))
    ///     });
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
    ///     CanonicalKernelIrVerificationResourceBudgetV1};
    /// use fe2o3_pliron::{with_canonical_analysis_scope_v1, CanonicalAnalysisScopeErrorV1};
    /// fn escape_budget(owner: &VerifiedCanonicalKernelIrModuleV12,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     let _ = with_canonical_analysis_scope_v1(owner, budget, |scope| {
    ///         scope.with_inventory_v1(|_, budget|
    ///             Ok::<_, CanonicalAnalysisScopeErrorV1>(budget))
    ///     });
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
    ///     CanonicalKernelIrVerificationResourceBudgetV1};
    /// use fe2o3_pliron::{with_canonical_analysis_scope_v1, CanonicalAnalysisScopeErrorV1};
    /// fn mutate(owner: &VerifiedCanonicalKernelIrModuleV12,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     let _ = with_canonical_analysis_scope_v1(owner, budget, |scope| {
    ///         scope.with_inventory_v1(|inventory, _| {
    ///             inventory.owner().module().functions.clear();
    ///             Ok::<_, CanonicalAnalysisScopeErrorV1>(())
    ///         })
    ///     });
    /// }
    /// ```
    pub fn with_inventory_v1<T, E>(
        &mut self,
        body: impl for<'borrow> FnOnce(
            &'borrow CanonicalKirInventoryV1<'graph>,
            &mut Budget<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        self.begin_request()?;
        let result = catch_unwind(AssertUnwindSafe(|| body(self.inventory, self.budget)));
        self.finish_request(result)
    }

    /// Derives sparse facts once, on first demand, with the fixed default limits.
    /// Each request prepays six checks, including the cache lookup and callback
    /// postflight. A first successful derivation also pays two receipt-transfer
    /// checks. Consumers must preserve the ledger and all live analysis floors.
    pub fn with_sparse_v1<T, E>(
        &mut self,
        body: impl FnOnce(&CanonicalKirSparseV1<'inventory, 'graph>, &mut Budget<'work>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        self.begin_request()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            if self.sparse.is_none() {
                let (report, storage) = CanonicalKirSparseV1::derive(
                    self.inventory,
                    CanonicalKirSparseLimitsV1::default(),
                    self.budget,
                )
                .map_err(CanonicalAnalysisScopeErrorV1::Sparse)?;
                self.transfer_storage(storage.retained_storage())?;
                self.sparse = Some(report);
            }
            body(
                self.sparse.as_ref().expect("installed sparse cache"),
                self.budget,
            )
        }));
        self.finish_request(result)
    }

    /// Derives MemorySSA once, without requiring sparse facts. The same six
    /// request checks and two first-transfer checks apply as for sparse facts.
    /// This is an analysis of the exact inventory, not memory-safety authority.
    pub fn with_memory_ssa_v1<T, E>(
        &mut self,
        body: impl FnOnce(
            &CanonicalKirMemorySsaV1<'inventory, 'graph>,
            &mut Budget<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        self.begin_request()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            if self.memory_ssa.is_none() {
                let (report, storage) = CanonicalKirMemorySsaV1::derive(
                    self.inventory,
                    CanonicalKirMemorySsaLimitsV1::default(),
                    self.budget,
                )
                .map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?;
                self.transfer_storage(storage.retained_storage())?;
                self.memory_ssa = Some(report);
            }
            body(
                self.memory_ssa.as_ref().expect("installed MemorySSA cache"),
                self.budget,
            )
        }));
        self.finish_request(result)
    }

    fn same_ledger(&self) -> bool {
        self.slot == std::ptr::from_ref(&*self.budget) as usize
            && self.ledger == self.budget.work_ledger_identity_v1()
    }

    fn poison(&mut self) -> CanonicalAnalysisScopeErrorV1 {
        self.poisoned.set(true);
        // The enclosing scope releases reservations only after inventory drops.
        self.sparse = None;
        self.memory_ssa = None;
        CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting)
    }

    fn begin_request(&mut self) -> Result<(), CanonicalAnalysisScopeErrorV1> {
        // Never debit a substituted Work meter, including on a poisoned retry.
        if !self.same_ledger()
            || self.poisoned.get()
            || self.budget.storage() < self.live_floor.get()
        {
            return Err(self.poison());
        }
        // Lookup, slot, Work identity, poison, floor, and prepaid postflight.
        self.budget
            .charge_work(6)
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        Ok(())
    }

    fn transfer_storage(&mut self, retained: usize) -> Result<(), CanonicalAnalysisScopeErrorV1> {
        self.budget
            .charge_work(2)
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        if self.budget.storage() != self.live_floor.get() {
            return Err(self.poison());
        }
        let floor = self.live_floor.get().checked_add(retained).ok_or(
            CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic),
        )?;
        self.budget
            .reserve_storage(retained)
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        self.live_floor.set(floor);
        Ok(())
    }

    fn finish_request<T, E>(&mut self, result: std::thread::Result<Result<T, E>>) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        if !self.same_ledger()
            || self.poisoned.get()
            || self.budget.storage() < self.live_floor.get()
        {
            return Err(self.poison().into());
        }
        // Consumer scratch must have been dropped before returning or unwinding.
        self.budget
            .release_storage(self.budget.storage() - self.live_floor.get())
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}

/// Borrows one exact owner, builds its inventory, and lends a lazy analysis cache.
/// The caller reserves the owner's payload in the incoming storage floor. On
/// success, Result error, and unwind, caches and inventory are dropped before
/// that floor is restored; work, peak storage and failures are never rewound.
/// Accounting loss dominates callback errors/panics. A foreign Work ledger is
/// neither charged nor cleaned up; a lost incoming floor is never recreated.
/// An observed ledger/floor violation permanently poisons both lazy caches,
/// even if the caller catches the error and later restores the budget.
///
/// Consumer scratch must be dropped before its callback returns/unwinds; its
/// surplus reservation is released by the scope. Storage escaping in returned
/// values or captured state must be reserved before scope entry. Fixed checks
/// do not meter arbitrary consumer allocations. Transient violations restored
/// inside a callback are not observable. `inventory()` is a structural borrow,
/// not a guarded analysis query or compiler authority.
/// Stack-only scope framing and the caller's unrelated allocations are not metered.
/// Returned values cannot borrow the local inventory or cached report.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
///     CanonicalKernelIrVerificationResourceBudgetV1};
/// use fe2o3_pliron::{with_canonical_analysis_scope_v1, CanonicalAnalysisScopeErrorV1};
/// fn escape(owner: &VerifiedCanonicalKernelIrModuleV12,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_canonical_analysis_scope_v1(owner, budget, |scope| {
///         scope.with_sparse_v1(|report, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(report))
///     });
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
///     CanonicalKernelIrVerificationResourceBudgetV1};
/// use fe2o3_pliron::{with_canonical_analysis_scope_v1, CanonicalAnalysisScopeErrorV1};
/// fn escape(owner: &VerifiedCanonicalKernelIrModuleV12,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_canonical_analysis_scope_v1(owner, budget, |scope| {
///         Ok::<_, CanonicalAnalysisScopeErrorV1>(scope.inventory())
///     });
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
///     CanonicalKernelIrVerificationResourceBudgetV1};
/// use fe2o3_pliron::{with_canonical_analysis_scope_v1, CanonicalAnalysisScopeErrorV1};
/// fn escape(owner: &VerifiedCanonicalKernelIrModuleV12,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = with_canonical_analysis_scope_v1(owner, budget, |scope| {
///         scope.with_memory_ssa_v1(|report, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(report))
///     });
/// }
/// ```
pub fn with_canonical_analysis_scope_v1<'graph, 'work, T, E>(
    owner: &'graph VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'work>,
    body: impl FnOnce(&mut CanonicalAnalysisScopeV1<'_, 'graph, '_, 'work>) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<CanonicalAnalysisScopeErrorV1>,
{
    // Slot, Work identity, incoming floor, and prepaid outer postflight.
    budget
        .charge_work(4)
        .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
    let floor = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let live_floor = Cell::new(floor);
    let poisoned = Cell::new(false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (inventory, storage) = CanonicalKirInventoryV1::derive(owner, budget)
            .map_err(CanonicalAnalysisScopeErrorV1::Inventory)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        live_floor.set(budget.storage());
        body(&mut CanonicalAnalysisScopeV1 {
            inventory: &inventory,
            sparse: None,
            memory_ssa: None,
            budget,
            slot,
            ledger,
            live_floor: &live_floor,
            poisoned: &poisoned,
        })
    }));
    if slot != std::ptr::from_ref(&*budget) as usize || ledger != budget.work_ledger_identity_v1() {
        return Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting).into());
    }
    let invalid = poisoned.get() || budget.storage() < live_floor.get();
    let released =
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(CanonicalAnalysisScopeErrorV1::Resource(
                Resource::Accounting,
            ))?;
    budget
        .release_storage(released)
        .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
    if invalid {
        return Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting).into());
    }
    match result {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "canonical_analysis_scope_v1_tests.rs"]
mod tests;
