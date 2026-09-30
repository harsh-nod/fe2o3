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
// Source-level policy sharing preserves the original preparation call graph.
// These private item macros introduce no closure, thunk, callback or runtime
// helper on the existing PreparationResources arm. Fixed invocations below
// are source-audited against the complete original methods.
macro_rules! preparation_reserve_policy_item_v1 {
    (
        ($($declaration:tt)*)
        ($meter:ident, $values:ident, $additional:ident, $element:ident)
        ($is_metered:ident, $work:ident, $storage:ident)
        ($($admission:tt)*)
    ) => {
        $($declaration)* {
            $($admission)*
        let requested = $values
            .len()
            .checked_add($additional)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if requested <= $values.capacity() {
            return Ok(());
        }
        if $meter.$is_metered() {
            // Full next capacity is admitted while all prior growth remains
            // owned. Relocation and initialization never use refunded scratch.
            $meter.$work($values.len())?;
            $meter.$storage(
                requested
                    .checked_mul(size_of::<$element>())
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )?;
            $values
                .try_reserve_exact($additional)
                .map_err(|_| resource(Resource::Allocation))?;
            if size_of::<$element>() != 0 && $values.capacity() != requested {
                return Err(resource(Resource::Allocation));
            }
        } else {
            // Legacy loops retain amortized growth, not cumulative exact growth.
            $values
                .try_reserve($additional)
                .map_err(|_| resource(Resource::Allocation))?;
        }
        Ok(())
        }
    };
}
macro_rules! preparation_push_policy_item_v1 {
    (
        ($($declaration:tt)*)
        ($meter:ident, $values:ident, $value:ident, $work:ident)
        ($($reserve:tt)*)
        ($($admission:tt)*)
    ) => {
        $($declaration)* {
            $($admission)*
        $meter.$work(1)?;
        $($reserve)*?;
        $values.push($value);
        Ok(())
        }
    };
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
    preparation_reserve_policy_item_v1! {
        (pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), Error>)
        (self, values, additional, T)
        (is_metered, work, reserve_storage)
        ()
    }
    preparation_push_policy_item_v1! {
        (pub(super) fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error>)
        (self, values, value, work)
        (self.reserve(values, 1))
        ()
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

// Only these two in-crate resource representations can implement the policy.
// This seal grants no source, owner-counter or Budget access.
mod preparation_policy_sealed {
    pub trait Sealed {}
    impl Sealed for super::PreparationResourcesV1<'_, '_> {}
    impl Sealed for super::super::assertion_resources_v1::AssertionResourcesV1<'_> {}
}
pub(super) trait PreparationPolicyMeterV1: preparation_policy_sealed::Sealed {
    fn preparation_admit_v1(&self) -> Result<(), Error>;
    fn preparation_is_metered_v1(&self) -> bool;
    fn preparation_work_v1(&mut self, amount: usize) -> Result<(), Error>;
    fn preparation_storage_v1(&mut self, amount: usize) -> Result<(), Error>;
}
impl PreparationPolicyMeterV1 for PreparationResourcesV1<'_, '_> {
    fn preparation_admit_v1(&self) -> Result<(), Error> {
        Ok(())
    }
    fn preparation_is_metered_v1(&self) -> bool {
        self.is_metered()
    }
    fn preparation_work_v1(&mut self, amount: usize) -> Result<(), Error> {
        self.work(amount)
    }
    fn preparation_storage_v1(&mut self, amount: usize) -> Result<(), Error> {
        self.reserve_storage(amount)
    }
}

// Exact extracted preparation policy; admission belongs to each existing entry.
preparation_reserve_policy_item_v1! {
    (pub(super) fn preparation_reserve_with_meter_v1<T, M: PreparationPolicyMeterV1>(
    meter: &mut M, values: &mut Vec<T>, additional: usize,
) -> Result<(), Error>)
    (meter, values, additional, T)
    (preparation_is_metered_v1, preparation_work_v1, preparation_storage_v1)
    (meter.preparation_admit_v1()?;)
}

preparation_push_policy_item_v1! {
    (pub(super) fn preparation_push_with_meter_v1<T, M: PreparationPolicyMeterV1>(
    meter: &mut M, values: &mut Vec<T>, value: T,
) -> Result<(), Error>)
    (meter, values, value, preparation_work_v1)
    (preparation_reserve_with_meter_v1(meter, values, 1))
    (meter.preparation_admit_v1()?;)
}

/// Read-only lexical custody census. Not authority and never a Budget loan.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct PreparationCustodySnapshotV1 {
    pub(super) budget_slot: usize,
    pub(super) work_ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    pub(super) owned_slot: usize,
    pub(super) owned: usize,
    pub(super) storage: usize,
    pub(super) work: usize,
    pub(super) peak: usize,
    pub(super) denied_work: bool,
    pub(super) denied_storage: bool,
}
impl PreparationResourcesV1<'_, '_> {
    pub(super) fn retained_custody_snapshot_v1(&self) -> Option<PreparationCustodySnapshotV1> {
        match &self.ledger {
            Ledger::Legacy => None,
            Ledger::Original { budget, owned } => {
                let budget: &Budget<'_> = budget;
                let owned: &usize = &**owned;
                Some(PreparationCustodySnapshotV1 {
                    budget_slot: budget as *const Budget<'_> as usize,
                    work_ledger: budget.work_ledger_identity_v1(),
                    owned_slot: owned as *const usize as usize,
                    owned: *owned,
                    storage: budget.storage(),
                    work: budget.work(),
                    peak: budget.peak_storage(),
                    denied_work: budget.failed_work().is_some(),
                    denied_storage: budget.failed_storage().is_some(),
                })
            }
        }
    }
}

