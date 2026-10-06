//! Ordinary rustc source custody through the actual checked tile expansion.
//! This stage does not turn a scalar-prefix receipt into an expanded proof.
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1;
use fe2o3_lower_mir_kernel::{
    ProductionSourceCorrespondenceV18 as Original, ProductionSourceTileExpansionV159 as Expanded,
};

struct ExpandedSource;

fn settle_callback<R>(
    result: std::thread::Result<Result<(R, usize), Error>>,
    measured: Option<usize>,
    released: Result<(), ProductionSourceOwnedViewErrorV18>,
    deny_accounting: impl FnOnce() -> ProductionSourceOwnedViewErrorV18,
) -> Result<(R, usize), Error> {
    match result {
        Ok(Ok((value, bytes))) => {
            if measured != Some(bytes) {
                formal_context_v19::discard(value);
                return Err(deny_accounting().into());
            }
            match released {
                Ok(()) => Ok((value, bytes)),
                Err(error) => {
                    formal_context_v19::discard(value);
                    Err(error.into())
                }
            }
        }
        Ok(Err(error)) => Err(error),
        Err(payload) => resume_unwind(payload),
    }
}

/// The first production policy supports one kernel root, not one function.
/// Helper functions remain in the complete retained module. Additional roots
/// must not disappear behind the lowerer's single-root selection API.
fn layout(
    source: &Source<'_>,
    target: TargetProfile,
    budget: &mut Budget<'_>,
) -> Result<ExecutionTileLayoutV1, Error> {
    budget.charge_work(2)?;
    selected_layout(source.root_count(budget)?, target)
}

fn selected_layout(roots: usize, target: TargetProfile) -> Result<ExecutionTileLayoutV1, Error> {
    if roots != 1 {
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
                    let measured = budget.storage().checked_sub(floor);
                    let released = tile.discard(budget);
                    let value = settle_callback(result, measured, released, || {
                        source.retain_query_resource_error_v18(Resource::Accounting)
                    })?;
                    payload_bytes.set(value.1);
                    Ok(value)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn denied() -> ProductionSourceOwnedViewErrorV18 {
        Resource::Accounting.into()
    }

    #[test]
    fn expanded_selection_requires_one_kernel_root_for_both_real_targets() {
        for target in [TargetProfile::Gfx942, TargetProfile::Gfx950] {
            assert_eq!(
                selected_layout(1, target).unwrap(),
                ExecutionTileLayoutV1::Blocked
            );
            for roots in [0, 2, usize::MAX] {
                assert!(matches!(
                    selected_layout(roots, target),
                    Err(Error::Unsupported(_))
                ));
            }
        }
    }

    #[test]
    fn expanded_entry_prepays_complete_error_payload_and_callback_frames() {
        type Consumer = for<'v, 's, 'o, 'scope, 't, 'a, 'w> fn(
            &'v Source<'s>,
            &'o Original<'scope>,
            &'t Expanded<'o, 'scope>,
            &[AbiRoot<'a>],
            TargetProfile,
            &mut Budget<'w>,
        )
            -> Result<((), usize), Error>;
        type Frame<'a> = (
            &'a Source<'a>,
            &'a Original<'a>,
            &'a Expanded<'a, 'a>,
            &'a [AbiRoot<'a>],
            TargetProfile,
            &'a mut Budget<'a>,
            &'a mut formal_context_v19::PendingConsumerV19<Consumer>,
            &'a std::cell::Cell<usize>,
        );
        let expected = entry_headers_for_handoff::<(), Consumer, Expanded<'static, 'static>>()
            .unwrap()
            + size_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
            + align_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
            + 2 * size_of::<Frame<'_>>()
            + align_of::<Frame<'_>>()
            + size_of::<AssertUnwindSafe<Frame<'_>>>()
            + size_of::<std::cell::Cell<usize>>()
            + size_of::<ExecutionTileLayoutV1>()
            + 6 * size_of::<usize>()
            + size_of::<std::thread::Result<Result<((), usize), Error>>>()
            + size_of::<fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18<Error>>()
            + align_of::<fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18<Error>>()
            + size_of::<fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18>()
            + size_of::<fe2o3_pliron::KirNeutralOwnedOriginStorageV1>();
        assert_eq!(
            <ExpandedSource as SourceHandoffPolicyV29<(), Consumer>>::entry_headers().unwrap(),
            expected
        );
        let mut work = Work::new(1);
        let mut exact = Budget::new(&mut work, expected);
        exact.reserve_storage(expected).unwrap();
        exact.release_storage(expected).unwrap();
        let mut work = Work::new(1);
        let mut short = Budget::new(&mut work, expected - 1);
        assert!(short.reserve_storage(expected).is_err());
    }

    #[test]
    fn expanded_payload_requires_exact_measured_credit_and_preserves_first_error() {
        assert_eq!(
            settle_callback(Ok(Ok((7, 3))), Some(3), Ok(()), denied).unwrap(),
            (7, 3)
        );
        for measured in [None, Some(2), Some(4)] {
            assert!(matches!(
                settle_callback(Ok(Ok((7, 3))), measured, Ok(()), denied),
                Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                    Resource::Accounting
                )))
            ));
        }
        assert!(matches!(
            settle_callback::<()>(
                Ok(Err(Error::Unsupported("selected refusal"))),
                None,
                Err(denied()),
                denied
            ),
            Err(Error::Unsupported("selected refusal"))
        ));
        assert!(matches!(
            settle_callback(Ok(Ok((7, 3))), Some(3), Err(denied()), denied),
            Err(Error::Source(ProductionSourceOwnedViewErrorV18::Resource(
                Resource::Accounting
            )))
        ));
    }

    #[test]
    fn expanded_settlement_drops_rejected_payload_and_preserves_raw_unwind() {
        struct Payload<'a>(&'a std::cell::Cell<usize>);
        impl Drop for Payload<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
                panic!("payload destructor does not replace selected accounting error");
            }
        }
        let dropped = std::cell::Cell::new(0);
        assert!(settle_callback(Ok(Ok((Payload(&dropped), 1))), Some(0), Ok(()), denied).is_err());
        assert_eq!(dropped.get(), 1);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            settle_callback::<()>(Err(Box::new(17usize)), None, Err(denied()), denied)
        }))
        .unwrap_err();
        assert_eq!(*panic.downcast::<usize>().unwrap(), 17);
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
