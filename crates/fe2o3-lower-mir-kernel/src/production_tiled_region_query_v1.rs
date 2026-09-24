use super::*;

/// Keeps any panic payload alive until the caller has dropped its own payload
/// and restored ONLY its accepted reservation. A callback may retain additional
/// inert storage, but may not replace the ledger or undercut the entry floor.
pub(super) struct Outcome<R> {
    pub result: Result<R>,
    pub valid: bool,
    pub panic: Option<Box<dyn std::any::Any + Send>>,
}
pub(super) fn callback<'work, R>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<R>,
) -> Outcome<R> {
    let slot = budget as *mut _ as usize;
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let (result, panic) = match catch_unwind(AssertUnwindSafe(|| {
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        let result = run(budget);
        if result.is_ok() && (budget.failed_work().is_some() || budget.failed_storage().is_some()) {
            drop(result);
            return Err(Resource::Accounting.into());
        }
        result
    })) {
        Ok(result) => (result, None),
        Err(payload) => (Err(Error::CallbackPanicked), Some(payload)),
    };
    let valid = budget as *mut _ as usize == slot
        && budget.work_ledger_identity_v1() == ledger
        && budget.storage() >= floor;
    Outcome {
        result,
        valid,
        panic,
    }
}

impl Capture {
    pub(super) fn prepare(
        owner: &ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        Self::allocate(budget, |budget| Recorder::prepare(owner, budget))
    }
    pub(super) fn allocate(
        budget: &mut Budget<'_>,
        prepare: impl FnOnce(&mut Budget<'_>) -> Result<Recorder>,
    ) -> Result<Self> {
        // Pay heap payload, constructor coexistence scratch and the small
        // handle before allocation or fixed-table initialization. The private
        // enum slot in legacy materialization remains two machine words.
        let payload = size_of::<Recorder>();
        budget.reserve_storage(
            payload
                .checked_mul(2)
                .and_then(|v| v.checked_add(size_of::<Self>()))
                .and_then(|v| v.checked_add(size_of::<Vec<Recorder>>()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        budget.charge_work(ALIASES + USES + 12)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(1)
            .map_err(|_| Resource::Allocation)?;
        if rows.capacity() != 1 {
            drop(rows);
            return unavailable("recorder allocator capacity differs");
        }
        rows.push(prepare(budget)?);
        let rows = rows.into_boxed_slice(); // len == capacity, no shrinking allocation
        budget.release_storage(payload + size_of::<Vec<Recorder>>())?;
        Ok(Self { rows })
    }
    pub(in super::super) fn record_function(
        &mut self,
        plan: &LoweredFunctionPlanV1,
        lowering: &SemanticFunctionLoweringV1<'_>,
        blocks: &[BasicBlock],
        spans: &[SemanticKirTerminatorOperationSpanV1],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.rows[0].record_function(plan, lowering, blocks, spans, budget)
    }
    pub(super) fn recorder(&self) -> &Recorder {
        &self.rows[0]
    }
    pub(super) fn recorder_mut(&mut self) -> &mut Recorder {
        &mut self.rows[0]
    }
}
