//! Complete original-root selection for one retained whole-module expansion.
use super::*;
use fe2o3_kernel_ir::{ExecutionOperationV15 as Execution, OperationKind};
use fe2o3_kernel_opt::TileScalarFunctionSelectionV18 as Selection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Policy {
    layout: ExecutionTileLayoutV1,
    lanes: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RootRow {
    function: CanonicalKirFunctionCoordinateV1,
    policy: Option<Policy>,
}

pub(super) enum RootPoliciesV260 {
    Single { root: usize, lanes: u16 },
    Complete(Vec<RootRow>),
}

impl RootPoliciesV260 {
    pub(super) fn policy(
        &self,
        root: usize,
        selections: &[Selection],
    ) -> SourceOwnedResultV18<Option<(CanonicalKirFunctionCoordinateV1, ExecutionTileLayoutV1, u16)>>
    {
        match self {
            Self::Single {
                root: selected,
                lanes,
            } => {
                if root != *selected {
                    return Ok(None);
                }
                let [selection] = selections else {
                    return resources::binding("source tile policy census differs");
                };
                Ok(Some((selection.function, selection.layout, *lanes)))
            }
            Self::Complete(rows) => {
                let row = rows
                    .get(root)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source tile root policy absent",
                    ))?;
                Ok(row
                    .policy
                    .map(|policy| (row.function, policy.layout, policy.lanes)))
            }
        }
    }
}

struct FunctionPlan {
    tile: bool,
    lanes: Option<u16>,
    invalid_geometry: bool,
    selected: Option<Policy>,
}

impl FunctionPlan {
    fn selected_policy(&self) -> SourceOwnedResultV18<Option<Policy>> {
        if self.tile && self.selected.is_none() {
            return resources::binding("source tile helper requires interprocedural expansion");
        }
        Ok(self.selected)
    }
}

fn function_plans(
    inventory: &Inventory<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<FunctionPlan>> {
    let mut plans = resources::vector::<FunctionPlan>(inventory.functions().len(), budget)?;
    for (index, function) in inventory.functions().iter().enumerate() {
        budget.charge_work(1)?;
        if function.coordinate.0 as usize != index {
            return resources::binding("source tile function inventory is not dense");
        }
        let mut tile = false;
        for row in &inventory.operations()[function.operations.clone()] {
            budget.charge_work(1)?;
            if row.coordinate.block.function != function.coordinate {
                return resources::binding("source tile operation function differs");
            }
            tile |= matches!(
                row.operation.kind,
                OperationKind::Execution(
                    Execution::MaskedTileLoadU32 { .. }
                        | Execution::TileIntoFragmentU32 { .. }
                        | Execution::FragmentIntoPartsU32 { .. }
                )
            );
        }
        plans.push(FunctionPlan {
            tile,
            lanes: None,
            invalid_geometry: false,
            selected: None,
        });
    }
    for entry in inventory.kernels() {
        budget.charge_work(1)?;
        let plan = plans.get_mut(entry.entry.0 as usize).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("source tile kernel function absent"),
        )?;
        let Some(size) = entry.kernel.workgroup_size else {
            plan.invalid_geometry = true;
            continue;
        };
        if size.x == 0
            || size.x > 256
            || size.y != 1
            || size.z != 1
            || plan.lanes.is_some_and(|old| old != size.x as u16)
        {
            plan.invalid_geometry = true;
        }
        plan.lanes = Some(size.x as u16);
    }
    Ok(plans)
}

fn select_root(
    function: CanonicalKirFunctionCoordinateV1,
    layout: Option<ExecutionTileLayoutV1>,
    plans: &mut [FunctionPlan],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<RootRow> {
    budget.charge_work(1)?;
    let plan =
        plans
            .get_mut(function.0 as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source tile root function absent",
            ))?;
    if plan.tile != layout.is_some() {
        return resources::binding("source tile root layout does not match actual operations");
    }
    let policy = if let Some(layout) = layout {
        if plan.invalid_geometry {
            return resources::binding("source tile policy launch differs");
        }
        let lanes = plan
            .lanes
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source tile policy kernel absent",
            ))?;
        let policy = Policy { layout, lanes };
        if plan.selected.is_some_and(|old| old != policy) {
            return resources::binding("source tile aliased root policies differ");
        }
        plan.selected = Some(policy);
        Some(policy)
    } else {
        None
    };
    Ok(RootRow { function, policy })
}

