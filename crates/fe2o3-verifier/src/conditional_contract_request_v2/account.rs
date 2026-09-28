use super::*;

struct Frame {
    required: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
}
pub(super) const HEADER: usize = size_of::<Frame>()
    + 2 * size_of::<usize>()
    + size_of::<R<()>>()
    + size_of::<Result<R<()>, Box<dyn std::any::Any + Send>>>();

/// Only our fixed reservation is released. Opaque child deltas are never ours.
pub(super) fn scope<'w>(
    budget: &mut Budget<'w>,
    bytes: usize,
    run: impl FnOnce(&mut Budget<'w>) -> R<()>,
) -> R<()> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    let bytes = bytes.checked_add(HEADER).ok_or(Resource::Arithmetic)?;
    let required = budget
        .storage()
        .checked_add(bytes)
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    let frame = Frame {
        required,
        ledger: budget.work_ledger_identity_v1(),
        slot: budget as *const Budget<'w> as usize,
    };
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let cleanup = if budget.work_ledger_identity_v1() != frame.ledger
        || frame.slot != budget as *const Budget<'w> as usize
        || budget.storage() < frame.required
    {
        Err(Resource::Accounting)
    } else {
        budget.release_storage(bytes)
    };
    match result {
        Err(payload) => {
            let _ = cleanup;
            resume_unwind(payload)
        }
        Ok(result) => {
            cleanup?;
            result
        }
    }
}
