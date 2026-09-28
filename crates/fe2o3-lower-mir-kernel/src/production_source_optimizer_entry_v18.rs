/// Failure while consuming the actual fixed optimizer under original source custody.
#[derive(Debug)]
pub enum ProductionSourceOptimizationErrorV18<E> {
    /// Original-source identity, custody, or construction failed before an
    /// optimized owner and its callback result could be retained together.
    Source(ProductionSourceOwnedViewErrorV18),
    /// The actual fixed-pass executor or its checked observation refused the
    /// input/output transition; no adopted optimized owner is returned.
    Observation(fe2o3_pliron::KirNeutralOptimizationErrorV18),
    /// Consuming transition adoption or its origin callback failed. An origin
    /// error retains the caller's exact `E`, including its first refusal.
    Adoption(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1<E>),
}

impl<E> From<ProductionSourceOwnedViewErrorV18> for ProductionSourceOptimizationErrorV18<E> {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

impl<E: fmt::Display> fmt::Display for ProductionSourceOptimizationErrorV18<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Observation(error) => error.fmt(out),
            Self::Adoption(error) => error.fmt(out),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ProductionSourceOptimizationErrorV18<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Observation(error) => Some(error),
            Self::Adoption(error) => Some(error),
        }
    }
}

fn observed_optimizer_refusal_v18(
    error: &fe2o3_pliron::KirNeutralOptimizationErrorV18,
) -> SourceOwnedQueryFailureV18 {
    use fe2o3_pliron::{
        KirBridgeErrorV12 as Bridge12, KirBridgeErrorV18 as Bridge18,
        KirNeutralOptimizationErrorV18 as Error, KirOptimizationMapErrorV12 as Mapping,
        PlironOptimizationErrorV12 as Execution,
    };
    let resource = match error {
        Error::Resource(error)
        | Error::Bridge(Bridge18::Resource(error))
        | Error::Bridge(Bridge18::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(error),
        ))
        | Error::Execution(Execution::Resources(error))
        | Error::Execution(Execution::Bridge(Bridge12::Resource(error)))
        | Error::Execution(Execution::Bridge(Bridge12::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Resource(error),
        )))
        | Error::Execution(Execution::Mapping(Mapping::Resources(error)))
        | Error::Mapping(Mapping::Resources(error)) => Some(*error),
        Error::Execution(Execution::Accounting) => Some(ArgumentResourceV1::Accounting),
        Error::Bridge(Bridge18::Allocation)
        | Error::Mapping(Mapping::Allocation)
        | Error::Execution(Execution::Mapping(Mapping::Allocation)) => {
            Some(ArgumentResourceV1::Allocation)
        }
        Error::Mapping(Mapping::Arithmetic)
        | Error::Execution(Execution::Mapping(Mapping::Arithmetic)) => {
            Some(ArgumentResourceV1::Arithmetic)
        }
        Error::Bridge(Bridge18::Bridge(_) | Bridge18::Canonical(_) | Bridge18::SessionSetup)
        | Error::Execution(
            Execution::Bridge(_)
            | Execution::Execution(_)
            | Execution::Mapping(_)
            | Execution::AlreadyExecuted,
        )
        | Error::Pass(_)
        | Error::Mapping(
            Mapping::Limit
            | Mapping::Identity
            | Mapping::Lifecycle
            | Mapping::Coverage
            | Mapping::Passes
            | Mapping::Relation
            | Mapping::UnsupportedMutation,
        )
        | Error::Endpoint
        | Error::Limit
        | Error::Panicked => None,
    };
    match resource {
        Some(error) => SourceOwnedQueryFailureV18::Resource(error),
        None => SourceOwnedQueryFailureV18::Binding("actual source optimizer observation rejected"),
    }
}

