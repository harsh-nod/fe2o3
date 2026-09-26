// Reuse the existing source scope; drain every nested payload before its refund.
fn cpc_discard_v1<T>(value: T) {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut pending = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = pending {
        pending = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}

fn cpc_scope_v1<'w, T>(
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> CsResultV1<T>,
) -> CsResultV1<T> {
    csa_scope_v1(budget, |budget| {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<std::thread::Result<CsResultV1<T>>>(),
            std::mem::size_of::<std::thread::Result<()>>(),
        ])?)?;
        let slot = std::ptr::from_ref(&*budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        let returned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(budget)));
        if slot != std::ptr::from_ref(&*budget) as usize
            || ledger != budget.work_ledger_identity_v1()
            || budget.storage() < floor
        {
            cpc_discard_v1(returned);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        match returned {
            Ok(result) => result,
            Err(payload) => {
                cpc_discard_v1(payload);
                Err(Failure::Panicked.into())
            }
        }
    })
}

fn cpc_error_v1(
    operation: CsOperationV1,
    step: ProductionCanonicalScalarAssertionStepV1,
    reason: &'static str,
) -> ProductionCanonicalScalarSourceErrorV1 {
    ProductionCanonicalScalarSourceErrorV1::PrivateCallTransport {
        operation,
        round: step.round,
        integer: step.integer,
        reason,
    }
}

fn cpc_vector_v1<T>(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<Vec<T>> {
    budget.reserve_storage(std::mem::size_of::<Vec<T>>())?;
    cs_vec_v1(count, budget)
}

fn cpc_index_v1(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<Vec<Option<usize>>> {
    let mut rows = cpc_vector_v1(count, budget)?;
    budget.charge_work(count)?;
    rows.resize(count, None);
    Ok(rows)
}

#[cfg(test)]
mod private_call_resource_tests {
    use super::*;
    include!("production_canonical_private_call_resources_v1_tests.rs");
}

#[cfg(test)]
pub(crate) mod private_call_whole_entry_oracle_v1_tests {
    use super::*;
    include!("production_canonical_private_call_whole_entry_oracle_v1_tests.rs");
}
