//! Ordinary rustc source custody through the actual checked tile expansion.
//! This stage does not turn a scalar-prefix receipt into an expanded proof.
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1;
use fe2o3_lower_mir_kernel::{
    ProductionSourceCorrespondenceV18 as Original, ProductionSourceTileExpansionV159 as Expanded,
};

#[path = "production_pipeline_expanded_pair_input_v279.rs"]
mod pair_input_v279;
pub(crate) use pair_input_v279::ExpandedPairInputV279;

#[path = "production_pipeline_source_expanded_model_v280.rs"]
mod model_v280;

struct ExpandedSource;
type CallbackPanic = Box<dyn std::any::Any + Send>;
type OptimizerResult<R> = Result<
    (
        fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18,
        R,
        fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
    ),
    fe2o3_lower_mir_kernel::ProductionSourceOptimizationErrorV18<Error>,
>;

fn resume_callback_panic<T, E>(result: Result<T, E>, panic: Option<CallbackPanic>) -> Result<T, E> {
    if let Some(payload) = panic {
        formal_context_v19::discard(result);
        resume_unwind(payload);
    }
    result
}

fn transfer_callback_payload<R>(
    payload: (R, usize),
    budget: &mut Budget<'_>,
) -> Result<(R, usize), Resource> {
    let transfer = (|| {
        budget.charge_work(2)?;
        let bytes = size_of::<R>()
            .checked_add(payload.1)
            .ok_or(Resource::Arithmetic)?;
        budget.release_storage(payload.1)?;
        Ok(bytes)
    })();
    match transfer {
        Ok(bytes) => Ok((payload.0, bytes)),
        Err(error) => {
            formal_context_v19::discard(payload);
            Err(error)
        }
    }
}

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

fn selected_layout(target: TargetProfile) -> ExecutionTileLayoutV1 {
    // Both admitted profiles use wave64. A fixed blocked policy is selected
    // here, rather than accepting a provisional layout from the consumer.
    match target {
        TargetProfile::Gfx942 | TargetProfile::Gfx950 => ExecutionTileLayoutV1::Blocked,
    }
}

