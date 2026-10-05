//! Source-bound scalar tile candidates, with explicit launch and SSA bounds.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirFunctionCoordinateV1, ExecutionTileScalarLoweringV1, Operation, Type, ValueDef,
    WorkgroupSize,
};

/// One metered inventory scan per containing root, reusable by every tile load
/// in that function. The live source binds launch geometry and the fresh SSA
/// floor. This is a candidate-building scope, not a native policy or a proof.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionTileScalarFunctionV156;
/// fn forge() -> ProductionTileScalarFunctionV156<'static, 'static> {
///     ProductionTileScalarFunctionV156 { first_unused: panic!() }
/// }
/// ```
pub struct ProductionTileScalarFunctionV156<'view, 'source> {
    source: &'view ProductionOptimizedSourceCorrespondenceV18<'source>,
    function: CanonicalKirFunctionCoordinateV1,
    lanes: u16,
    first_unused: ValueId,
    floor: usize,
}

/// An actual scalar replacement recipe tied to one retained source effect site.
/// Candidate operations do not discharge lifecycle, source refinement, memory,
/// or target obligations. Whole-graph integration must preserve this effect
/// position, transport tile/fragment/parts uses, and independently check the
/// complete resulting graph before activating native lowering.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionTileScalarLoadV156;
/// fn forge() -> ProductionTileScalarLoadV156<'static, 'static, 'static> {
///     ProductionTileScalarLoadV156 { recipe: panic!() }
/// }
/// ```
pub struct ProductionTileScalarLoadV156<'function, 'view, 'source> {
    function: &'function ProductionTileScalarFunctionV156<'view, 'source>,
    original: OpCoordinate,
    output: OpCoordinate,
    recipe: ExecutionTileScalarLoweringV1,
    floor: usize,
}

impl<'source> ProductionOptimizedSourceCorrespondenceV18<'source> {
    /// The caller prepays the returned fixed header, as for the borrowed CFG
    /// root API. No graph copy, kernel-name selection or implicit layout occurs.
    pub fn tile_scalar_function_v156<'view>(
        &'view self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionTileScalarFunctionV156<'view, 'source>> {
        self.retain((|| {
            let cfg = self.output_root_cfg_v18(root, budget)?;
            let function = cfg.function();
            let inventory = cfg.inventory();
            let mut geometry = None;
            for entry in inventory.kernels() {
                budget.charge_work(1)?;
                if entry.entry != function.coordinate {
                    continue;
                }
                let Some(size) = entry.kernel.workgroup_size else {
                    return resources::binding(
                        "tile scalar root has no explicit workgroup geometry",
                    );
                };
                if size.y != 1 || size.z != 1 || size.x == 0 || size.x > 256 {
                    return resources::binding(
                        "tile scalar root requires bounded one-dimensional geometry",
                    );
                }
                if geometry.is_some_and(|previous| previous != size) {
                    return resources::binding(
                        "tile scalar root has conflicting launch geometries",
                    );
                }
                geometry = Some(size);
            }
            let Some(WorkgroupSize { x, .. }) = geometry else {
                return resources::binding("tile scalar function is not an actual kernel root");
            };
            let mut first_unused = 0_u32;
            for definition in &inventory.definitions()[function.definitions.clone()] {
                budget.charge_work(1)?;
                if let Some(value) = definition.value {
                    first_unused = first_unused.max(value.0.checked_add(1).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "tile scalar SSA range exhausted",
                        ),
                    )?);
                }
            }
            self.check(budget)?;
            Ok(ProductionTileScalarFunctionV156 {
                source: self,
                function: function.coordinate,
                lanes: x as u16,
                first_unused: ValueId(first_unused),
                floor: budget.storage(),
            })
        })())
    }
}

impl<'view, 'source> ProductionTileScalarFunctionV156<'view, 'source> {
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

