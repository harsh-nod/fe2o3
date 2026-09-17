// Erases only the lifetime of the shared work ledger, not its identity. This
// permits the existing one-lifetime lowerer to borrow the enclosing call budget.
trait BorrowedAggregateBudgetV1 {
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn storage(&self) -> usize;
}

impl BorrowedAggregateBudgetV1 for ArgumentBudgetV1<'_> {
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        ArgumentBudgetV1::charge_work(self, amount).map_err(Into::into)
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        ArgumentBudgetV1::reserve_storage(self, amount).map_err(Into::into)
    }
    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        ArgumentBudgetV1::release_storage(self, amount).map_err(Into::into)
    }
    fn storage(&self) -> usize {
        ArgumentBudgetV1::storage(self)
    }
}

fn borrowed_aggregate_vec_v1<T>(
    count: usize,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    budget.reserve_storage(argument_product_v1(count, std::mem::size_of::<T>())?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    budget.reserve_storage(argument_product_v1(
        rows.capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?,
        std::mem::size_of::<T>(),
    )?)?;
    Ok(rows)
}

// Shared planner vectors also contain legacy entries whose capacity was charged
// on the closure ledger. Do not release that capacity on the borrowing ledger.
fn borrowed_aggregate_push_shared_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if rows.len() == rows.capacity() {
        let next = argument_product_v1(rows.capacity().max(2), 2)?;
        let mut replacement = borrowed_aggregate_vec_v1(next, budget)?;
        budget.charge_work(rows.len())?;
        replacement.append(rows);
        *rows = replacement;
    }
    rows.push(row);
    Ok(())
}