impl<R: 'static, F> SourceHandoffPolicyV29<R, F> for ExpandedSource
where
    F: for<'view, 'source, 'original, 'scope, 'tile, 'abi, 'pair, 'work> FnOnce(
        &'view Source<'source>,
        &'original Original<'scope>,
        &'tile Expanded<'original, 'scope>,
        &[AbiRoot<'abi>],
        TargetProfile,
        &'pair ExpandedPairInputV279<'pair, 'source, 'original, 'scope>,
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
            &'a ExpandedPairInputV279<'a, 'a, 'a, 'a>,
            &'a mut Budget<'a>,
            &'a mut formal_context_v19::PendingConsumerV19<F>,
            &'a std::cell::Cell<usize>,
            &'a mut Option<CallbackPanic>,
        );
        [
            entry_headers_for_handoff::<R, F, Expanded<'static, 'static>>()?,
            size_of::<formal_context_v19::PendingConsumerV19<F>>(),
            align_of::<formal_context_v19::PendingConsumerV19<F>>(),
            2 * size_of::<CallbackFrame<'_, F>>(),
            align_of::<CallbackFrame<'_, F>>(),
            size_of::<AssertUnwindSafe<CallbackFrame<'_, F>>>(),
            size_of::<std::cell::Cell<usize>>(),
            2 * size_of::<Option<CallbackPanic>>(),
            2 * size_of::<OptimizerResult<R>>(),
            size_of::<ExecutionTileLayoutV1>(),
            6 * size_of::<usize>(),
            size_of::<std::thread::Result<Result<(R, usize), Error>>>(),
            2 * size_of::<(R, usize)>(),
            size_of::<usize>(),
            size_of::<Result<usize, Resource>>(),
            size_of::<&mut Budget<'_>>(),
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
        budget.charge_work(2)?;
        let selected = selected_layout(target);
        let mut pending = formal_context_v19::PendingConsumerV19::new(consume);
        let payload_bytes = std::cell::Cell::new(0);
        let mut callback_panic = None;
        let optimized = source.with_checked_mixed_fixedpoint_optimization_v18(
            budget,
            |original, optimized, budget| {
                let tile = optimized.prepare_tile_expansion_with_layout_v260(selected, budget)?;
                let pair = pair_input_v279::prepare(source, original, &tile, context, budget)?;
                budget.reserve_storage(pair.retained_storage())?;
                pair.check(source, original, &tile, budget)?;
                let floor = budget.storage();
                let result = {
                    let consumer = pending.take();
                    let tile = &tile;
                    let pair = &pair;
                    let budget = &mut *budget;
                    catch_unwind(AssertUnwindSafe(move || {
                        consumer(source, original, tile, roots, target, pair, budget)
                    }))
                };
                // Adoption normalizes unwinds. Retain this caller's original
                // payload outside that scope and let an ordinary refusal
                // drive the same owned-graph cleanup before resuming it.
                let result = match result {
                    Err(payload) => {
                        callback_panic = Some(payload);
                        Ok(Err(Error::Unsupported("expanded consumer unwound")))
                    }
                    other => other,
                };
                // A callback's retained payload cannot borrow any of these
                // graph owners. Preserve its exact measured credit while
                // destroying the tile owner before the optimizer scope ends.
                let measured = budget.storage().checked_sub(floor);
                let pair_released = pair.discard(budget);
                let tile_released = tile.discard(budget);
                let released = pair_released.and(tile_released);
                let value = settle_callback(result, measured, released, || {
                    source.retain_query_resource_error_v18(Resource::Accounting)
                })?;
                payload_bytes.set(value.1);
                // The checked optimizer accepts an unreserved ownership
                // transfer, including R's inline header. Its callback floor
                // must be restored before it reserves that complete receipt.
                transfer_callback_payload(value, budget)
                    .map_err(|error| source.retain_query_resource_error_v18(error).into())
            },
        );
        let (neutral, value, receipt) = resume_callback_panic(optimized, callback_panic)
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
    fn expanded_layout_is_fixed_for_both_real_targets() {
        for target in [TargetProfile::Gfx942, TargetProfile::Gfx950] {
            assert_eq!(selected_layout(target), ExecutionTileLayoutV1::Blocked);
        }
    }

    #[test]
    fn expanded_entry_prepays_complete_error_payload_and_callback_frames() {
        type Consumer = for<'v, 's, 'o, 'scope, 't, 'a, 'p, 'w> fn(
            &'v Source<'s>,
            &'o Original<'scope>,
            &'t Expanded<'o, 'scope>,
            &[AbiRoot<'a>],
            TargetProfile,
            &'p ExpandedPairInputV279<'p, 's, 'o, 'scope>,
            &mut Budget<'w>,
        )
            -> Result<((), usize), Error>;
        type Frame<'a> = (
            &'a Source<'a>,
            &'a Original<'a>,
            &'a Expanded<'a, 'a>,
            &'a [AbiRoot<'a>],
            TargetProfile,
            &'a ExpandedPairInputV279<'a, 'a, 'a, 'a>,
            &'a mut Budget<'a>,
            &'a mut formal_context_v19::PendingConsumerV19<Consumer>,
            &'a std::cell::Cell<usize>,
            &'a mut Option<CallbackPanic>,
        );
        let expected = entry_headers_for_handoff::<(), Consumer, Expanded<'static, 'static>>()
            .unwrap()
            + size_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
            + align_of::<formal_context_v19::PendingConsumerV19<Consumer>>()
            + 2 * size_of::<Frame<'_>>()
            + align_of::<Frame<'_>>()
            + size_of::<AssertUnwindSafe<Frame<'_>>>()
            + size_of::<std::cell::Cell<usize>>()
            + 2 * size_of::<Option<CallbackPanic>>()
            + 2 * size_of::<OptimizerResult<()>>()
            + size_of::<ExecutionTileLayoutV1>()
            + 6 * size_of::<usize>()
            + size_of::<std::thread::Result<Result<((), usize), Error>>>()
            + 2 * size_of::<((), usize)>()
            + size_of::<usize>()
            + size_of::<Result<usize, Resource>>()
            + size_of::<&mut Budget<'_>>()
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

    #[test]
    fn expanded_transfer_includes_inline_header_and_exact_dynamic_credit() {
        for bytes in [0, 17] {
            let mut work = Work::new(2);
            let mut budget = Budget::new(&mut work, 31 + bytes);
            budget.reserve_storage(31 + bytes).unwrap();
            let (value, receipt) = transfer_callback_payload((7usize, bytes), &mut budget).unwrap();
            assert_eq!(value, 7);
            assert_eq!(receipt, size_of::<usize>() + bytes);
            assert_eq!(budget.storage(), 31);
            assert_eq!(budget.work(), 2);
        }
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, 48);
        budget.reserve_storage(48).unwrap();
        assert!(matches!(
            transfer_callback_payload((7usize, 17), &mut budget),
            Err(Resource::Work(_))
        ));
        assert_eq!(budget.storage(), 48);
        let mut work = Work::new(2);
        let mut budget = Budget::new(&mut work, 16);
        budget.reserve_storage(16).unwrap();
        assert!(matches!(
            transfer_callback_payload((7usize, 17), &mut budget),
            Err(Resource::Accounting)
        ));
        assert_eq!(budget.storage(), 16);
        let mut work = Work::new(2);
        let mut budget = Budget::new(&mut work, 31);
        budget.reserve_storage(31).unwrap();
        assert!(matches!(
            transfer_callback_payload((7usize, usize::MAX), &mut budget),
            Err(Resource::Arithmetic)
        ));
        assert_eq!(budget.storage(), 31);
    }

    #[test]
    fn expanded_callback_panic_precedes_later_optimizer_disposition() {
        struct Later<'a>(&'a std::cell::Cell<usize>);
        impl Drop for Later<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
                panic!("later disposition cannot replace the consumer panic");
            }
        }
        for success in [false, true] {
            let drops = std::cell::Cell::new(0);
            let result = if success {
                Ok(Later(&drops))
            } else {
                Err(Later(&drops))
            };
            let panic = catch_unwind(AssertUnwindSafe(|| {
                resume_callback_panic(result, Some(Box::new(259usize)))
            }))
            .err()
            .unwrap();
            assert_eq!(*panic.downcast::<usize>().unwrap(), 259);
            assert_eq!(drops.get(), 1);
        }
        assert_eq!(resume_callback_panic::<_, ()>(Ok(7), None), Ok(7));
        assert_eq!(resume_callback_panic::<(), _>(Err(9), None), Err(9));
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Imports ordinary rustc source, runs the checked fixed-point prefix and
    /// retains its actual expanded successor on the caller's existing account.
    /// The lowerer derives a complete original-root policy table from authentic
    /// operations and expands the module once, retaining scalar roots/helpers.
    /// Residual tile-bearing helpers still require interprocedural admission.
    /// The consumer returns `(value, dynamic_bytes)`: exactly the storage growth
    /// it reserved for the returned value's owned backing, excluding `size_of::<R>()`
    /// and never including borrowed graph owners. The stage transfers that credit
    /// to the optimizer as an unreserved `size_of::<R>() + dynamic_bytes` receipt,
    /// then restores only the surviving backing credit; outer inline frames stay
    /// prepaid. Refinement, native completion and publication remain separate.
    pub(crate) fn with_original_source_expanded_v259<R: 'static, F>(
        self,
        budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'view, 'source, 'original, 'scope, 'tile, 'abi, 'pair, 'work> FnOnce(
            &'view Source<'source>,
            &'original Original<'scope>,
            &'tile Expanded<'original, 'scope>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &'pair ExpandedPairInputV279<'pair, 'source, 'original, 'scope>,
            &mut Budget<'work>,
        )
            -> Result<
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