impl ProductionSourceOwnedViewV18<'_> {
    /// Executes the same checked optimization entrance and retains both returned
    /// owners on this continuing ledger before releasing the original source.
    /// The caller must keep the reservation until both owners are destroyed or
    /// their exact storage is transferred. This is not ranked or target authority.
    pub fn with_retained_checked_optimization_v18<T: 'static, E: 'static, F>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: F,
    ) -> Result<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
        F: for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    {
        self.with_retained_checked_optimization_policy_v18::<ScalarSourceOptimizerV18, T, E, F>(
            budget, consume,
        )
    }

    fn with_retained_checked_optimization_policy_v18<
        P: SourceOptimizerPolicyV18,
        T: 'static,
        E: 'static,
        F,
    >(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: F,
    ) -> Result<
        (P::Output, T, fe2o3_pliron::KirNeutralOwnedOriginStorageV1),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
        F: for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    {
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let accepted = std::cell::Cell::new(0_usize);
        let header_bytes = std::cell::Cell::new(0);
        let caught = {
            let budget = &mut *budget;
            let accepted = &accepted;
            let header_bytes = &header_bytes;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                self.query(budget)?;
                let headers = self.retain_query(
                    optimized_source_consumer_resources_v18::retained_entry_headers_typed::<
                        T,
                        E,
                        F,
                        P::Output,
                    >()
                    .map_err(Into::into),
                )?;
                self.retain_query(
                    optimized_source_consumer_resources_v18::reserve_entry(
                        accepted, headers, budget,
                    )
                    .map_err(Into::into),
                )?;
                header_bytes.set(headers);
                self.retain_query(
                    budget
                        .charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)
                        .map_err(Into::into),
                )?;
                let output = self
                    .with_checked_optimization_policy_v18::<P, T, E>(budget, consume)
                    .map_err(SourceConsumerErrorV18)?;
                // No controlled allocation occurs between the unreserved transfer
                // and this single reservation, including on arithmetic failure.
                let reservation = self.retain_query((|| {
                    let bytes =
                        optimized_source_consumer_resources_v18::optimizer_transfer_storage(
                            P::checked_storage(&output.0),
                            output.2.retained_storage(),
                        )?;
                    optimized_source_consumer_resources_v18::reserve_entry(accepted, bytes, budget)
                        .map_err(Into::into)
                })());
                if let Err(error) = reservation {
                    source_reference_discard_v29(output);
                    return Err(SourceConsumerErrorV18(
                        ProductionSourceOptimizationErrorV18::Source(error),
                    ));
                }
                if let Err(error) = self.guard.check(self.owner, self.cleanup, budget) {
                    source_reference_discard_v29(output);
                    return Err(SourceConsumerErrorV18(
                        ProductionSourceOptimizationErrorV18::Source(error),
                    ));
                }
                Ok::<_, SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>>(output)
            }))
        };
        let custody = optimized_source_consumer_resources_v18::entry_custody(
            self.cleanup,
            budget,
            floor,
            accepted.get(),
            slot,
            ledger,
        )
        .map_err(Into::into)
        .and(self.guard.observe_custody(self.cleanup, budget));
        match caught {
            Ok(Ok(output)) if custody.is_ok() => {
                if let Err(error) = self.retain_query(
                    budget
                        .release_storage(header_bytes.get())
                        .map_err(Into::into),
                ) {
                    self.cleanup.deny_refund();
                    source_reference_discard_v29(output);
                    return Err(ProductionSourceOptimizationErrorV18::Source(error));
                }
                Ok(output)
            }
            Ok(Ok(output)) => {
                source_reference_discard_v29(output);
                let error = self
                    .retain_query::<()>(Err(ArgumentResourceV1::Accounting.into()))
                    .unwrap_err();
                Err(ProductionSourceOptimizationErrorV18::Source(error))
            }
            Ok(Err(error)) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                Err(error.0)
            }
            Err(payload) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                std::panic::resume_unwind(payload)
            }
        }
    }

    /// Executes the fixed V18 pass sequence on this actual source-owned graph,
    /// then independently adopts its observed successor. Both correspondence
    /// borrows end before the checked output and callback payload are returned.
    ///
    /// The callback's owned result uses the existing Stage A transfer contract:
    /// all scratch must be dropped, its incoming budget floor restored, and its
    /// complete retained capacity (including the inline value) returned. Reserve
    /// both returned receipts before another controlled allocation. Neither this
    /// entrance nor the returned optimizer owner grants source-final authority.
    pub fn with_checked_optimization_v18<T: 'static, E: 'static>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    ) -> Result<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerV18,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_checked_optimization_policy_v18::<ScalarSourceOptimizerV18, T, E>(budget, consume)
    }

    /// Execute the distinct fixed integer-neutral/DCE continuation while the
    /// original source is live. This is not Policy3 evidence or final admission.
    /// The same transfer, callback-floor, and sticky-error contract applies as
    /// `with_checked_optimization_v18`; memory/CFG payloads stay independently checked.
    pub fn with_checked_integer_optimization_v18<T: 'static, E: 'static>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    ) -> Result<
        (
            fe2o3_pliron::CheckedNeutralKernelIrOwnerIntegerContinuationV18,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_checked_optimization_policy_v18::<IntegerSourceOptimizerV18, T, E>(
            budget, consume,
        )
    }

    fn with_checked_optimization_policy_v18<P: SourceOptimizerPolicyV18, T: 'static, E: 'static>(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: impl for<'scope, 'work> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &ProductionOptimizedSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(T, usize), E>,
    ) -> Result<
        (P::Output, T, fe2o3_pliron::KirNeutralOwnedOriginStorageV1),
        ProductionSourceOptimizationErrorV18<E>,
    >
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let accepted = std::cell::Cell::new(0_usize);
        let caught = {
            let budget = &mut *budget;
            let accepted = &accepted;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                self.query(budget)?;
                let headers =
                    self.retain_query(P::headers::<T, E, _>(&consume).map_err(Into::into))?;
                self.retain_query(
                    optimized_source_consumer_resources_v18::reserve_entry(
                        accepted, headers, budget,
                    )
                    .map_err(Into::into),
                )?;
                let adoption_floor = self.retain_query(
                    floor
                        .checked_add(headers)
                        .ok_or_else(|| ArgumentResourceV1::Accounting.into()),
                )?;
                self.retain_query(
                    budget
                        .charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)
                        .map_err(Into::into),
                )?;
                let layouts = self.limits(budget)?.storage_layout_limits();
                let observed = P::observe(&self.owner.inner.pending.graph, layouts, budget)
                    .map_err(|error| {
                        let refusal = observed_optimizer_refusal_v18(&error);
                        if matches!(
                            refusal,
                            SourceOwnedQueryFailureV18::Resource(ArgumentResourceV1::Accounting)
                        ) {
                            self.cleanup.deny_refund();
                        }
                        let _ = self.guard.reject::<()>(refusal);
                        SourceConsumerErrorV18(ProductionSourceOptimizationErrorV18::Observation(
                            error,
                        ))
                    })?;
                // The observation and adoption must see this same budget object.
                self.retain_query(
                    optimized_source_consumer_resources_v18::reserve_entry(
                        accepted,
                        P::storage(&observed),
                        budget,
                    )
                    .map_err(Into::into),
                )?;
                Ok::<_, SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>>((
                    observed,
                    headers,
                    adoption_floor,
                    consume,
                ))
            }))
        };
        let entry_custody = optimized_source_consumer_resources_v18::entry_custody(
            self.cleanup,
            budget,
            floor,
            accepted.get(),
            slot,
            ledger,
        )
        .map_err(Into::into)
        .and(self.guard.observe_custody(self.cleanup, budget));
        let (observed, headers, adoption_floor, consume) = match caught {
            Ok(Ok(entry)) if entry_custody.is_ok() => entry,
            Ok(Ok(entry)) => {
                let disposed =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(entry)));
                if let Err(payload) = disposed {
                    std::panic::resume_unwind(payload);
                }
                let error = self
                    .retain_query::<()>(Err(ArgumentResourceV1::Accounting.into()))
                    .unwrap_err();
                return Err(ProductionSourceOptimizationErrorV18::Source(error));
            }
            Ok(Err(error)) => {
                if entry_custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                return Err(error.0);
            }
            Err(payload) => {
                if entry_custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                std::panic::resume_unwind(payload);
            }
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let adopted = P::adopt(observed, budget, |checked, budget| {
                self.with_ranked_correspondence_v18(checked.input(), budget, |original, budget| {
                    original.with_optimized_correspondence_v18(
                        checked,
                        budget,
                        |optimized, budget| consume(original, optimized, budget),
                    )
                })
            });
            match adopted {
                Ok(result) => {
                    if let Err(error) = self.guard.check(self.owner, self.cleanup, budget) {
                        source_reference_discard_v29(Ok::<_, ProductionSourceOwnedViewErrorV18>(
                            result,
                        ));
                        return Err(SourceConsumerErrorV18(
                            ProductionSourceOptimizationErrorV18::Source(error),
                        ));
                    }
                    Ok(result)
                }
                Err(error) => {
                    use fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1 as Error;
                    let refusal = match &error {
                        Error::Resource(error) => {
                            Some(SourceOwnedQueryFailureV18::Resource(*error))
                        }
                        Error::OriginAccounting => Some(SourceOwnedQueryFailureV18::Resource(
                            ArgumentResourceV1::Accounting,
                        )),
                        Error::Inventory(
                            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error),
                        )
                        | Error::Transition(
                            fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Resource(error),
                        ) => Some(SourceOwnedQueryFailureV18::Resource(*error)),
                        Error::Inventory(_) | Error::Transition(_) | Error::Panicked => {
                            Some(SourceOwnedQueryFailureV18::Binding(
                                "actual source optimizer adoption rejected",
                            ))
                        }
                        // A selected downstream error is not a new source-query
                        // refusal. Existing guards already retain any earlier one.
                        Error::Origin(_) => None,
                    };
                    if let Some(refusal) = refusal {
                        if matches!(
                            refusal,
                            SourceOwnedQueryFailureV18::Resource(ArgumentResourceV1::Accounting)
                        ) {
                            self.cleanup.deny_refund();
                        }
                        let _ = self.guard.reject::<()>(refusal);
                    }
                    Err(SourceConsumerErrorV18(
                        ProductionSourceOptimizationErrorV18::Adoption(error),
                    ))
                }
            }
        }));
        // Check every disposition, including raw unwind, before any enclosing
        // refund. Bounded adoption retains its entire unbalanced residual.
        if budget.storage() != adoption_floor
            || self.guard.observe_custody(self.cleanup, budget).is_err()
        {
            self.cleanup.deny_refund();
        }
        let result =
            source_owned_finish_callback_v18(caught, None, Ok(()), self.cleanup, budget, headers);
        result.map_err(
            |error: SourceConsumerErrorV18<ProductionSourceOptimizationErrorV18<E>>| error.0,
        )
    }
}

