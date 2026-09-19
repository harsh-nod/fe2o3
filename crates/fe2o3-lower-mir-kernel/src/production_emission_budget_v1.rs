// Erase the shared ledger's borrow lifetime without changing its identity.
trait SemanticEmissionBudgetV1 {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn storage(&self) -> usize;
}

impl SemanticEmissionBudgetV1 for ArgumentBudgetV1<'_> {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
        ArgumentBudgetV1::work_ledger_identity_v1(self)
    }

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

fn emission_vec_v1<T>(
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
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

fn emission_push_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if rows.len() == rows.capacity() {
        let next = argument_product_v1(rows.capacity().max(2), 2)?;
        let mut replacement = emission_vec_v1(next, budget)?;
        budget.charge_work(rows.len())?;
        let old_bytes = argument_product_v1(rows.capacity(), std::mem::size_of::<T>())?;
        replacement.append(rows);
        *rows = replacement;
        budget.release_storage(old_bytes)?;
    }
    rows.push(row);
    Ok(())
}

// Shared planner vectors may have capacity paid on the closure ledger.
// Growing them must not refund that capacity to the emission ledger.
fn emission_push_shared_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if rows.len() == rows.capacity() {
        let next = argument_product_v1(rows.capacity().max(2), 2)?;
        let mut replacement = emission_vec_v1(next, budget)?;
        budget.charge_work(rows.len())?;
        replacement.append(rows);
        *rows = replacement;
    }
    rows.push(row);
    Ok(())
}

impl SemanticFunctionLoweringV1<'_> {
    fn with_emission_budget_v1<T>(
        &mut self,
        body: impl FnOnce(
            &mut Self,
            &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let budget = self.emission_work.take().ok_or_else(|| {
            unsupported(
                self.semantic_function.index(),
                None,
                None,
                "semantic emission has no shared resource ledger",
            )
        })?;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self, &mut *budget)));
        self.emission_work = Some(budget);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

#[cfg(test)]
mod emission_budget_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

    #[test]
    fn owned_growth_charges_coexisting_buffers_then_releases_old_capacity() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100);
        let mut rows = emission_vec_v1::<u8>(2, &mut budget).unwrap();
        let old_capacity = rows.capacity();
        rows.resize(old_capacity, 7);
        emission_push_v1(&mut rows, 9, &mut budget).unwrap();
        let final_capacity = rows.capacity();
        assert_eq!(budget.storage(), final_capacity);
        assert_eq!(&rows[..old_capacity], vec![7; old_capacity]);
        assert_eq!(rows[old_capacity], 9);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut short = ArgumentBudgetV1::new(&mut work, old_capacity + final_capacity - 1);
        let mut rows = emission_vec_v1::<u8>(2, &mut short).unwrap();
        rows.resize(old_capacity, 7);
        assert!(emission_push_v1(&mut rows, 9, &mut short).is_err());
        assert_eq!(rows, vec![7; old_capacity]);
        assert_eq!(rows.capacity(), old_capacity);
    }

    #[test]
    fn shared_growth_does_not_refund_capacity_from_another_ledger() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100);
        let mut rows = vec![3_u8; 2];
        let old_capacity = rows.capacity();
        rows.resize(old_capacity, 3);
        assert_eq!(budget.storage(), 0);
        emission_push_shared_v1(&mut rows, 5, &mut budget).unwrap();
        assert_eq!(budget.storage(), rows.capacity());
        assert_eq!(&rows[..old_capacity], vec![3; old_capacity]);
        assert_eq!(rows[old_capacity], 5);
    }

    #[test]
    fn work_failure_before_copy_keeps_original_rows_and_accepted_charges() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100);
        let mut rows = emission_vec_v1::<u8>(2, &mut budget).unwrap();
        let old_capacity = rows.capacity();
        rows.resize(old_capacity, 7);
        assert!(emission_push_v1(&mut rows, 9, &mut budget).is_err());
        assert_eq!(rows, vec![7; old_capacity]);
        assert_eq!(rows.capacity(), old_capacity);
        assert!(budget.storage() >= old_capacity + 2 * old_capacity.max(2));
        assert!(budget.work() >= 8);
    }
}
