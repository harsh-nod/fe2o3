//! Original Rust, ABI and target-bound conditional mixed-memory Policy9 output.
use super::*;
use fe2o3_kernel_ir::{CanonicalFormalLaunchInputV19, ExplicitLaunchExtent};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedOutputHandoffV26 as MixedHandoff,
    ProductionMixedSourceHandoffErrorV26,
};

impl From<ProductionMixedSourceHandoffErrorV26> for Error {
    fn from(error: ProductionMixedSourceHandoffErrorV26) -> Self {
        Self::ConditionalMixedHandoff(error)
    }
}

fn explicit_mixed_launches_v26(
    original: &[CanonicalFormalLaunchInputV19],
    budget: &mut Budget<'_>,
) -> Result<Vec<ExplicitLaunchExtent>, Error> {
    budget.check_prior_denials_v1()?;
    let mut launches = paid_vec(original.len(), budget)?;
    // Mixed conditional admission quantifies distinct invocations over this
    // physical domain. This does not assert equality with source-static extents
    // or discharge the later concrete launch and address-formation conditions.
    for launch in original {
        budget.charge_work(4)?;
        let CanonicalFormalLaunchInputV19::PhysicalEnvelope(
            extent @ ExplicitLaunchExtent::Exact { .. },
        ) = launch
        else {
            return Err(Error::Unsupported(
                "mixed source requires exact physical launch bounds",
            ));
        };
        launches.push(*extent);
    }
    Ok(launches)
}

source_handoff_policy_v29!(@impl ConditionalMixedWorklist, MixedHandoff,
    |source, roots, context, budget, handoff| {
        let (physical, width) = context.launches(source, budget)?;
        let launches = explicit_mixed_launches_v26(&physical, budget)?;
        let handoff = source.conditional_mixed_worklist_output_v26(
            ProductionKernelArgumentAbiInputV18 { roots }, &launches, width, budget,
        )?;
    }, [
        formal_context_v19::launch_context_headers_v19()?,
        size_of::<Vec<ExplicitLaunchExtent>>(),
        align_of::<Vec<ExplicitLaunchExtent>>(),
        size_of::<fe2o3_kernel_ir::FormalIndexWidth>(),
        size_of::<ProductionMixedSourceHandoffErrorV26>(),
    ]
);

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Executes the fixed original-source mixed Policy9 route and borrows its
    /// actual output and mandatory runtime premises while compiler custody lives.
    /// The target and launch bounds come from the retained compiler bindings.
    /// This continuation does not select the default route, emit target code, or
    /// establish proof, runtime discharge, artifact or launch authority.
    pub(crate) fn with_original_source_conditional_mixed_worklist_v26<R, F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &MixedHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<ConditionalMixedWorklist, R, F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            consume,
        )
    }

    #[cfg(test)]
    pub(crate) fn with_original_source_conditional_mixed_test_limits_v26<R, F>(
        self,
        work_limit: usize,
        storage_limit: usize,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &MixedHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<ConditionalMixedWorklist, R, F>(
            ImportProfile::NominalV35,
            work_limit,
            storage_limit,
            consume,
        )
    }
}

#[cfg(test)]
#[path = "production_pipeline_source_mixed_worklist_v26_tests.rs"]
mod tests;
