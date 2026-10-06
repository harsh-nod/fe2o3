//! Ordinary rustc source custody through the actual checked tile expansion.
//! This stage does not turn a scalar-prefix receipt into an expanded proof.
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1;
use fe2o3_lower_mir_kernel::{
    ProductionSourceCorrespondenceV18 as Original, ProductionSourceTileExpansionV159 as Expanded,
};

struct ExpandedSource;

/// The first production policy supports one kernel root, not one function.
/// Helper functions remain in the complete retained module. Additional roots
/// must not disappear behind the lowerer's single-root selection API.
fn layout(
    source: &Source<'_>,
    target: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<ExecutionTileLayoutV1, Error> {
    budget.charge_work(2)?;
    if source.root_count(budget)? != 1 {
        return Err(Error::Unsupported(
            "expanded production selection requires one complete kernel root",
        ));
    }
    // Both admitted profiles use wave64. A fixed blocked policy is selected
    // here, rather than accepting a provisional layout from the consumer.
    Ok(match target {
        TargetProfile::Gfx942 | TargetProfile::Gfx950 => ExecutionTileLayoutV1::Blocked,
    })
}

impl<R: 'static, F> SourceHandoffPolicyV29<R, F> for ExpandedSource
where
    F: for<'view, 'source, 'original, 'scope, 'tile, 'abi, 'work> FnOnce(
        &'view Source<'source>,
        &'original Original<'scope>,
        &'tile Expanded<'original, 'scope>,
        &[AbiRoot<'abi>],
        TargetProfile,
        &mut Budget<'work>,
    ) -> Result<
        (R, usize),
        Error,
    >,
{
    fn entry_headers() -> Result<usize, Resource> {
        type CallbackFrame<'a, F> = (
            &'a Source<'a>,
            &'a Original<'a>,
            &'a Expanded<'a, 'a>,
            &'a [AbiRoot<'a>],
            TargetProfile,
            &'a mut Budget<'a>,
            &'a mut formal_context_v19::PendingConsumerV19<F>,
            &'a std::cell::Cell<usize>,
        );
        [
            entry_headers_for_handoff::<R, F, Expanded<'static, 'static>>()?,
            size_of::<formal_context_v19::PendingConsumerV19<F>>(),
            align_of::<formal_context_v19::PendingConsumerV19<F>>(),
            2 * size_of::<CallbackFrame<'_, F>>(),
            align_of::<CallbackFrame<'_, F>>(),
            size_of::<AssertUnwindSafe<CallbackFrame<'_, F>>>(),
            size_of::<std::cell::Cell<usize>>(),
            size_of::<ExecutionTileLayoutV1>(),
            6 * size_of::<usize>(),
            size_of::<std::thread::Result<Result<(R, usize), Error>>>(),
            size_of::<fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18<Error>>(),
            align_of::<fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18<Error>>(),
            size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18>(),
            size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>(),
        ]
        .into_iter()
        .try_fold(0usize, |sum, value| {
            sum.checked_add(value).ok_or(Resource::Arithmetic)
        })
    }

    fn consume<'view, 'source, 'abi, 'work>(
        source: &'view Source<'source>,
        roots: &[AbiRoot<'abi>],
        context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'work>,
        consume: F,
    ) -> Result<R, Error> {
        let target = context.bindings.rustc_target.profile();
        let selected = layout(source, target, budget)?;
        let mut pending = formal_context_v19::PendingConsumerV19::new(consume);
        let payload_bytes = std::cell::Cell::new(0);
        let (neutral, value, receipt) = source
            .with_checked_mixed_fixedpoint_optimization_v18(
                budget,
                |original, optimized, budget| {
                    let tile = optimized.prepare_tile_expansion_v159(0, selected, budget)?;
                    let floor = budget.storage();
                    let result = {
                        let consumer = pending.take();
                        let tile = &tile;
                        let budget = &mut *budget;
                        catch_unwind(AssertUnwindSafe(move || {
                            consumer(source, original, tile, roots, target, budget)
                        }))
                    };
                    // A callback's retained payload cannot borrow any of these
                    // graph owners. Preserve its exact measured credit while
                    // destroying the tile owner before the optimizer scope ends.
                    let result = match result {
                        Ok(Ok((value, bytes))) => {
                            if budget.storage().checked_sub(floor) != Some(bytes) {
                                formal_context_v19::discard(value);
                                Err(Error::Source(
                                    source.retain_query_resource_error_v18(Resource::Accounting),
                                ))
                            } else {
                                payload_bytes.set(bytes);
                                Ok((value, bytes))
                            }
                        }
                        Ok(Err(error)) => Err(error),
                        Err(payload) => {
                            let _ = tile.discard(budget);
                            resume_unwind(payload)
                        }
                    };
                    let released = tile.discard(budget);
                    match result {
                        Ok(value) => match released {
                            Ok(()) => Ok(value),
                            Err(error) => {
                                formal_context_v19::discard(value);
                                Err(error.into())
                            }
                        },
                        Err(error) => Err(error),
                    }
                },
            )
            .map_err(|error| Error::ExpandedSource(Box::new(error)))?;
        // The non-retained optimizer API transfers unreserved output credit.
        // Drop both neutral objects before restoring only the callback payload.
        drop(neutral);
        let _ = receipt;
        if let Err(error) = budget.reserve_storage(payload_bytes.get()) {
            formal_context_v19::discard(value);
            return Err(error.into());
        }
        Ok(value)
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Imports ordinary rustc source, runs the checked fixed-point prefix and
    /// retains its actual expanded successor on the caller's existing account.
    /// The consumer returns its measured owned payload credit, never graph
    /// borrows. Refinement, native completion and publication remain separate.
    pub(crate) fn with_original_source_expanded_v259<R: 'static, F>(
        self,
        budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'original, 'scope, 'tile, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &'original Original<'scope>,
            &'tile Expanded<'original, 'scope>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<
            (R, usize),
            Error,
        >,
    {
        self.with_source_owned_custody_policy_on_account_v29::<ExpandedSource, R, F>(
            ImportProfile::SourceOwnedV29,
            budget,
            consume,
        )
    }
}