trait SourceOptimizerPolicyV18 {
    type Observed<'a>;
    type Output;
    fn checked_storage(output: &Self::Output) -> usize;
    fn observe<'a>(
        input: &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self::Observed<'a>, fe2o3_pliron::KirNeutralOptimizationErrorV18>;
    fn storage(observed: &Self::Observed<'_>) -> usize;
    fn headers<T, E, F>(consume: &F) -> Result<usize, ArgumentResourceV1>;
    fn adopt<T: 'static, E: 'static, F>(
        observed: Self::Observed<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: F,
    ) -> Result<
        (
            Self::Output,
            T,
            fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
        ),
        fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1<E>,
    >
    where
        F: for<'view, 'inventory, 'input, 'output, 'rows, 'work> FnOnce(
            &'view fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<
                'inventory,
                'input,
                'output,
                'rows,
            >,
            &mut ArgumentBudgetV1<'work>,
        )
            -> Result<(T, usize), E>;
}

macro_rules! source_optimizer_policy_v18 {
    ($name:ident, $observed:ident, $output:ident, $observe:ident) => {
        struct $name;
        impl SourceOptimizerPolicyV18 for $name {
            type Observed<'a> = fe2o3_pliron::$observed<'a>;
            type Output = fe2o3_pliron::$output;
            fn checked_storage(output: &Self::Output) -> usize {
                output.storage().retained_storage()
            }
            fn observe<'a>(
                input: &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
                layouts: fe2o3_kernel_ir::StorageLayoutLimitsV1,
                budget: &mut ArgumentBudgetV1<'_>,
            ) -> Result<Self::Observed<'a>, fe2o3_pliron::KirNeutralOptimizationErrorV18> {
                fe2o3_pliron::$observe(input, layouts, budget)
            }
            fn storage(observed: &Self::Observed<'_>) -> usize {
                observed.storage().retained_storage()
            }
            fn headers<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
                optimized_source_consumer_resources_v18::optimizer_entry_headers_typed::<
                    T,
                    E,
                    F,
                    Self::Output,
                    Self::Observed<'static>,
                >()
            }
            fn adopt<T: 'static, E: 'static, F>(
                observed: Self::Observed<'_>,
                budget: &mut ArgumentBudgetV1<'_>,
                consume: F,
            ) -> Result<
                (
                    Self::Output,
                    T,
                    fe2o3_pliron::KirNeutralOwnedOriginStorageV1,
                ),
                fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1<E>,
            >
            where
                F: for<'view, 'inventory, 'input, 'output, 'rows, 'work> FnOnce(
                    &'view fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<
                        'inventory,
                        'input,
                        'output,
                        'rows,
                    >,
                    &mut ArgumentBudgetV1<'work>,
                ) -> Result<
                    (T, usize),
                    E,
                >,
            {
                observed.try_check_and_finish_with_v18(budget, consume)
            }
        }
    };
}
source_optimizer_policy_v18!(
    ScalarSourceOptimizerV18,
    KirNeutralOptimizationOutputV18,
    CheckedNeutralKernelIrOwnerV18,
    optimize_neutral_kernel_ir_v18
);
source_optimizer_policy_v18!(
    IntegerSourceOptimizerV18,
    KirNeutralOptimizationOutputIntegerContinuationV18,
    CheckedNeutralKernelIrOwnerIntegerContinuationV18,
    optimize_neutral_kernel_ir_integer_continuation_v18
);
