// Actual table capacities are paid; the foundation inventory retains its separate logical receipt.
fn sum(values: &[usize]) -> R<usize> {
    Ok(argument_sum_v1(values)?)
}
fn bytes<T>(count: usize) -> R<usize> {
    Ok(argument_product_v1(count, size_of::<T>())?)
}
fn vector<T>(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> R<Vec<T>> {
    budget.reserve_storage(bytes::<T>(count)?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    let slack = rows
        .capacity()
        .checked_sub(count)
        .ok_or(ArgumentResourceV1::Accounting)?;
    budget.reserve_storage(bytes::<T>(slack)?)?;
    Ok(rows)
}
fn push<T>(rows: &mut Vec<T>, value: T, budget: &mut ArgumentBudgetV1<'_>) -> R<()> {
    budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        return Err(invalid("unpaid table growth"));
    }
    rows.push(value);
    Ok(())
}
fn optional_slots(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> R<Vec<Option<usize>>> {
    let mut rows = vector(count, budget)?;
    for _ in 0..count {
        push(&mut rows, None, budget)?;
    }
    Ok(rows)
}
fn u32_index(value: usize) -> R<u32> {
    u32::try_from(value).map_err(|_| ArgumentResourceV1::Arithmetic.into())
}
fn block_coordinate(function: usize, block: usize) -> R<BlockCoordinate> {
    Ok(BlockCoordinate {
        function: FunctionCoordinate(u32_index(function)?),
        block: u32_index(block)?,
    })
}
fn operation_coordinate(point: TileScalarPointV29) -> R<OperationCoordinate> {
    Ok(OperationCoordinate {
        block: block_coordinate(point.function, point.block)?,
        operation: u32_index(point.operation)?,
    })
}
fn catalog_error(
    error: ProductionSourceOutputCatalogErrorV1,
) -> ProductionTileScalarTransportErrorV29 {
    match error {
        ProductionSourceOutputCatalogErrorV1::Resource(error) => error.into(),
        _ => invalid("coordinate index"),
    }
}
fn sort<T, K: Ord>(
    rows: &mut [T],
    key: impl Fn(&T) -> K,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    source_catalog_sort_v1(rows, key, budget).map_err(catalog_error)
}
fn find<'a, T, K: Ord>(
    rows: &'a [T],
    key: impl Fn(&T) -> K,
    wanted: K,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<Option<&'a T>> {
    source_catalog_find_v1(rows, wanted, key, budget).map_err(catalog_error)
}

// A callback may return a value or a panic payload whose destructor also panics.
// Drain all rejected backing before any storage refund; no nesting cap or leak.
fn discard<T>(value: T) {
    let mut result = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = result {
        result = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}
struct QueryGuard {
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    failure: Cell<Option<ProductionTileScalarTransportErrorV29>>,
}
impl QueryGuard {
    fn new(budget: &ArgumentBudgetV1<'_>) -> Self {
        Self {
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            failure: Cell::new(None),
        }
    }
    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> R<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            self.failure
                .set(Some(ArgumentResourceV1::Accounting.into()));
        }
        self.failure.get().map_or(Ok(()), Err)
    }
    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> R<()> {
        self.check(budget)?;
        if let Err(error) = budget.charge_work(1) {
            self.failure.set(Some(error.into()));
        }
        self.check(budget)
    }
    fn reject<T>(&self, reason: &'static str) -> R<T> {
        let error = self.failure.get().unwrap_or_else(|| invalid(reason));
        self.failure.set(Some(error));
        Err(error)
    }
}
fn scope<'w, T>(
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> R<T>,
) -> R<T> {
    scope_impl(None, budget, run)
}

fn scope_with_cleanup<'w, T>(
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> R<T>,
) -> R<T> {
    scope_impl(Some(cleanup), budget, run)
}