// The checked dense inventory supplies canonical order without sorting or an
// O(functions * roots) search; aliases share one convergence check and selection.
fn selections(
    inventory: &Inventory<'_>,
    plans: &[FunctionPlan],
    roots: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<Selection>> {
    let mut selections = resources::vector::<Selection>(roots, budget)?;
    if plans.len() != inventory.functions().len() {
        return resources::binding("source tile function policy census differs");
    }
    for (function, plan) in inventory.functions().iter().zip(plans) {
        budget.charge_work(1)?;
        if let Some(policy) = plan.selected_policy()? {
            if selections.len() == roots {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            fe2o3_kernel_analysis::check_canonical_tile_convergence_v160(
                inventory,
                function.coordinate,
                budget,
            )
            .map_err(|error| match error {
                fe2o3_kernel_analysis::CanonicalTileConvergenceErrorV160::Resource(error) => {
                    error.into()
                }
                _ => ProductionSourceOwnedViewErrorV18::Binding(
                    "source tile input uniformity or workgroup arrival refused",
                ),
            })?;
            selections.push(Selection {
                function: function.coordinate,
                layout: policy.layout,
            });
        }
    }
    Ok(selections)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile_plan() -> FunctionPlan {
        FunctionPlan {
            tile: true,
            lanes: Some(64),
            invalid_geometry: false,
            selected: None,
        }
    }

    #[test]
    fn complete_root_aliases_are_checked_without_duplicate_function_selections() {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let mut plans = [tile_plan()];
        let function = CanonicalKirFunctionCoordinateV1(0);
        let first = select_root(
            function,
            Some(ExecutionTileLayoutV1::Blocked),
            &mut plans,
            &mut budget,
        )
        .unwrap();
        let alias = select_root(
            function,
            Some(ExecutionTileLayoutV1::Blocked),
            &mut plans,
            &mut budget,
        )
        .unwrap();
        assert_eq!(first, alias);
        let prior = plans[0].selected;
        assert!(matches!(
            select_root(
                function,
                Some(ExecutionTileLayoutV1::Striped),
                &mut plans,
                &mut budget
            ),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source tile aliased root policies differ"
            ))
        ));
        assert_eq!(plans[0].selected, prior);
        assert_eq!(budget.work(), 3);
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn complete_root_plan_refuses_omitted_helper_phantom_and_conflicting_launch() {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let mut plans = [tile_plan()];
        assert!(matches!(
            plans[0].selected_policy(),
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source tile helper requires interprocedural expansion"
            ))
        ));
        assert!(
            select_root(
                CanonicalKirFunctionCoordinateV1(0),
                None,
                &mut plans,
                &mut budget
            )
            .is_err()
        );
        plans[0].tile = false;
        assert!(
            select_root(
                CanonicalKirFunctionCoordinateV1(0),
                Some(ExecutionTileLayoutV1::Blocked),
                &mut plans,
                &mut budget
            )
            .is_err()
        );
        plans[0] = tile_plan();
        plans[0].invalid_geometry = true;
        assert!(
            select_root(
                CanonicalKirFunctionCoordinateV1(0),
                Some(ExecutionTileLayoutV1::Blocked),
                &mut plans,
                &mut budget
            )
            .is_err()
        );
        assert!(
            select_root(
                CanonicalKirFunctionCoordinateV1(1),
                Some(ExecutionTileLayoutV1::Blocked),
                &mut plans,
                &mut budget
            )
            .is_err()
        );
        assert!(plans[0].selected.is_none());
    }

    #[test]
    fn complete_root_selection_debits_exact_work_before_mutating_policy() {
        for limit in [0, 1] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let mut plans = [tile_plan()];
            let result = select_root(
                CanonicalKirFunctionCoordinateV1(0),
                Some(ExecutionTileLayoutV1::Blocked),
                &mut plans,
                &mut budget,
            );
            assert_eq!(result.is_ok(), limit == 1);
            assert_eq!(plans[0].selected.is_some(), limit == 1);
            if limit == 0 {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Work(_)
                    ))
                ));
            }
        }
    }
}

