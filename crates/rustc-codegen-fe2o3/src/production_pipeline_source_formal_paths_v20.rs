//! Fixed compiler-binding continuation through genuine paired path reports.
use super::*;
use fe2o3_kernel_analysis::FormalPaidPathViewV20 as PathView;
use fe2o3_lower_mir_kernel::{
    ProductionFormalPathSideV20 as Side, ProductionOptimizedSourcePathsErrorV20 as PathsError,
};

struct OriginalFormalPathsV20;

impl<F> SourceHandoffPolicyV29<(), F> for OriginalFormalPathsV20
where
    F: for<'before, 'input, 'after, 'output, 'path, 'work> FnMut(
        Side,
        &Report<'before, 'input>,
        &Report<'after, 'output>,
        &PathView<'path>,
        &mut Budget<'work>,
    ) -> Result<(), Error>,
{
    fn entry_headers() -> Result<usize, Resource> {
        entry_headers_for_handoff::<(), F, IntegerHandoff<'static, 'static>>()?
            .checked_add(launch_context_headers_v19()?)
            .and_then(|bytes| bytes.checked_add(size_of::<Option<F>>()))
            .and_then(|bytes| bytes.checked_add(size_of::<Result<(), Error>>()))
            .and_then(|bytes| {
                bytes.checked_add(size_of::<std::thread::Result<Result<(), Error>>>())
            })
            .and_then(|bytes| bytes.checked_add(size_of::<PathOptimizationErrorV20>()))
            .and_then(|bytes| bytes.checked_add(align_of::<PathOptimizationErrorV20>()))
            .ok_or(Resource::Arithmetic)
    }

    fn consume<'view, 'source, 'abi, 'work>(
        source: &'view Source<'source>,
        roots: &[AbiRoot<'abi>],
        context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'work>,
        consume: F,
    ) -> Result<(), Error> {
        let mut pending = Some(consume);
        let mut execute = |budget: &mut Budget<'work>| {
            budget.check_prior_denials_v1()?;
            source.require_kernel_argument_abi_v18(
                ProductionKernelArgumentAbiInputV18 { roots },
                budget,
            )?;
            let (launches, width) = context.launches(source, budget)?;
            let (output, (), receipt) = source
                .with_checked_integer_optimization_v18(budget, |original, optimized, budget| {
                    original.with_optimized_formal_paths_v20(
                        optimized,
                        &launches,
                        width,
                        ControlFlowLimits::DEFAULT,
                        budget,
                        pending
                            .take()
                            .expect("single original-source paired-path consumer"),
                    )?;
                    Ok::<_, PathsError<Error>>(((), 0))
                })
                .map_err(|error| Error::FormalPaths(Box::new(error)))?;
            drop((output, receipt, launches));
            source.check_original_source(source.source_ssa(budget)?, budget)?;
            budget.check_prior_denials_v1()?;
            Ok(())
        };
        let headers = std::mem::size_of_val(&execute)
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(std::mem::align_of_val(&execute)))
            .ok_or(Resource::Arithmetic);
        let result = match headers.and_then(|headers| {
            budget.check_prior_denials_v1()?;
            budget.charge_work(headers)?;
            budget.reserve_storage(headers)
        }) {
            Ok(()) => catch_unwind(AssertUnwindSafe(|| execute(budget))),
            Err(error) => Ok(Err(Error::Resource(error))),
        };
        drop(execute);
        discard(pending);
        match result {
            Ok(Err(error)) => Err(error),
            Ok(Ok(())) => {
                budget.check_prior_denials_v1()?;
                Ok(())
            }
            Err(payload) => {
                discard(payload);
                Err(Error::Unsupported(
                    "original-source paired-path consumer panicked",
                ))
            }
        }
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Observes paired original/output path obligations under authentic ABI,
    /// descriptor, target, source and launch custody. The same cumulative solver
    /// spans every root and both owners. Residual obligations remain unchanged;
    /// this explicit continuation does not activate final/default compilation.
    pub(crate) fn with_original_source_formal_paths_v20<F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<()>, Error>
    where
        F: for<'before, 'input, 'after, 'output, 'path, 'work> FnMut(
            Side,
            &Report<'before, 'input>,
            &Report<'after, 'output>,
            &PathView<'path>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    {
        self.with_source_owned_custody_policy_v29::<OriginalFormalPathsV20, (), F>(
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

    type Consumer = fn(
        Side,
        &Report<'_, '_>,
        &Report<'_, '_>,
        &PathView<'_>,
        &mut Budget<'_>,
    ) -> Result<(), Error>;

    #[test]
    fn paired_path_policy_prepays_context_and_exact_box_payload_before_errors() {
        let expected =
            entry_headers_for_handoff::<(), Consumer, IntegerHandoff<'static, 'static>>().unwrap()
                + size_of::<Vec<Launch>>()
                + size_of::<crate::production_geometry_v1::ProductionCoordinateGeometryV19>()
                + size_of::<fe2o3_amd_target::AmdTargetCapabilities>()
                + size_of::<ProductionPipelineError>()
                + align_of::<ProductionPipelineError>()
                + size_of::<Option<Consumer>>()
                + size_of::<Result<(), Error>>()
                + size_of::<std::thread::Result<Result<(), Error>>>()
                + size_of::<PathOptimizationErrorV20>()
                + align_of::<PathOptimizationErrorV20>();
        assert_eq!(
            <OriginalFormalPathsV20 as SourceHandoffPolicyV29<(), Consumer>>::entry_headers()
                .unwrap(),
            expected
        );
        for short in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = Budget::new(&mut work, expected - usize::from(short));
            let result = budget.reserve_storage(expected);
            if short {
                assert!(
                    matches!(result, Err(Resource::Storage(error)) if error.actual() == expected && error.limit() == expected - 1)
                );
                assert_eq!(budget.storage(), 0);
            } else {
                result.unwrap();
                assert_eq!(budget.storage(), expected);
            }
        }
    }

    #[test]
    fn paired_path_backend_box_preserves_the_typed_query_cause() {
        use std::error::Error as _;
        let query = fe2o3_kernel_analysis::PresburgerQueryErrorV2::Limit {
            resource: fe2o3_kernel_analysis::PresburgerQueryResourceV2::Work,
            actual: 4,
            limit: 3,
        };
        let path = PathsError::<Error>::Query {
            error: query.clone(),
            source_refusal: ProductionSourceOwnedViewErrorV18::Binding("path query retained"),
        };
        let error = Error::FormalPaths(Box::new(ProductionSourceOptimizationErrorV18::Adoption(
            fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1::Origin(path),
        )));
        assert_eq!(
            error
                .source()
                .unwrap()
                .source()
                .unwrap()
                .source()
                .unwrap()
                .source()
                .unwrap()
                .downcast_ref::<fe2o3_kernel_analysis::PresburgerQueryErrorV2>(),
            Some(&query)
        );
    }
}
