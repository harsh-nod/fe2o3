// All backing is owned inside its paid scope. A substituted ledger is never refunded.
struct Cleanup<'a, 'w> {
    budget: &'a mut ArgumentBudgetV1<'w>,
    slot: usize,
    ledger: ArgumentLedgerV1,
    floor: usize,
    active: bool,
}
impl Cleanup<'_, '_> {
    fn valid(&self) -> bool {
        self.slot == std::ptr::from_ref(&*self.budget) as usize
            && self.ledger == self.budget.work_ledger_identity_v1()
            && self.budget.storage() >= self.floor
    }
    fn finish(&mut self) -> Result<(), ArgumentResourceV1> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        if !self.valid() {
            return Err(ArgumentResourceV1::Accounting);
        }
        self.budget
            .release_storage(self.budget.storage() - self.floor)
    }
}
impl Drop for Cleanup<'_, '_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
fn scoped<'w, T>(
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> R<T>,
) -> R<T> {
    let mut cleanup = Cleanup {
        slot: std::ptr::from_ref(&*budget) as usize,
        ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
        budget,
        active: true,
    };
    let returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cleanup
            .budget
            .reserve_storage(std::mem::size_of::<Cleanup<'_, '_>>())?;
        cleanup
            .budget
            .reserve_storage(std::mem::size_of::<std::thread::Result<R<T>>>())?;
        run(cleanup.budget)
    }));
    if !cleanup.valid() {
        drop(returned);
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let result = match returned {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Failure::Panicked)
        }
    };
    cleanup.finish()?;
    result
}
fn rows<T>(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> R<Vec<T>> {
    let bytes = argument_product_v1(count, std::mem::size_of::<T>())?;
    budget.reserve_storage(bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Failure::Allocation)?;
    let actual = argument_product_v1(rows.capacity(), std::mem::size_of::<T>())?;
    budget.reserve_storage(
        actual
            .checked_sub(bytes)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    Ok(rows)
}
fn binding(span: Option<usize>, detail: &'static str) -> Failure {
    Failure::Binding { span, detail }
}
fn query_origin(error: SemanticKirAssertOriginErrorV1) -> Failure {
    Failure::Policy(ProductionCanonicalRankedPolicyErrorV1::Source(
        ProductionSemanticKirErrorV1::from(error).into(),
    ))
}
struct CallableMeter<'a, 'w>(&'a mut ArgumentBudgetV1<'w>);
impl fe2o3_mir_model::SemanticAssertionMeterV1 for CallableMeter<'_, '_> {
    type Error = ArgumentResourceV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount)
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.0.reserve_storage(bytes)
    }
}
fn callable_error(
    error: fe2o3_mir_model::SemanticAssertionMeteredErrorV1<ArgumentResourceV1>,
) -> Failure {
    match error {
        fe2o3_mir_model::SemanticAssertionMeteredErrorV1::Analysis(error) => {
            Failure::Callable(error)
        }
        fe2o3_mir_model::SemanticAssertionMeteredErrorV1::Meter(error) => error.into(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Accepted source-ledger usage at one observation point, not a new reservation.
pub struct ProductionCanonicalAssertionResourcesV1 {
    /// Cumulative accepted work in the caller's KIR/source ledger.
    pub work: usize,
    /// Current paid bytes, including the caller's retained floor.
    pub storage: usize,
    /// Cumulative peak; not process RSS or a native-analysis byte count.
    pub peak: usize,
    /// First denied caller storage request, if present.
    pub failed_storage: Option<usize>,
}
fn observation(budget: &ArgumentBudgetV1<'_>) -> ProductionCanonicalAssertionResourcesV1 {
    ProductionCanonicalAssertionResourcesV1 {
        work: budget.work(),
        storage: budget.storage(),
        peak: budget.peak_storage(),
        failed_storage: budget.failed_storage(),
    }
}

#[cfg(test)]
include!("production_canonical_assertion_reader_coverage_v1_tests.rs");
