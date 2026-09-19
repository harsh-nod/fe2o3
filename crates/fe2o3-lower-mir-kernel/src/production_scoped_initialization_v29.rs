// Source whole-local entry states from the existing CFG analysis. This does
// not prove alias-mediated reads, physical memory history or relocation safety.
#[derive(Clone, Copy, Eq, PartialEq)]
struct ScopedInitializationSubjectV29 {
    source: ExecutionCallSourceV29,
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl ScopedInitializationSubjectV29 {
    fn from_cursor(cursor: &ExecutionAvailabilityV29<'_>) -> Self {
        Self {
            source: cursor.source,
            instance: cursor.instance,
            function: cursor.function_id,
            ledger: cursor.ledger,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ScopedInitializedEntryV29 {
    block: SemanticBlockIdV1,
    initialized: std::ops::Range<usize>,
}

struct ScopedRetainedInitializationV29 {
    subject: ScopedInitializationSubjectV29,
    blocks: Vec<ScopedInitializedEntryV29>,
    initialized_locals: Vec<u32>,
    retained_storage: usize,
}

fn scoped_initialization_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "retained initialization differs from its scoped source instance",
    )
}

fn scoped_initialization_search_work_v29(count: usize) -> usize {
    (usize::BITS - count.leading_zeros()) as usize + 1
}

fn capture_scoped_initialization_v29(
    subject: ScopedInitializationSubjectV29,
    function: &SemanticFunctionDeclV1,
    ssa: &ProductionSemanticSsaFunctionPlanV1,
    slots: &BTreeMap<u32, SemanticRetainedLocalSlotV1>,
    entries: &BTreeMap<u32, BTreeSet<u32>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ScopedRetainedInitializationV29, ProductionSemanticKirErrorV1> {
    if subject.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    scoped_slot_attempt_v29(budget, |budget| {
        let floor = budget.storage();
        let rpo = ssa.plan().reverse_postorder();
        budget.charge_work(argument_sum_v1(&[entries.len(), 3])?)?;
        if rpo.first().map(|block| block.get()) != Some(function.entry().index())
            || if slots.is_empty() {
                !entries.is_empty()
            } else {
                entries.len() != rpo.len()
            }
        {
            return Err(scoped_initialization_error_v29());
        }
        let count = entries.values().try_fold(0_usize, |count, locals| {
            argument_sum_v1(&[count, locals.len()])
        })?;
        let mut blocks = emission_vec_v1(rpo.len(), budget)?;
        let mut initialized_locals = emission_vec_v1(count, budget)?;
        for block in rpo {
            budget.charge_work(scoped_initialization_search_work_v29(entries.len()))?;
            let start = initialized_locals.len();
            if !slots.is_empty() {
                let locals = entries
                    .get(&block.get())
                    .ok_or_else(scoped_initialization_error_v29)?;
                for &local in locals {
                    budget.charge_work(scoped_initialization_search_work_v29(slots.len()))?;
                    if !slots.contains_key(&local) {
                        return Err(scoped_initialization_error_v29());
                    }
                    initialized_locals.push(local);
                }
            }
            blocks.push(ScopedInitializedEntryV29 {
                block: SemanticBlockIdV1::from_index(block.get()),
                initialized: start..initialized_locals.len(),
            });
        }
        if initialized_locals.len() != count {
            return Err(scoped_initialization_error_v29());
        }
        Ok(ScopedRetainedInitializationV29 {
            subject,
            blocks,
            initialized_locals,
            retained_storage: budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        })
    })
}

impl ScopedRetainedInitializationV29 {
    // Custody and shape checks; the entry facts come from the single original
    // analysis, not from this validation or a second dataflow reconstruction.
    fn check_custody(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        id: ProductionCallInstanceIdV1,
        events: &PendingLifecycleEventsV29,
        origins: &[ScopedSlotOriginV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.subject.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(7)?;
        let instance = instances
            .instance(id)
            .ok_or_else(scoped_initialization_error_v29)?;
        if self.subject.instance != id
            || self.subject.function != instance.function()
            || self.subject.source != ExecutionCallSourceV29::from_instances(instances, budget)?
            || self.subject.source != events.source
            || self.subject.instance != events.instance
            || self.subject.function != events.function
            || self.subject.ledger != events.ledger
        {
            return Err(scoped_initialization_error_v29());
        }
        let rpo = instance.ssa().plan().reverse_postorder();
        if self.blocks.len() != rpo.len() {
            return Err(scoped_initialization_error_v29());
        }
        budget.charge_work(origins.len())?;
        if origins
            .windows(2)
            .any(|pair| pair[0].local >= pair[1].local)
        {
            return Err(scoped_initialization_error_v29());
        }
        let bytes = argument_sum_v1(&[
            argument_product_v1(
                self.blocks.capacity(),
                std::mem::size_of::<ScopedInitializedEntryV29>(),
            )?,
            argument_product_v1(
                self.initialized_locals.capacity(),
                std::mem::size_of::<u32>(),
            )?,
        ])?;
        if self.retained_storage != bytes {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut end = 0;
        for (row, block) in self.blocks.iter().zip(rpo) {
            budget.charge_work(4)?;
            if row.block.index() != block.get() || row.initialized.start != end {
                return Err(scoped_initialization_error_v29());
            }
            let locals = self
                .initialized_locals
                .get(row.initialized.clone())
                .ok_or_else(scoped_initialization_error_v29)?;
            let mut previous = None;
            for &local in locals {
                budget.charge_work(scoped_initialization_search_work_v29(origins.len()))?;
                if previous.is_some_and(|prior| prior >= local)
                    || origins
                        .binary_search_by_key(&local, |origin| origin.local)
                        .is_err()
                {
                    return Err(scoped_initialization_error_v29());
                }
                previous = Some(local);
            }
            end = row.initialized.end;
        }
        if end != self.initialized_locals.len() {
            return Err(scoped_initialization_error_v29());
        }
        Ok(())
    }
}
