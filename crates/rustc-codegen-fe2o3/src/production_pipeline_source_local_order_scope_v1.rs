//! Same-ledger cleanup for transferred artifacts and retained compiler stages.
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

pub(super) fn scoped<'w, T>(
    required: usize,
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> ResultL<T>,
) -> ResultL<T> {
    scope(required, false, budget, run)
}
pub(super) fn retained<'w, T>(
    required: usize,
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> ResultL<T>,
) -> ResultL<T> {
    scope(required, true, budget, run)
}
fn scope<'w, T>(
    required: usize,
    retain_success: bool,
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> ResultL<T>,
) -> ResultL<T> {
    if budget.storage() < required {
        return Err(resource(Resource::Accounting));
    }
    let floor = budget.storage();
    let slot = budget as *const Budget<'_> as usize;
    let ledger = budget.work_ledger_identity_v1();
    let (result, payload) = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => (result, None),
        Err(payload) => (
            Err(error(SourceLocalOrderStageErrorV1::Panicked)),
            Some(payload),
        ),
    };
    if slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        drop(result);
        drop(payload);
        return Err(resource(Resource::Accounting));
    }
    let cleanup = if retain_success && result.is_ok() {
        Ok(())
    } else {
        budget.release_storage(budget.storage() - floor)
    };
    if let Err(value) = cleanup {
        drop(result);
        drop(payload);
        return Err(resource(value));
    }
    // A hostile payload destructor may unwind only after valid-ledger cleanup.
    drop(payload);
    result
}
