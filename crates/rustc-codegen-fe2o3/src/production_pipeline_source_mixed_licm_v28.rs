//! Original Rust and retained compiler bindings through actual V18 LICM.
//! This intermediate is not a final-native or publication continuation.

use super::*;
use fe2o3_kernel_ir::ExplicitLaunchExtent;
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedPureCseOutputHandoffV26 as Prefix,
    ProductionMixedLicmRelocationErrorV28 as MotionError,
    ProductionMixedLicmRelocationV28 as Relocation, ProductionMixedSourceHandoffErrorV26,
};

fn prepare_prefix<'view, 'source>(
    source: &'view Source<'source>,
    abi: ProductionKernelArgumentAbiInputV18<'_>,
    launches: &'view [ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<Prefix<'view, 'source>, ProductionMixedSourceHandoffErrorV26> {
    source.conditional_mixed_pure_cse_output_v26(abi, launches, width, budget)
}
fn prepare_relocation<'prefix, 'view, 'source>(
    prefix: &'prefix Prefix<'view, 'source>,
    budget: &mut Budget<'_>,
) -> Result<Relocation<'prefix, 'view, 'source>, MotionError> {
    prefix.prepare_mixed_licm_v28(budget)
}
include!("production_pipeline_source_mixed_licm_policy_v29.rs");

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Runs the fixed source-owned Policy10 prefix and actual checked LICM in
    /// one lexical compiler transaction. Source, full ABI, target and both
    /// canonical endpoints remain genuine owners, not reconstructed digests.
    /// Final native/source relocation, target emission, refinement and Worker
    /// publication still need their separate consuming admission stages.
    pub(crate) fn with_original_source_mixed_licm_v28<R, F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'prefix, 'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &Relocation<'prefix, 'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<MixedLicm, R, F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            consume,
        )
    }

    #[cfg(test)]
    pub(crate) fn with_original_source_mixed_licm_test_limits_v28<R, F>(
        self,
        work: usize,
        storage: usize,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'prefix, 'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &Relocation<'prefix, 'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<MixedLicm, R, F>(
            ImportProfile::NominalV35,
            work,
            storage,
            consume,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_licm_retains_nominal_prefix_and_both_settlement_frames() {
        type Consumer = for<'p, 'v, 's, 'a, 'w> fn(
            &'v Source<'s>,
            &Relocation<'p, 'v, 's>,
            &[AbiRoot<'a>],
            TargetProfile,
            &mut Budget<'w>,
        ) -> Result<(), Error>;
        type Frame<'a> = (
            &'a Source<'a>,
            &'a Prefix<'a, 'a>,
            &'a [AbiRoot<'a>],
            TargetProfile,
            &'a mut Budget<'a>,
            &'a mut formal_context_v19::PendingConsumerV19<Consumer>,
        );
        type Invoke<'a> = (
            &'a Source<'a>,
            &'a Relocation<'a, 'a, 'a>,
            &'a [AbiRoot<'a>],
            TargetProfile,
            &'a mut Budget<'a>,
            &'a mut formal_context_v19::PendingConsumerV19<Consumer>,
        );
        let expected =
            entry_headers_for_handoff::<(), Consumer, Relocation<'static, 'static, 'static>>()
                .unwrap()
                + size_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
                + align_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
                + formal_context_v19::launch_context_headers_v19().unwrap()
                + size_of::<Vec<ExplicitLaunchExtent>>()
                + align_of::<Vec<ExplicitLaunchExtent>>()
                + size_of::<fe2o3_kernel_ir::FormalIndexWidth>()
                + size_of::<ProductionMixedSourceHandoffErrorV26>()
                + size_of::<Frame<'_>>()
                + align_of::<Frame<'_>>()
                + size_of::<AssertUnwindSafe<Frame<'_>>>()
                + size_of::<Invoke<'_>>()
                + align_of::<Invoke<'_>>()
                + size_of::<AssertUnwindSafe<Invoke<'_>>>()
                + 2 * size_of::<std::thread::Result<Result<(), Error>>>()
                + 2 * size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>()
                + size_of::<Result<Relocation<'_, '_, '_>, MotionError>>();
        assert_eq!(
            <MixedLicm as SourceHandoffPolicyV29<(), Consumer>>::entry_headers().unwrap(),
            expected
        );
        let error = Error::from(MotionError::Binding("exact relocation owner"));
        assert!(matches!(
            error,
            Error::MixedLicm(MotionError::Binding("exact relocation owner"))
        ));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn mixed_licm_settlement_preserves_selected_error_and_raw_unwind() {
        let release = || {
            Err(ProductionSourceOwnedViewErrorV18::Resource(
                Resource::Accounting,
            ))
        };
        assert!(matches!(
            settled::<()>(Ok(Err(Error::Unsupported("selected consumer"))), release()),
            Err(Error::Unsupported("selected consumer"))
        ));
        let panic = catch_unwind(AssertUnwindSafe(|| {
            settled::<()>(Err(Box::new(17usize)), release())
        }))
        .unwrap_err();
        assert_eq!(*panic.downcast::<usize>().unwrap(), 17);
        struct Rejected<'a>(&'a std::cell::Cell<usize>);
        impl Drop for Rejected<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
                panic!("selected release remains primary");
            }
        }
        let dropped = std::cell::Cell::new(0);
        assert!(matches!(
            settled(Ok(Ok(Rejected(&dropped))), release()),
            Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                Resource::Accounting
            )))
        ));
        assert_eq!(dropped.get(), 1);
    }
}