pub(super) fn retained_custody_snapshot_frame_v1() -> usize {
    size_of::<(
        &PreparationResourcesV1<'static, 'static>,
        &Ledger<'static, 'static>,
        &&mut Budget<'static>,
        &&mut usize,
        &Budget<'static>,
        &usize,
        *const Budget<'static>,
        *const usize,
        PreparationCustodySnapshotV1,
        Option<PreparationCustodySnapshotV1>,
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        usize,
        usize,
        usize,
        usize,
        usize,
        Option<usize>,
        Option<usize>,
        bool,
        bool,
    )>()
}

impl PreparationResourcesV1<'_, '_> {
    /// Domain-specific operation only: no generic original-Budget callback or loan.
    pub(super) fn prepare_shared_reads<'s>(
        &mut self,
        pending: &mut fe2o3_pliron::ProductionSemanticSharedReadsPreparationV1<'s>,
        owner: &'s fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        function: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    ) -> Result<(), Error> {
        match &mut self.ledger {
            Ledger::Original { budget, owned } => pending
                .prepare_into(owner, function, budget, owned)
                .map_err(super::canonical_assertion_facts_v1::shared_read_error_v1),
            Ledger::Legacy => Err(resource(Resource::Accounting)),
        }
    }
    pub(super) fn check_shared_reads(
        &self,
        pending: &fe2o3_pliron::ProductionSemanticSharedReadsPreparationV1<'_>,
        owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        function: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    ) -> Result<(), Error> {
        match &self.ledger {
            Ledger::Original { budget, owned } => pending
                .completed_for(owner, function, budget, owned)
                .map(|_| ())
                .map_err(super::canonical_assertion_facts_v1::shared_read_error_v1),
            Ledger::Legacy => Err(resource(Resource::Accounting)),
        }
    }
}

/// Source-level frame for the fixed Shared operations below. Keeping the
/// private match carriers here exposes neither Ledger nor an original-Budget loan.
pub(super) fn retained_shared_frame_v1() -> usize {
    size_of::<(
        Ledger<'static, 'static>,
        &mut Ledger<'static, 'static>,
        &Ledger<'static, 'static>,
        &mut &mut Budget<'static>,
        &&mut Budget<'static>,
        &mut &mut usize,
        &&mut usize,
        &mut Budget<'static>,
        &Budget<'static>,
        &mut usize,
        &usize,
    )>()
}
