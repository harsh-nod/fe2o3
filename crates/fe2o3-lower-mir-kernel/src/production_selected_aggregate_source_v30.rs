//! Distinct selected intake over the one actual aggregate source stage service.
//! The memory census and original graphs remain borrowed through consumption.
use super::*;
use fe2o3_pliron::PendingCanonicalSelectedMemoryPoliciesV30 as Native;

struct SelectedAggregateRootV30 {
    function: FunctionCoordinate,
    rows: SelectedFinalRowsV30,
    external: [usize; 2],
    private: [usize; 5],
}

pub(crate) struct CheckedSelectedAggregateSourcesV30<'scope, 'graph> {
    memory: &'scope ProductionScopedAggregateMemoryV31<'scope, 'scope, 'scope>,
    initial: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    native: &'scope Native<'scope, 'graph>,
    domains: &'scope Domains<'scope, 'scope>,
    output: &'scope Inventory<'scope>,
    transport: &'scope SelectedAggregateTransportV30<'scope>,
    roots: &'scope [SelectedAggregateRootV30],
    scope: DescriptorRoleScopeV18,
}

fn native_error(
    error: fe2o3_pliron::CanonicalRankedPolicyFailureV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match ProductionOptimizedExecutionRecipesV18::policy_resource(&error) {
        Some(resource) => resource.into(),
        None => ProductionSourceOwnedViewErrorV18::Binding(
            "selected aggregate native owner or epoch differs",
        ),
    }
}

fn observe_native(
    original: &ProductionSourceCorrespondenceV18<'_>,
    native: &Native<'_, '_>,
    domains: &Domains<'_, '_>,
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    // Sample both ancestors even when the first has already refused. In
    // particular, an ignored native/domain query can veto a panic-path refund.
    let actual = native.owner(budget).map_err(native_error);
    let domain = domains.owner(budget).map_err(|error| match error {
        fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1::Resource(resource) => {
            optimized_source_formal_resource_v18(resource).into()
        }
        _ => ProductionSourceOwnedViewErrorV18::Binding(
            "selected aggregate domain owner or epoch differs",
        ),
    });
    if matches!(
        &actual,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ) || matches!(
        &domain,
        Err(ProductionSourceOwnedViewErrorV18::Resource(
            ArgumentResourceV1::Accounting
        ))
    ) {
        original.source.cleanup.deny_refund();
        native.refuse_retained_custody();
        domains.refuse_retained_custody();
    }
    (|| {
        if !std::ptr::eq(actual?, owner) || !std::ptr::eq(domain?, owner) {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "selected aggregate native final owner differs",
            ));
        }
        Ok(())
    })()
}

impl CheckedSelectedAggregateSourcesV30<'_, '_> {
    fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let result = self.scope.observe(self.initial.original, budget);
        if result.is_err() {
            self.native.refuse_retained_custody();
            self.domains.refuse_retained_custody();
        }
        result
    }

    fn check(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        let result = self.check_inner(budget);
        self.initial
            .original
            .source
            .retain_aggregate_result_v30(result)
    }

    fn check_inner(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        self.custody(budget)?;
        self.initial
            .original
            .global_expression_entry_v23(self.initial, budget)?;
        self.memory.check_final_owner(self.output.owner(), budget)?;
        budget.charge_work(8)?;
        if !self.transport.finalized
            || !std::ptr::eq(
                self.native.owner(budget).map_err(native_error)?,
                self.output.owner(),
            )
            || !std::ptr::eq(
                self.native.selected_domains(budget).map_err(native_error)?,
                self.domains,
            )
            || !std::ptr::eq(
                formal(self.initial.original, self.domains.owner(budget))?,
                self.output.owner(),
            )
            || self.roots.len() != self.initial.index.selected_roots.len()
        {
            return self
                .initial
                .original
                .source
                .missing("selected aggregate final handoff differs")
                .map_err(Into::into);
        }
        Ok(())
    }

    pub(crate) fn root_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionAggregateSourceErrorV30> {
        self.check(budget)?;
        Ok(self.roots.len())
    }

    pub(crate) fn counts(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(usize, usize, [usize; 2], [usize; 5]), ProductionAggregateSourceErrorV30> {
        let result = (|| {
            self.check(budget)?;
            budget.charge_work(1)?;
            let row = self
                .roots
                .get(root)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "selected aggregate root ordinal",
                ))?;
            Ok((
                row.rows.accesses.len(),
                row.rows.choices.len(),
                row.external,
                row.private,
            ))
        })();
        self.initial
            .original
            .source
            .retain_aggregate_result_v30(result)
    }

    pub(crate) fn memory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&ProductionAggregateMemoryChainV31, ProductionAggregateSourceErrorV30> {
        let result = (|| {
            self.check(budget)?;
            self.memory.memory(budget).map_err(Into::into)
        })();
        self.initial
            .original
            .source
            .retain_aggregate_result_v30(result)
    }

    pub(crate) const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }
    pub(crate) const fn whole_effect_refinement_is_complete(&self) -> bool {
        false
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn root_native(
    original: &ProductionSourceCorrespondenceV18<'_>,
    native: &Native<'_, '_>,
    domains: &Domains<'_, '_>,
    function: FunctionCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<([usize; 2], [usize; 5]), ProductionAggregateSourceErrorV30> {
    let (_, _, reads, writes) = formal(original, domains.function_conditions(function, budget))?;
    let report = native
        .report(function.0 as usize, budget)
        .map_err(native_error)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "selected aggregate native root report absent",
        ))?;
    budget.charge_work(2)?;
    if report.paired_stage_count() != 9 {
        return original
            .source
            .missing("selected aggregate native stage census differs")
            .map_err(Into::into);
    }
    let private =
        report
            .private_access_counts(0)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "selected aggregate native private census absent",
            ))?;
    for stage in 0..9 {
        budget.charge_work(2)?;
        if report.global_access_counts(stage) != Some([reads, writes])
            || report.private_access_counts(stage) != Some(private)
        {
            return original
                .source
                .missing("selected aggregate native effect census differs")
                .map_err(Into::into);
        }
    }
    Ok(([reads, writes], private))
}

