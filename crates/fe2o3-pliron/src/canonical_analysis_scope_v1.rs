//! Lexically scoped analyses of one immutable, connected canonical owner.
//! Mutation requires ending this scope and deriving fresh output analyses.

use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryStorageV1, CanonicalKirInventoryV1,
    CanonicalKirInventoryV18, CanonicalKirMemorySsaErrorV1, CanonicalKirMemorySsaLimitsV1,
    CanonicalKirMemorySsaStorageV1, CanonicalKirMemorySsaV1, CanonicalKirMemorySsaV18,
    CanonicalKirSparseErrorV1, CanonicalKirSparseLimitsV1, CanonicalKirSparseStorageV1,
    CanonicalKirSparseV1, CanonicalKirSparseV18,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, VerifiedCanonicalKernelIrModuleV12,
    VerifiedCanonicalKernelIrModuleV18,
};
use std::{
    cell::Cell,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};
use crate::kir_bridge_v1::{
    BOUNDED_PAYLOAD_CLEANUP_ATTEMPTS_V1, discard_bounded_payload_v1,
};

type AnalysisPanicPayloadV1 = Box<dyn std::any::Any + Send>;

fn callback_disposal_headers_v1<T, E>() -> Result<usize, Resource> {
    use std::mem::size_of;
    if !std::mem::needs_drop::<T>() {
        return Ok(0);
    }
    // The selected callback result and rejected value remain live while a
    // destructor panic is settled by the existing bounded payload helper.
    [
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<AssertUnwindSafe<T>>(),
        4 * size_of::<AnalysisPanicPayloadV1>(),
        size_of::<AssertUnwindSafe<AnalysisPanicPayloadV1>>(),
        2 * size_of::<std::thread::Result<()>>(),
        size_of::<std::ops::Range<usize>>(),
        size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, part| sum.checked_add(part).ok_or(Resource::Arithmetic))
}

fn callback_disposal_work_v1<T>() -> usize {
    if std::mem::needs_drop::<T>() {
        1 + BOUNDED_PAYLOAD_CLEANUP_ATTEMPTS_V1
    } else {
        0
    }
}

fn discard_rejected_callback_value_v1<T>(value: T) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(value))) {
        discard_bounded_payload_v1(payload);
    }
}

#[derive(Clone, Copy, Debug)]
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
pub struct CanonicalAnalysisScopeV1<
    'inventory,
    'graph,
    'budget,
    'work,
    O = VerifiedCanonicalKernelIrModuleV12,
> {
    inventory: &'inventory CanonicalKirInventoryV1<'graph, O>,
    sparse: Option<CanonicalKirSparseV1<'inventory, 'graph, O>>,
    memory_ssa: Option<CanonicalKirMemorySsaV1<'inventory, 'graph, O>>,
    budget: &'budget mut Budget<'work>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    live_floor: &'budget Cell<usize>,
    poisoned: &'budget Cell<bool>,
    cleanup: &'budget CanonicalAnalysisCleanupV1<'budget>,
    factories: &'budget AnalysisFactoriesV1<O>,
}

/// The same lazy analysis scope borrowing a storage-capable V18 owner.
pub type CanonicalAnalysisScopeV18<'inventory, 'graph, 'budget, 'work> = CanonicalAnalysisScopeV1<
    'inventory,
    'graph,
    'budget,
    'work,
    VerifiedCanonicalKernelIrModuleV18,
>;

/// A monotone notification that a nested analysis lost concrete ledger custody.
/// It grants no authority, cannot be reset, and must remain alive until a
/// containing owner has settled both normal returns and raw unwinds.
#[derive(Default)]
pub struct CanonicalAnalysisCleanupV1<'parent> {
    denied: Cell<bool>,
    parent: Option<&'parent Cell<bool>>,
}

impl<'parent> CanonicalAnalysisCleanupV1<'parent> {
    /// Creates a fresh enclosing-attempt notification, initially not denied.
    pub const fn new() -> Self {
        Self {
            denied: Cell::new(false),
            parent: None,
        }
    }

    /// Links one containing attempt's private, never-reset denial cell.
    /// The cell is only a veto: false grants no custody, and once this scope
    /// observes true it remains denied even if an external caller resets it.
    /// Analysis custody loss also denies the containing attempt immediately.
    pub const fn linked(parent: &'parent Cell<bool>) -> Self {
        Self {
            denied: Cell::new(false),
            parent: Some(parent),
        }
    }

