//! Lexically scoped analyses of one immutable, connected canonical owner.
//! Mutation requires ending this scope and deriving fresh output analyses.

use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, CanonicalKirSparseErrorV1,
    CanonicalKirSparseLimitsV1, CanonicalKirSparseV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedCanonicalKernelIrModuleV12,
};
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum CanonicalAnalysisScopeErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    Sparse(CanonicalKirSparseErrorV1),
}

impl fmt::Display for CanonicalAnalysisScopeErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Sparse(error) => error.fmt(f),
        }
    }
}

impl Error for CanonicalAnalysisScopeErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Resource(error) => error,
            Self::Inventory(error) => error,
            Self::Sparse(error) => error,
        })
    }
}

/// An Inventory -> Sparse cache borrowing the actual immutable graph owner.
/// There is no public constructor, replacement-owner API, or hash-based reuse.
/// Algorithms and their payload receipts remain in `fe2o3-kernel-analysis`.
pub struct CanonicalAnalysisScopeV1<'inventory, 'graph, 'budget, 'work> {
    inventory: &'inventory CanonicalKirInventoryV1<'graph>,
    sparse: Option<CanonicalKirSparseV1<'inventory, 'graph>>,
    budget: &'budget mut Budget<'work>,
}

impl<'inventory, 'graph, 'work> CanonicalAnalysisScopeV1<'inventory, 'graph, '_, 'work> {
    pub fn inventory(&self) -> &CanonicalKirInventoryV1<'graph> {
        self.inventory
    }

    /// Derives sparse facts once, on first demand, with the fixed default limits.
    /// Each request charges one cache lookup before reading the cache. Consumers
    /// retain the shared ledger and must not release the live analysis floors.
    pub fn with_sparse_v1<T, E>(
        &mut self,
        body: impl FnOnce(&CanonicalKirSparseV1<'inventory, 'graph>, &mut Budget<'work>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<CanonicalAnalysisScopeErrorV1>,
    {
        self.budget
            .charge_work(1)
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        if self.sparse.is_none() {
            let (report, storage) = CanonicalKirSparseV1::derive(
                self.inventory,
                CanonicalKirSparseLimitsV1::default(),
                self.budget,
            )
            .map_err(CanonicalAnalysisScopeErrorV1::Sparse)?;
            self.budget
                .reserve_storage(storage.retained_storage())
                .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
            self.sparse = Some(report);
        }
        // Installation happens only after both derivation and payload transfer.
        body(
            self.sparse.as_ref().expect("installed sparse cache"),
            self.budget,
        )
    }
}

/// Borrows one exact owner, builds its inventory, and lends a lazy analysis cache.
/// The caller reserves the owner's payload in the incoming storage floor. On
/// success and every Result error, caches and inventory are dropped before that
/// floor is restored; work, peak storage and first failures are never rewound.
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
pub fn with_canonical_analysis_scope_v1<'graph, 'work, T, E>(
    owner: &'graph VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'work>,
    body: impl FnOnce(&mut CanonicalAnalysisScopeV1<'_, 'graph, '_, 'work>) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<CanonicalAnalysisScopeErrorV1>,
{
    let floor = budget.storage();
    let result = (|| {
        let (inventory, storage) = CanonicalKirInventoryV1::derive(owner, budget)
            .map_err(CanonicalAnalysisScopeErrorV1::Inventory)?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(CanonicalAnalysisScopeErrorV1::Resource)?;
        body(&mut CanonicalAnalysisScopeV1 {
            inventory: &inventory,
            sparse: None,
            budget,
        })
    })();
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
    result
}

#[cfg(test)]
#[path = "canonical_analysis_scope_v1_tests.rs"]
mod tests;
