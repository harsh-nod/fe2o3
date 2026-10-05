//! Explicit tile scheduling against the actual source-bound optimized owner.

use super::*;
use fe2o3_kernel_ir::{
    ExecutionOperationV15, ExecutionTileLayoutV1, ExecutionTileScheduleV1,
    VerifiedCanonicalKernelIrModuleV18,
};

#[path = "production_optimized_source_tile_scalar_v156.rs"]
mod scalar;
pub use scalar::{ProductionTileScalarFunctionV156, ProductionTileScalarLoadV156};

/// A borrowed schedule for one retained tile-load effect site. The original
/// source, checked transition and output owner remain live throughout its use.
/// This does not discharge lifecycle, target, uniformity, memory or refinement
/// obligations. In particular, constructing it does not activate native lowering.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOptimizedTileLoadScheduleV155;
/// fn forge() -> ProductionOptimizedTileLoadScheduleV155<'static, 'static> {
///     ProductionOptimizedTileLoadScheduleV155 { schedule: panic!() }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOptimizedSourceCorrespondenceV18,
///     ProductionOptimizedTileLoadScheduleV155};
/// use fe2o3_kernel_ir::{CanonicalKirOperationCoordinateV1, ExecutionTileLayoutV1,
///     CanonicalKernelIrVerificationResourceBudgetV1};
/// fn escape(view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
///     site: CanonicalKirOperationCoordinateV1,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>)
///     -> ProductionOptimizedTileLoadScheduleV155<'static, 'static> {
///     view.tile_load_schedule_v155(site, ExecutionTileLayoutV1::Blocked, budget).unwrap()
/// }
/// ```
pub struct ProductionOptimizedTileLoadScheduleV155<'view, 'source> {
    source: &'view ProductionOptimizedSourceCorrespondenceV18<'source>,
    original: OpCoordinate,
    output: OpCoordinate,
    workgroup: ValueId,
    input: ValueId,
    base: ValueId,
    schedule: ExecutionTileScheduleV1,
    floor: usize,
}

impl<'source> ProductionOptimizedSourceCorrespondenceV18<'source> {
    /// Selects an explicit layout for an original tile-load operation that the
    /// checked optimizer retained. No kernel name, fixture identity, implicit
    /// layout default, or replacement source graph participates in this query.
    ///
    /// As for `output_root_cfg_v18`, the caller reserves the returned fixed
    /// header before calling and retains it until the borrowed schedule is
    /// dropped. The query allocates nothing. Selection remains provisional until
    /// a target policy, launch geometry and scalar refinement are checked.
    pub fn tile_load_schedule_v155<'view>(
        &'view self,
        original: OpCoordinate,
        layout: ExecutionTileLayoutV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedTileLoadScheduleV155<'view, 'source>> {
        self.retain((|| {
            self.query(budget)?;
            let input = self.checked.input();
            let index = resources::operation_index(input, original, budget)?;
            budget.charge_work(1)?;
            let OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                lanes: original_lanes,
                elements: original_elements,
                ..
            }) = input.operations()[index].operation.kind
            else {
                return resources::binding("tile schedule original operation is not a tile load");
            };
            let output = match self.operation(original, budget)? {
                ProductionOptimizedSourceOperationV18::Retained { input, output }
                    if input == original =>
                {
                    output
                }
                _ => return resources::binding("tile schedule requires a retained load effect"),
            };
            let inventory = self.checked.output();
            let index = resources::operation_index(inventory, output, budget)?;
            budget.charge_work(1)?;
            let OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                workgroup,
                input,
                base,
                lanes,
                elements,
            }) = inventory.operations()[index].operation.kind
            else {
                return resources::binding("tile schedule output operation is not a tile load");
            };
            if (lanes, elements) != (original_lanes, original_elements) {
                return resources::binding("tile schedule changed source geometry");
            }
            let schedule = ExecutionTileScheduleV1::new(layout, lanes, elements).map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding("tile schedule invalid geometry")
            })?;
            self.check(budget)?;
            Ok(ProductionOptimizedTileLoadScheduleV155 {
                source: self,
                original,
                output,
                workgroup,
                input,
                base,
                schedule,
                floor: budget.storage(),
            })
        })())
    }
}

impl ProductionOptimizedTileLoadScheduleV155<'_, '_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.source.check(budget)?;
        if budget.storage() < self.floor {
            self.source.original.source.cleanup.deny_refund();
            return self
                .source
                .retain(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.source.query(budget)
    }

    /// Rejects substitution even by a separately admitted byte-identical owner.
    pub fn check_output_owner(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.source.retain((|| {
            budget.charge_work(1)?;
            if !std::ptr::eq(owner, self.source.checked.output().owner()) {
                return resources::binding("tile schedule substituted its checked output owner");
            }
            Ok(())
        })())
    }

    /// Original and actual output coordinates. Scalar reads must remain at this
    /// output effect site, including when no fragment component is later used.
    pub fn effect_sites(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(OpCoordinate, OpCoordinate)> {
        self.check(budget)?;
        Ok((self.original, self.output))
    }

    /// Actual output SSA operands: workgroup owner, slice, and base index.
    pub fn operands(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(ValueId, ValueId, ValueId)> {
        self.check(budget)?;
        Ok((self.workgroup, self.input, self.base))
    }

    /// Returns inert arithmetic semantics, not a transferable source proof.
    pub fn scalar_semantics(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ExecutionTileScheduleV1> {
        self.check(budget)?;
        Ok(self.schedule)
    }

    /// Checks the live source/owner/ledger before evaluating one component.
    /// Dynamic values are observations, not evidence of source uniformity.
    pub fn address(
        &self,
        lane: u16,
        element: u16,
        base: u64,
        length: u64,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<u64>> {
        self.check(budget)?;
        self.source.retain((|| {
            budget.charge_work(8)?;
            self.schedule
                .address(lane, element, base, length)
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding("tile schedule component coordinate")
                })
        })())
    }
}