    /// A true result forbids every containing attempt from refunding a delta.
    /// False is only absence of an observed denial, never a custody certificate.
    pub fn refund_denied(&self) -> bool {
        if self.parent.is_some_and(Cell::get) {
            self.denied.set(true);
        }
        self.denied.get()
    }

    fn deny(&self) {
        self.deny_refund();
    }

    /// Permanently forwards a nested owner's observed custody loss. This is
    /// only a veto: it cannot establish custody or reset an earlier denial.
    pub fn deny_refund(&self) {
        self.denied.set(true);
        if let Some(parent) = self.parent {
            parent.set(true);
        }
    }
}

type InventoryFactoryV1<O> = for<'g, 'w> fn(
    &'g O,
    &mut Budget<'w>,
) -> Result<
    (
        CanonicalKirInventoryV1<'g, O>,
        CanonicalKirInventoryStorageV1,
    ),
    CanonicalKirInventoryErrorV1,
>;
type SparseFactoryV1<O> = for<'i, 'g, 'w> fn(
    &'i CanonicalKirInventoryV1<'g, O>,
    CanonicalKirSparseLimitsV1,
    &mut Budget<'w>,
) -> Result<
    (CanonicalKirSparseV1<'i, 'g, O>, CanonicalKirSparseStorageV1),
    CanonicalKirSparseErrorV1,
>;
type MemoryFactoryV1<O> = for<'i, 'g, 'w> fn(
    &'i CanonicalKirInventoryV1<'g, O>,
    CanonicalKirMemorySsaLimitsV1,
    &mut Budget<'w>,
) -> Result<
    (
        CanonicalKirMemorySsaV1<'i, 'g, O>,
        CanonicalKirMemorySsaStorageV1,
    ),
    CanonicalKirMemorySsaErrorV1,
>;

struct AnalysisFactoriesV1<O> {
    inventory: InventoryFactoryV1<O>,
    sparse: SparseFactoryV1<O>,
    memory: MemoryFactoryV1<O>,
}

