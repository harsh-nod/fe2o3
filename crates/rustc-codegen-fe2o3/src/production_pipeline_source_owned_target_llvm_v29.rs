//! Explicit source-owned target LLVM continuation, not default compilation.
use super::target_result::BoundScalarTargetLlvmV19 as BoundTargetLlvm;
use super::target_result::ClosedScalarTargetLlvmV29 as TargetLlvm;
use super::target_result::ScalarCfgTargetLlvmV29 as CfgTargetLlvm;
use super::*;

fn callback_headers<R, F>() -> Result<usize, Resource> {
    callback_headers_for::<R, F, Handoff<'_, '_>, TargetLlvm<'_, '_, '_>>()
}

fn callback_headers_for<R, F, H, T>() -> Result<usize, Resource> {
    type Capture<'a, 'source, 'work, F, H, T> =
        (F, &'a Source<'source>, &'a H, &'a T, &'a mut Budget<'work>);
    [
        size_of::<Capture<'_, '_, '_, F, H, T>>(),
        align_of::<Capture<'_, '_, '_, F, H, T>>(),
        size_of::<AssertUnwindSafe<Capture<'_, '_, '_, F, H, T>>>(),
        size_of::<Result<R, Error>>(),
        align_of::<Result<R, Error>>(),
        size_of::<AssertUnwindSafe<Result<R, Error>>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
        size_of::<formal_context_v19::PendingConsumerV19<F>>(),
        align_of::<formal_context_v19::PendingConsumerV19<F>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

macro_rules! source_target_llvm_continuation_v29 {
    ($method:ident, $entry:ident, $handoff:ident, $target:ident, $lower:ident) => {
        impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
            /// Selects the target solely from the same live-rustc authenticated owner.
            /// The actual original/optimized graphs receive fresh typed formal
            /// checks and borrowed-owner emission. The result is inert target LLVM IR,
            /// not final ISA, a worker request, or artifact/publication authority.
            pub(crate) fn $method<R, F>(self, consume: F) -> Result<R, Error>
            where
                F: for<'handoff, 'view, 'source, 'work> FnOnce(
                    &'view Source<'source>,
                    &'handoff $handoff<'view, 'source>,
                    &$target<'handoff, 'view, 'source>,
                    &mut Budget<'work>,
                ) -> Result<R, Error>,
            {
                self.$entry(
                    WORK_LIMIT,
                    STORAGE_LIMIT,
                    move |source, handoff, _, target, budget| {
                        let mut pending = formal_context_v19::PendingConsumerV19::new(consume);
                        // F is already owned by the outer source/handoff catch. These
                        // additional invocation frames stay paid to the root ledger's
                        // end, independently of the target String's exact receipt.
                        let headers =
                            callback_headers_for::<R, F, $handoff<'_, '_>, $target<'_, '_, '_>>()?;
                        budget.charge_work(headers)?;
                        budget.reserve_storage(headers)?;
                        let native = target_result::$lower(source, handoff, target, budget)?;
                        let borrowed = &native;
                        let callback_budget = &mut *budget;
                        let consume = pending.take();
                        let result = catch_unwind(AssertUnwindSafe(move || {
                            consume(source, handoff, borrowed, callback_budget)
                        }));
                        let settled = native.discard(budget).map_err(Error::from);
                        match result {
                            Ok(Ok(value)) => match settled {
                                Ok(()) => Ok(value),
                                Err(error) => {
                                    discard(value);
                                    Err(error)
                                }
                            },
                            Ok(Err(error)) => Err(error),
                            Err(payload) => resume_unwind(payload),
                        }
                    },
                )
            }
        }
    };
}

source_target_llvm_continuation_v29!(
    with_source_owned_scalar_target_llvm_v29,
    with_source_owned_scalar_limits_v29,
    Handoff,
    TargetLlvm,
    check_and_lower_target_llvm_v18
);
source_target_llvm_continuation_v29!(
    with_original_source_scalar_cfg_target_llvm_v18,
    with_original_source_scalar_cfg_limits_v18,
    CfgHandoff,
    CfgTargetLlvm,
    check_and_lower_scalar_cfg_target_llvm_v18
);
source_target_llvm_continuation_v29!(
    with_original_source_bound_scalar_target_llvm_v19,
    with_original_source_bound_scalar_limits_v19,
    BoundHandoff,
    BoundTargetLlvm,
    check_and_lower_bound_scalar_target_llvm_v19
);

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    fn with_original_source_bound_scalar_limits_v19<R, F>(
        self,
        work_limit: usize,
        storage_limit: usize,
        consume: F,
    ) -> Result<R, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &BoundHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<BoundScalar, R, F>(
            ImportProfile::NominalV35,
            work_limit,
            storage_limit,
            consume,
        )
        .map(SourceOwnedCompilationContinuationV29::into_observation)
    }

    fn with_original_source_scalar_cfg_limits_v18<R, F>(
        self,
        work_limit: usize,
        storage_limit: usize,
        consume: F,
    ) -> Result<R, Error>
    where
        F: for<'view, 'source, 'abi, 'work> FnOnce(
            &'view Source<'source>,
            &CfgHandoff<'view, 'source>,
            &[AbiRoot<'abi>],
            TargetProfile,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        // Same fixed nominal-preserving import policy as the original-source
        // entrance. This does not reencode its producer-selected MIR version.
        self.with_source_owned_custody_policy_v29::<ScalarCfg, R, F>(
            ImportProfile::NominalV35,
            work_limit,
            storage_limit,
            consume,
        )
        .map(SourceOwnedCompilationContinuationV29::into_observation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_llvm_callback_headers_pay_owned_aligned_capture_and_result() {
        #[repr(align(256))]
        struct Large([u8; 16_384]);
        fn check<R, F>(_: &F) -> usize {
            type Capture<'a, 's, 'w, F> = (
                F,
                &'a Source<'s>,
                &'a Handoff<'a, 's>,
                &'a TargetLlvm<'a, 'a, 's>,
                &'a mut Budget<'w>,
            );
            let expected = size_of::<Capture<'_, '_, '_, F>>()
                + align_of::<Capture<'_, '_, '_, F>>()
                + size_of::<AssertUnwindSafe<Capture<'_, '_, '_, F>>>()
                + size_of::<Result<R, Error>>()
                + align_of::<Result<R, Error>>()
                + size_of::<AssertUnwindSafe<Result<R, Error>>>()
                + size_of::<std::thread::Result<Result<R, Error>>>()
                + size_of::<formal_context_v19::PendingConsumerV19<F>>()
                + align_of::<formal_context_v19::PendingConsumerV19<F>>();
            assert_eq!(callback_headers::<R, F>().unwrap(), expected);
            expected
        }
        let small = || ();
        let capture = Large([7; 16_384]);
        let owned = move || capture;
        let baseline = check::<(), _>(&small);
        assert!(check::<(), _>(&owned) >= baseline + 2 * size_of::<Large>());
        assert!(check::<Large, _>(&small) > baseline + 2 * size_of::<Large>());
        assert_eq!(owned().0[0], 7);
    }

    #[test]
    fn scalar_cfg_target_callback_headers_pay_concrete_aligned_capture_and_result() {
        #[repr(align(256))]
        struct Large([u8; 16_384]);
        fn check<R, F>(_: &F) -> usize {
            type Capture<'a, 's, 'w, F> = (
                F,
                &'a Source<'s>,
                &'a CfgHandoff<'a, 's>,
                &'a CfgTargetLlvm<'a, 'a, 's>,
                &'a mut Budget<'w>,
            );
            let expected = size_of::<Capture<'_, '_, '_, F>>()
                + align_of::<Capture<'_, '_, '_, F>>()
                + size_of::<AssertUnwindSafe<Capture<'_, '_, '_, F>>>()
                + size_of::<Result<R, Error>>()
                + align_of::<Result<R, Error>>()
                + size_of::<AssertUnwindSafe<Result<R, Error>>>()
                + size_of::<std::thread::Result<Result<R, Error>>>()
                + size_of::<formal_context_v19::PendingConsumerV19<F>>()
                + align_of::<formal_context_v19::PendingConsumerV19<F>>();
            assert_eq!(
                callback_headers_for::<R, F, CfgHandoff<'_, '_>, CfgTargetLlvm<'_, '_, '_>>()
                    .unwrap(),
                expected
            );
            expected
        }
        let small = || ();
        let capture = Large([7; 16_384]);
        let owned = move || capture;
        let baseline = check::<(), _>(&small);
        assert!(check::<(), _>(&owned) >= baseline + 2 * size_of::<Large>());
        assert!(check::<Large, _>(&small) > baseline + 2 * size_of::<Large>());
        assert_eq!(owned().0[0], 7);
    }
}
