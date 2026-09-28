//! Private original-source context; no verification or formal-admission token.
use super::*;
use crate::control_flow::{
    ByteAccountedIndexedControlFlowV2, MeteredControlFlowErrorV1,
    analyze_control_flow_with_byte_budget_v2,
};
use crate::verification_typed_storage_v2::{prior_denial_v2, vector_bytes_v2};
use crate::{
    ByteFunctionStateV2, ControlFlowError, ControlFlowLimits, VerificationDefinitionSiteV1,
};
use meter::LiveGuardMeter;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::formal_memory_obligations) enum SourceContextErrorV2 {
    Resource(ResourceError),
    ControlFlow(ControlFlowError),
}

impl From<ResourceError> for SourceContextErrorV2 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}
impl From<VerificationResourceError> for SourceContextErrorV2 {
    fn from(error: VerificationResourceError) -> Self {
        Self::Resource(error.into())
    }
}
impl From<MeteredControlFlowErrorV1> for SourceContextErrorV2 {
    fn from(error: MeteredControlFlowErrorV1) -> Self {
        match error {
            MeteredControlFlowErrorV1::ControlFlow(error) => Self::ControlFlow(error),
            MeteredControlFlowErrorV1::Resource(error) => error.into(),
        }
    }
}
type ContextResult<T> = std::result::Result<T, SourceContextErrorV2>;

#[must_use = "dropping the source context without release retains its resource charge"]
pub(in crate::formal_memory_obligations) struct ByteSourceContextV2<'source, 'work> {
    source: &'source Function,
    flow: ByteAccountedIndexedControlFlowV2<'source, 'work>,
    index: ByteFunctionStateV2<'source, 'work>,
    origins: Vec<runtime_slice_read_v1::Origin<'source>>,
    slot: usize,
    ledger: crate::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained: usize,
    first_error: Option<SourceContextErrorV2>,
}