impl<'inventory, 'graph, 'budget, 'work, O>
    CanonicalAnalysisScopeV1<'inventory, 'graph, 'budget, 'work, O>
{
    pub fn inventory(&self) -> &CanonicalKirInventoryV1<'graph, O> {
        self.inventory
    }

    /// Borrows this attempt's one-way cleanup notification. Nested concrete
    /// owners must still check their own slot, ledger and full retained floor;
    /// an undenied notification never authorizes a refund.
    pub fn cleanup_notification_v1(&self) -> &'budget CanonicalAnalysisCleanupV1<'budget> {
        self.cleanup
    }

    /// Lends the existing inventory and original budget without deriving a cache.
    /// Each request prepays the same six entry/postflight checks as cache queries,
    /// plus bounded disposal work and headers for a rejected callback value.
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
            &'borrow CanonicalKirInventoryV1<'graph, O>,
            &mut Budget<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        let headers = self.begin_request::<T, E>()?;
        let result = catch_unwind(AssertUnwindSafe(|| body(self.inventory, self.budget)));
        self.finish_request(result, headers)
    }

    /// Derives sparse facts once, on first demand, with the fixed default limits.
    /// Each request prepays six checks and bounded callback disposal, including
    /// the cache lookup and postflight. A first derivation pays two receipt-transfer
    /// checks. Consumers must preserve the ledger and all live analysis floors.
    pub fn with_sparse_v1<T, E>(
        &mut self,
        body: impl FnOnce(
            &CanonicalKirSparseV1<'inventory, 'graph, O>,
            &mut Budget<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        let headers = self.begin_request::<T, E>()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.ensure_sparse()?;
            body(
                self.sparse.as_ref().expect("installed sparse cache"),
                self.budget,
            )
        }));
        self.finish_request(result, headers)
    }

    /// Derives MemorySSA once, without requiring sparse facts. The same request,
    /// bounded-disposal and first-transfer checks apply as for sparse facts.
    /// This is an analysis of the exact inventory, not memory-safety authority.
    pub fn with_memory_ssa_v1<T, E>(
        &mut self,
        body: impl FnOnce(
            &CanonicalKirMemorySsaV1<'inventory, 'graph, O>,
            &mut Budget<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        let headers = self.begin_request::<T, E>()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.ensure_memory_ssa()?;
            body(
                self.memory_ssa.as_ref().expect("installed MemorySSA cache"),
                self.budget,
            )
        }));
        self.finish_request(result, headers)
    }

    /// Lends both existing caches for the same immutable inventory and ledger.
    /// Missing caches are derived once with the same factories and limits as
    /// the individual queries; this grants no source or memory-safety authority.
    ///
    /// ```no_run
    /// use fe2o3_pliron::{CanonicalAnalysisScopeV18, CanonicalAnalysisScopeErrorV1};
    /// fn inspect(scope: &mut CanonicalAnalysisScopeV18<'_, '_, '_, '_>)
    ///     -> Result<(), CanonicalAnalysisScopeErrorV1> {
    ///     scope.with_sparse_and_memory_ssa_v1(|sparse, memory, _| {
    ///         assert!(memory.belongs_to(sparse.inventory()));
    ///         Ok(())
    ///     })
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::{CanonicalAnalysisScopeV18, CanonicalAnalysisScopeErrorV1};
    /// fn escape(scope: &mut CanonicalAnalysisScopeV18<'_, '_, '_, '_>) {
    ///     let _ = scope.with_sparse_and_memory_ssa_v1(|_, memory, _|
    ///         Ok::<_, CanonicalAnalysisScopeErrorV1>(memory));
    /// }
    /// ```
    pub fn with_sparse_and_memory_ssa_v1<T, E>(
        &mut self,
        body: impl FnOnce(
            &CanonicalKirSparseV1<'inventory, 'graph, O>,
            &CanonicalKirMemorySsaV1<'inventory, 'graph, O>,
            &mut Budget<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        let headers = self.begin_request::<T, E>()?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.ensure_sparse()?;
            self.ensure_memory_ssa()?;
            body(
                self.sparse.as_ref().expect("installed sparse cache"),
                self.memory_ssa.as_ref().expect("installed MemorySSA cache"),
                self.budget,
            )
        }));
        self.finish_request(result, headers)
    }

    fn ensure_sparse(&mut self) -> Result<(), CanonicalAnalysisScopeErrorV1> {
        if self.sparse.is_none() {
            let (report, storage) = (self.factories.sparse)(
                self.inventory,
                CanonicalKirSparseLimitsV1::default(),
                self.budget,
            )
            .map_err(CanonicalAnalysisScopeErrorV1::Sparse)?;
            self.transfer_storage(storage.retained_storage())?;
            self.sparse = Some(report);
        }
        Ok(())
    }

    fn ensure_memory_ssa(&mut self) -> Result<(), CanonicalAnalysisScopeErrorV1> {
        if self.memory_ssa.is_none() {
            let (report, storage) = (self.factories.memory)(
                self.inventory,
                CanonicalKirMemorySsaLimitsV1::default(),
                self.budget,
            )
            .map_err(CanonicalAnalysisScopeErrorV1::MemorySsa)?;
            self.transfer_storage(storage.retained_storage())?;
            self.memory_ssa = Some(report);
        }
        Ok(())
    }

    fn same_ledger(&self) -> bool {
        self.slot == std::ptr::from_ref(&*self.budget) as usize
            && self.ledger == self.budget.work_ledger_identity_v1()
    }

    fn poison(&mut self) -> CanonicalAnalysisScopeErrorV1 {
        self.poisoned.set(true);
        self.cleanup.deny();
        // Drop cache payloads, but no containing scope may refund lost custody.
        self.sparse = None;
        self.memory_ssa = None;
        CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting)
    }

    fn begin_request<T, E>(&mut self) -> Result<usize, CanonicalAnalysisScopeErrorV1> {
        // Never debit a substituted Work meter, including on a poisoned retry.
        if !self.same_ledger()
            || self.poisoned.get()
            || self.cleanup.refund_denied()
            || self.budget.storage() < self.live_floor.get()
        {
            return Err(self.poison());
        }
        let headers = callback_disposal_headers_v1::<T, E>()
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        let floor = self.live_floor.get().checked_add(headers).ok_or(
            CanonicalAnalysisScopeErrorV1::Resource(Resource::Arithmetic),
        )?;
        // Six custody checks plus initial rejected-value drop and bounded retry.
        self.budget
            .charge_work(6 + callback_disposal_work_v1::<T>())
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        self.budget.reserve_storage(headers)
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        self.live_floor.set(floor);
        Ok(headers)
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

    fn finish_request<T, E>(
        &mut self,
        result: std::thread::Result<Result<T, E>>,
        headers: usize,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        let invalid = !self.same_ledger()
            || self.poisoned.get()
            || self.cleanup.refund_denied()
            || self.budget.storage() < self.live_floor.get();
        let cleanup_error = if invalid {
            Some(self.poison())
        } else {
            // Consumer scratch has dropped; never settle a denied inner floor.
            self.budget
                .release_storage(self.budget.storage() - self.live_floor.get())
                .and_then(|()| self.budget.release_storage(headers))
                .map(|()| self.live_floor.set(self.live_floor.get() - headers))
                .err()
                .map(|error| {
                    self.poison();
                    CanonicalAnalysisScopeErrorV1::Resource(error)
                })
        };
        match result {
            Ok(Err(error)) => Err(error),
            Ok(Ok(value)) => match cleanup_error {
                Some(error) => {
                    discard_rejected_callback_value_v1(value);
                    Err(error.into())
                }
                None => Ok(value),
            },
            Err(payload) => resume_unwind(payload),
        }
    }
}