fn roots(
    initial: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    state: &SelectedAggregateTransportV30<'_>,
    output: &Inventory<'_>,
    native: &Native<'_, '_>,
    domains: &Domains<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<SelectedAggregateRootV30>, ProductionAggregateSourceErrorV30> {
    let count = initial.index.selected_roots.len();
    budget.charge_work(2)?;
    if state.coordinates.functions.len() != count {
        return initial
            .original
            .source
            .missing("selected aggregate final root census differs")
            .map_err(Into::into);
    }
    let mut roots =
        source_reference_emission_vec_v29(count, budget).map_err(source_argument_error_v18)?;
    let facts = SelectedFinalFactsV30 {
        original: initial.original,
        output,
        definitions: SelectedFinalDefinitionsV30::Aggregate(&state.outputs),
    };
    // Every original root is checked, including roots with no selected source
    // access. An empty selected graph supplies no private/singleton semantics.
    for root in 0..count {
        let floor = budget.storage();
        let row =
            scoped_source_attempt_v29(initial.original.source.cleanup, budget, floor, |budget| {
                let result = (|| {
                    let entry = budget.storage();
                    budget.charge_work(4)?;
                    let range = initial.index.selected_roots.get(root).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "selected aggregate original root range",
                        ),
                    )?;
                    let transport = state.current.get(range.clone()).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "selected aggregate final root range",
                        ),
                    )?;
                    let source = scoped_raw_admission_v29::checked_selected_source_rows_v30(
                        initial.original,
                        root,
                        budget,
                    )?;
                    let function = state.coordinates.functions[root];
                    let projection = index::ProjectionIndexV30::build_from_facts(
                        &facts, root, transport, source, budget,
                    )?;
                    let rows = build::build_from_facts(
                        &facts,
                        domains,
                        function,
                        &projection,
                        source,
                        budget,
                    )?;
                    let (external, private) =
                        root_native(initial.original, native, domains, function, budget)?;
                    let kept = rows.retained_storage()?;
                    drop(projection);
                    let scratch = budget
                        .storage()
                        .checked_sub(argument_sum_v1(&[entry, kept])?)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    budget.release_storage(scratch)?;
                    Ok(SelectedAggregateRootV30 {
                        function,
                        rows,
                        external,
                        private,
                    })
                })();
                initial.original.source.retain_aggregate_result_v30(result)
            })?;
        roots.push(row);
    }
    Ok(roots)
}

