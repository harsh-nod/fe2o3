fn csa_scope_v1<'w, T>(
    budget: &mut ArgumentBudgetV1<'w>,
    run: impl FnOnce(&mut ArgumentBudgetV1<'w>) -> CsResultV1<T>,
) -> CsResultV1<T> {
    scoped(budget, |budget| Ok(run(budget)))?
}

fn csa_transport_error_v1(
    span: usize,
    step: ProductionCanonicalScalarAssertionStepV1,
    reason: &'static str,
) -> ProductionCanonicalScalarSourceErrorV1 {
    ProductionCanonicalScalarSourceErrorV1::AssertionTransport {
        span,
        round: step.round,
        integer: step.integer,
        reason,
    }
}

fn csa_allocate_v1(count: usize, budget: &mut ArgumentBudgetV1<'_>) -> CsResultV1<CsaTransportV1> {
    let requested = count
        .checked_mul(std::mem::size_of::<ProductionCanonicalScalarAssertionV1>())
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    argument_sum_v1(&[std::mem::size_of::<CsaTransportV1>(), requested])?;
    budget.charge_work(1)?;
    budget.reserve_storage(std::mem::size_of::<CsaTransportV1>())?;
    let rows = cs_vec_v1(count, budget)?;
    let storage = argument_sum_v1(&[std::mem::size_of::<CsaTransportV1>(), cs_extent_v1(&rows)?])?;
    Ok(CsaTransportV1 { rows, storage })
}

#[cfg(test)]
mod scalar_assertion_resource_tests {
    use super::*;
    include!("production_canonical_scalar_assertion_resources_v1_tests.rs");
}

#[cfg(test)]
include!("production_canonical_scalar_assertion_qualification_readers_v1_tests.rs");
