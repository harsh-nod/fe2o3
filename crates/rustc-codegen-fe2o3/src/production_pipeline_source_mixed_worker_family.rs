// One fixed pipeline algorithm instantiated only by concrete compiler owners.
macro_rules! mixed_worker_pipeline_family {
    ($worker_entry:ident, $worker_test_entry:ident, $prepare_composed:path,
     $prepare_consensus:ident, $complete_native:ident) => {
struct MixedWorker<P>(std::marker::PhantomData<P>);
struct WorkerInput;

// Constructed only after the fixed pipeline has retained all five owners.
struct FinalInputs<'a, 'bindings, 'v, 's> {
    source: &'v Source<'s>,
    native: &'a Native<'v, 'v, 'v, 's>,
    composed: &'a Composed<'a, 'v, 'v, 'v, 's>,
    worker: &'a Worker<'a, 'a, 'v, 's, 'a, 'a>,
    context: &'a SourceBindingContextV29<'bindings>,
}

trait FinalConsumer<R, F> {
    fn headers() -> Result<usize, Resource>;
    fn check_reference_obligations(
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        bindings: &AuthenticatedProductionBindings,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        reference_obligations_v69::require_discharged(source, bindings, budget)
    }
    fn consume(
        inputs: FinalInputs<'_, '_, '_, '_>,
        budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<R, Error>;
}

impl<R, F> FinalConsumer<R, F> for WorkerInput
where
    F: for<'n, 'h, 'v, 's, 't, 'wire, 'w> FnOnce(
        &Worker<'n, 'h, 'v, 's, 't, 'wire>,
        &mut Budget<'w>,
    ) -> Result<R, Error>,
{
    fn headers() -> Result<usize, Resource> {
        Ok(0)
    }
    fn consume(
        inputs: FinalInputs<'_, '_, '_, '_>,
        budget: &mut Budget<'_>,
        consume: F,
    ) -> Result<R, Error> {
        consume(inputs.worker, budget)
    }
}
type AdapterCapture<'a, 'bindings, F> = (&'a SourceBindingContextV29<'bindings>, F);
type Pending<F> = formal_context_v19::PendingConsumerV19<F>;
type ForwardCapture<'a, 'bindings, 'v, 's, 'abi, 'w, F> = (
    &'v Source<'s>,
    &'a Consensus<'a, 'a, 'v, 's>,
    &'a [AbiRoot<'abi>],
    &'a SourceBindingContextV29<'bindings>,
    &'a mut Budget<'w>,
    F,
);
type NativeCapture<'a, 'bindings, 'v, 's, 'abi, 'w, F> = (
    &'v Source<'s>,
    &'a Native<'a, 'a, 'v, 's>,
    &'a [AbiRoot<'abi>],
    &'a SourceBindingContextV29<'bindings>,
    &'a mut Budget<'w>,
    &'a mut Pending<F>,
);
type ComposedCapture<'a, 'bindings, 'v, 's, 'abi, 'w, F> = (
    &'v Source<'s>,
    &'a Native<'v, 'v, 'v, 's>,
    &'a Composed<'a, 'v, 'v, 'v, 's>,
    &'a [AbiRoot<'abi>],
    &'a SourceBindingContextV29<'bindings>,
    &'a mut Budget<'w>,
    &'a mut Pending<F>,
);
type TargetCapture<'a, 'bindings, 'v, 's, 'abi, 'w, F> = (
    &'v Source<'s>,
    &'a Native<'v, 'v, 'v, 's>,
    &'a Composed<'a, 'v, 'v, 'v, 's>,
    &'a Target<'a, 'v, 's>,
    &'a [AbiRoot<'abi>],
    &'a SourceBindingContextV29<'bindings>,
    &'a mut Budget<'w>,
    &'a mut Pending<F>,
);
type DescriptorCapture<'a, 'bindings, 'v, 's, 'abi, 'w, F> = (
    &'v Source<'s>,
    &'a Native<'v, 'v, 'v, 's>,
    &'a Composed<'a, 'v, 'v, 'v, 's>,
    &'a Target<'a, 'v, 's>,
    &'a MixedDescriptorWireV28<'a, 'v, 's>,
    &'a [AbiRoot<'abi>],
    &'a SourceBindingContextV29<'bindings>,
    &'a mut Budget<'w>,
    &'a mut Pending<F>,
);
type WorkerCapture<'a, 'bindings, 'v, 's, 'w, F> = (
    FinalInputs<'a, 'bindings, 'v, 's>,
    &'a mut Budget<'w>,
    &'a mut Pending<F>,
);