impl<'source, 'work> ByteSourceContextV2<'source, 'work> {
    pub(in crate::formal_memory_obligations) fn build(
        source: &'source Function,
        limits: ControlFlowLimits,
        budget: &mut Budget<'work>,
    ) -> ContextResult<Option<Self>> {
        prior_denial_v2(budget)?;
        budget.charge_work(1)?;
        if source.body.is_none() {
            return Ok(None);
        }
        let floor = budget.storage();
        let slot = budget as *mut Budget<'_> as usize;
        let ledger = budget.work_ledger_identity_v1();
        let frame = context_frame_v2();
        budget.reserve_storage(frame)?;
        let result = catch_unwind(AssertUnwindSafe(|| -> ContextResult<_> {
            let flow = analyze_control_flow_with_byte_budget_v2(source, limits, budget)?;
            let index =
                ByteFunctionStateV2::build(source, budget)?.ok_or(ResourceError::Accounting)?;
            let origins_floor = budget.storage();
            let mut origins = Vec::new();
            let flow_view = flow.indexed_v2(source, budget)?;
            let lookup_work = lookup_work_v2(flow_view.block_count())?;
            {
                let mut meter = LiveGuardMeter::new(budget, usize::MAX, usize::MAX, usize::MAX);
                canonical_reads::collect_source_origins_v2(
                    &mut meter,
                    source,
                    flow_view,
                    &mut origins,
                    |meter, block| {
                        meter.charge(lookup_work)?;
                        Ok(flow_view.is_reachable(block))
                    },
                )?;
            }
            // Original rows contain only copied IDs and borrowed source Types.
            // The shared collector's input/SCC scratch has been dropped here.
            let origin_bytes = vector_bytes_v2(&origins)?;
            let scratch = budget
                .storage()
                .checked_sub(origins_floor)
                .and_then(|bytes| bytes.checked_sub(origin_bytes))
                .ok_or(ResourceError::Accounting)?;
            budget.release_storage(scratch)?;
            Ok((flow, index, origins))
        }));
        match result {
            Ok(Ok((flow, index, origins))) => {
                // Both nested owners bound their protected floors after this
                // envelope was paid. Retain it until the whole context drops.
                let retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ResourceError::Accounting)?;
                Ok(Some(Self {
                    source,
                    flow,
                    index,
                    origins,
                    slot,
                    ledger,
                    floor,
                    retained,
                    first_error: None,
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

    fn identity(&self, budget: &Budget<'_>) -> ContextResult<()> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            return Err(ResourceError::Accounting.into());
        }
        Ok(())
    }

    fn keep<T>(&mut self, result: ContextResult<T>) -> ContextResult<T> {
        if let Err(error) = &result {
            self.first_error.get_or_insert_with(|| error.clone());
        }
        result
    }

    fn check(&mut self, source: &Function, budget: &Budget<'_>) -> ContextResult<()> {
        // A foreign Budget does not mutate or retire the original context.
        self.identity(budget)?;
        if let Some(error) = &self.first_error {
            return Err(error.clone());
        }
        let result = (|| {
            prior_denial_v2(budget)?;
            if !std::ptr::eq(source, self.source)
                || budget.storage()
                    < self
                        .floor
                        .checked_add(self.retained)
                        .ok_or(ResourceError::Arithmetic)?
            {
                return Err(ResourceError::Accounting.into());
            }
            Ok(())
        })();
        self.keep(result)
    }

    pub(in crate::formal_memory_obligations) fn definition_rows_v2(
        &mut self,
        source: &Function,
        budget: &mut Budget<'_>,
    ) -> ContextResult<
        &[crate::VerificationNumericIndexRowV1<crate::VerificationDefinitionV1<'source>>],
    > {
        self.check(source, budget)?;
        match self.index.definition_rows_v2(source, budget) {
            Ok(rows) => Ok(rows),
            Err(error) => {
                let error = SourceContextErrorV2::from(error);
                self.first_error.get_or_insert_with(|| error.clone());
                Err(error)
            }
        }
    }

    pub(in crate::formal_memory_obligations) fn value_type(
        &mut self,
        source: &Function,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<&'source Type>> {
        self.check(source, budget)?;
        let result = self
            .index
            .definition(source, value, budget)
            .map(|row| row.map(|row| row.ty))
            .map_err(Into::into);
        self.keep(result)
    }

    pub(in crate::formal_memory_obligations) fn operation(
        &mut self,
        source: &Function,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<(&'source Operation, FunctionOperationLocation)>> {
        self.check(source, budget)?;
        let result = (|| {
            let Some(row) = self.index.definition(source, value, budget)? else {
                return Ok(None);
            };
            let VerificationDefinitionSiteV1::Operation(block, ordinal) = row.site else {
                return Ok(None);
            };
            let flow = self.flow.indexed_v2(source, budget)?;
            budget.charge_work(lookup_work_v2(flow.block_count())?)?;
            if !flow.is_reachable(block) {
                return Ok(None);
            }
            let block_row = self
                .index
                .block(source, block, budget)?
                .ok_or(ResourceError::Accounting)?;
            budget.charge_work(1)?;
            let operation = block_row
                .operations
                .get(ordinal)
                .ok_or(ResourceError::Accounting)?;
            Ok(Some((
                operation,
                FunctionOperationLocation::new(block, ordinal),
            )))
        })();
        self.keep(result)
    }

    // As in the original Definitions helper, a value not present in the phi
    // origin table maps to itself. This is not evidence that it is defined.
    pub(in crate::formal_memory_obligations) fn unique_origin(
        &mut self,
        source: &Function,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<ValueId>> {
        self.check(source, budget)?;
        let result =
            verification_find_last_by_v1(&self.origins, 1, budget, |row| row.value.cmp(&value))
                .map(|found| found.map_or(Some(value), |index| self.origins[index].origin))
                .map_err(Into::into);
        self.keep(result)
    }

    pub(in crate::formal_memory_obligations) fn reachable(
        &mut self,
        source: &Function,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<bool> {
        self.check(source, budget)?;
        let result = (|| {
            let flow = self.flow.indexed_v2(source, budget)?;
            budget.charge_work(lookup_work_v2(flow.block_count())?)?;
            Ok(flow.is_reachable(block))
        })();
        self.keep(result)
    }

    pub(in crate::formal_memory_obligations) fn block(
        &mut self,
        source: &Function,
        block: BlockId,
        budget: &mut Budget<'_>,
    ) -> ContextResult<Option<&'source crate::BasicBlock>> {
        self.check(source, budget)?;
        let result = self.index.block(source, block, budget).map_err(Into::into);
        self.keep(result)
    }

    pub(in crate::formal_memory_obligations) fn release(
        self,
        budget: &mut Budget<'_>,
    ) -> ContextResult<()> {
        self.identity(budget)?;
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
        budget.release_storage(retained).map_err(Into::into)
    }
}

fn lookup_work_v2(blocks: usize) -> std::result::Result<usize, ResourceError> {
    crate::verification_index_v1::verification_ceil_log2_v1(blocks)
        .checked_add(4)
        .ok_or(ResourceError::Arithmetic)
}

fn context_frame_v2() -> usize {
    type Owner = ByteSourceContextV2<'static, 'static>;
    type Rows = (
        ByteAccountedIndexedControlFlowV2<'static, 'static>,
        ByteFunctionStateV2<'static, 'static>,
        Vec<runtime_slice_read_v1::Origin<'static>>,
    );
    size_of::<Owner>()
        + size_of::<std::thread::Result<ContextResult<Rows>>>()
        + size_of::<ContextResult<Option<Owner>>>()
        + size_of::<LiveGuardMeter<'static, 'static>>()
        + size_of::<(&Function, ControlFlowLimits, &mut Budget<'static>)>()
        + size_of::<(&IndexedControlFlow, usize)>()
}

#[cfg(test)]
#[path = "source_context_bytes_v2_tests.rs"]
mod tests;