fn scope_impl<'w, T>(
    cleanup: Option<&ScopedSourceCleanupV29>,
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> R<T>,
) -> R<T> {
    if cleanup.is_some_and(ScopedSourceCleanupV29::is_denied) {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    let slot = std::ptr::from_ref(budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let headers = sum(&[
        size_of::<std::thread::Result<R<T>>>(),
        size_of::<std::thread::Result<R<T>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<QueryGuard>(),
        size_of::<ProductionCheckedTileScalarTransportV29<'_>>(),
    ])?;
    budget.reserve_storage(headers)?;
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let valid = slot == std::ptr::from_ref(budget) as usize
        && ledger == budget.work_ledger_identity_v1()
        && budget.storage() >= sum(&[floor, headers])?;
    if !valid {
        if let Some(cleanup) = cleanup {
            cleanup.deny_refund();
        } else {
            discard(result);
            return Err(ArgumentResourceV1::Accounting.into());
        }
    }
    let result = match result {
        Ok(result) => result,
        Err(payload) => {
            discard(payload);
            Err(ProductionTileScalarTransportErrorV29::Panicked)
        }
    };
    if cleanup.is_some_and(ScopedSourceCleanupV29::is_denied) {
        return match result {
            Err(error) => Err(error),
            Ok(value) => {
                discard(value);
                Err(ArgumentResourceV1::Accounting.into())
            }
        };
    }
    // Callers drop temporary inventories/tables within run, before this refund.
    let settlement = budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    );
    if let Err(error) = settlement {
        if let Some(cleanup) = cleanup {
            cleanup.deny_refund();
            return match result {
                Err(original) => Err(original),
                Ok(value) => {
                    discard(value);
                    Err(error.into())
                }
            };
        }
        return Err(error.into());
    }
    result
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) enum TestFault {
    OmittedOrigin,
    OriginRange,
    OmittedPiece,
    PieceComponent,
    PieceStage,
    DuplicateOperation,
    OmittedUse,
    UseDefinition,
    OmittedPayload,
    PayloadOrdinal,
    OmittedAttachment,
    OmittedSourceAlias,
    SourceInstance,
    PendingOmission,
    ReadAssociation,
    MissingAlias,
    DuplicatedAlias,
    ForeignAliasRange,
}
#[cfg(test)]
pub(super) fn inject_fault(owner: &mut ProductionTileScalarTransportOwnerV29, fault: TestFault) {
    // Unauthenticated hostile-row injection, never an alternate source constructor.
    let tables = &mut owner.tables;
    match fault {
        TestFault::OmittedOrigin => {
            tables.origins.pop().unwrap();
        }
        TestFault::OriginRange => {
            tables.origins[0].pieces.end = usize::MAX;
        }
        TestFault::OmittedPiece => {
            tables.pieces.pop().unwrap();
        }
        TestFault::PieceComponent => {
            tables.pieces[0].component = Some(u32::MAX);
        }
        TestFault::PieceStage => {
            tables.pieces[0].stage = ProductionTileExpansionStageV29::Parts;
        }
        TestFault::DuplicateOperation => {
            tables.operations[1].coordinate = tables.operations[0].coordinate;
        }
        TestFault::OmittedUse => {
            tables.uses.pop().unwrap();
        }
        TestFault::UseDefinition => {
            tables.uses[0].definition = Some(usize::MAX);
        }
        TestFault::OmittedPayload => {
            tables.edge_arguments.pop().unwrap();
        }
        TestFault::PayloadOrdinal => {
            tables.edge_arguments[0].coordinate = tables.edge_arguments[1].coordinate;
        }
        TestFault::OmittedAttachment => {
            tables.attachments.pop().unwrap();
        }
        TestFault::OmittedSourceAlias => {
            tables.source_aliases.pop().unwrap();
        }
        TestFault::SourceInstance => {
            tables.source_aliases[0].root = usize::MAX;
        }
        TestFault::PendingOmission => {
            tables.obligations.pop().unwrap();
        }
        TestFault::ReadAssociation => {
            let read = tables
                .obligations
                .iter_mut()
                .find_map(|row| row.global_read.as_mut())
                .unwrap();
            read.source_alias = usize::MAX;
        }
        TestFault::MissingAlias => {
            tables.piece_aliases.pop().unwrap();
        }
        TestFault::DuplicatedAlias => {
            tables.piece_aliases[1] = tables.piece_aliases[0];
        }
        TestFault::ForeignAliasRange => {
            tables.operations[0].piece_aliases = usize::MAX..usize::MAX;
        }
    }
}
#[cfg(test)]
pub(super) fn foreign_inventory(
    owner: &ProductionTileScalarTransportOwnerV29,
    foreign: &ProductionTileScalarTransportOwnerV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    scope(budget, |budget| {
        let (inventory, receipt) = Inventory::derive_v18(&foreign.candidate.output, budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        budget.reserve_storage(size_of::<Tables>())?;
        let mut tables = Tables::default();
        index_graph(&owner.candidate, &inventory, &mut tables, budget)
    })
}
#[cfg(test)]
pub(super) fn callback_headers<T>() -> usize {
    2 * size_of::<std::thread::Result<R<T>>>()
        + size_of::<std::thread::Result<()>>()
        + size_of::<QueryGuard>()
        + size_of::<ProductionCheckedTileScalarTransportV29<'_>>()
}
#[cfg(test)]
pub(super) fn allocation_boundary<T>(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> R<()> {
    scope(budget, |budget| {
        let rows = vector::<T>(count, budget)?;
        drop(rows);
        Ok(())
    })
}