fn roots_credit(
    rows: &[SelectedAggregateRootV30],
    capacity: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ArgumentResourceV1> {
    let mut retained = argument_sum_v1(&[
        source_reference_emission_headers_v29::<Vec<SelectedAggregateRootV30>>()?,
        argument_product_v1(capacity, size_of::<SelectedAggregateRootV30>())?,
    ])?;
    for row in rows {
        budget.charge_work(1)?;
        retained = argument_sum_v1(&[retained, row.rows.retained_storage()?])?;
    }
    Ok(retained)
}

fn execute<T, F>(
    memory: &ProductionScopedAggregateMemoryV31<'_, '_, '_>,
    initial: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    native: &Native<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: SourceCallbackCustodyV29<F>,
    #[cfg(test)] fault: Option<u8>,
) -> Result<T, ProductionAggregateSourceErrorV30>
where
    F: for<'scope, 'work> FnOnce(
        &CheckedSelectedAggregateSourcesV30<'scope, '_>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, ProductionAggregateSourceErrorV30>,
{
    let source = initial.original.source;
    let floor = budget.storage();
    let prepared = scoped_source_attempt_v29(source.cleanup, budget, floor, move |budget| {
        consume.prepare(|| {
            let result = (|| {
                let entry = budget.storage();
                let headers = argument_sum_v1(&[
                    headers::<T, F>()?,
                    source_callback_custody_finish_preflight_v29::<
                        T,
                        ProductionAggregateSourceErrorV30,
                    >(budget)?,
                    source_owned_finish_preflight_v26::<T, ProductionAggregateSourceErrorV30>(
                        budget,
                    )?,
                ])?;
                budget.reserve_storage(headers)?;
                let owner = memory.chain(budget)?.output(budget)?.owner();
                memory.check_final_owner(native.owner(budget).map_err(native_error)?, budget)?;
                let domains = native.selected_domains(budget).map_err(native_error)?;
                if !std::ptr::eq(formal(initial.original, domains.owner(budget))?, owner) {
                    return source
                        .missing("selected aggregate domain final owner differs")
                        .map_err(Into::into);
                }
                let (output, receipt) = Inventory::derive_v18(owner, budget)
                    .map_err(ProductionAggregateSourceErrorV30::Inventory)?;
                budget.reserve_storage(receipt.retained_storage())?;
                let retained = argument_sum_v1(&[headers, receipt.retained_storage()])?;
                if budget.storage() != argument_sum_v1(&[entry, retained])? {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok((output, domains, retained))
            })();
            source.retain_aggregate_result_v30(result)
        })
    })?;
    let ((output, domains, retained), mut consume) = prepared;
    let credit = std::cell::Cell::new(retained);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let handoff = memory.chain(budget)?;
        let state_floor = budget.storage();
        let (state, state_credit) =
            scoped_source_attempt_v29(source.cleanup, budget, state_floor, |budget| {
                let result = (|| {
                    let entry = budget.storage();
                    let mut state = fold_aggregate_source_stages_v30(
                        handoff,
                        |stage, state: Option<SelectedAggregateTransportV30<'_>>, budget| {
                            let state = match state {
                                Some(state) => state,
                                None => SelectedAggregateTransportV30::seed(
                                    initial, memory, stage, budget,
                                )?,
                            };
                            #[cfg(test)]
                            if stage.ordinal == 1 {
                                match fault {
                                    Some(0) => return Ok(state),
                                    Some(1) => {
                                        return state
                                            .advance(memory, stage, budget)?
                                            .advance(memory, stage, budget);
                                    }
                                    _ => (),
                                }
                            }
                            state.advance(memory, stage, budget)
                        },
                        budget,
                    )?;
                    #[cfg(test)]
                    match fault {
                        Some(2) => state.coordinates.definitions.next_stage = 0,
                        Some(3) => state.endpoints[2] = state.endpoints[1],
                        _ => (),
                    }
                    state.finish(memory, handoff, &output, budget)?;
                    let retained = state.retained_storage()?;
                    if budget.storage() != argument_sum_v1(&[entry, retained])? {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    Ok((state, retained))
                })();
                source.retain_aggregate_result_v30(result)
            })?;
        credit.set(argument_sum_v1(&[credit.get(), state_credit])?);
        let root_floor = budget.storage();
        let (roots, root_credit) =
            scoped_source_attempt_v29(source.cleanup, budget, root_floor, |budget| {
                let result = (|| {
                    let entry = budget.storage();
                    let roots = roots(initial, &state, &output, native, domains, budget)?;
                    let retained = roots_credit(&roots, roots.capacity(), budget)?;
                    if budget.storage() != argument_sum_v1(&[entry, retained])? {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    Ok((roots, retained))
                })();
                source.retain_aggregate_result_v30(result)
            })?;
        credit.set(argument_sum_v1(&[credit.get(), root_credit])?);
        let view = CheckedSelectedAggregateSourcesV30 {
            memory,
            initial,
            native,
            domains,
            output: &output,
            transport: &state,
            roots: &roots,
            scope: DescriptorRoleScopeV18::new(budget),
        };
        view.check(budget)?;
        let Some(callback) = consume.take() else {
            source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        };
        let result = callback(&view, budget)
            .inspect_err(|error| source.deny_aggregate_accounting_v30(error));
        if result.is_ok() {
            let _ = view.check(budget);
        }
        let prior = source.guard.first.get();
        let postflight = view.custody(budget);
        // Dispose rejected caller-owned values while this borrowed view and
        // all its backing remain live. Only the outer scope owns credit.
        source_owned_finish_callback_v18(Ok(result), prior, postflight, source.cleanup, budget, 0)
    }));
    let callback_floor = budget.storage();
    let caught = consume.finish(caught);
    let prior = source.guard.first.get();
    let native_postflight =
        observe_native(initial.original, native, domains, output.owner(), budget);
    if budget.storage() < callback_floor
        || floor
            .checked_add(credit.get())
            .is_none_or(|minimum| budget.storage() < minimum)
    {
        source.cleanup.deny_refund();
        native.refuse_retained_custody();
        domains.refuse_retained_custody();
    }
    let source_postflight = source.guard.observe_custody(source.cleanup, budget);
    let postflight = native_postflight.and(source_postflight);
    // A returned compiler error predates these postflight observations and
    // remains the selected error. A panic has no returned error to retain, so
    // record any observed refusal before resuming its owned payload.
    let postflight = if caught.is_err() {
        source.retain_aggregate_source_result_v30(postflight)
    } else {
        postflight
    };
    if source.cleanup.is_denied() {
        native.refuse_retained_custody();
        domains.refuse_retained_custody();
    }
    drop(output);
    let result = source_owned_finish_callback_v18(
        caught,
        prior,
        postflight,
        source.cleanup,
        budget,
        credit.get(),
    );
    source.retain_aggregate_result_v30(result)
}

fn headers<T, F>() -> Result<usize, ArgumentResourceV1> {
    type Error = ProductionAggregateSourceErrorV30;
    fn h<V>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<V>(),
            argument_product_v1(2, size_of::<Result<V, Error>>())?,
        ])
    }
    type Frame<'a, 'w, T, F> = (
        &'a ProductionScopedAggregateMemoryV31<'a, 'a, 'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a ProductionAggregateSourceOutputHandoffV30<'a, 'a>,
        &'a Native<'a, 'a>,
        &'a Domains<'a, 'a>,
        &'a Inventory<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        &'a SelectedAggregateTransportV30<'a>,
        &'a [SelectedAggregateRootV30],
        &'a [SelectedTransportRowV30],
        &'a [PendingSourceSelectedAccessV30],
        &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        [usize; 24],
        [SourceOwnedResultV18<()>; 3],
        [Result<(), Error>; 3],
        Option<SourceOwnedQueryFailureV18>,
        [Result<T, Error>; 2],
        std::thread::Result<Result<T, Error>>,
        Result<
            (
                (Inventory<'a>, &'a Domains<'a, 'a>, usize),
                SourceCallbackCustodyV29<F>,
            ),
            Error,
        >,
        Result<(SelectedAggregateTransportV30<'a>, usize), Error>,
        Result<(Vec<SelectedAggregateRootV30>, usize), Error>,
        Result<([usize; 2], [usize; 5]), Error>,
        [SourceOwnedResultV18<&'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>; 2],
        [SourceOwnedResultV18<()>; 3],
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        [Range<usize>; 2],
        [usize; 2],
        [usize; 5],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_, T, F>>(),
        std::mem::align_of::<Frame<'_, '_, T, F>>(),
        h::<CheckedSelectedAggregateSourcesV30<'_, '_>>()?,
        h::<Inventory<'_>>()?,
        h::<fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1>()?,
        h::<Vec<SelectedAggregateRootV30>>()?,
        h::<SelectedAggregateRootV30>()?,
        h::<SourceCallbackCustodyV29<F>>()?,
        h::<std::cell::Cell<usize>>()?,
        h::<DescriptorRoleScopeV18>()?,
        h::<SelectedFinalFactsV30<'_>>()?,
        h::<SelectedFinalRowsV30>()?,
        h::<FunctionCoordinate>()?,
        h::<fe2o3_pliron::CanonicalRankedPolicyFailureV1>()?,
        selected_aggregate_transport_headers_v30()?,
        index::ProjectionIndexV30::headers()?,
        build::headers()?,
    ])
}

