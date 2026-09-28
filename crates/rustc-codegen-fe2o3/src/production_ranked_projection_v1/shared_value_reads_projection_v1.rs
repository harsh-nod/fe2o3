use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_pliron::{ProductionSemanticSharedReadsV1 as Reads, ProductionSemanticSsaOwnerV1};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[cfg(test)]
#[path = "shared_value_reads_diagnostic_v1_tests.rs"]
mod diagnostic;

pub(super) fn with_reads<'s, T, F: ProjectedAssertionFactsV1, C>(
    owner: &'s ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    facts: &mut F,
    next: C,
) -> Result<T, ProductionRankedProjectionErrorV1>
where
    C: FnOnce(&Reads<'s>, &mut F) -> Result<T, ProductionRankedProjectionErrorV1>,
{
    type E = ProductionRankedProjectionErrorV1;
    let resource = ranked_projection_source_v1::resource;
    let sizes = [
        size_of::<C>(),
        size_of::<Option<Reads<'s>>>(),
        size_of::<Result<Reads<'s>, E>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<(), E>>(),
        size_of::<Result<(), E>>(),
        size_of::<Option<E>>(),
        size_of::<&ProductionSemanticSsaOwnerV1>(),
        size_of::<&Reads<'s>>(),
        size_of::<(
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        )>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
    ];
    let headers = sizes.into_iter().try_fold(0usize, |sum, size| {
        sum.checked_add(size)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })?;
    facts.charge_private_array_work(4)?;
    let identity = facts.helper_value_ledger_v1()?;
    let floor = facts.scalar_private_storage_v1()?;
    let required = floor
        .checked_add(headers)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    facts.reserve_scalar_private_storage_v1(headers)?;
    let mut reads = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        reads = Some(facts.shared_value_reads_v1(owner, function)?);
        #[cfg(test)]
        diagnostic::observe(owner, function, reads.as_ref().unwrap(), facts)?;
        next(reads.as_ref().expect("installed Shared read view"), facts)
    }));
    let cleanup = reads.map_or(Ok(()), |reads| facts.release_shared_value_reads_v1(reads));
    let frame = (|| {
        if facts.helper_value_ledger_v1()? == identity
            && facts.scalar_private_storage_v1()? >= required
        {
            facts.release_scalar_private_storage_v1(headers)
        } else {
            Err(resource(Resource::Accounting))
        }
    })();
    match result {
        Ok(Err(error)) => Err(error),
        Ok(Ok(value)) => {
            cleanup?;
            frame?;
            Ok(value)
        }
        Err(panic) => resume_unwind(panic),
    }
}
