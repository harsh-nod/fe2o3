// These credits own only the temporary resolver's query frames and maps.
// No carrier/type output allocation is routed through this record.
#[derive(Clone, Copy)]
struct CapabilityOriginCreditV29 {
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    entry: usize,
    bytes: usize,
}

struct CapabilityOriginStorageV29<'source> {
    source: Option<&'source SourceReferencePlanV29<'source, 'source>>,
    growth: Option<EmissionServiceGrowthV1<'source>>,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    entry: usize,
}

#[cfg(test)]
std::thread_local! {
    static CAPABILITY_ORIGIN_SETTLEMENT_V29: std::cell::Cell<[usize; 5]> = const {
        std::cell::Cell::new([0; 5])
    };
}

fn capability_origin_storage_headers_v29() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            std::mem::size_of::<Result<T, ArgumentResourceV1>>(),
            std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
        ])
    }
    type Start<'a, 's> = (
        &'a mut SemanticCapabilityOriginResolverV1<'a>,
        &'a ExecutionAvailabilityV29<'s>,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type Reserve<'a> = (
        &'a mut SemanticCapabilityOriginResolverV1<'a>,
        usize,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type Finish<'a> = (
        CapabilityOriginStorageV29<'a>,
        SemanticCapabilityOriginResolverV1<'a>,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type Check<'a> = (
        &'a CapabilityOriginCreditV29,
        &'a dyn SemanticEmissionBudgetV1,
    );
    type Capture<'a, 's> = (
        Option<&'a ExecutionAvailabilityV29<'s>>,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type GrowthCheck<'a> = (
        &'a EmissionServiceGrowthV1<'a>,
        &'a CapabilityOriginStorageV29<'a>,
        &'a CapabilityOriginCreditV29,
        &'a dyn SemanticEmissionBudgetV1,
    );
    type Permit<'a> = (
        &'a dyn SemanticEmissionBudgetV1,
        Option<&'a SourceReferencePlanV29<'a, 'a>>,
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        usize,
        usize,
    );
    type Release<'a> = (
        &'a mut dyn SemanticEmissionBudgetV1,
        Option<&'a SourceReferencePlanV29<'a, 'a>>,
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        usize,
        usize,
    );
    argument_sum_v1(&[
        h::<CapabilityOriginCreditV29>()?,
        h::<Option<CapabilityOriginCreditV29>>()?,
        h::<&CapabilityOriginCreditV29>()?,
        h::<Option<&CapabilityOriginCreditV29>>()?,
        h::<CapabilityOriginStorageV29<'_>>()?,
        h::<Option<CapabilityOriginStorageV29<'_>>>()?,
        h::<Start<'_, '_>>()?,
        h::<Reserve<'_>>()?,
        h::<Finish<'_>>()?,
        h::<Check<'_>>()?,
        h::<Capture<'_, '_>>()?,
        h::<GrowthCheck<'_>>()?,
        h::<Permit<'_>>()?,
        h::<Release<'_>>()?,
        h::<&CapabilityOriginStorageV29<'_>>()?,
        h::<&dyn SemanticEmissionBudgetV1>()?,
        h::<Option<&SourceReferencePlanV29<'_, '_>>>()?,
        h::<&SourceReferencePlanV29<'_, '_>>()?,
        h::<Option<EmissionServiceGrowthV1<'_>>>()?,
        h::<&EmissionServiceGrowthV1<'_>>()?,
        h::<Option<&EmissionServiceGrowthV1<'_>>>()?,
        h::<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()?,
        h::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()?,
        h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()?,
        h::<Option<usize>>()?,
        h::<[usize; 6]>()?,
        h::<&[usize]>()?,
        h::<[usize; 2]>()?,
        h::<bool>()?,
        h::<()>()?,
    ])
}

impl CapabilityOriginCreditV29 {
    fn check(
        &self,
        budget: &dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if budget.emission_service_slot_v1() != Some(self.slot)
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() < argument_sum_v1(&[self.entry, self.bytes])?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
}

impl SemanticCapabilityOriginResolverV1<'_> {
    fn reserve_scoped_storage_v29(
        &mut self,
        amount: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(mut credit) = self.scoped_storage else {
            return budget.reserve_storage(amount);
        };
        credit.check(budget)?;
        budget.charge_work(8)?;
        let bytes = argument_sum_v1(&[credit.bytes, amount])?;
        argument_sum_v1(&[credit.entry, bytes])?;
        budget.reserve_storage(amount)?;
        credit.bytes = bytes;
        self.scoped_storage = Some(credit);
        Ok(())
    }
}

impl<'source> CapabilityOriginStorageV29<'source> {
    fn new(
        resolver: &mut SemanticCapabilityOriginResolverV1<'_>,
        cursor: &ExecutionAvailabilityV29<'source>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        cursor.check_ledger(budget)?;
        if resolver.scoped_storage.is_some() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let slot = budget
            .emission_service_slot_v1()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let ledger = budget.work_ledger_identity_v1();
        let source = cursor.references.map(|references| references.plan);
        let entry = budget.storage();
        let bytes = capability_origin_storage_headers_v29()?;
        argument_sum_v1(&[entry, bytes])?;
        // Fixed entrance/settlement checks; cleanup spends no new work.
        budget.charge_work(12)?;
        budget.reserve_storage(bytes)?;
        let growth = capture_emission_service_growth_v1(Some(cursor), budget)?;
        resolver.scoped_storage = Some(CapabilityOriginCreditV29 {
            slot,
            ledger,
            entry,
            bytes,
        });
        Ok(Self {
            source,
            growth,
            slot,
            ledger,
            entry,
        })
    }

    fn finish(
        self,
        resolver: SemanticCapabilityOriginResolverV1<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let credit = resolver
            .scoped_storage
            .ok_or(ArgumentResourceV1::Accounting)?;
        let required = argument_sum_v1(&[credit.entry, credit.bytes])?;
        let allowed = credit.slot == self.slot
            && credit.ledger == self.ledger
            && credit.entry == self.entry
            && credit.check(budget).is_ok()
            && budget.permits_prepared_input_refund_v1(
                self.source,
                self.slot,
                self.ledger,
                required,
                credit.bytes,
            )
            && self.growth.as_ref().is_none_or(|growth| {
                growth.permits_refund(self.entry, required, budget.storage(), credit.bytes)
            });
        let source = self.source;
        // The maps and all recursive/query captures are dead before settlement.
        drop(resolver);
        drop(self);
        if !allowed {
            if let Some(root) = source.and_then(|plan| plan.storage_root.as_ref()) {
                root.deny_active_root_refund();
            }
            return Err(ArgumentResourceV1::Accounting.into());
        }
        #[cfg(test)]
        let before = budget.storage();
        budget.release_emission_service_storage_v1(
            source,
            credit.slot,
            credit.ledger,
            required,
            credit.bytes,
        )?;
        #[cfg(test)]
        CAPABILITY_ORIGIN_SETTLEMENT_V29.with(|observed| {
            let [count, total, ..] = observed.get();
            observed.set([
                count + 1,
                total + credit.bytes,
                before,
                budget.storage(),
                credit.bytes,
            ]);
        });
        Ok(())
    }
}
