//! Fixed authentic original-source Policy9/private-class continuation.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionBoundPrivateHandoffErrorV21,
    ProductionBoundPrivateWorklistOutputHandoffV21 as WorklistHandoff,
};

impl From<ProductionBoundPrivateHandoffErrorV21> for Error {
    fn from(error: ProductionBoundPrivateHandoffErrorV21) -> Self {
        Self::BoundPrivateWorklistHandoff(error)
    }
}

source_handoff_policy_v29!(@impl BoundPrivateWorklist, WorklistHandoff,
    |source, roots, context, budget, handoff| {
        let (launches, width) = context.launches(source, budget)?;
        let handoff = source.checked_bound_private_worklist_output_v21(
            ProductionKernelArgumentAbiInputV18 { roots }, &launches, width, budget,
        )?;
    }, [
        formal_context_v19::launch_context_headers_v19()?,
        size_of::<fe2o3_kernel_ir::FormalIndexWidth>(),
        size_of::<ProductionBoundPrivateHandoffErrorV21>(),
    ]
);

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Binds the original nominal Rust/ABI/descriptor/target, executes Policy9
    /// and consumes its real checked private-class output while custody lives.
    /// No default, target, artifact or later reconstructed-output authority is
    /// implied by the observation or returned original compiler bindings.
    pub(crate) fn with_original_source_bound_private_worklist_v21<R, F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &WorklistHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<BoundPrivateWorklist, R, F>(
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
    use std::error::Error as _;

    #[test]
    fn bound_worklist_policy_keeps_typed_refusal_chain_and_real_handoff_headers() {
        use fe2o3_lower_mir_kernel::ProductionBoundPrivateCheckErrorV21 as Check;
        let error = Error::from(ProductionBoundPrivateHandoffErrorV21::Check(Check::Source(
            ProductionSourceOwnedViewErrorV18::Resource(Resource::Accounting),
        )));
        assert!(
            error
                .source()
                .unwrap()
                .is::<ProductionBoundPrivateHandoffErrorV21>()
        );
        assert!(error.source().unwrap().source().unwrap().is::<Check>());
        type Consumer = for<'view, 'source, 'abi, 'work> fn(
            &'view Source<'source>,
            &WorklistHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<(), Error>;
        let expected =
            entry_headers_for_handoff::<(), Consumer, WorklistHandoff<'static, 'static>>().unwrap()
                + size_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
                + align_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
                + formal_context_v19::launch_context_headers_v19().unwrap()
                + size_of::<fe2o3_kernel_ir::FormalIndexWidth>()
                + size_of::<ProductionBoundPrivateHandoffErrorV21>();
        assert_eq!(
            <BoundPrivateWorklist as SourceHandoffPolicyV29<(), Consumer>>::entry_headers()
                .unwrap(),
            expected
        );
    }
}
