//! Scoped source transport through the actual independently checked successor.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18 as Inventory, CanonicalKirTransitionErrorV1 as TransitionError,
    CheckedCanonicalKirControlIndexV18 as Control, CheckedCanonicalKirTransitionV18 as Transition,
};
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as OpCoordinate,
    CanonicalKirUseCoordinateV1 as UseCoordinate,
};

#[path = "production_optimized_source_attachments_v18.rs"]
mod attachments;
#[path = "production_optimized_source_control_v18.rs"]
mod control;
#[path = "production_optimized_source_cfg_v18.rs"]
mod cfg;
#[path = "production_optimized_source_execution_v18.rs"]
mod execution;
#[path = "production_optimized_source_index_v18.rs"]
mod index;
#[path = "production_optimized_source_memory_v18.rs"]
mod memory;
#[path = "production_optimized_source_resources_v18.rs"]
mod resources;
pub use attachments::{
    ProductionOptimizedSourceGapV18, ProductionOptimizedSourceOperationV18,
    ProductionOptimizedSourceSpanV18, ProductionOptimizedSourceTerminatorV18,
};
pub use control::ProductionOptimizedSourceEffectsV18;
pub use cfg::{ProductionOptimizedSourceCfgEventV18, ProductionOptimizedSourceCfgRootV18};
pub use execution::{
    ProductionLifecycleCheckedNativePoliciesV18, ProductionPrivateMemoryCheckedNativePoliciesV18,
    ProductionSourcePrivateMemoryRootRequestV18, ProductionOptimizedExecutionKindV18,
    ProductionOptimizedExecutionRecipesV18, ProductionSourceNativeLifecycleErrorV18,
    ProductionSourceNativeLifecycleDiagnosticV18,
};
pub use index::ProductionOptimizedSourceGapIntervalV18;
use index::SourceIndex;
pub use memory::{
    ProductionOptimizedSourceAllocationV18, ProductionOptimizedSourceMemoryAccessV18,
    ProductionOptimizedSourcePayloadV18,
};

fn transition_error(error: TransitionError) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        TransitionError::Resource(error) => error.into(),
        _ => {
            ProductionSourceOwnedViewErrorV18::Binding("checked optimized source relation differs")
        }
    }
}

/// An exact original-source relation composed with one checked optimized graph.
/// Original attachments remain historical evidence. Only guarded output queries
/// refer to the successor; neither graph is copied or given legacy authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18;
/// fn forge() -> ProductionOptimizedSourceCorrespondenceV18<'static> {
///     ProductionOptimizedSourceCorrespondenceV18 { original: panic!() }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionSourceCorrespondenceV18,
///     ProductionSourceOwnedViewErrorV18};
/// use fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape(original: &ProductionSourceCorrespondenceV18<'_>,
///     checked: &CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _escaped = original.with_optimized_correspondence_v18(checked, budget,
///         |view, _| Ok::<_, ProductionSourceOwnedViewErrorV18>(view));
/// }
/// ```
pub struct ProductionOptimizedSourceCorrespondenceV18<'scope> {
    original: &'scope ProductionSourceCorrespondenceV18<'scope>,
    checked: &'scope Transition<'scope, 'scope, 'scope, 'scope>,
    control: &'scope Control<'scope, 'scope, 'scope>,
    index: &'scope SourceIndex,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Retains the original owner checks and composes the complete attachment
    /// census through this actual checked transition. Every borrowed owner and
    /// inventory must stay reserved. This scoped view cannot authorize final
    /// safety, source currentness, ranked/formal, target or native conclusions.
    pub fn with_optimized_correspondence_v18<'work, T, E>(
        &self,
        checked: &Transition<'_, '_, '_, '_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.query(budget)?;
        self.retain_query((|| {
            budget.charge_work(1)?;
            if !std::ptr::eq(self.inventory, checked.input()) {
                return self
                    .source
                    .missing("optimized source substituted its original inventory");
            }
            Ok(())
        })())?;
        let floor = budget.storage();
        let (control, index, storage) = scoped_source_attempt_v29(self.source.cleanup, budget, floor, |budget| {
            self.source.retain_construction(|| {
                let headers = resources::headers::<T, E>()?;
                budget.reserve_storage(headers)?;
                let (control, receipt) =
                    Control::derive_v18(checked, budget).map_err(transition_error)?;
                budget.reserve_storage(receipt.retained_storage())?;
                let index = SourceIndex::build(self, checked, &control, budget)?;
                let storage = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                Ok((control, index, storage))
            })
        })?;
            let view = ProductionOptimizedSourceCorrespondenceV18 {
                original: self,
                checked,
                control: &control,
                index: &index,
                slot: std::ptr::from_ref(budget) as usize,
                ledger: budget.work_ledger_identity_v1(),
                floor: budget.storage(),
            };
            let caught = match view.check(budget) {
                Ok(()) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(&view, budget))),
                Err(error) => { drop(consume); Ok(Err(error.into())) }
            };
            let prior = self.source.guard.first.get();
            let postflight = if matches!(&caught, Ok(Ok(_))) {
                view.check(budget)
            } else {
                view.observe_custody(budget)
            };
            drop(view);
            drop(index);
            drop(control);
            source_owned_finish_callback_v18(caught, prior, postflight, self.source.cleanup, budget, storage)
    }
}

impl<'scope> ProductionOptimizedSourceCorrespondenceV18<'scope> {
    pub(super) fn check_exact_original_v18(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        original.check(budget)?;
        if !std::ptr::eq(self.original, original) {
            return original.source.missing("optimized source substituted its exact correspondence");
        }
        self.query(budget)
    }

    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            self.original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.original.observe_custody(budget)
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.observe_custody(budget).is_err() {
            return self
                .original
                .source
                .guard
                .reject(SourceOwnedQueryFailureV18::Resource(
                    ArgumentResourceV1::Accounting,
                ));
        }
        self.original.check(budget)
    }

    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.original.query(budget)
    }

    fn retain<T>(&self, result: SourceOwnedResultV18<T>) -> SourceOwnedResultV18<T> {
        self.original.retain_query(result)
    }

    /// The unchanged original source view, not a view rebound to output.
    pub fn original_source(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&ProductionSourceOwnedViewV18<'_>> {
        self.query(budget)?;
        self.original.source(budget)
    }

    /// Borrows the exact original checked inventory after source custody checks.
    pub fn input_inventory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&Inventory<'scope>> {
        self.query(budget)?;
        Ok(self.checked.input())
    }

    /// Borrows the distinct actual successor inventory, never a rebound original.
    pub fn output_inventory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&Inventory<'scope>> {
        self.query(budget)?;
        Ok(self.checked.output())
    }
}
