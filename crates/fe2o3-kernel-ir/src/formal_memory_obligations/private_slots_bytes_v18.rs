//! Actual-owner private-slot extraction through the shared original algorithm.
use super::*;
use crate::formal_memory_obligations::private_slots::engine::{
    self, Environment, LoadSource, Slot,
};
use crate::{verification_radix_sort_u32_bytes_v2, verification_types_equal_v1};

#[path = "pointer_source_bytes_v18.rs"]
pub(super) mod pointer_source_bytes_v18;

struct Paid<'borrow, 'owner, 'work, 'budget, 'budget_work> {
    affine: &'borrow mut ActualOwnerAffineV18<'owner, 'work>,
    root_index: usize,
    budget: &'budget mut Budget<'budget_work>,
}
impl<'owner> Environment<'owner> for Paid<'_, 'owner, '_, '_, '_> {
    type Error = Failure;
    fn source(&self) -> &'owner Function {
        self.affine.source
    }
    fn step(&mut self, work: usize) -> Result<()> {
        Ok(self.budget.charge_work(work)?)
    }
    fn arithmetic(&self) -> Failure {
        ResourceError::Arithmetic.into()
    }
    fn reachable(&mut self, block: BlockId) -> Result<bool> {
        self.affine
            .context
            .reachable(self.affine.source, block, self.budget)
    }
    fn block(&mut self, block: BlockId) -> Result<Option<&'owner crate::BasicBlock>> {
        self.affine
            .context
            .block(self.affine.source, block, self.budget)
    }
    fn origin(&mut self, value: ValueId) -> Result<Option<ValueId>> {
        self.affine
            .exact_origin(self.affine.owner, self.root_index, value, self.budget)
    }
    fn ty(&mut self, value: ValueId) -> Result<Option<&'owner Type>> {
        self.affine
            .context
            .value_type(self.affine.source, value, self.budget)
    }
    fn equal(&mut self, left: &Type, right: &Type) -> Result<bool> {
        Ok(verification_types_equal_v1(left, right, self.budget)?)
    }
    fn empty<T: Copy>(&mut self) -> Result<Vec<T>> {
        Ok(allocate_vector_v2(0, self.budget)?)
    }
    fn filled<T: Copy>(&mut self, count: usize, value: T) -> Result<Vec<T>> {
        self.budget.charge_work(count)?;
        let mut rows = allocate_vector_v2(count, self.budget)?;
        rows.resize(count, value);
        Ok(rows)
    }
    fn push<T: Copy>(&mut self, rows: &mut Vec<T>, value: T) -> Result<()> {
        Ok(
            meter::LiveGuardMeter::new(self.budget, usize::MAX, usize::MAX, usize::MAX)
                .push(rows, value)?,
        )
    }
    fn sort<T: Copy>(&mut self, rows: &mut [T], key: impl Fn(&T) -> u32 + Copy) -> Result<()> {
        Ok(verification_radix_sort_u32_bytes_v2(
            rows,
            self.budget,
            key,
        )?)
    }
    fn find<T>(
        &mut self,
        rows: &[T],
        compare: impl Fn(&T) -> std::cmp::Ordering,
    ) -> Result<Option<usize>> {
        Ok(verification_find_last_by_v1(rows, 1, self.budget, compare)?)
    }
}

#[must_use = "dropping private-slot facts without release retains their resource charge"]
pub(in crate::formal_memory_obligations) struct ActualOwnerPrivateSlotsV18<'borrow, 'owner, 'work> {
    affine: &'borrow mut ActualOwnerAffineV18<'owner, 'work>,
    root_index: usize,
    slots: Vec<Slot>,
    loads: Vec<LoadSource>,
    floor: usize,
    retained: usize,
}

