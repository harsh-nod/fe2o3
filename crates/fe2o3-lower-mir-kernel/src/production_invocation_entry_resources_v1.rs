fn invocation_entry_plan_bytes_v1() -> usize {
    std::mem::size_of::<InvocationEntryPlanV1<'_>>()
}

fn derive_paid_invocation_entry_plan_v1<'source>(
    source: &'source SemanticFunctionDeclV1,
    ssa: &'source ProductionSemanticSsaFunctionPlanV1,
    placement: SemanticEmissionPlacementV1,
    has_runtime_trap: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<InvocationEntryPlanV1<'source>, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(invocation_entry_plan_bytes_v1())?;
    InvocationEntryPlanV1::derive(source, ssa, placement, has_runtime_trap, budget)
}

// Audited callbacks may return paid owned outputs but cannot retain the borrowed
// plan. The existing attempt scope drops failed outputs before restoring floor.
fn with_invocation_entry_plan_v1<R>(
    source: &SemanticFunctionDeclV1,
    ssa: &ProductionSemanticSsaFunctionPlanV1,
    placement: SemanticEmissionPlacementV1,
    has_runtime_trap: bool,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl FnOnce(
        &InvocationEntryPlanV1<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let bytes = invocation_entry_plan_bytes_v1();
    let required = argument_sum_v1(&[floor, bytes])?;
    let ledger = budget.work_ledger_identity_v1();
    scoped_slot_attempt_v29(budget, |budget| {
        let plan =
            derive_paid_invocation_entry_plan_v1(source, ssa, placement, has_runtime_trap, budget)?;
        let result = consume(&plan, budget)?;
        if budget.work_ledger_identity_v1() != ledger || budget.storage() < required {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        drop(plan);
        budget.release_storage(bytes)?;
        Ok(result)
    })
}

// The resumable constructor settles this actual plan before capturing a frame
// seal. No previously captured seal is adjusted for an assumed header refund.
mod invocation_entry_plan_owner_v1 {
    use super::*;

    pub(super) struct OwnedInvocationEntryPlanV1<'source> {
        plan: InvocationEntryPlanV1<'source>,
        source: Option<&'source SourceReferencePlanV29<'source, 'source>>,
        growth: Option<EmissionServiceGrowthV1<'source>>,
        entry: usize,
        required: usize,
        owned: usize,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    }

    pub(super) fn owned_invocation_entry_plan_headers_v1()
    -> Result<usize, ProductionSemanticKirErrorV1> {
        argument_sum_v1(&[
            std::mem::size_of::<OwnedInvocationEntryPlanV1<'static>>(),
            std::mem::size_of::<
                Result<OwnedInvocationEntryPlanV1<'static>, ProductionSemanticKirErrorV1>,
            >(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        ])
        .map_err(Into::into)
    }

    impl<'source> OwnedInvocationEntryPlanV1<'source> {
        pub(super) fn new(
            source: &'source SemanticFunctionDeclV1,
            ssa: &'source ProductionSemanticSsaFunctionPlanV1,
            placement: SemanticEmissionPlacementV1,
            has_runtime_trap: bool,
            cursor: Option<&ExecutionAvailabilityV29<'source>>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            scoped_slot_attempt_v29(budget, |budget| {
                let entry = budget.storage();
                let owned = owned_invocation_entry_plan_headers_v1()?;
                // The shared primitive pays for the embedded plan itself.
                budget.reserve_storage(
                    owned
                        .checked_sub(invocation_entry_plan_bytes_v1())
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )?;
                budget.charge_work(4 + 4)?;
                if let Some(cursor) = cursor {
                    cursor.check_ledger(budget)?;
                    if !std::ptr::eq(cursor.function, source) || !std::ptr::eq(cursor.ssa, ssa) {
                        return Err(emission_service_error_v1());
                    }
                }
                let retained_source = cursor
                    .and_then(|cursor| cursor.references)
                    .map(|references| references.plan);
                let growth = capture_emission_service_growth_v1(cursor, budget)?;
                let plan = derive_paid_invocation_entry_plan_v1(
                    source,
                    ssa,
                    placement,
                    has_runtime_trap,
                    budget,
                )?;
                let required = argument_sum_v1(&[entry, owned])?;
                if budget.storage() != required {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok(Self {
                    plan,
                    source: retained_source,
                    growth,
                    entry,
                    required,
                    owned,
                    slot: budget as *const ArgumentBudgetV1<'_> as usize,
                    ledger: budget.work_ledger_identity_v1(),
                })
            })
        }

        pub(super) fn plan(&self) -> &InvocationEntryPlanV1<'source> {
            &self.plan
        }

        pub(super) fn settle(
            self,
            budget: &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            let allowed = budget.permits_prepared_input_refund_v1(
                self.source,
                self.slot,
                self.ledger,
                self.required,
                self.owned,
            ) && self.growth.as_ref().is_none_or(|growth| {
                growth.permits_refund(self.entry, self.required, budget.storage(), self.owned)
            });
            let owned = self.owned;
            let source = self.source;
            // In particular the source-borrowing plan is gone before its credit.
            drop(self);
            if !allowed {
                if let Some(root) = source.and_then(|plan| plan.storage_root.as_ref()) {
                    root.deny_active_root_refund();
                }
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if let Err(error) = budget.release_storage(owned) {
                if let Some(root) = source.and_then(|plan| plan.storage_root.as_ref()) {
                    root.deny_active_root_refund();
                }
                return Err(error.into());
            }
            Ok(())
        }
    }
}

use invocation_entry_plan_owner_v1::OwnedInvocationEntryPlanV1;