/// Borrows one exact owner, builds its inventory, and lends a lazy analysis cache.
/// The caller reserves the owner's payload in the incoming storage floor. On
/// success, Result error, and unwind, caches and inventory are dropped before
/// that floor is restored; work, peak storage and failures are never rewound.
/// Selected callback errors and raw panics survive a later accounting loss.
/// A foreign Work ledger is neither charged nor cleaned up. Any detected loss
/// of the full live analysis floor forbids refunds, even above the entry floor.
/// An observed ledger/floor violation permanently poisons both lazy caches,
/// even if the caller catches the error and later restores the budget.
///
/// Consumer scratch must be dropped before its callback returns/unwinds; its
/// surplus reservation is released by the scope. Storage escaping in returned
/// values or captured state must be reserved before scope entry. Fixed checks
/// do not meter arbitrary consumer allocations. Transient violations restored
/// inside a callback are not observable. `inventory()` is a structural borrow,
/// not a guarded analysis query or compiler authority.
/// Rejected-result disposal headers and its bounded retry are prepaid. Other
/// stack-only scope framing and unrelated caller allocations are not metered.
/// A `T` with no drop glue needs no disposal credit; this does not imply that
/// callback execution cannot panic. Every raw callback panic still propagates.
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
    let cleanup = CanonicalAnalysisCleanupV1::new();
    with_canonical_analysis_scope_cleanup_v1(owner, budget, &cleanup, body)
}

