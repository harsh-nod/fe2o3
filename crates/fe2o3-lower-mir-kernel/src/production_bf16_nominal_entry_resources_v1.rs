// Strict original-entry resource scope for NEW nominal consumers. This lends
// only the caller's Budget, not a checked source view or continuation authority.
// The old N1 query and its work/storage/custody behavior remain unchanged.
const BF16_NOMINAL_ENTRY_WORK_V1: usize = 32;
const BF16_NOMINAL_ENTRY_FIXED_FRAME_V1: usize = 8192;

#[derive(Clone, Copy)]
struct Bf16NominalEntryCheckpointV1 {
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    storage: usize,
    work: usize,
    peak: usize,
}
impl Bf16NominalEntryCheckpointV1 {
    fn take(budget: &ArgumentBudgetV1<'_>) -> Self {
        Self {
            slot: budget as *const ArgumentBudgetV1<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            storage: budget.storage(),
            work: budget.work(),
            peak: budget.peak_storage(),
        }
    }
    fn check(self, budget: &ArgumentBudgetV1<'_>, owned: usize) -> Bf16CallQueryResultV1<()> {
        let protected = self
            .storage
            .checked_add(owned)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if self.slot != budget as *const ArgumentBudgetV1<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < protected
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
}

fn bf16_nominal_entry_frame_v1<R>(callback_bytes: usize) -> Bf16CallQueryResultV1<usize> {
    let size = |bytes: usize| bytes.checked_mul(4).ok_or(ArgumentResourceV1::Arithmetic);
    Ok(argument_sum_v1(&[
        BF16_NOMINAL_ENTRY_FIXED_FRAME_V1,
        size(callback_bytes)?,
        size(std::mem::size_of::<Bf16CallQueryResultV1<R>>())?,
        size(std::mem::size_of::<Bf16NominalEntryCheckpointV1>())?,
    ])?)
}

fn bf16_nominal_require_original_floor_v1(
    incoming: usize,
    required: usize,
) -> Bf16CallQueryResultV1<()> {
    if incoming < required {
        Err(ArgumentResourceV1::Accounting.into())
    } else {
        Ok(())
    }
}

// The original incoming number is captured HERE, never accepted from callers.
// The private inner closure may inspect it only while the entry envelope lives.
fn bf16_nominal_entry_scope_v1<'w, R, F>(
    budget: &mut ArgumentBudgetV1<'w>,
    inspect: F,
) -> Bf16CallQueryResultV1<R>
where
    R: Copy + 'static,
    F: FnOnce(&mut ArgumentBudgetV1<'w>, usize) -> Bf16CallQueryResultV1<R>,
{
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let incoming = Bf16NominalEntryCheckpointV1::take(budget);
    let frame = bf16_nominal_entry_frame_v1::<R>(std::mem::size_of::<F>())?;
    budget.reserve_storage(frame)?;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.charge_work(BF16_NOMINAL_ENTRY_WORK_V1)?;
        inspect(budget, incoming.storage)
    }));
    // The body owns and drops its entire callback capture before this return;
    // initialized data and panic payloads are gone before the own-only refund.
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(ArgumentResourceV1::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Bf16NominalCallQueryErrorV1::CallbackPanicked)
        }
    };
    incoming.check(budget, frame)?;
    budget.release_storage(frame)?;
    result
}

impl ProductionPreRankedKirOwnerV1 {
    /// Runs a new nominal consumer with its complete generic entry frame paid
    /// before any nested query. This grants only resource custody, never source,
    /// ranked, formal, artifact, target, execution or launch authority.
    ///
    /// The true incoming floor is captured before the callback/result frame is
    /// reserved. The exact same retained owner/capture/inventory requirement as
    /// N1 is checked against THAT original floor. A large frame cannot mask F-1.
    /// No caller-authored floor value or reconstructed budget is accepted.
    ///
    /// Every nested source query must still run and finish its own checks.
    /// Caller supplies the original materialization account; no global ledger
    /// identity token is stored in the immutable owner. This logical envelope
    /// accounts initialized frames, not native allocator or RSS bounds.
    ///
    /// Callback outputs are Copy+'static. Callback-added storage survives;
    /// scratch/captures/panic payloads drop before only this frame is refunded.
    /// A replaced/undercut ledger is not repaired, and ignored sticky denials
    /// cannot become success.
    pub fn with_bf16_nominal_entry_resources_v1<'w, R, F>(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        budget: &mut ArgumentBudgetV1<'w>,
        inspect: F,
    ) -> Bf16CallQueryResultV1<R>
    where
        R: Copy + 'static,
        F: FnOnce(&mut ArgumentBudgetV1<'w>) -> Bf16CallQueryResultV1<R>,
    {
        bf16_nominal_entry_scope_v1(budget, move |budget, original_storage| {
            bf16_query_policy_v1(self.helper_source_policy_v1())?;
            bf16_query_require_v1(
                inventory.belongs_to(self.executable()),
                "foreign canonical inventory at nominal resource entry",
            )?;
            let required = bf16_nominal_retained_floor_v1(self, inventory, budget)?;
            bf16_nominal_require_original_floor_v1(original_storage, required)?;
            inspect(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_bf16_nominal_entry_resources_v1_tests.rs"]
mod bf16_nominal_entry_resources_tests_v1;
