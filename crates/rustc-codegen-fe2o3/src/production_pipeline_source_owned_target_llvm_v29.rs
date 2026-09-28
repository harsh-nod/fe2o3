//! Explicit source-owned target LLVM continuation, not default compilation.
use super::target_result::ClosedScalarTargetLlvmV29 as TargetLlvm;
use super::*;

fn callback_headers<R, F>() -> Result<usize, Resource> {
    type Capture<'a, 'source, 'work, F> = (
        F,
        &'a Source<'source>,
        &'a Handoff<'a, 'source>,
        &'a TargetLlvm<'a, 'a, 'source>,
        &'a mut Budget<'work>,
    );
    [
        size_of::<Capture<'_, '_, '_, F>>(),
        align_of::<Capture<'_, '_, '_, F>>(),
        size_of::<AssertUnwindSafe<Capture<'_, '_, '_, F>>>(),
        size_of::<Result<R, Error>>(),
        align_of::<Result<R, Error>>(),
        size_of::<AssertUnwindSafe<Result<R, Error>>>(),
        size_of::<std::thread::Result<Result<R, Error>>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Selects the target solely from the same live-rustc authenticated owner.
    /// The actual original/optimized graphs receive fresh typed formal
    /// checks and borrowed-owner emission. The result is inert target LLVM IR,
    /// not final ISA, a worker request, or artifact/publication authority.
    pub(crate) fn with_source_owned_scalar_target_llvm_v29<R, F>(
        self,
        consume: F,
    ) -> Result<R, Error>
    where
        F: for<'handoff, 'view, 'source, 'work> FnOnce(
            &'view Source<'source>,
            &'handoff Handoff<'view, 'source>,
            &TargetLlvm<'handoff, 'view, 'source>,
            &mut Budget<'work>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_scalar_limits_v29(
            WORK_LIMIT,
            STORAGE_LIMIT,
            move |source, handoff, _, target, budget| {
                // F is already owned by the outer source/handoff catch. These
                // additional invocation frames stay paid to the root ledger's
                // end, independently of the target String's exact receipt.
                let headers = callback_headers::<R, F>()?;
                budget.charge_work(headers)?;
                budget.reserve_storage(headers)?;
                let native = target_result::check_and_lower_target_llvm_v18(
                    source, handoff, target, budget,
                )?;
                let borrowed = &native;
                let callback_budget = &mut *budget;
                let result = catch_unwind(AssertUnwindSafe(move || {
                    consume(source, handoff, borrowed, callback_budget)
                }));
                let settled = native.discard(budget).map_err(Error::from);
                match result {
                    Ok(Ok(value)) => {
                        settled?;
                        Ok(value)
                    }
                    Ok(Err(error)) => Err(error),
                    Err(payload) => resume_unwind(payload),
                }
            },
        )
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
                + size_of::<std::thread::Result<Result<R, Error>>>();
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
}
