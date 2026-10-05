//! Original allocation identities through the retained one-to-many tile graph.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
use fe2o3_kernel_ir::{OperationKind, VerifiedCanonicalKernelIrModuleV18 as Owner};

pub(in super::super) struct TileAllocationSlotsV164<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    rows: Vec<(SourceKey, usize)>,
    required: usize,
}

fn key(operation: Operation) -> SourceKey {
    [
        operation.block.function.0 as usize,
        operation.block.block as usize,
        operation.operation as usize,
        0,
        0,
    ]
}

impl<'slots, 'view, 'source> TileAllocationSlotsV164<'slots, 'view, 'source> {
    pub(in super::super) fn derive(
        slots: &'slots SourceSlots<'view, 'source>,
        output: &Inventory<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.with_source_query_v42(out, |out| {
            out.budget.reserve_storage(
                size_of::<Self>() + 2 * size_of::<Result<Self>>() + 32 * size_of::<usize>(),
            )?;
            let tile = slots.tile.ok_or_else(mismatch)?;
            if !std::ptr::eq(tile.output(out.budget)?, output.owner()) {
                return Err(mismatch());
            }
            let original = slots.correspondence(out)?.inventory(out.budget)?;
            let mut count = 0usize;
            for row in original.operations() {
                out.budget.charge_work(1)?;
                if matches!(row.operation.kind, OperationKind::Alloca { .. }) {
                    count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
            }
            let mut rows = vector(count, out)?;
            for (index, row) in original.operations().iter().enumerate() {
                out.budget.charge_work(1)?;
                if !matches!(row.operation.kind, OperationKind::Alloca { .. }) {
                    continue;
                }
                // Query the original index too: an unowned allocation must not
                // become acceptable merely because the scalar graph retains it.
                slots.site(row.coordinate, out)?;
                let Some(span) = tile.operation_span(row.coordinate, out.budget)? else {
                    continue;
                };
                let span = span.expansion;
                out.budget.charge_work(3)?;
                if span.end.checked_sub(span.first) != Some(1) {
                    return Err(mismatch());
                }
                let operation = Operation {
                    block: span.input.block,
                    operation: span.first,
                };
                out.budget.charge_work(
                    (usize::BITS - output.operations().len().leading_zeros()) as usize + 1,
                )?;
                let at = output
                    .operations()
                    .binary_search_by_key(&operation, |row| row.coordinate)
                    .map_err(|_| mismatch())?;
                let actual = &output.operations()[at];
                out.budget.charge_work(3)?;
                if actual.operation.kind != row.operation.kind
                    || actual.results.len() != 1
                    || row.results.len() != 1
                {
                    return Err(mismatch());
                }
                rows.push((key(operation), index));
            }
            sort_source(&mut rows, out)?;
            let mut actual = 0usize;
            for row in output.operations() {
                out.budget.charge_work(1)?;
                if matches!(row.operation.kind, OperationKind::Alloca { .. }) {
                    actual = actual.checked_add(1).ok_or(Resource::Arithmetic)?;
                }
            }
            if actual != rows.len() {
                return Err(mismatch());
            }
            Ok(Self {
                slots,
                rows,
                required: out.budget.storage(),
            })
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        self.slots.correspondence(out)?;
        Ok(())
    }
}

impl ByteAllocationResolverV30 for TileAllocationSlotsV164<'_, '_, '_> {
    fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            out.budget.charge_work(1)?;
            let tile = self.slots.tile.ok_or_else(mismatch)?;
            if !std::ptr::eq(tile.output(out.budget)?, owner) {
                return Err(mismatch());
            }
            Ok(())
        })
    }

    fn site(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<ByteAllocationSiteV30> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            out.budget
                .charge_work((usize::BITS - self.rows.len().leading_zeros()) as usize + 1)?;
            let at = self
                .rows
                .binary_search_by_key(&key(operation), |row| row.0)
                .map_err(|_| mismatch())?;
            let original = self.slots.correspondence(out)?.inventory(out.budget)?;
            let row = original
                .operations()
                .get(self.rows[at].1)
                .ok_or_else(mismatch)?;
            self.slots.site(row.coordinate, out)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, ExecutionTileLayoutV1 as Layout};

    #[test]
    fn original_mir_tile_allocations_join_the_complete_original_and_expanded_census() {
        for layout in [Layout::Blocked, Layout::Striped] {
            super::super::tests::run_tile_slots(layout, |slots, out| {
                let tile = slots.tile.unwrap();
                let (inventory, receipt) =
                    Inventory::derive_v18(tile.output(out.budget)?, out.budget)?;
                out.budget.reserve_storage(receipt.retained_storage())?;
                let allocations = TileAllocationSlotsV164::derive(slots, &inventory, out)?;
                allocations.check_owner(inventory.owner(), out)?;
                assert_eq!(allocations.rows.len(), 2);
                for frame in slots.frames.iter().flatten() {
                    let span = tile
                        .operation_span(frame.allocation(), out.budget)?
                        .unwrap();
                    let expanded = Operation {
                        block: span.expansion.input.block,
                        operation: span.expansion.first,
                    };
                    assert_eq!(
                        allocations.site(expanded, out)?,
                        slots.site(frame.allocation(), out)?
                    );
                }
                let original = slots.correspondence(out)?.inventory(out.budget)?;
                assert!(allocations.check_owner(original.owner(), out).is_err());
                assert!(TileAllocationSlotsV164::derive(slots, original, out).is_err());
                let nonallocation = inventory
                    .operations()
                    .iter()
                    .find(|row| !matches!(row.operation.kind, OperationKind::Alloca { .. }))
                    .unwrap()
                    .coordinate;
                assert!(allocations.site(nonallocation, out).is_err());
                Ok(())
            })
            .0
            .unwrap();
        }
    }

    #[test]
    fn original_mir_tile_allocations_reject_foreign_and_refunded_accounts() {
        for foreign in [false, true] {
            let mut reached = false;
            let result = super::super::tests::run_tile_slots(Layout::Blocked, |slots, out| {
                let tile = slots.tile.unwrap();
                let (inventory, receipt) =
                    Inventory::derive_v18(tile.output(out.budget)?, out.budget)?;
                out.budget.reserve_storage(receipt.retained_storage())?;
                let allocations = TileAllocationSlotsV164::derive(slots, &inventory, out)?;
                reached = true;
                let error = if foreign {
                    let mut work = Work::new(100_000_000);
                    let mut budget = Budget::new(&mut work, 100_000_000);
                    budget.reserve_storage(out.budget.storage())?;
                    let mut writer = Writer::new(&mut budget)?;
                    allocations
                        .check_owner(inventory.owner(), &mut writer)
                        .unwrap_err()
                } else {
                    out.budget.release_storage(1)?;
                    allocations.check_owner(inventory.owner(), out).unwrap_err()
                };
                assert!(matches!(
                    error,
                    Error::Resource(Resource::Accounting)
                        | Error::Source(SourceError::Resource(Resource::Accounting))
                ));
                assert!(allocations.check_owner(inventory.owner(), out).is_err());
                Err(error)
            });
            assert!(reached);
            assert!(result.0.is_err());
        }
    }
}
