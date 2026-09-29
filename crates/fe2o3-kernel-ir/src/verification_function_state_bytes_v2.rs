//! Private byte-accounted source index, not verification or formal admission.
use super::*;
use crate::verification_index_v1::{
    verification_find_last_by_v1, verification_radix_sort_u32_bytes_v2,
};
use crate::verification_typed_storage_v2::{allocate_vector_v2, prior_denial_v2};
use std::{
    marker::PhantomData,
    mem::{size_of, size_of_val},
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

type Budget<'w> = CanonicalKernelIrVerificationResourceBudgetV1<'w>;
type Error = CanonicalKernelIrVerificationResourceErrorV1;

#[must_use = "dropping the source index without release retains its resource charge"]
pub(crate) struct ByteFunctionStateV2<'source, 'work> {
    source: &'source Function,
    blocks: Vec<VerificationNumericIndexRowV1<&'source BasicBlock>>,
    definitions: Vec<VerificationNumericIndexRowV1<VerificationDefinitionV1<'source>>>,
    slot: usize,
    ledger: crate::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained_bytes: usize,
    work_borrow: PhantomData<&'work crate::CanonicalKernelIrWorkBudgetV1>,
}

impl<'source, 'work> ByteFunctionStateV2<'source, 'work> {
    pub(crate) fn build(
        function: &'source Function,
        budget: &mut Budget<'work>,
    ) -> Result<Option<Self>, Error> {
        prior_denial_v2(budget)?;
        budget.charge_work(1)?;
        let Some(body) = function.body.as_ref() else {
            return Ok(None);
        };
        let floor = budget.storage();
        let slot = budget as *mut Budget<'_> as usize;
        let ledger = budget.work_ledger_identity_v1();
        let frame = frame_bytes_v2();
        budget.reserve_storage(frame)?;
        let result = catch_unwind(AssertUnwindSafe(|| -> Result<_, Error> {
            let mut blocks = allocate_vector_v2(body.blocks.len(), budget)?;
            budget.charge_work(body.blocks.len())?;
            blocks.extend(
                body.blocks
                    .iter()
                    .map(|block| VerificationNumericIndexRowV1 {
                        key: block.id.0,
                        value: block,
                    }),
            );
            verification_radix_sort_u32_bytes_v2(&mut blocks, budget, |row| row.key)?;

            let (count, operation_visits) = function_definition_count_v2(function, body, budget)?;
            budget.charge_work(body.blocks.len())?;
            budget.charge_work(operation_visits)?;
            let mut input = function_definition_rows_v2(function, body);
            let input_frame = size_of_val(&input);
            budget.reserve_storage(input_frame)?;
            let mut definitions = allocate_vector_v2(count, budget)?;
            budget.charge_work(count)?;
            for _ in 0..count {
                let (value, definition) = input.next().ok_or(Error::Accounting)?;
                definitions.push(VerificationNumericIndexRowV1 {
                    key: value.0,
                    value: definition,
                });
            }
            // Drive through empty-result suffixes, as the legacy index does.
            budget.charge_work(1)?;
            if input.next().is_some() {
                return Err(Error::Accounting);
            }
            drop(input);
            budget.release_storage(input_frame)?;
            verification_radix_sort_u32_bytes_v2(&mut definitions, budget, |row| row.key)?;
            Ok((blocks, definitions))
        }));
        match result {
            Ok(Ok((blocks, definitions))) => {
                budget.release_storage(frame - size_of::<Self>())?;
                let retained_bytes = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Error::Accounting)?;
                Ok(Some(Self {
                    source: function,
                    blocks,
                    definitions,
                    slot,
                    ledger,
                    floor,
                    retained_bytes,
                    work_borrow: PhantomData,
                }))
            }
            Ok(Err(error)) => {
                budget.rollback_storage(floor)?;
                Err(error)
            }
            Err(panic) => {
                budget.rollback_storage(floor)?;
                resume_unwind(panic)
            }
        }
    }

    fn same_identity(&self, budget: &Budget<'_>) -> Result<(), Error> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            return Err(Error::Accounting);
        }
        Ok(())
    }

    fn check(&self, source: &Function, budget: &Budget<'_>) -> Result<(), Error> {
        self.same_identity(budget)?;
        prior_denial_v2(budget)?;
        if !std::ptr::eq(source, self.source)
            || budget.storage()
                < self
                    .floor
                    .checked_add(self.retained_bytes)
                    .ok_or(Error::Arithmetic)?
        {
            return Err(Error::Accounting);
        }
        Ok(())
    }

    pub(crate) fn block(
        &self,
        source: &Function,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&'source BasicBlock>, Error> {
        self.check(source, budget)?;
        verification_find_last_by_v1(&self.blocks, 1, budget, |row| row.key.cmp(&block.0))
            .map(|position| position.map(|position| self.blocks[position].value))
    }

    pub(crate) fn definition(
        &self,
        source: &Function,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<VerificationDefinitionV1<'source>>, Error> {
        self.check(source, budget)?;
        verification_find_last_by_v1(&self.definitions, 1, budget, |row| row.key.cmp(&value.0))
            .map(|position| position.map(|position| self.definitions[position].value))
    }

    pub(crate) fn definition_count_v20(
        &self,
        source: &Function,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Error> {
        self.check(source, budget)?;
        budget.charge_work(1)?;
        Ok(self.definitions.len())
    }

    pub(crate) fn definition_ordinal_v20(
        &self,
        source: &Function,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(usize, VerificationDefinitionV1<'source>)>, Error> {
        self.check(source, budget)?;
        verification_find_last_by_v1(&self.definitions, 1, budget, |row| row.key.cmp(&value.0))
            .map(|position| position.map(|position| (position, self.definitions[position].value)))
    }

    pub(crate) fn release(self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.same_identity(budget)?;
        if budget.storage()
            < self
                .floor
                .checked_add(self.retained_bytes)
                .ok_or(Error::Arithmetic)?
        {
            return Err(Error::Accounting);
        }
        let retained = self.retained_bytes;
        drop(self);
        budget.release_storage(retained)
    }

    // Pays a complete traversal. Consumers must separately pay owned copies.
    pub(crate) fn definition_rows_v2(
        &self,
        source: &Function,
        budget: &mut Budget<'_>,
    ) -> Result<&[VerificationNumericIndexRowV1<VerificationDefinitionV1<'source>>], Error> {
        self.check(source, budget)?;
        budget.charge_work(
            self.definitions
                .len()
                .checked_add(1)
                .ok_or(Error::Arithmetic)?,
        )?;
        Ok(&self.definitions)
    }
}

fn frame_bytes_v2() -> usize {
    type State = ByteFunctionStateV2<'static, 'static>;
    type Rows = (
        Vec<VerificationNumericIndexRowV1<&'static BasicBlock>>,
        Vec<VerificationNumericIndexRowV1<VerificationDefinitionV1<'static>>>,
    );
    size_of::<State>()
        + size_of::<std::thread::Result<Result<Rows, Error>>>()
        + size_of::<Result<Option<State>, Error>>()
        + size_of::<(&Function, &crate::FunctionBody, &mut Budget<'static>)>()
}

#[cfg(test)]
#[path = "verification_function_state_bytes_v2_tests.rs"]
mod tests;
