//! Private preparation ledger adapter. No result/source/capability authority.
//! Accepted reservations belong to the factory; this adapter never releases.
use super::{CanonicalAssertionErrorV1, ProductionRankedProjectionErrorV1 as Error};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::mem::size_of;

enum Ledger<'b, 'w> {
    Legacy,
    Original {
        budget: &'b mut Budget<'w>,
        owned: &'b mut usize,
    },
}
pub(super) struct PreparationResourcesV1<'b, 'w> {
    ledger: Ledger<'b, 'w>,
}

pub(super) fn resource(error: Resource) -> Error {
    Error::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error))
}
impl<'b, 'w> PreparationResourcesV1<'b, 'w> {
    pub(super) fn unmetered() -> Self {
        Self {
            ledger: Ledger::Legacy,
        }
    }
    pub(super) fn new(budget: &'b mut Budget<'w>, owned: &'b mut usize) -> Self {
        Self {
            ledger: Ledger::Original { budget, owned },
        }
    }
    pub(super) fn is_metered(&self) -> bool {
        matches!(self.ledger, Ledger::Original { .. })
    }
    /// Read-only sticky-denial projection; never lends the original Budget.
    pub(super) fn has_denial(&self) -> bool {
        match &self.ledger {
            Ledger::Legacy => false,
            Ledger::Original { budget, .. } => {
                budget.failed_work().is_some() || budget.failed_storage().is_some()
            }
        }
    }
    /// Lexical identity only, not source/owner authority or a globally unique
    /// address token. The caller must retain the exclusive adapter-borrow
    /// lifetime while using any container tagged with this pair.
    pub(super) fn original_ledger_v1(
        &self,
    ) -> Option<(
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    )> {
        match &self.ledger {
            Ledger::Legacy => None,
            Ledger::Original { budget, .. } => {
                let budget: &Budget<'_> = budget;
                Some((
                    budget as *const Budget<'_> as usize,
                    budget.work_ledger_identity_v1(),
                ))
            }
        }
    }
    pub(super) fn work(&mut self, amount: usize) -> Result<(), Error> {
        if let Ledger::Original { budget, .. } = &mut self.ledger {
            budget.charge_work(amount).map_err(resource)?;
        }
        Ok(())
    }
    pub(super) fn reserve_storage(&mut self, amount: usize) -> Result<(), Error> {
        if let Ledger::Original { budget, owned } = &mut self.ledger {
            let next = (**owned)
                .checked_add(amount)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(amount).map_err(resource)?;
            **owned = next;
        }
        Ok(())
    }
    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), Error> {
        let requested = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if requested <= values.capacity() {
            return Ok(());
        }
        if self.is_metered() {
            // Full next capacity is admitted while all prior growth remains
            // owned. Relocation and initialization never use refunded scratch.
            self.work(values.len())?;
            self.reserve_storage(
                requested
                    .checked_mul(size_of::<T>())
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )?;
            values
                .try_reserve_exact(additional)
                .map_err(|_| resource(Resource::Allocation))?;
            if size_of::<T>() != 0 && values.capacity() != requested {
                return Err(resource(Resource::Allocation));
            }
        } else {
            // Legacy loops retain amortized growth, not cumulative exact growth.
            values
                .try_reserve(additional)
                .map_err(|_| resource(Resource::Allocation))?;
        }
        Ok(())
    }
    pub(super) fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        self.work(1)?;
        self.reserve(values, 1)?;
        values.push(value);
        Ok(())
    }
    pub(super) fn filled<T: Clone>(&mut self, count: usize, value: T) -> Result<Vec<T>, Error> {
        self.work(count)?;
        let mut values = Vec::new();
        self.reserve(&mut values, count)?;
        values.resize(count, value);
        Ok(values)
    }
    pub(super) fn nested<T>(&mut self, count: usize) -> Result<Vec<Vec<T>>, Error> {
        self.work(count)?;
        let mut values = Vec::new();
        self.reserve(&mut values, count)?;
        values.resize_with(count, Vec::new);
        Ok(values)
    }
    pub(super) fn sort_unique_indices(&mut self, values: &mut Vec<usize>) -> Result<(), Error> {
        if self.is_metered() {
            // Explicit comparison/move admission rather than an assumed hidden
            // library-sort comparison constant. Source ordering is immaterial;
            // the same sorted unique definition-local set is produced.
            for end in 1..values.len() {
                self.work(1)?;
                let value = values[end];
                let mut cursor = end;
                while cursor > 0 {
                    self.work(1)?;
                    if values[cursor - 1] <= value {
                        break;
                    }
                    self.work(1)?;
                    values[cursor] = values[cursor - 1];
                    cursor -= 1;
                }
                self.work(1)?;
                values[cursor] = value;
            }
        } else {
            values.sort_unstable();
        }
        self.work(values.len())?;
        values.dedup();
        Ok(())
    }
}