    /// First value ID strictly above every actual definition in this root.
    pub fn first_unused(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<ValueId> {
        self.check(budget)?;
        Ok(self.first_unused)
    }

    /// `first` comes from the caller's monotonic candidate SSA allocator. It
    /// cannot overlap the source graph; the caller advances it to `next_value`
    /// between replacements so different candidate expansions do not overlap.
    /// The returned borrowed fixed header must be prepaid and retained.
    pub fn load<'function>(
        &'function self,
        schedule: &ProductionOptimizedTileLoadScheduleV155<'_, '_>,
        first: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionTileScalarLoadV156<'function, 'view, 'source>> {
        self.check(budget)?;
        self.source.retain((|| {
            schedule.check(budget)?;
            budget.charge_work(8)?;
            if !std::ptr::eq(self.source, schedule.source)
                || schedule.output.block.function != self.function
                || schedule.schedule.lanes() != self.lanes
                || first.0 < self.first_unused.0
            {
                return resources::binding("tile scalar load source, launch or SSA range differs");
            }
            let recipe = ExecutionTileScalarLoweringV1::new(
                schedule.schedule,
                schedule.input,
                schedule.base,
                first,
            )
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding("tile scalar candidate SSA range")
            })?;
            Ok(ProductionTileScalarLoadV156 {
                function: self,
                original: schedule.original,
                output: schedule.output,
                recipe,
                floor: budget.storage(),
            })
        })())
    }
}

impl ProductionTileScalarLoadV156<'_, '_, '_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.function.check(budget)?;
        if budget.storage() < self.floor {
            self.function.source.original.source.cleanup.deny_refund();
            return self
                .function
                .source
                .retain(Err(ArgumentResourceV1::Accounting.into()));
        }
        Ok(())
    }

    /// Original and retained output coordinates where replacement reads belong.
    pub fn effect_sites(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(OpCoordinate, OpCoordinate)> {
        self.check(budget)?;
        Ok((self.original, self.output))
    }

    /// Exclusive end of this replacement's contiguous fresh SSA range.
    pub fn next_value(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<ValueId> {
        self.check(budget)?;
        Ok(self.recipe.next_value())
    }

    /// Exact bounded number of ordinary operations emitted by this replacement.
    pub fn operation_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.recipe.operation_count())
    }

    /// Scalar value and active-mask SSA definitions for one component.
    pub fn component(
        &self,
        element: u16,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(ValueId, ValueId)> {
        self.check(budget)?;
        self.function
            .source
            .retain(self.recipe.component(element).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("tile scalar component coordinate"),
            ))
    }

    /// Streams one owned operation through a borrowed callback. Scratch is
    /// charged before emission and released only after the operation is dropped.
    /// The consumer separately prepays any output it copies into its candidate.
    pub fn with_operation<T>(
        &self,
        position: usize,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl FnOnce(&Operation, &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<T>,
    ) -> SourceOwnedResultV18<T> {
        self.check(budget)?;
        self.function.source.retain((|| {
            budget.charge_work(32)?;
            if position >= self.recipe.operation_count() {
                return resources::binding("tile scalar operation coordinate");
            }
            // At most two inline definitions and one pointer pointee allocation.
            let bytes = std::mem::size_of::<Operation>()
                + 2 * std::mem::size_of::<ValueDef>()
                + std::mem::size_of::<Type>()
                + std::mem::size_of_val(&consume);
            budget.reserve_storage(bytes)?;
            let retained = budget.storage();
            let operation = self.recipe.emit_operation(position).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("tile scalar emitter position"),
            )?;
            let result = consume(&operation, budget);
            drop(operation);
            if budget.storage() < retained {
                self.function.source.original.source.cleanup.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.check(budget)?;
            budget.release_storage(bytes)?;
            result
        })())
    }

    /// Replays the complete emitted scalar sequence independently. This checks
    /// candidate contents, not a caller-claimed position in a rewritten module.
    /// Whole-graph effect/SSA/fragment transport remains an explicit obligation.
    pub fn check_replacement(
        &self,
        operations: &[Operation],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.function.source.retain((|| {
            budget.charge_work(1)?;
            if operations.len() != self.recipe.operation_count() {
                return resources::binding("tile scalar replacement operation count");
            }
            for (position, operation) in operations.iter().enumerate() {
                budget.charge_work(32)?;
                self.recipe
                    .check_operation(position, operation)
                    .map_err(|_| {
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "tile scalar independent replacement replay",
                        )
                    })?;
            }
            self.check(budget)
        })())
    }
}
