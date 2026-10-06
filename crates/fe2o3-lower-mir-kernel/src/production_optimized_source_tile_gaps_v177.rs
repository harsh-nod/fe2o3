//! Exact neutral-gap candidates through the one-to-many scalar tail.
use super::*;

/// A live source gap and its checked prefix disposition. Candidate points are
/// scalar-expansion boundaries only, not all integers in their bounding range.
/// Neither reachability nor a candidate coordinate proves state equivalence.
pub struct ProductionSourceTileGapV177<'owner, 'view, 'source> {
    tile: &'owner ProductionSourceTileExpansionV159<'view, 'source>,
    prefix: ProductionOptimizedSourceGapV18,
    required: usize,
}

impl<'view, 'source> ProductionSourceTileExpansionV159<'view, 'source> {
    /// Retains a checked original physical gap through the neutral prefix.
    /// The borrowed header is charged until the enclosing consumer releases
    /// its facts; querying after that credit is refunded refuses.
    pub fn original_gap_v177<'owner>(
        &'owner self,
        block: Block,
        operation: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceTileGapV177<'owner, 'view, 'source>> {
        self.check(budget)?;
        self.source.retain((|| {
            budget.reserve_storage(
                std::mem::size_of::<ProductionSourceTileGapV177<'_, '_, '_>>()
                    + 2 * std::mem::size_of::<
                        SourceOwnedResultV18<ProductionSourceTileGapV177<'_, '_, '_>>,
                    >(),
            )?;
            let prefix = self
                .source
                .physical_gap(block, operation as usize, budget)?;
            self.check(budget)?;
            Ok(ProductionSourceTileGapV177 {
                tile: self,
                prefix,
                required: budget.storage(),
            })
        })())
    }

    /// Original source block entry, without assuming that optimization keeps
    /// its original block or places the entry at operation zero in the output.
    pub fn source_block_entry_gap_v177<'owner>(
        &'owner self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceTileGapV177<'owner, 'view, 'source>>> {
        self.check(budget)?;
        self.source.retain((|| {
            let original = self
                .source
                .original
                .source_block_entry(root, instance, block, budget)?;
            original
                .map(|block| self.original_gap_v177(block, 0, budget))
                .transpose()
        })())
    }

    fn expanded_gap_point_v177(
        &self,
        block: Block,
        operation: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceGapIntervalV18> {
        self.check(budget)?;
        let neutral = self.source.output_inventory(budget)?;
        budget.charge_work((usize::BITS - neutral.blocks().len().leading_zeros()) as usize + 1)?;
        let at = neutral
            .blocks()
            .binary_search_by_key(&block, |row| row.coordinate)
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding("tile gap neutral block absent")
            })?;
        let count = neutral.blocks()[at].operations.len();
        budget.charge_work(4)?;
        if operation as usize > count {
            return resources::binding("tile gap lies beyond neutral block");
        }
        let point = if count == 0 {
            0
        } else {
            let end = operation as usize == count;
            let input = OpCoordinate {
                block,
                operation: if end { operation - 1 } else { operation },
            };
            let projections = self.tail.projections_v159();
            budget.charge_work((usize::BITS - projections.len().leading_zeros()) as usize + 1)?;
            let at = projections
                .binary_search_by_key(&input, |row| row.input)
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding("tile gap expansion absent")
                })?;
            if end {
                projections[at].end
            } else {
                projections[at].first
            }
        };
        let output = self
            .tail
            .output()
            .module()
            .functions
            .get(block.function.0 as usize)
            .and_then(|function| function.body.as_ref())
            .and_then(|body| body.blocks.get(block.block as usize))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "tile gap output block absent",
            ))?;
        if point as usize > output.operations.len() {
            return resources::binding("tile gap lies beyond expanded block");
        }
        self.check(budget)?;
        Ok(ProductionOptimizedSourceGapIntervalV18 {
            block,
            first: point,
            last: point,
        })
    }
}

impl ProductionSourceTileGapV177<'_, '_, '_> {
    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.tile.check(budget)?;
        if budget.storage() < self.required {
            self.tile.source.original.source.cleanup.deny_refund();
            return Err(self
                .tile
                .source
                .original
                .retain_query_resource_error_v18(ArgumentResourceV1::Accounting));
        }
        Ok(())
    }

    /// The original neutral-prefix reachability and candidate interval. An
    /// unreachable placement stays unreachable even when scalar output remains.
    pub fn prefix_disposition(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceGapV18> {
        self.check(budget)?;
        Ok(self.prefix)
    }

    fn interval(&self) -> Option<ProductionOptimizedSourceGapIntervalV18> {
        match self.prefix {
            ProductionOptimizedSourceGapV18::Reachable(interval) => Some(interval),
            ProductionOptimizedSourceGapV18::Unreachable { placement } => placement,
        }
    }

    /// Number of original prefix-gap candidates, including coincident points
    /// when an intervening original role operation has an empty expansion.
    pub fn candidate_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        self.tile.source.retain((|| {
            budget.charge_work(1)?;
            match self.interval() {
                None => Ok(0),
                Some(interval) => (interval.last as usize)
                    .checked_sub(interval.first as usize)
                    .and_then(|count| count.checked_add(1))
                    .ok_or(ArgumentResourceV1::Arithmetic.into()),
            }
        })())
    }

    /// Resolves one exact candidate to a singleton expanded gap. This never
    /// admits a cursor inside the expansion of one neutral operation. Selecting
    /// a candidate still requires independent value/effect/control obligations.
    pub fn candidate(
        &self,
        index: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceGapIntervalV18> {
        self.check(budget)?;
        self.tile.source.retain((|| {
            let count = self.candidate_count(budget)?;
            if index >= count {
                return resources::binding("tile gap candidate index outside prefix interval");
            }
            let interval = self
                .interval()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "tile gap placement absent",
                ))?;
            let operation = interval
                .first
                .checked_add(u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            self.tile
                .expanded_gap_point_v177(interval.block, operation, budget)
        })())
    }
}