impl<'owner, 'work> ActualOwnerAffineV18<'owner, 'work> {
    pub(in crate::formal_memory_obligations) fn private_slots<'borrow>(
        &'borrow mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        budget: &mut Budget<'_>,
    ) -> Result<ActualOwnerPrivateSlotsV18<'borrow, 'owner, 'work>> {
        self.check(owner, root_index, budget)?;
        let floor = budget.storage();
        if let Err(error) = budget.reserve_storage(frame_bytes_v18()) {
            return self.keep(Err(error.into()));
        }
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<_> {
            budget.charge_work(1)?;
            let mut paid = Paid {
                affine: self,
                root_index,
                budget,
            };
            let slots = engine::classify(&mut paid)?;
            let loads = engine::loads(&mut paid, &slots)?;
            drop(paid);
            // The shared engine has dropped its matrices and indices. Settle
            // only after that backing is gone, inside the refusal boundary.
            budget.charge_work(1)?;
            let retained = vector_bytes_v2(&slots)?
                .checked_add(vector_bytes_v2(&loads)?)
                .and_then(|n| n.checked_add(frame_bytes_v18()))
                .ok_or(ResourceError::Arithmetic)?;
            let scratch = budget
                .storage()
                .checked_sub(floor)
                .and_then(|n| n.checked_sub(retained))
                .ok_or(ResourceError::Accounting)?;
            budget.release_storage(scratch)?;
            Ok((slots, loads, retained))
        }));
        match result {
            Ok(Ok((slots, loads, retained))) => Ok(ActualOwnerPrivateSlotsV18 {
                affine: self,
                root_index,
                slots,
                loads,
                floor,
                retained,
            }),
            Ok(Err(error)) => {
                budget.rollback_storage(floor)?;
                self.keep(Err(error))
            }
            Err(payload) => {
                budget.rollback_storage(floor)?;
                resume_unwind(payload)
            }
        }
    }
}

impl ActualOwnerPrivateSlotsV18<'_, '_, '_> {
    fn check(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        budget: &Budget<'_>,
    ) -> Result<()> {
        self.affine.check(owner, root_index, budget)?;
        let result = if root_index != self.root_index
            || budget.storage()
                < self
                    .floor
                    .checked_add(self.retained)
                    .ok_or(ResourceError::Arithmetic)?
        {
            Err(ResourceError::Accounting.into())
        } else {
            Ok(())
        };
        self.affine.keep(result)
    }
    pub(in crate::formal_memory_obligations) fn eligible(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<bool> {
        self.check(owner, root_index, budget)?;
        let result =
            verification_find_last_by_v1(&self.slots, 1, budget, |row| row.value.cmp(&value))
                .map(|position| position.is_some_and(|index| self.slots[index].escape.is_none()))
                .map_err(Into::into);
        self.affine.keep(result)
    }
    pub(in crate::formal_memory_obligations) fn load_source(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<ValueId>> {
        self.check(owner, root_index, budget)?;
        let result =
            verification_find_last_by_v1(&self.loads, 1, budget, |row| row.value.cmp(&value))
                .map(|position| position.map(|index| self.loads[index].source))
                .map_err(Into::into);
        self.affine.keep(result)
    }
    // Returns the original first escape per original slot. The final report
    // assembler must still preserve its original ordered reason set.
    pub(in crate::formal_memory_obligations) fn escape(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        ordinal: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(ValueId, Option<(FunctionOperationLocation, ValueId)>)>> {
        self.check(owner, root_index, budget)?;
        let result = budget
            .charge_work(1)
            .map(|()| self.slots.get(ordinal).map(|row| (row.value, row.escape)))
            .map_err(Into::into);
        self.affine.keep(result)
    }
    pub(in crate::formal_memory_obligations) fn release(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.affine.identity(budget)?;
        if budget.storage()
            < self
                .floor
                .checked_add(self.retained)
                .ok_or(ResourceError::Arithmetic)?
        {
            return Err(ResourceError::Accounting.into());
        }
        let retained = self.retained;
        drop(self);
        Ok(budget.release_storage(retained)?)
    }
}

fn frame_bytes_v18() -> usize {
    size_of::<ActualOwnerPrivateSlotsV18<'static, 'static, 'static>>()
        + size_of::<Paid<'static, 'static, 'static, 'static, 'static>>()
        + size_of::<meter::LiveGuardMeter<'static, 'static>>()
        + size_of::<std::thread::Result<Result<(Vec<Slot>, Vec<LoadSource>, usize)>>>()
        + size_of::<Result<ActualOwnerPrivateSlotsV18<'static, 'static, 'static>>>()
        + size_of::<(
            &mut ActualOwnerAffineV18<'static, 'static>,
            usize,
            usize,
            &mut Budget<'static>,
        )>()
}

#[cfg(test)]
#[path = "private_slots_bytes_v18_tests.rs"]
mod tests;