/// The V12 entry with explicit monotone denial propagation to a containing owner.
/// The owner must inspect `cleanup.refund_denied()` after both returns and unwind
/// and suppress its own cleanup if true. The notification cannot authorize a
/// refund, replace an owner, or clear an earlier denial.
pub fn with_canonical_analysis_scope_cleanup_v1<'graph, 'work, T, E>(
    owner: &'graph VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'work>,
    cleanup: &CanonicalAnalysisCleanupV1<'_>,
    body: impl FnOnce(&mut CanonicalAnalysisScopeV1<'_, 'graph, '_, 'work>) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<CanonicalAnalysisScopeErrorV1>,
{
    with_analysis_scope(
        owner,
        budget,
        cleanup,
        AnalysisFactoriesV1 {
            inventory: |owner, budget| CanonicalKirInventoryV1::derive(owner, budget),
            sparse: |inventory, limits, budget| {
                CanonicalKirSparseV1::derive(inventory, limits, budget)
            },
            memory: |inventory, limits, budget| {
                CanonicalKirMemorySsaV1::derive(inventory, limits, budget)
            },
        },
        body,
    )
}

/// Runs the shared lexical cache over one actual V18 owner and its layout table.
/// The caller keeps the owner paid and propagates the monotone cleanup denial
/// through every containing scope, including raw unwinds. This is analysis only,
/// not source correspondence, initializedness, execution or rewrite authority.
///
/// ```no_run
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV18,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_pliron::{CanonicalAnalysisCleanupV1, CanonicalAnalysisScopeErrorV1,
///     with_canonical_analysis_scope_v18};
/// fn inspect(owner: &VerifiedCanonicalKernelIrModuleV18, budget: &mut Budget<'_>)
///     -> Result<(), CanonicalAnalysisScopeErrorV1> {
///     let cleanup = CanonicalAnalysisCleanupV1::new();
///     with_canonical_analysis_scope_v18(owner, budget, &cleanup, |scope|
///         scope.with_sparse_v1(|_, _| Ok(())))
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV18,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_pliron::{CanonicalAnalysisCleanupV1, CanonicalAnalysisScopeErrorV1,
///     with_canonical_analysis_scope_v18};
/// fn escape(owner: &VerifiedCanonicalKernelIrModuleV18, budget: &mut Budget<'_>) {
///     let cleanup = CanonicalAnalysisCleanupV1::new();
///     let _ = with_canonical_analysis_scope_v18(owner, budget, &cleanup, |scope|
///         scope.with_sparse_v1(|report, _| Ok::<_, CanonicalAnalysisScopeErrorV1>(report)));
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_pliron::{CanonicalAnalysisCleanupV1, CanonicalAnalysisScopeErrorV1,
///     with_canonical_analysis_scope_v18};
/// fn wrong_profile(owner: &VerifiedCanonicalKernelIrModuleV12, budget: &mut Budget<'_>) {
///     let cleanup = CanonicalAnalysisCleanupV1::new();
///     let _ = with_canonical_analysis_scope_v18(owner, budget, &cleanup,
///         |_| Ok::<_, CanonicalAnalysisScopeErrorV1>(()));
/// }
/// ```
pub fn with_canonical_analysis_scope_v18<'graph, 'work, T, E>(
    owner: &'graph VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'work>,
    cleanup: &CanonicalAnalysisCleanupV1<'_>,
    body: impl FnOnce(&mut CanonicalAnalysisScopeV18<'_, 'graph, '_, 'work>) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<CanonicalAnalysisScopeErrorV1>,
{
    with_analysis_scope(
        owner,
        budget,
        cleanup,
        AnalysisFactoriesV1 {
            inventory: |owner, budget| CanonicalKirInventoryV18::derive_v18(owner, budget),
            sparse: |inventory, limits, budget| {
                CanonicalKirSparseV18::derive_v18(inventory, limits, budget)
            },
            memory: |inventory, limits, budget| {
                CanonicalKirMemorySsaV18::derive_v18(inventory, limits, budget)
            },
        },
        body,
    )
}

fn with_analysis_scope<'graph, 'work, T, E, O>(
    owner: &'graph O,
    budget: &mut Budget<'work>,
    cleanup: &CanonicalAnalysisCleanupV1<'_>,
    factories: AnalysisFactoriesV1<O>,
    body: impl FnOnce(&mut CanonicalAnalysisScopeV1<'_, 'graph, '_, 'work, O>) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<CanonicalAnalysisScopeErrorV1>,
{
    if cleanup.refund_denied() {
        return Err(CanonicalAnalysisScopeErrorV1::Resource(Resource::Accounting).into());
    }
    let headers = callback_disposal_headers_v1::<T, E>()
        .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
    // Four custody checks plus initial rejected-value drop and bounded retry.
    budget
        .charge_work(4 + callback_disposal_work_v1::<T>())
        .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
    let floor = budget.storage();
    budget.reserve_storage(headers)
        .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let live_floor = Cell::new(budget.storage());
    let poisoned = Cell::new(false);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (inventory, storage) = (factories.inventory)(owner, budget)
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
            cleanup,
            factories: &factories,
        })
    }));
    let invalid = slot != std::ptr::from_ref(&*budget) as usize
        || ledger != budget.work_ledger_identity_v1()
        || poisoned.get()
        || cleanup.refund_denied()
        || budget.storage() < live_floor.get();
    let cleanup_error = if invalid {
        cleanup.deny();
        Some(CanonicalAnalysisScopeErrorV1::Resource(
            Resource::Accounting,
        ))
    } else {
        // The closure's caches and inventory dropped before this known credit.
        budget
            .release_storage(budget.storage() - floor)
            .err()
            .map(|error| {
                cleanup.deny();
                CanonicalAnalysisScopeErrorV1::Resource(error)
            })
    };
    match result {
        Ok(Err(error)) => Err(error),
        Ok(Ok(value)) => match cleanup_error {
            Some(error) => {
                discard_rejected_callback_value_v1(value);
                Err(error.into())
            }
            None => Ok(value),
        },
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "canonical_analysis_scope_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "canonical_analysis_scope_v18_tests.rs"]
mod v18_tests;
