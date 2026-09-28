// Only this short-lived index is owned here. Rebuilt plan rows remain paid by
// their original producers, even when a later memo operation fails.
struct SourceReferenceMemoV29 {
    rows: BTreeMap<usize, usize>,
    owned: usize,
}

impl SourceReferenceMemoV29 {
    fn new() -> Self {
        Self {
            rows: BTreeMap::new(),
            owned: std::mem::size_of::<BTreeMap<usize, usize>>(),
        }
    }

    fn len(&self) -> usize {
        self.rows.len()
    }

    fn get(&self, node: &usize) -> Option<&usize> {
        self.rows.get(node)
    }

    fn insert(
        &mut self,
        node: usize,
        value: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        let bytes = execution_cfg_map_entry_storage_v29::<usize, usize>(self.rows.len())?;
        let owned = argument_sum_v1(&[self.owned, bytes])?;
        // Preserve the original insertion's lookup charge and single reserve.
        reserve_execution_cfg_map_entry_v29::<usize, usize>(self.rows.len(), budget)?;
        self.owned = owned;
        self.rows.insert(node, value);
        Ok(())
    }
}

fn source_reference_memo_headers_v29<F>(run: &F) -> Result<usize, ArgumentResourceV1> {
    use std::mem::size_of;
    type Output = Result<Option<usize>, ProductionSemanticKirErrorV1>;
    type Capture<'b, 'a, 'root, 'source, 'work> = (
        &'b mut SourceReferenceBuilderV29<'a, 'root, 'source>,
        &'b mut SourceReferenceMemoV29,
        &'b mut ArgumentBudgetV1<'work>,
        &'b Option<source_storage_v29::SourceStorageRootCustodyViewV29<'a, 'source>>,
        &'b mut Option<source_storage_v29::SourceStorageRootGrowthV29<'b, 'a, 'source>>,
    );
    argument_sum_v1(&[
        size_of::<SourceReferenceMemoV29>(),
        argument_product_v1(2, std::mem::size_of_val(run))?,
        argument_product_v1(2, std::mem::align_of_val(run))?,
        size_of::<Capture<'_, '_, '_, '_, '_>>(),
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, '_, '_, '_>>>(),
        size_of::<Option<source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>(),
        size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>(),
        size_of::<ArgumentLedgerV1>(),
        argument_product_v1(12, size_of::<usize>())?,
        argument_product_v1(3, size_of::<Option<usize>>())?,
        argument_product_v1(2, size_of::<Output>())?,
        size_of::<std::thread::Result<Output>>(),
        size_of::<&std::thread::Result<Output>>(),
        size_of::<Option<ProductionSemanticKirErrorV1>>(),
        size_of::<&ProductionSemanticKirErrorV1>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
        size_of::<bool>(),
        // Scoped refund arguments and its fixed arithmetic/permission results.
        size_of::<(
            Option<&SourceReferencePlanV29<'_, '_>>,
            ArgumentLedgerV1,
            usize,
            usize,
            usize,
            Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
            Option<usize>,
            &mut dyn SemanticEmissionBudgetV1,
        )>(),
        size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
    ])
}

impl<'a, 'root, 'source> SourceReferenceBuilderV29<'a, 'root, 'source> {
    fn with_memo_v29<'work>(
        &mut self,
        budget: &mut ArgumentBudgetV1<'work>,
        run: impl FnOnce(
            &mut Self,
            &mut SourceReferenceMemoV29,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<Option<usize>, ProductionSemanticKirErrorV1>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        let root = self.storage_root.as_ref().map(|root| root.custody_view());
        budget.source_reference_charge_v29(
            &self.plan,
            argument_sum_v1(&[
                12,
                if root.is_some() {
                    source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29
                } else {
                    0
                },
            ])?,
        )?;
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *const ArgumentBudgetV1<'_> as usize;
        let headers = source_reference_memo_headers_v29(&run)?;
        let preserved = argument_sum_v1(&[floor, headers])?
            .checked_sub(std::mem::size_of::<BTreeMap<usize, usize>>())
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.source_reference_reserve_v29(&self.plan, headers)?;
        let mut memo = SourceReferenceMemoV29::new();
        let mut growth = None;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(root) = &root {
                growth = Some(
                    root.capture_retained_growth()
                        .ok_or(ArgumentResourceV1::Accounting)?,
                );
            }
            run(self, &mut memo, budget)
        }));
        let owned = memo.owned;
        drop(memo);
        if let Ok(Err(error)) = &caught {
            source_reference_record_failure_v29(&self.plan, error);
        }
        let first = self.plan.failure.first_error();
        // Result/catch/caller frames remain live. Only the destroyed map header
        // and its exact accepted split allowances leave this attempt's floor.
        let required = preserved.checked_add(owned);
        let bytes = budget
            .storage()
            .checked_sub(owned)
            .filter(|after| *after >= preserved)
            .and(required)
            .map(|_| owned);
        let settled = scoped_emission_refund_v29(
            Some(&self.plan),
            ledger,
            slot,
            preserved,
            required.unwrap_or(usize::MAX),
            growth,
            bytes,
            budget,
        );
        if !settled {
            if let Some(root) = &root {
                root.deny_active_root_refund();
            }
            // An earlier selected owned error must reach the outer checkpoint
            // before any later Accounting observation can occupy its first slot.
            if matches!(&caught, Ok(Ok(_))) {
                self.plan
                    .failure
                    .record_resource(ArgumentResourceV1::Accounting);
            }
        }
        match caught {
            Ok(Err(error)) => Err(first.unwrap_or(error)),
            Ok(Ok(value)) => match self.plan.failure.first_error() {
                Some(error) => Err(error),
                None => Ok(value),
            },
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
