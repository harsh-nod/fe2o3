//! Original Rust, ABI and target-bound conditional mixed-memory Policy10 output.
use super::*;
use fe2o3_kernel_ir::ExplicitLaunchExtent;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedPureCseOutputHandoffV26 as MixedHandoff,
    ProductionMixedSourceHandoffErrorV26,
};

source_handoff_policy_v29!(@impl ConditionalMixedPureCse, MixedHandoff,
    |source, roots, context, budget, handoff| {
        let (physical, width) = context.launches(source, budget)?;
        let launches = mixed_worklist_v26::explicit_mixed_launches_v26(&physical, budget)?;
        let handoff = source.conditional_mixed_pure_cse_output_v26(
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
    #[cfg(test)]
    pub(crate) fn with_original_source_conditional_mixed_pure_cse_test_limits_v27<R, F>(
        self,
        work: usize,
        storage: usize,
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
        self.with_source_owned_custody_policy_v29::<ConditionalMixedPureCse, R, F>(
            ImportProfile::NominalV35,
            work,
            storage,
            consume,
        )
    }

    /// Executes the distinct fixed Policy10 source route: integer worklist,
    /// local pure CSE, dominance pure CSE, and DCE. Original source, complete
    /// ABI and authenticated target launch bounds remain attached to its owner.
    /// This explicit continuation is not default activation or launch authority.
    pub(crate) fn with_original_source_conditional_mixed_pure_cse_v26<R, F>(
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
        self.with_source_owned_custody_policy_v29::<ConditionalMixedPureCse, R, F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            consume,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_pure_cse_backend_headers_capture_the_nominal_policy10_owner() {
        type Consumer = for<'view, 'source, 'abi, 'work> fn(
            &'view Source<'source>,
            &MixedHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<(), Error>;
        let expected = entry_headers_for_handoff::<(), Consumer, MixedHandoff<'static, 'static>>()
            .unwrap()
            + size_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
            + align_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
            + formal_context_v19::launch_context_headers_v19().unwrap()
            + size_of::<Vec<ExplicitLaunchExtent>>()
            + align_of::<Vec<ExplicitLaunchExtent>>()
            + size_of::<fe2o3_kernel_ir::FormalIndexWidth>()
            + size_of::<ProductionMixedSourceHandoffErrorV26>();
        assert_eq!(
            <ConditionalMixedPureCse as SourceHandoffPolicyV29<(), Consumer>>::entry_headers()
                .unwrap(),
            expected
        );
        assert_ne!(
            std::any::type_name::<MixedHandoff<'static, 'static>>(),
            std::any::type_name::<
                fe2o3_lower_mir_kernel::ProductionConditionalMixedOutputHandoffV26<
                    'static,
                    'static,
                >,
            >()
        );
    }
}
