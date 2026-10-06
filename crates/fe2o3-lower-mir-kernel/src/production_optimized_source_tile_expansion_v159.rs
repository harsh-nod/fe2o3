//! Live original-source transport through the checked one-to-many tile graph.
use super::*;
use fe2o3_kernel_ir::{CanonicalKirFunctionCoordinateV1, ExecutionRoleV15, ScalarType};
use fe2o3_kernel_opt::{OwnedTileScalarContinuationV18 as Tail, OwnedTileScalarErrorV18 as Error};

#[path = "production_optimized_source_tile_gaps_v177.rs"]
mod gaps;
pub use gaps::ProductionSourceTileGapV177;

#[path = "production_optimized_source_tile_roots_v260.rs"]
mod roots;
use roots::RootPoliciesV260;

#[cfg(test)]
thread_local! {
    static PANIC_AFTER_TILE_REPLAY_V159: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Original source operation, its actual checked-prefix operation, and complete
/// scalar output interval in that same block. The interval can be empty for a
/// consumed fragment transport. This record alone grants no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceTileOperationSpanV159 {
    /// Original operation coordinate before the checked neutral prefix.
    pub original: OpCoordinate,
    /// Complete one-to-many correspondence after the neutral prefix.
    pub expansion: fe2o3_kernel_opt::TileScalarOperationProjectionV159,
}

/// Original aggregate leaf resolved against the actual retained scalar graph.
/// A copied result is descriptive, not a source or native proof certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceTileLeafV162 {
    /// Scalar value in the authenticated expanded function.
    Scalar {
        /// Function coordinate in the expanded module.
        function: CanonicalKirFunctionCoordinateV1,
        /// Exact value produced by the checked scalar recipe.
        value: ValueId,
        /// Scalar type of the selected original aggregate component.
        scalar: ScalarType,
    },
    /// Erased zero-sized marker, with no physical scalar value.
    Unit,
}

/// Retains the live original-source relation and a complete checked successor,
/// including explicit launch geometry and layout selection. It is not a source
/// refinement proof, native completion, or permission to activate a target.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionSourceTileExpansionV159;
/// fn forge() -> ProductionSourceTileExpansionV159<'static, 'static> {
///     ProductionSourceTileExpansionV159 { source: panic!() }
/// }
/// ```
#[must_use = "discard the exact retained credit before releasing the source view"]
pub struct ProductionSourceTileExpansionV159<'view, 'source> {
    source: &'view ProductionOptimizedSourceCorrespondenceV18<'source>,
    tail: Tail,
    retained: usize,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    roots: RootPoliciesV260,
}

fn tile_error(error: Error) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        Error::Resource(error)
        | Error::Inventory(fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error)) => {
            error.into()
        }
        _ => ProductionSourceOwnedViewErrorV18::Binding("source tile expansion replay refused"),
    }
}