impl<'source> ProductionOptimizedSourceCorrespondenceV18<'source> {
    /// Selects every original root exactly once for one complete module. `None`
    /// retains a scalar root without imposing tile launch geometry; `Some` is
    /// only a provisional layout and must match actual tile operations. Aliased
    /// entry functions must agree. Scalar helpers and all ABI/reference roots
    /// remain in the same graph; tile-bearing non-root helpers currently refuse.
    ///
    /// The returned original-source owner reserves its full capacity on this
    /// same budget until `discard`. This is neither a semantic proof nor a
    /// public import-profile choice, and grants no artifact or launch authority.
    pub fn prepare_tile_expansion_roots_v260<'view>(
        &'view self,
        root_layouts: &[Option<ExecutionTileLayoutV1>],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceTileExpansionV159<'view, 'source>> {
        self.query(budget)?;
        let floor = budget.storage();
        let result =
            scoped_source_attempt_v29(self.original.source.cleanup, budget, floor, |budget| {
                let entry = budget.storage();
                let header = size_of::<ProductionSourceTileExpansionV159<'_, '_>>()
                    .checked_sub(size_of::<Tail>())
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let scratch = argument_sum_v1(&[
                    size_of::<ProductionOptimizedSourceCfgRootV18<'_, '_>>(),
                    size_of::<Vec<Selection>>(),
                    size_of::<RootRow>(),
                    size_of::<Option<Policy>>(),
                    size_of::<Vec<FunctionPlan>>(),
                    size_of::<FunctionPlan>(),
                    size_of::<&[Option<ExecutionTileLayoutV1>]>(),
                    argument_product_v1(
                        root_layouts.len(),
                        size_of::<Option<ExecutionTileLayoutV1>>(),
                    )?,
                    size_of::<SourceOwnedResultV18<Tail>>(),
                ])?;
                budget.reserve_storage(argument_sum_v1(&[header, scratch])?)?;
                let count = self.original.source.root_count(budget)?;
                budget.charge_work(1)?;
                if root_layouts.len() != count {
                    return resources::binding("source tile complete root roster differs");
                }
                let mut rows = resources::vector::<RootRow>(count, budget)?;
                let inventory = self.output_inventory(budget)?;
                let mut plans = function_plans(inventory, budget)?;
                for (root, layout) in root_layouts.iter().enumerate() {
                    budget.charge_work(1)?;
                    let cfg = self.output_root_cfg_v18(root, budget)?;
                    let function = cfg.function().coordinate;
                    rows.push(select_root(function, *layout, &mut plans, budget)?);
                }
                let selections = selections(inventory, &plans, count, budget)?;
                let selection_bytes =
                    argument_product_v1(selections.capacity(), size_of::<Selection>())?;
                let plan_bytes = argument_product_v1(plans.capacity(), size_of::<FunctionPlan>())?;
                let root_bytes = argument_product_v1(rows.capacity(), size_of::<RootRow>())?;
                let layouts = self.original.source.limits(budget)?.storage_layout_limits();
                let tail = fe2o3_kernel_opt::prepare_owned_tile_scalar_v18(
                    self.checked.output().owner(),
                    &selections,
                    layouts,
                    budget,
                )
                .map_err(tile_error)?;
                budget.reserve_storage(tail.retained_storage())?;
                tail.replay_against(self.checked.output().owner(), budget)
                    .map_err(tile_error)?;
                budget.charge_work(selections.len())?;
                if tail.selections() != selections {
                    return resources::binding("source tile complete policy census differs");
                }
                #[cfg(test)]
                if PANIC_AFTER_TILE_REPLAY_V159.replace(false) {
                    panic!("test panic after retaining and replaying the complete tile owner");
                }
                self.check(budget)?;
                let retained = argument_sum_v1(&[header, root_bytes, tail.retained_storage()])?;
                let temporary = argument_sum_v1(&[scratch, selection_bytes, plan_bytes])?;
                if argument_sum_v1(&[entry, retained, temporary])? != budget.storage() {
                    self.original.source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                drop(selections);
                drop(plans);
                budget.release_storage(temporary)?;
                Ok((tail, RootPoliciesV260::Complete(rows), retained))
            });
        let (tail, roots, retained) = self.retain(result)?;
        Ok(ProductionSourceTileExpansionV159 {
            source: self,
            tail,
            roots,
            retained,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}
