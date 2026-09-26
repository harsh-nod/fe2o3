use super::*;

pub(super) struct Accounting {
    slot: usize,
    ledger: Ledger,
    floor: usize,
    reserved: usize,
    first: Option<ProductionSemanticAssertionQueryErrorV1>,
    cleanup: bool,
}
impl Accounting {
    pub(super) fn new(budget: &Budget<'_>) -> Self {
        Self {
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            reserved: 0,
            first: None,
            cleanup: true,
        }
    }
    pub(super) fn fail(
        &mut self,
        error: ProductionSemanticAssertionQueryErrorV1,
    ) -> ProductionSemanticAssertionQueryErrorV1 {
        *self.first.get_or_insert(error)
    }
    pub(super) fn check(&mut self, budget: &Budget<'_>) -> R<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            self.cleanup = false;
            self.first = Some(Resource::Accounting.into());
        } else if self.floor.checked_add(self.reserved) != Some(budget.storage()) {
            self.first = Some(Resource::Accounting.into());
        }
        self.first.map_or(Ok(()), Err)
    }
    pub(super) fn charge(&mut self, budget: &mut Budget<'_>, amount: usize) -> R<()> {
        self.check(budget)?;
        budget
            .charge_work(amount)
            .map_err(|error| self.fail(error.into()))
    }
    pub(super) fn reserve(&mut self, budget: &mut Budget<'_>, amount: usize) -> R<()> {
        self.check(budget)?;
        let reserved = self
            .reserved
            .checked_add(amount)
            .ok_or_else(|| self.fail(Resource::Arithmetic.into()))?;
        budget
            .reserve_storage(amount)
            .map_err(|error| self.fail(error.into()))?;
        self.reserved = reserved;
        Ok(())
    }
    pub(super) fn model_error(
        &mut self,
        error: MeteredError<Resource>,
    ) -> ProductionSemanticAssertionQueryErrorV1 {
        self.fail(match error {
            MeteredError::Analysis(error) => {
                ProductionSemanticAssertionQueryErrorV1::Analysis(error)
            }
            MeteredError::Meter(error) => error.into(),
        })
    }
    pub(super) fn finish<T>(
        &mut self,
        budget: &mut Budget<'_>,
        result: std::thread::Result<R<T>>,
    ) -> R<T> {
        let mut cleanup = Cleanup {
            accounting: self,
            budget,
            active: true,
        };
        let result = match result {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(ProductionSemanticAssertionQueryErrorV1::Panicked)
            }
        };
        let result = match cleanup.accounting.check(cleanup.budget) {
            Ok(()) => result,
            Err(error) => {
                drop(result);
                Err(error)
            }
        };
        cleanup.refund()?;
        result
    }
}

pub(super) struct Cleanup<'a, 'work> {
    accounting: &'a mut Accounting,
    budget: &'a mut Budget<'work>,
    active: bool,
}
impl Cleanup<'_, '_> {
    fn refund(&mut self) -> Result<(), Resource> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let _ = self.accounting.check(self.budget);
        if self.accounting.cleanup {
            let bytes = self
                .budget
                .storage()
                .checked_sub(self.accounting.floor)
                .ok_or(Resource::Accounting)?;
            self.budget.release_storage(bytes)?;
        }
        Ok(())
    }
}
impl Drop for Cleanup<'_, '_> {
    fn drop(&mut self) {
        let _ = self.refund();
    }
}

pub(super) struct Meter<'a, 'work> {
    pub(super) accounting: &'a mut Accounting,
    pub(super) budget: &'a mut Budget<'work>,
}
impl SemanticAssertionMeterV1 for Meter<'_, '_> {
    type Error = Resource;
    fn charge_work(&mut self, amount: usize) -> Result<(), Resource> {
        self.accounting
            .charge(self.budget, amount)
            .map_err(|error| match error {
                ProductionSemanticAssertionQueryErrorV1::Resource(error) => error,
                _ => Resource::Accounting,
            })
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Resource> {
        self.accounting
            .reserve(self.budget, bytes)
            .map_err(|error| match error {
                ProductionSemanticAssertionQueryErrorV1::Resource(error) => error,
                _ => Resource::Accounting,
            })
    }
}