impl<'source> ProductionOptimizedSourceCorrespondenceV18<'source> {
    #[cfg(test)]
    pub(crate) fn test_tile_expansion_panic_after_replay_v159(&self) {
        PANIC_AFTER_TILE_REPLAY_V159.set(true);
    }
    /// Retains this actual source owner and its independently checked scalar
    /// successor. The caller selects a provisional layout; final production
    /// policy, source semantics and native admission remain mandatory gates.
    /// Success reserves the complete returned owner's storage until `discard`.
    pub fn prepare_tile_expansion_v159<'view>(
        &'view self,
        root: usize,
        layout: ExecutionTileLayoutV1,
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
                budget.reserve_storage(header)?;
                let layouts = self.original.source.limits(budget)?.storage_layout_limits();
                let tail =
                    self.prepare_tile_scalar_candidate_v157(root, layout, layouts, budget)?;
                budget.reserve_storage(tail.retained_storage())?;
                tail.replay_against(self.checked.output().owner(), budget)
                    .map_err(tile_error)?;
                budget.charge_work(3)?;
                let [selection] = tail.selections() else {
                    return resources::binding("source tile policy census differs");
                };
                if selection.layout != layout {
                    return resources::binding("source tile policy layout differs");
                }
                let module = tail.output().module();
                let function = module.functions.get(selection.function.0 as usize).ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source tile policy function absent",
                    ),
                )?;
                let mut lanes = None;
                for kernel in &module.kernels {
                    budget.charge_work(1)?;
                    if kernel.entry != function.id {
                        continue;
                    }
                    let Some(size) = kernel.workgroup_size else {
                        return resources::binding("source tile policy launch absent");
                    };
                    if size.x == 0
                        || size.x > 256
                        || size.y != 1
                        || size.z != 1
                        || lanes.is_some_and(|old| old != size.x as u16)
                    {
                        return resources::binding("source tile policy launch differs");
                    }
                    lanes = Some(size.x as u16);
                }
                let lanes = lanes.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source tile policy kernel absent",
                ))?;
                #[cfg(test)]
                if PANIC_AFTER_TILE_REPLAY_V159.replace(false) {
                    panic!("test panic after retaining and replaying the complete tile owner");
                }
                self.check(budget)?;
                let retained = header
                    .checked_add(tail.retained_storage())
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                if entry.checked_add(retained) != Some(budget.storage()) {
                    self.original.source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((tail, retained, lanes))
            });
        let (tail, retained, lanes) = self.retain(result)?;
        Ok(ProductionSourceTileExpansionV159 {
            source: self,
            tail,
            retained,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            roots: RootPoliciesV260::Single { root, lanes },
        })
    }
}

impl<'view, 'source> ProductionSourceTileExpansionV159<'view, 'source> {
    fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            self.source.original.source.cleanup.deny_refund();
            return self
                .source
                .retain(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.source.observe_custody(budget)
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let custody = self.custody(budget);
        self.source.check(budget).and(custody)
    }

