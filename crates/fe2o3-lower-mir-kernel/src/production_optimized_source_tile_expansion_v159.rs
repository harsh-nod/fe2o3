//! Live original-source transport through the checked one-to-many tile graph.
use super::*;
use fe2o3_kernel_opt::{OwnedTileScalarContinuationV18 as Tail, OwnedTileScalarErrorV18 as Error};

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
                Ok((tail, retained))
            });
        let (tail, retained) = self.retain(result)?;
        Ok(ProductionSourceTileExpansionV159 {
            source: self,
            tail,
            retained,
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}

impl ProductionSourceTileExpansionV159<'_, '_> {
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
            retained,
            ..
        } = self;
        drop(tail);
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
