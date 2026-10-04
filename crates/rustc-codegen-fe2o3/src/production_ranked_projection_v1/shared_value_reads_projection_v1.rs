use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_pliron::{
    ProductionSemanticSharedReadsPreparationV1 as Preparation,
    ProductionSemanticSharedReadsV1 as Reads, ProductionSemanticSsaOwnerV1,
};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[cfg(test)]
#[path = "shared_value_reads_diagnostic_v1_tests.rs"]
mod diagnostic;

// This is the actual first root scope's retained backing, not the later whole
// recipe backing. Later singleton/borrow/induction/lazy scopes stay in place.
struct Pending<'s> {
    reads: Preparation<'s>,
    owned: usize,
}

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
        size_of::<Pending<'s>>(),
        size_of::<Preparation<'s>>(),
        size_of::<Option<Reads<'s>>>(),
        size_of::<Result<Reads<'s>, E>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<(Result<(), E>, Result<(), E>, Result<(), E>, Option<E>)>(),
        size_of::<(
            &ProductionSemanticSsaOwnerV1,
            &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &Reads<'s>,
            &mut F,
            &mut Pending<'s>,
            &mut Preparation<'s>,
            &Preparation<'s>,
            &mut usize,
        )>(),
        size_of::<(
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        )>(),
        size_of::<Box<dyn std::any::Any + Send>>(),
        size_of::<(
            Resource,
            CanonicalAssertionErrorV1,
            E,
            fe2o3_pliron::ProductionSemanticSharedReadErrorV1,
            Result<(), fe2o3_pliron::ProductionSemanticSharedReadErrorV1>,
            Result<&Reads<'s>, fe2o3_pliron::ProductionSemanticSharedReadErrorV1>,
            Result<&Reads<'s>, E>,
            Option<&Reads<'s>>,
        )>(),
        size_of::<(
            bf16_nominal_preparation_resources_v1::PreparationResourcesV1<'static, 'static>,
            &mut bf16_nominal_preparation_resources_v1::PreparationResourcesV1<'static, 'static>,
            &bf16_nominal_preparation_resources_v1::PreparationResourcesV1<'static, 'static>,
            &mut canonical_assertion_facts_v1::CanonicalSourceAssertionFactsV1<
                'static,
                'static,
                'static,
                'static,
                'static,
            >,
            &guarded_source_progress_v1::GuardedSourceProgressV1<'static, 'static>,
            &mut guarded_source_progress_v1::GuardedSourceProgressV1<'static, 'static>,
            SemanticFunctionIdV1,
            &usize,
            Option<usize>,
            Result<usize, E>,
            Result<
                (
                    usize,
                    fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
                ),
                E,
            >,
        )>(),
        size_of::<(
            usize,
            usize,
            usize,
            bool,
            Result<(), E>,
            &mut dyn FnMut() -> Result<T, E>,
        )>(),
        size_of::<([usize; 16], std::array::IntoIter<usize, 16>, usize, usize)>(),
        bf16_nominal_preparation_resources_v1::retained_shared_frame_v1(),
    ];
    let headers = sizes.into_iter().try_fold(0usize, |sum, size| {
        sum.checked_add(size)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })?;
    facts.charge_private_array_work(4)?;
    let identity = facts.helper_value_ledger_v1()?;
    let floor = facts.scalar_private_storage_v1()?;
    let _required = floor
        .checked_add(headers)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let mut pending = Pending {
        reads: Preparation::new(),
        owned: 0,
    };
    facts.reserve_scalar_private_storage_v1(headers)?;
    pending.owned = headers;
    let result = catch_unwind(AssertUnwindSafe(|| {
        facts.prepare_retained_shared_reads_v1(
            &mut pending.reads,
            owner,
            function,
            &mut pending.owned,
        )?;
        facts.check_retained_shared_reads_v1(
            &pending.reads,
            owner,
            function,
            &mut pending.owned,
        )?;
        let reads = pending
            .reads
            .view()
            .ok_or_else(|| resource(Resource::Accounting))?;
        #[cfg(test)]
        diagnostic::observe(owner, function, reads, facts)?;
        next(reads, facts)
    }));
    // Postflight precedes every drop/refund, including ignored query denial.
    let postflight =
        facts.check_retained_shared_reads_v1(&pending.reads, owner, function, &mut pending.owned);
    let intact = (|| {
        let required = floor
            .checked_add(pending.owned)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if facts.helper_value_ledger_v1()? != identity
            || facts.scalar_private_storage_v1()? < required
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    })();
    let bytes = pending.owned;
    drop(pending);
    let cleanup = intact.and_then(|()| facts.release_scalar_private_storage_v1(bytes));
    match result {
        Ok(Err(error)) => Err(error),
        Ok(Ok(value)) => {
            postflight?;
            cleanup?;
            Ok(value)
        }
        Err(panic) => resume_unwind(panic),
    }
}