    /// Actual freshly admitted scalar graph, borrowing this retained relation.
    pub fn output(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&VerifiedCanonicalKernelIrModuleV18> {
        self.check(budget)?;
        Ok(self.tail.output())
    }

    /// Exact provisional policy roster; canonical input/output owners also bind
    /// all actual launch metadata. This is never an implicit target policy.
    pub fn selections(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[fe2o3_kernel_opt::TileScalarFunctionSelectionV18]> {
        self.check(budget)?;
        Ok(self.tail.selections())
    }

    /// The live neutral-prefix relation, not rebound to the expanded output.
    pub fn neutral_source_v162(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&'view ProductionOptimizedSourceCorrespondenceV18<'source>> {
        self.check(budget)?;
        Ok(self.source)
    }

    /// The genuine original correspondence needed for original MIR interpretation.
    pub fn original_source_v162(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&'source ProductionSourceCorrespondenceV18<'source>> {
        self.check(budget)?;
        Ok(self.source.original)
    }

    /// Explicit selected output function, layout and one-dimensional lane count
    /// for an actual original root. Unselected roots have no tile policy.
    pub fn root_policy_v162(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<(CanonicalKirFunctionCoordinateV1, ExecutionTileLayoutV1, u16)>>
    {
        self.check(budget)?;
        self.source.original.source.root(root, budget)?;
        self.source.retain((|| {
            budget.charge_work(2)?;
            self.roots.policy(root, self.tail.selections())
        })())
    }

    /// Resolves original `[values, element]` / `[masks, element]` leaves through
    /// the checked prefix and exact scalar recipe. Marker fields 2 and 3 are
    /// unit leaves. Rewritten, ambiguous and non-tile definitions refuse.
    pub fn aggregate_leaf_v162(
        &self,
        original: Definition,
        path: &[u32],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceTileLeafV162> {
        self.check(budget)?;
        self.source.retain((|| {
            let input = self.source.checked.input();
            let index = resources::definition_index(input, original, budget)?;
            let definition = &input.definitions()[index];
            let elements = match definition.ty {
                Type::Execution(
                    ExecutionRoleV15::MaskedTileU32 { elements, .. }
                    | ExecutionRoleV15::LaneFragmentU32 { elements, .. },
                ) => *elements,
                _ => return resources::binding("source tile leaf is not a tile or fragment"),
            };
            let Definition::Result { operation, result } = original else {
                return resources::binding("source tile leaf has no operation definition");
            };
            let span = self.operation_span(operation, budget)?.ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source tile leaf is unreachable"),
            )?;
            let descendants = self.source.definition_descendants(original, budget)?;
            budget.charge_work(3)?;
            let [descendant] = descendants else {
                return resources::binding("source tile leaf has ambiguous prefix descendants");
            };
            let expected = Definition::Result {
                operation: span.expansion.input,
                result,
            };
            if descendant.kind != fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Retained
                || descendant.output != expected
            {
                return resources::binding("source tile leaf prefix definition differs");
            }
            let rows = self.tail.role_projections_v162();
            budget.charge_work((usize::BITS - rows.len().leading_zeros()) as usize + 1)?;
            let index = rows
                .binary_search_by_key(&expected, |row| row.input)
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source tile leaf role binding absent",
                    )
                })?;
            let (field, element) = match path {
                [2] | [3] => return Ok(ProductionSourceTileLeafV162::Unit),
                [field @ (0 | 1), element] if *element < u32::from(elements) => (*field, *element),
                _ => return resources::binding("source tile leaf field path differs"),
            };
            let (value, mask) = rows[index].component(element as u16).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source tile leaf geometry differs"),
            )?;
            Ok(ProductionSourceTileLeafV162::Scalar {
                function: span.expansion.input.block.function,
                value: if field == 0 { value } else { mask },
                scalar: if field == 0 {
                    ScalarType::U32
                } else {
                    ScalarType::Bool
                },
            })
        })())
    }

    /// Composes an actual original operation through the neutral prefix and the
    /// scalar expansion in logarithmic time. Unreachable source operations have
    /// no output span. Other prefix rewrites refuse instead of inventing a site.
    pub fn operation_span(
        &self,
        original: OpCoordinate,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceTileOperationSpanV159>> {
        self.check(budget)?;
        self.source.retain((|| {
            let output = match self.source.operation(original, budget)? {
                ProductionOptimizedSourceOperationV18::Retained { output, .. } => output,
                ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                    return Ok(None);
                }
                ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                    return resources::binding("source tile span prefix operation was rewritten");
                }
            };
            let rows = self.tail.projections_v159();
            budget.charge_work((usize::BITS - rows.len().leading_zeros()) as usize + 1)?;
            let index = rows
                .binary_search_by_key(&output, |row| row.input)
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "source tile span has no actual operation",
                    )
                })?;
            Ok(Some(ProductionSourceTileOperationSpanV159 {
                original,
                expansion: rows[index],
            }))
        })())
    }

    /// Independently re-derives the original role census and replays the entire
    /// output graph, spans, CFG edges and launch metadata against this live source.
    pub fn replay(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.source.retain(
            self.tail
                .replay_against(self.source.checked.output().owner(), budget)
                .map_err(tile_error),
        )?;
        self.check(budget)
    }

    /// Complete owned credit, excluding the borrowed original and neutral owners.
    pub fn retained_storage(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.retained)
    }

    /// Drops graph and correspondence storage before refund, including after a
    /// selected refusal when the source and continuing ledger remain intact.
    pub fn discard(self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let selected = self.check(budget);
        let custody = self.custody(budget);
        let Self {
            source,
            tail,
            roots,
            retained,
            ..
        } = self;
        drop(tail);
        drop(roots);
        let settled = custody
            .and_then(|()| source.retain(budget.release_storage(retained).map_err(Into::into)));
        selected?;
        settled
    }

    /// Structural replay is not final source, native, artifact, or launch proof.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
