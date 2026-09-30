//! Exact origin queries use the original paid context and retained dense state.
use super::*;
use crate::formal_memory_obligations::exact_origin_v18::{self, Compare, State};
use crate::verification_types_equal_v1;

struct Query<'borrow, 'owner, 'owner_work, 'budget, 'budget_work> {
    owner: &'borrow mut ActualOwnerAffineV18<'owner, 'owner_work>,
    budget: &'budget mut Budget<'budget_work>,
    epoch: usize,
}
impl Compare for Query<'_, '_, '_, '_, '_> {
    type Error = Failure;
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool> {
        Ok(verification_types_equal_v1(left, right, self.budget)?)
    }
    fn step(&mut self) -> Result<()> {
        Ok(self.budget.charge_work(1)?)
    }
}
impl<'owner> State<'owner> for Query<'_, 'owner, '_, '_, '_> {
    fn enter(&mut self, value: ValueId) -> Result<bool> {
        let position = verification_find_last_by_v1(&self.owner.rows, 1, self.budget, |row| {
            row.key.cmp(&value.0)
        })?;
        self.budget.charge_work(1)?;
        let Some(position) = position else {
            return Ok(true);
        };
        let row = &mut self.owner.rows[position];
        if row.origin_epoch == self.epoch {
            return Ok(false);
        }
        row.origin_epoch = self.epoch;
        Ok(true)
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>> {
        self.owner
            .context
            .unique_origin(self.owner.source, value, self.budget)
    }
    fn ty(&mut self, value: ValueId) -> Result<Option<&'owner Type>> {
        self.owner
            .context
            .value_type(self.owner.source, value, self.budget)
    }
    fn operation(&mut self, value: ValueId) -> Result<Option<&'owner Operation>> {
        self.owner
            .context
            .operation(self.owner.source, value, self.budget)
            .map(|row| row.map(|(op, _)| op))
    }
}

impl ActualOwnerAffineV18<'_, '_> {
    // This returns a syntactic exact origin, not private-slot eligibility or
    // evidence of a defined value. Unknown IDs retain the legacy Some(id).
    pub(in crate::formal_memory_obligations) fn exact_origin(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<ValueId>> {
        self.check(owner, root_index, budget)?;
        let floor = budget.storage();
        if let Err(error) = budget.reserve_storage(query_frame_v18()) {
            return self.keep(Err(error.into()));
        }
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<_> {
            budget.charge_work(1)?;
            let epoch = self
                .next_origin_epoch
                .checked_add(1)
                .ok_or(ResourceError::Arithmetic)?;
            self.next_origin_epoch = epoch;
            exact_origin_v18::run(
                &mut Query {
                    owner: self,
                    budget,
                    epoch,
                },
                value,
            )
        }));
        // The query carries only fixed-size results and borrowed context; all
        // marks remain in already credited rows, with a fresh checked epoch.
        budget.rollback_storage(floor)?;
        match result {
            Ok(result) => self.keep(result),
            Err(payload) => resume_unwind(payload),
        }
    }
}

fn query_frame_v18() -> usize {
    size_of::<Query<'static, 'static, 'static, 'static, 'static>>()
        + size_of::<std::thread::Result<Result<Option<ValueId>>>>()
        + size_of::<Result<Option<ValueId>>>()
        + size_of::<(
            &mut ActualOwnerAffineV18<'static, 'static>,
            &mut Budget<'static>,
            ValueId,
        )>()
}

#[cfg(test)]
#[path = "exact_source_origin_v18_tests.rs"]
mod tests;
