// Same admitted runtime, execution and cleanup for each concrete typed request.
macro_rules! mixed_publication_execution_family {
    ($prepare_execution:ident) => {
        type OuterCapture<'a, 'w, F, P> = (
            &'a P,
            &'a Runtime,
            &'a u32,
            &'a mut Budget<'w>,
            &'a mut Pending<F>,
        );
        type InnerCapture<'a, 'w, F, P, E> = (&'a P, &'a E, &'a mut Budget<'w>, &'a mut Pending<F>);
        fn headers<R, F, P, E>() -> Result<usize, Resource> {
            [
                size_of::<OuterCapture<'_, '_, F, P>>(),
                align_of::<OuterCapture<'_, '_, F, P>>(),
                size_of::<AssertUnwindSafe<OuterCapture<'_, '_, F, P>>>(),
                size_of::<InnerCapture<'_, '_, F, P, E>>(),
                align_of::<InnerCapture<'_, '_, F, P, E>>(),
                size_of::<AssertUnwindSafe<InnerCapture<'_, '_, F, P, E>>>(),
                size_of::<Pending<F>>(),
                align_of::<Pending<F>>(),
                2 * size_of::<Result<R, Error>>(),
                2 * size_of::<std::thread::Result<Result<R, Error>>>(),
                2 * size_of::<Result<(), Error>>(),
                size_of::<ExecutedProtectedMixedPublicationV29<'_, '_, '_, '_, '_>>(),
                align_of::<ExecutedProtectedMixedPublicationV29<'_, '_, '_, '_, '_>>(),
                size_of::<Result<ExecutedProtectedMixedPublicationV29<'_, '_, '_, '_, '_>, Error>>(
                ),
            ]
            .into_iter()
            .try_fold(0usize, |n, bytes| {
                n.checked_add(bytes).ok_or(Resource::Arithmetic)
            })
        }

        #[cfg(test)]
        impl<'a, 'v, 's> PreparedMixedPublicationV28<'a, 'v, 's> {
            pub(crate) fn prepare_execution_for_test<'r>(
                &'r self,
                budget: &mut Budget<'_>,
                timeout_seconds: u32,
            ) -> Result<PreparedExecution<'r, 'a, 'v, 'v, 'v, 's>, Error> {
                self.check(budget)?;
                self.inputs
                    .composed
                    .$prepare_execution(budget, timeout_seconds)
                    .map_err(Error::MixedRelocationExpressions)
            }
        }

        impl<'a, 'v, 's> ProtectedMixedPublicationV28<'a, 'v, 's> {
            /// Revalidate original protected compiler custody, execute the exact
            /// original/Policy11/final CFG model, and lend the joined nominal owner to
            /// the existing publication continuation. Its finalizer path mandatorily
            /// checks the actual Worker; this is still not publication admission.
            pub(crate) fn with_executed_composition_v29<R, F>(
                &self,
                runtime: &Runtime,
                timeout_seconds: u32,
                budget: &mut Budget<'_>,
                consume: F,
            ) -> Result<R, Error>
            where
                F: for<'e, 'r, 'w> FnOnce(
                    &ExecutedProtectedMixedPublicationV29<'e, 'r, 'a, 'v, 's>,
                    &mut Budget<'w>,
                ) -> Result<R, Error>,
            {
                let mut consume = Pending::new(consume);
                self.revalidate(budget)?;
                let source = self.prepared.inputs.source;
                let floor = budget.storage();
                let header = headers::<R, F, Self, Executed<'_, 'a, 'v, 'v, 'v, 's>>()?;
                budget
                    .reserve_storage(header)
                    .map_err(|error| source.retain_query_resource_error_v18(error))?;
                let capture: OuterCapture<'_, '_, F, Self> =
                    (self, runtime, &timeout_seconds, &mut *budget, &mut consume);
                let execute = move || {
                    let (candidate, runtime, timeout_seconds, budget, consume) =
                        std::convert::identity(capture);
                    let pending = candidate
                        .prepared
                        .inputs
                        .composed
                        .$prepare_execution(budget, *timeout_seconds)
                        .map_err(Error::MixedRelocationExpressions)?;
                    let executed = pending
                        .execute(runtime, budget)
                        .map_err(Error::MixedRelocationExpressions)?;
                    let capture: InnerCapture<'_, '_, F, Self, Executed<'_, 'a, 'v, 'v, 'v, 's>> =
                        (candidate, &executed, &mut *budget, &mut *consume);
                    let callback = move || {
                        let (candidate, executed, budget, consume) =
                            std::convert::identity(capture);
                        executed
                            .replay_signed_receipt(budget)
                            .map_err(Error::MixedRelocationExpressions)?;
                        candidate.revalidate(budget)?;
                        let joined =
                            ExecutedProtectedMixedPublicationV29::new(candidate, executed, budget)?;
                        consume.take()(&joined, budget)
                    };
                    #[cfg(test)]
                    {
                        assert_eq!(
                            std::mem::size_of_val(&callback),
                            size_of::<
                                InnerCapture<'_, '_, F, Self, Executed<'_, 'a, 'v, 'v, 'v, 's>>,
                            >()
                        );
                        assert_eq!(
                            std::mem::align_of_val(&callback),
                            align_of::<
                                InnerCapture<'_, '_, F, Self, Executed<'_, 'a, 'v, 'v, 'v, 's>>,
                            >()
                        );
                    }
                    let observed = catch_unwind(AssertUnwindSafe(callback));
                    let settled = executed
                        .discard(budget)
                        .map_err(Error::MixedRelocationExpressions);
                    match observed {
                        Ok(Ok(result)) => {
                            settled?;
                            Ok(result)
                        }
                        Ok(Err(error)) => {
                            let _ = settled;
                            Err(error)
                        }
                        Err(payload) => resume_unwind(payload),
                    }
                };
                #[cfg(test)]
                {
                    assert_eq!(
                        std::mem::size_of_val(&execute),
                        size_of::<OuterCapture<'_, '_, F, Self>>()
                    );
                    assert_eq!(
                        std::mem::align_of_val(&execute),
                        align_of::<OuterCapture<'_, '_, F, Self>>()
                    );
                }
                let selected = catch_unwind(AssertUnwindSafe(execute));
                // The pending closure and every executed receipt are dead before the
                // wrapper frame is refunded. Existing source custody refuses repair by
                // restoring storage after a foreign-ledger or missing-credit query.
                drop(consume);
                let custody = self.prepared.inputs.native.observe_retained_storage_v28(
                    floor.checked_add(header).ok_or(Resource::Arithmetic)?,
                    budget,
                );
                let settled = custody.and_then(|()| {
                    if budget.storage() != floor + header {
                        return Err(source.retain_query_resource_error_v18(Resource::Accounting));
                    }
                    budget
                        .release_storage(header)
                        .map_err(|error| source.retain_query_resource_error_v18(error))
                });
                match selected {
                    Ok(Ok(value)) => {
                        settled?;
                        Ok(value)
                    }
                    Ok(Err(error)) => {
                        let _ = settled;
                        Err(error)
                    }
                    Err(payload) => resume_unwind(payload),
                }
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            #[test]
            fn mixed_publication_execution_frames_cover_both_catches_and_pending_callback() {
                type F = fn();
                type R = [u8; 37];
                let pointers = (5 + 4) * size_of::<&()>();
                let expected = 2 * pointers
                    + 2 * align_of::<&()>()
                    + size_of::<Option<F>>()
                    + align_of::<Option<F>>()
                    + 2 * size_of::<Result<R, Error>>()
                    + 2 * size_of::<std::thread::Result<Result<R, Error>>>()
                    + 2 * size_of::<Result<(), Error>>();
                let expected = expected
                    + size_of::<(
                        &ProtectedMixedPublicationV28<'_, '_, '_>,
                        &Executed<'_, '_, '_, '_, '_, '_>,
                    )>()
                    + align_of::<(
                        &ProtectedMixedPublicationV28<'_, '_, '_>,
                        &Executed<'_, '_, '_, '_, '_, '_>,
                    )>()
                    + size_of::<
                        Result<ExecutedProtectedMixedPublicationV29<'_, '_, '_, '_, '_>, Error>,
                    >();
                assert_eq!(size_of::<Pending<F>>(), size_of::<Option<F>>());
                assert_eq!(
                    headers::<
                        R,
                        F,
                        ProtectedMixedPublicationV28<'static, 'static, 'static>,
                        Executed<'static, 'static, 'static, 'static, 'static, 'static>,
                    >()
                    .unwrap(),
                    expected
                );
            }
        }
    };
}
