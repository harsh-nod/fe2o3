pub(super) struct MixedLicm;

type PrepareCapture<'a, 'view, 'source, 'abi, 'work, F> = (
    &'view Source<'source>,
    &'a Prefix<'view, 'source>,
    &'a [AbiRoot<'abi>],
    TargetProfile,
    &'a mut Budget<'work>,
    &'a mut formal_context_v19::PendingConsumerV19<F>,
);

type ConsumerCapture<'a, 'prefix, 'view, 'source, 'abi, 'work, F> = (
    &'view Source<'source>,
    &'a Relocation<'prefix, 'view, 'source>,
    &'a [AbiRoot<'abi>],
    TargetProfile,
    &'a mut Budget<'work>,
    &'a mut formal_context_v19::PendingConsumerV19<F>,
);

pub(super) fn headers<R, F>() -> Result<usize, Resource> {
    [
        entry_headers_for_handoff::<R, F, Relocation<'static, 'static, 'static>>()?,
        size_of::<formal_context_v19::PendingConsumerV19<F>>(),
        align_of::<formal_context_v19::PendingConsumerV19<F>>(),
        formal_context_v19::launch_context_headers_v19()?,
        size_of::<Vec<ExplicitLaunchExtent>>(),
        align_of::<Vec<ExplicitLaunchExtent>>(),
        size_of::<fe2o3_kernel_ir::FormalIndexWidth>(),
        size_of::<ProductionMixedSourceHandoffErrorV26>(),
        size_of::<PrepareCapture<'_, '_, '_, '_, '_, F>>(),
        align_of::<PrepareCapture<'_, '_, '_, '_, '_, F>>(),
        size_of::<AssertUnwindSafe<PrepareCapture<'_, '_, '_, '_, '_, F>>>(),
        size_of::<ConsumerCapture<'_, '_, '_, '_, '_, '_, F>>(),
        align_of::<ConsumerCapture<'_, '_, '_, '_, '_, '_, F>>(),
        size_of::<AssertUnwindSafe<ConsumerCapture<'_, '_, '_, '_, '_, '_, F>>>(),
        2 * size_of::<std::thread::Result<Result<R, Error>>>(),
        2 * size_of::<Result<(), ProductionSourceOwnedViewErrorV18>>(),
        size_of::<Result<Relocation<'_, '_, '_>, MotionError>>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

pub(super) fn settled<R>(
    selected: std::thread::Result<Result<R, Error>>,
    released: Result<(), ProductionSourceOwnedViewErrorV18>,
) -> Result<R, Error> {
    match selected {
        Ok(Ok(value)) => match released {
            Ok(()) => Ok(value),
            Err(error) => {
                formal_context_v19::discard(value);
                Err(error.into())
            }
        },
        Ok(Err(error)) => Err(error),
        Err(payload) => resume_unwind(payload),
    }
}

impl<R, F> SourceHandoffPolicyV29<R, F> for MixedLicm
where
    F: for<'prefix, 'view, 'source, 'abi, 'work> FnOnce(
        &'view Source<'source>,
        &Relocation<'prefix, 'view, 'source>,
        &[AbiRoot<'abi>],
        TargetProfile,
        &mut Budget<'work>,
    ) -> Result<R, Error>,
{
    fn entry_headers() -> Result<usize, Resource> {
        headers::<R, F>()
    }

    fn consume<'view, 'source, 'abi, 'work>(
        source: &'view Source<'source>,
        roots: &[AbiRoot<'abi>],
        context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'work>,
        consume: F,
    ) -> Result<R, Error> {
        let mut pending = formal_context_v19::PendingConsumerV19::new(consume);
        let (physical, width) = context.launches(source, budget)?;
        let launches = mixed_worklist_v26::explicit_mixed_launches_v26(&physical, budget)?;
        let prefix = prepare_prefix(
            source,
            ProductionKernelArgumentAbiInputV18 { roots },
            &launches,
            width,
            budget,
        )?;
        let target = context.bindings.rustc_target.profile();
        // Each owner is settled before its enclosing owner. Preparation,
        // selected consumer errors and raw unwinds all use the same path.
        let prepare_capture: PrepareCapture<'_, '_, '_, '_, '_, F> =
            (source, &prefix, roots, target, &mut *budget, &mut pending);
        let prepare = move || -> Result<R, Error> {
            let (source, prefix, roots, target, budget, pending) =
                std::convert::identity(prepare_capture);
            let relocation = prepare_relocation(prefix, budget)?;
            let consumer_capture: ConsumerCapture<'_, '_, '_, '_, '_, '_, F> =
                (source, &relocation, roots, target, &mut *budget, pending);
            let invoke = move || {
                let (source, relocation, roots, target, budget, pending) =
                    std::convert::identity(consumer_capture);
                relocation.check_original_source(source.source_ssa(budget)?, budget)?;
                relocation.replay(budget)?;
                pending.take()(source, relocation, roots, target, budget)
            };
            #[cfg(test)]
            check_capture_layout::<ConsumerCapture<'_, '_, '_, '_, '_, '_, F>, _>(&invoke);
            let selected = catch_unwind(AssertUnwindSafe(invoke));
            let released = relocation.discard(budget);
            settled(selected, released)
        };
        #[cfg(test)]
        check_capture_layout::<PrepareCapture<'_, '_, '_, '_, '_, F>, _>(&prepare);
        let selected = catch_unwind(AssertUnwindSafe(prepare));
        let released = prefix.discard(budget);
        settled(selected, released)
    }
}

#[cfg(test)]
fn check_capture_layout<T, F>(callback: &F) {
    assert_eq!(std::mem::size_of_val(callback), size_of::<T>());
    assert_eq!(std::mem::align_of_val(callback), align_of::<T>());
}