fn frame_headers<T>() -> Result<usize, Resource> {
    size_of::<T>()
        .checked_add(align_of::<T>())
        .and_then(|n| n.checked_add(size_of::<AssertUnwindSafe<T>>()))
        .ok_or(Resource::Arithmetic)
}

fn headers<R, F, P: FinalConsumer<R, F>>() -> Result<usize, Resource> {
    [
        P::headers()?,
        mixed_fixedpoint_licm_v29::headers::<R, AdapterCapture<'_, '_, F>>()?,
        size_of::<Pending<F>>(),
        align_of::<Pending<F>>(),
        size_of::<FinalInputs<'_, '_, '_, '_>>(),
        align_of::<FinalInputs<'_, '_, '_, '_>>(),
        frame_headers::<NativeCapture<'_, '_, '_, '_, '_, '_, F>>()?,
        frame_headers::<ForwardCapture<'_, '_, '_, '_, '_, '_, F>>()?,
        frame_headers::<ComposedCapture<'_, '_, '_, '_, '_, '_, F>>()?,
        frame_headers::<TargetCapture<'_, '_, '_, '_, '_, '_, F>>()?,
        frame_headers::<DescriptorCapture<'_, '_, '_, '_, '_, '_, F>>()?,
        frame_headers::<WorkerCapture<'_, '_, '_, '_, '_, F>>()?,
        6 * size_of::<std::thread::Result<Result<R, Error>>>(),
        6 * size_of::<Result<(), Error>>(),
        size_of::<
            Result<
                Native<'_, '_, '_, '_>,
                fe2o3_lower_mir_kernel::ProductionMixedLicmCompletionErrorV28,
            >,
        >(),
        size_of::<
            Result<
                Consensus<'_, '_, '_, '_>,
                fe2o3_lower_mir_kernel::ProductionMixedLicmRelocationErrorV28,
            >,
        >(),
        size_of::<
            Result<Composed<'_, '_, '_, '_, '_>, fe2o3_verifier::MixedOptimizerRelocationErrorV28>,
        >(),
        size_of::<Result<Target<'_, '_, '_>, target_result::ClosedScalarTargetLlvmErrorV29>>(),
        size_of::<Result<MixedDescriptorWireV28<'_, '_, '_>, Error>>(),
        size_of::<Result<DeviceDescriptorTableV3<'_>, Error>>(),
        size_of::<
            Result<
                Worker<'_, '_, '_, '_, '_, '_>,
                target_result::mixed_licm_v28::worker_input_v26::MixedWorkerInputErrorV26,
            >,
        >(),
    ]
    .into_iter()
    .try_fold(0usize, |n, part| {
        n.checked_add(part).ok_or(Resource::Arithmetic)
    })
}