impl ProductionAggregateSourceOutputHandoffV30<'_, '_> {
    pub(crate) fn with_selected_aggregate_sources_v30<T, F>(
        &self,
        native: &Native<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: F,
    ) -> Result<T, ProductionAggregateSourceErrorV30>
    where
        F: for<'scope, 'work> FnOnce(
            &CheckedSelectedAggregateSourcesV30<'scope, '_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, ProductionAggregateSourceErrorV30>,
    {
        self.with_selected_aggregate_sources_inner_v30(
            native,
            budget,
            consume,
            #[cfg(test)]
            None,
        )
    }

    fn with_selected_aggregate_sources_inner_v30<T, F>(
        &self,
        native: &Native<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
        consume: F,
        #[cfg(test)] fault: Option<u8>,
    ) -> Result<T, ProductionAggregateSourceErrorV30>
    where
        F: for<'scope, 'work> FnOnce(
            &CheckedSelectedAggregateSourcesV30<'scope, '_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, ProductionAggregateSourceErrorV30>,
    {
        let consume = SourceCallbackCustodyV29::new(consume);
        self.owned.check(budget)?;
        self.with_private_memory_v31(budget, |memory, budget| {
            let chain = memory.chain(budget)?.output(budget)?;
            with_aggregate_initial_source_v30(
                self.owned.source,
                chain,
                budget,
                |_, initial, budget| {
                    execute(
                        memory,
                        initial,
                        native,
                        budget,
                        consume,
                        #[cfg(test)]
                        fault,
                    )
                },
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn with_selected_aggregate_sources_fault_v30<T, F>(
        &self,
        native: &Native<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
        fault: u8,
        consume: F,
    ) -> Result<T, ProductionAggregateSourceErrorV30>
    where
        F: for<'scope, 'work> FnOnce(
            &CheckedSelectedAggregateSourcesV30<'scope, '_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, ProductionAggregateSourceErrorV30>,
    {
        self.with_selected_aggregate_sources_inner_v30(native, budget, consume, Some(fault))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_aggregate_retained_rows_match_independent_capacity_and_constructor_credit() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        let mut roots = source_reference_emission_vec_v29(2, &mut budget).unwrap();
        for ordinal in 0..2 {
            roots.push(SelectedAggregateRootV30 {
                function: FunctionCoordinate(ordinal),
                external: [0; 2],
                private: [0; 5],
                rows: SelectedFinalRowsV30 {
                    accesses: resources::vector(ordinal as usize + 1, &mut budget).unwrap(),
                    choices: resources::vector(ordinal as usize + 2, &mut budget).unwrap(),
                    edges: resources::vector(ordinal as usize + 3, &mut budget).unwrap(),
                    obligations: resources::vector(ordinal as usize + 4, &mut budget).unwrap(),
                    forwarding: resources::vector(ordinal as usize + 5, &mut budget).unwrap(),
                },
            });
        }
        let expected = size_of::<Vec<SelectedAggregateRootV30>>()
            + 2 * size_of::<Result<Vec<SelectedAggregateRootV30>, ProductionSemanticKirErrorV1>>()
            + roots.capacity() * size_of::<SelectedAggregateRootV30>()
            + roots
                .iter()
                .map(|root| {
                    root.rows.accesses.capacity() * size_of::<SelectedFinalAccessV30>()
                        + root.rows.choices.capacity() * size_of::<SelectedFinalChoiceJoinV30>()
                        + root.rows.edges.capacity() * size_of::<SelectedFinalEdgeJoinV30>()
                        + root.rows.obligations.capacity()
                            * size_of::<SelectedFinalObligationJoinV30>()
                        + root.rows.forwarding.capacity()
                            * size_of::<SelectedFinalForwardingStepV30>()
                })
                .sum::<usize>();
        assert_eq!(budget.storage(), 17 + expected);
        let before = budget.work();
        assert_eq!(
            roots_credit(&roots, roots.capacity(), &mut budget).unwrap(),
            expected
        );
        assert_eq!(budget.work() - before, 2);
        drop(roots);
        budget.release_storage(expected).unwrap();
        assert_eq!(budget.storage(), 17);
        let empty = SelectedFinalRowsV30 {
            accesses: Vec::new(),
            choices: Vec::new(),
            edges: Vec::new(),
            obligations: Vec::new(),
            forwarding: Vec::new(),
        };
        assert_eq!(empty.retained_storage().unwrap(), 0);
    }
}
