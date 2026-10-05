// Private prerequisite: typed vector/header bytes, not legacy logical row cells
// and not a claim about whole-process RSS or unmodelled compiler stack frames.
#[cfg(test)]
use crate::verification_typed_storage_v2::admit_capacity_v2 as admit_control_flow_capacity_v2;
use crate::verification_typed_storage_v2::{
    allocate_vector_v2 as allocate_control_flow_bytes_v2,
    prior_denial_v2 as control_flow_prior_denial_v2,
    vector_bytes_v2 as control_flow_vector_bytes_v2,
};

#[must_use = "dropping the CFG without release retains its resource charge"]
pub(crate) struct ByteAccountedIndexedControlFlowV2<'source, 'work> {
    source: &'source Function,
    flow: IndexedControlFlow,
    slot: usize,
    ledger: crate::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    retained_bytes: usize,
    work_borrow: std::marker::PhantomData<&'work crate::CanonicalKernelIrWorkBudgetV1>,
}

impl ByteAccountedIndexedControlFlowV2<'_, '_> {
    fn same_identity_v2(
        &self,
        budget: &CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if self.slot != budget as *const CanonicalKernelIrVerificationResourceBudgetV1<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
        {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        }
        Ok(())
    }

    fn same_account_v2(
        &self,
        budget: &CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        use CanonicalKernelIrVerificationResourceErrorV1 as Error;
        self.same_identity_v2(budget)?;
        if budget.storage()
            < self
                .floor
                .checked_add(self.retained_bytes)
                .ok_or(Error::Arithmetic)?
        {
            return Err(Error::Accounting);
        }
        Ok(())
    }

    // This borrows the exact source graph's derived index. Each consuming
    // lookup/traversal must still charge the same budget; no proof is issued.
    pub(crate) fn indexed_v2(
        &self,
        source: &Function,
        budget: &CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<&IndexedControlFlow, CanonicalKernelIrVerificationResourceErrorV1> {
        self.same_identity_v2(budget)?;
        control_flow_prior_denial_v2(budget)?;
        self.same_account_v2(budget)?;
        if !std::ptr::eq(source, self.source) {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        }
        Ok(&self.flow)
    }

    pub(crate) fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        self.same_account_v2(budget)?;
        let bytes = self.retained_bytes;
        drop(self);
        budget.release_storage(bytes)
    }
}

fn control_flow_frame_bytes_v2() -> usize {
    type Owner = ByteAccountedIndexedControlFlowV2<'static, 'static>;
    std::mem::size_of::<Owner>()
        + std::mem::size_of::<
            std::thread::Result<Result<IndexedControlFlow, MeteredControlFlowErrorV1>>,
        >()
        + std::mem::size_of::<Result<Owner, MeteredControlFlowErrorV1>>()
        + std::mem::size_of::<ControlFlowResourcesV1<'static, 'static>>()
        + std::mem::size_of::<(
            &Function,
            ControlFlowLimits,
            &mut CanonicalKernelIrVerificationResourceBudgetV1<'static>,
        )>()
}

pub(crate) fn analyze_control_flow_with_byte_budget_v2<'source, 'work>(
    function: &'source Function,
    limits: ControlFlowLimits,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'work>,
) -> Result<ByteAccountedIndexedControlFlowV2<'source, 'work>, MeteredControlFlowErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    control_flow_prior_denial_v2(budget)?;
    let floor = budget.storage_checkpoint();
    let slot = budget as *mut CanonicalKernelIrVerificationResourceBudgetV1<'_> as usize;
    let ledger = budget.work_ledger_identity_v1();
    budget.charge_work(1)?;
    let frame = control_flow_frame_bytes_v2();
    budget.reserve_storage(frame)?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        analyze_control_flow_shared_v1(
            function,
            limits,
            &mut ControlFlowResourcesV1 {
                budget: Some(budget),
                storage: ControlFlowStorageV2::TypedBytes,
            },
        )
    }));
    match result {
        Ok(Ok(flow)) => {
            budget.release_storage(
                frame - std::mem::size_of::<ByteAccountedIndexedControlFlowV2<'_, '_>>(),
            )?;
            let retained_bytes = budget
                .storage()
                .checked_sub(floor)
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            Ok(ByteAccountedIndexedControlFlowV2 {
                source: function,
                flow,
                slot,
                ledger,
                floor,
                retained_bytes,
                work_borrow: std::marker::PhantomData,
            })
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