fn settled<R, E: Into<Error>>(
    selected: std::thread::Result<Result<R, Error>>,
    released: Result<(), E>,
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

#[cfg(test)]
fn check_capture<T, F>(callback: &F) {
    assert_eq!(std::mem::size_of_val(callback), size_of::<T>());
    assert_eq!(std::mem::align_of_val(callback), align_of::<T>());
}

#[cfg(test)]
fn check_fixedpoint_chain(inputs: &FinalInputs<'_, '_, '_, '_>, budget: &mut Budget<'_>) {
    use sha2::Digest as _;
    let relocation = inputs.native.relocation(budget).unwrap();
    let prefix = relocation.prefix(budget).unwrap().output(budget).unwrap();
    let subject = inputs.composed.subject(budget).unwrap();
    assert_eq!(prefix.execution().policy_version(), 11);
    assert!((1..=32).contains(&prefix.execution().rounds()));
    let digest: [u8; 32] = sha2::Sha256::digest(prefix.execution().canonical_bytes()).into();
    assert_eq!(subject.prefix_execution_identity(), digest);
    assert_eq!(
        subject.graph_identities()[0],
        *inputs.source.canonical(budget).unwrap().identity()
    );
    assert_eq!(subject.graph_identities()[1], *prefix.owner().identity());
    assert_eq!(
        subject.graph_identities()[2],
        *relocation.tail(budget).unwrap().output().identity()
    );
    assert_eq!(
        subject.graph_identities()[3],
        *inputs.native.output(budget).unwrap().identity()
    );
    assert!(inputs.native.store_consensus_v46(budget).unwrap().is_some());
    let generated = std::str::from_utf8(inputs.composed.generated_source(budget).unwrap()).unwrap();
    assert!(generated.contains("mod forwarding_v46 {"));
    assert!(generated.contains("proof fn typed_final_native_source_trace_"));
    assert!(!generated.contains("op: spec_fn(int, int, Seq<int>, int) -> int"));
    assert!(!inputs.composed.authenticates_executed_proof());
    assert!(!inputs.worker.grants_worker_or_artifact_authority());
}

fn consume_final<R, F, P: FinalConsumer<R, F>>(
    source: &Source<'_>,
    relocation: &Relocation<'_, '_, '_>,
    roots: &[AbiRoot<'_>],
    context: &SourceBindingContextV29<'_>,
    budget: &mut Budget<'_>,
    consume: F,
) -> Result<R, Error> {
    let forwarding = relocation
        .$prepare_consensus(budget)
        .map_err(Error::MixedLicm)?;
    let capture: ForwardCapture<'_, '_, '_, '_, '_, '_, F> =
        (source, &forwarding, roots, context, &mut *budget, consume);
    let with_forwarding = move || {
        let (source, forwarding, roots, context, budget, consume) = std::convert::identity(capture);
        consume_forwarded::<R, F, P>(source, forwarding, roots, context, budget, consume)
    };
    #[cfg(test)]
    check_capture::<ForwardCapture<'_, '_, '_, '_, '_, '_, F>, _>(&with_forwarding);
    let selected = catch_unwind(AssertUnwindSafe(with_forwarding));
    let released = forwarding.discard(budget);
    settled(selected, released)
}

fn consume_forwarded<R, F, P: FinalConsumer<R, F>>(
    source: &Source<'_>,
    forwarding: &Consensus<'_, '_, '_, '_>,
    roots: &[AbiRoot<'_>],
    context: &SourceBindingContextV29<'_>,
    budget: &mut Budget<'_>,
    consume: F,
) -> Result<R, Error> {
    let mut pending = Pending::new(consume);
    let native = forwarding
        .$complete_native(budget)
        .map_err(Error::MixedLicmCompletion)?;
    let capture: NativeCapture<'_, '_, '_, '_, '_, '_, F> =
        (source, &native, roots, context, &mut *budget, &mut pending);
    let with_native = move || -> Result<R, Error> {
        let (source, native, roots, context, budget, pending) = std::convert::identity(capture);
        native.check_original_source(source.source_ssa(budget)?, budget)?;
        native.check_original_argument_abi_v26(
            ProductionKernelArgumentAbiInputV18 { roots },
            budget,
        )?;
        let (_, _, endian) = publication::original_mir_v30::checked_runtime_for_native(
            source, native, context, budget,
        )?;
        reference_obligations_v69::with_inputs(
            source.source_ssa(budget)?,
            context.bindings,
            budget,
            |references, budget| {
                let composed = $prepare_composed(
                    source, native, endian, references, budget,
                )
                .map_err(Error::MixedRelocationExpressions)?;
                let capture: ComposedCapture<'_, '_, '_, '_, '_, '_, F> = (
                    source,
                    native,
                    &composed,
                    roots,
                    context,
                    &mut *budget,
                    pending,
                );
                let with_composed = move || -> Result<R, Error> {
                    let (source, native, composed, roots, context, budget, pending) =
                        std::convert::identity(capture);
                    let target = check_and_lower_mixed_target_llvm_v26(
                        source,
                        native,
                        context.bindings.rustc_target.profile(),
                        budget,
                    )?;
                    let capture: TargetCapture<'_, '_, '_, '_, '_, '_, F> = (
                        source,
                        native,
                        composed,
                        &target,
                        roots,
                        context,
                        &mut *budget,
                        pending,
                    );
                    let with_target = move || -> Result<R, Error> {
                        let (source, native, composed, target, roots, context, budget, pending) =
                            std::convert::identity(capture);
                        let wire = descriptor::produce(
                            &context.bindings.typed_descriptor_roots,
                            source,
                            native,
                            context.bindings.rustc_target.profile(),
                            context
                                .bindings
                                .rustc_target
                                .rustc_layout()
                                .default_pointer_width_bits(),
                            budget,
                        )?;
                        let capture: DescriptorCapture<'_, '_, '_, '_, '_, '_, F> = (
                            source,
                            native,
                            composed,
                            target,
                            &wire,
                            roots,
                            context,
                            &mut *budget,
                            pending,
                        );
                        let with_descriptor = move || -> Result<R, Error> {
                            let (
                                source,
                                native,
                                composed,
                                target,
                                wire,
                                roots,
                                context,
                                budget,
                                pending,
                            ) = std::convert::identity(capture);
                            let table = wire.table(budget)?;
                            let worker = prepare_mixed_worker_input_v26(
                                target,
                                ProductionKernelArgumentAbiInputV18 { roots },
                                &table,
                                budget,
                            )?;
                            let inputs = FinalInputs {
                                source,
                                native,
                                composed,
                                worker: &worker,
                                context,
                            };
                            let capture: WorkerCapture<'_, '_, '_, '_, '_, F> =
                                (inputs, &mut *budget, pending);
                            let invoke = move || {
                                let (inputs, budget, pending) = std::convert::identity(capture);
                                // The nominal original/prefix/final request stays owned
                                // through this callback. Replay checks owners and custody;
                                // generated obligations are not executed proof evidence.
                                inputs
                                    .composed
                                    .replay(budget)
                                    .map_err(Error::MixedRelocationExpressions)?;
                                #[cfg(test)]
                                check_fixedpoint_chain(&inputs, budget);
                                P::consume(inputs, budget, pending.take())
                            };
                            #[cfg(test)]
                            check_capture::<WorkerCapture<'_, '_, '_, '_, '_, F>, _>(&invoke);
                            let selected = catch_unwind(AssertUnwindSafe(invoke));
                            let released = worker.discard(budget);
                            settled(selected, released)
                        };
                        #[cfg(test)]
                        check_capture::<DescriptorCapture<'_, '_, '_, '_, '_, '_, F>, _>(
                            &with_descriptor,
                        );
                        let selected = catch_unwind(AssertUnwindSafe(with_descriptor));
                        let released = wire.discard(budget);
                        settled(selected, released)
                    };
                    #[cfg(test)]
                    check_capture::<TargetCapture<'_, '_, '_, '_, '_, '_, F>, _>(&with_target);
                    let selected = catch_unwind(AssertUnwindSafe(with_target));
                    let released = target.discard(budget);
                    settled(selected, released)
                };
                #[cfg(test)]
                check_capture::<ComposedCapture<'_, '_, '_, '_, '_, '_, F>, _>(&with_composed);
                let selected = catch_unwind(AssertUnwindSafe(with_composed));
                let released = composed
                    .discard(budget)
                    .map_err(Error::MixedRelocationExpressions);
                settled(selected, released)
            },
        )
    };
    #[cfg(test)]
    check_capture::<NativeCapture<'_, '_, '_, '_, '_, '_, F>, _>(&with_native);
    let selected = catch_unwind(AssertUnwindSafe(with_native));
    let released = native.discard(budget);
    settled(selected, released)
}

impl<R, F, P: FinalConsumer<R, F>> SourceHandoffPolicyV29<R, F> for MixedWorker<P> {
    fn entry_headers() -> Result<usize, Resource> {
        headers::<R, F, P>()
    }

    fn check_reference_obligations(
        source: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        bindings: &AuthenticatedProductionBindings,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        P::check_reference_obligations(source, bindings, budget)
    }

    fn consume<'view, 'source, 'abi, 'work>(
        source: &'view Source<'source>,
        roots: &[AbiRoot<'abi>],
        context: &SourceBindingContextV29<'_>,
        budget: &mut Budget<'work>,
        consume: F,
    ) -> Result<R, Error> {
        let capture: AdapterCapture<'_, '_, F> = (context, consume);
        let adapter = move |source: &Source<'_>,
                            relocation: &Relocation<'_, '_, '_>,
                            roots: &[AbiRoot<'_>],
                            target,
                            budget: &mut Budget<'_>| {
            let (context, consume) = std::convert::identity(capture);
            if target != context.bindings.rustc_target.profile() {
                return Err(Error::Unsupported("original mixed target binding changed"));
            }
            consume_final::<R, F, P>(source, relocation, roots, context, budget, consume)
        };
        #[cfg(test)]
        check_capture::<AdapterCapture<'_, '_, F>, _>(&adapter);
        <mixed_fixedpoint_licm_v29::MixedLicm as SourceHandoffPolicyV29<R, _>>::consume(
            source, roots, context, budget, adapter,
        )
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Prepare the complete inert mixed Worker input from one original Rust
    /// transaction. The fixed policy cannot accept replacement descriptors,
    /// final graphs, targets, optimizer selectors or fabricated proof receipts.
    /// Its composed original-to-final CFG request remains owned through the
    /// consumer, without executing Verus or closing any Worker admission gate.
    pub(crate) fn $worker_entry<R, F>(
        self,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'n, 'h, 'v, 's, 't, 'wire, 'w> FnOnce(
            &Worker<'n, 'h, 'v, 's, 't, 'wire>,
            &mut Budget<'w>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<MixedWorker<WorkerInput>, R, F>(
            ImportProfile::NominalV35,
            WORK_LIMIT,
            STORAGE_LIMIT,
            consume,
        )
    }

    #[cfg(test)]
    pub(crate) fn $worker_test_entry<R, F>(
        self,
        work: usize,
        storage: usize,
        consume: F,
    ) -> Result<SourceOwnedCompilationContinuationV29<R>, Error>
    where
        F: for<'n, 'h, 'v, 's, 't, 'wire, 'w> FnOnce(
            &Worker<'n, 'h, 'v, 's, 't, 'wire>,
            &mut Budget<'w>,
        ) -> Result<R, Error>,
    {
        self.with_source_owned_custody_policy_v29::<MixedWorker<WorkerInput>, R, F>(
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
    fn composed_worker_frames_have_an_independent_header_oracle() {
        type Consumer = for<'n, 'h, 'v, 's, 't, 'wire, 'w> fn(
            &Worker<'n, 'h, 'v, 's, 't, 'wire>,
            &mut Budget<'w>,
        ) -> Result<usize, Error>;
        type NativeFields<'a> = (&'a (), &'a (), &'a [()], &'a (), &'a mut (), &'a mut ());
        type ComposedFields<'a> = (
            &'a (),
            &'a (),
            &'a (),
            &'a [()],
            &'a (),
            &'a mut (),
            &'a mut (),
        );
        type TargetFields<'a> = (
            &'a (),
            &'a (),
            &'a (),
            &'a (),
            &'a [()],
            &'a (),
            &'a mut (),
            &'a mut (),
        );
        type DescriptorFields<'a> = (
            &'a (),
            &'a (),
            &'a (),
            &'a (),
            &'a (),
            &'a [()],
            &'a (),
            &'a mut (),
            &'a mut (),
        );
        type InputFields<'a> = (&'a (), &'a (), &'a (), &'a (), &'a ());
        type WorkerFields<'a> = (InputFields<'a>, &'a mut (), &'a mut ());
        type ForwardFields<'a> = (&'a (), &'a (), &'a [()], &'a (), &'a mut (), Consumer);
        let frames = [
            (
                size_of::<ForwardFields<'_>>(),
                align_of::<ForwardFields<'_>>(),
            ),
            (
                size_of::<NativeFields<'_>>(),
                align_of::<NativeFields<'_>>(),
            ),
            (
                size_of::<ComposedFields<'_>>(),
                align_of::<ComposedFields<'_>>(),
            ),
            (
                size_of::<TargetFields<'_>>(),
                align_of::<TargetFields<'_>>(),
            ),
            (
                size_of::<DescriptorFields<'_>>(),
                align_of::<DescriptorFields<'_>>(),
            ),
            (
                size_of::<WorkerFields<'_>>(),
                align_of::<WorkerFields<'_>>(),
            ),
        ];
        let captures = frames
            .into_iter()
            .map(|(size, align)| 2 * size + align)
            .sum::<usize>();
        let expected = mixed_fixedpoint_licm_v29::headers::<usize, (&(), Consumer)>().unwrap()
            + size_of::<Pending<Consumer>>()
            + align_of::<Pending<Consumer>>()
            + size_of::<InputFields<'_>>()
            + align_of::<InputFields<'_>>()
            + captures
            + 6 * size_of::<std::thread::Result<Result<usize, Error>>>()
            + 6 * size_of::<Result<(), Error>>()
            + size_of::<
                Result<
                    Native<'_, '_, '_, '_>,
                    fe2o3_lower_mir_kernel::ProductionMixedLicmCompletionErrorV28,
                >,
            >()
            + size_of::<
                Result<
                    Consensus<'_, '_, '_, '_>,
                    fe2o3_lower_mir_kernel::ProductionMixedLicmRelocationErrorV28,
                >,
            >()
            + size_of::<
                Result<
                    Composed<'_, '_, '_, '_, '_>,
                    fe2o3_verifier::MixedOptimizerRelocationErrorV28,
                >,
            >()
            + size_of::<Result<Target<'_, '_, '_>, target_result::ClosedScalarTargetLlvmErrorV29>>(
            )
            + size_of::<Result<MixedDescriptorWireV28<'_, '_, '_>, Error>>()
            + size_of::<Result<DeviceDescriptorTableV3<'_>, Error>>()
            + size_of::<
                Result<
                    Worker<'_, '_, '_, '_, '_, '_>,
                    target_result::mixed_licm_v28::worker_input_v26::MixedWorkerInputErrorV26,
                >,
            >();
        assert_eq!(headers::<usize, Consumer, WorkerInput>().unwrap(), expected);
    }

    #[test]
    fn composed_worker_cleanup_refusal_discards_success_once_even_when_drop_panics() {
        use std::cell::Cell;
        struct ResultOwner<'a>(&'a Cell<usize>);
        impl Drop for ResultOwner<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
                std::panic::panic_any(811u32);
            }
        }
        let drops = Cell::new(0);
        let result = settled(
            Ok(Ok(ResultOwner(&drops))),
            Err(Error::MixedRelocationExpressions(
                fe2o3_verifier::MixedOptimizerRelocationErrorV28::Binding("composed custody"),
            )),
        );
        assert!(matches!(
            result,
            Err(Error::MixedRelocationExpressions(
                fe2o3_verifier::MixedOptimizerRelocationErrorV28::Binding("composed custody")
            ))
        ));
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn worker_scope_preserves_selected_errors_and_unwinds_over_cleanup_refusals() {
        let selected: std::thread::Result<Result<(), Error>> =
            Ok(Err(Error::Unsupported("consumer")));
        assert!(matches!(
            settled(selected, Err(Resource::Accounting)),
            Err(Error::Unsupported("consumer"))
        ));
        let result = catch_unwind(|| {
            settled::<(), Resource>(Err(Box::new(47usize)), Err(Resource::Accounting))
        });
        assert_eq!(*result.unwrap_err().downcast::<usize>().unwrap(), 47);
        assert!(matches!(
            settled(Ok(Ok(())), Err(Resource::Accounting)),
            Err(Error::Resource(Resource::Accounting))
        ));
    }
}

    };
}
