// Neutral safety census over the same checked aggregate stage service. This
// does not complete either the legacy or selected external-memory family.
pub(crate) struct ProductionScopedAggregateMemoryV31<'scope, 'view, 'source> {
    chain: &'scope ProductionAggregateSourceOutputHandoffV30<'view, 'source>,
    memory: &'scope ProductionAggregateMemoryChainV31,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl ProductionScopedAggregateMemoryV31<'_, '_, '_> {
    fn custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        let source = self.chain.owned.source;
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            source.cleanup.deny_refund();
            return source.retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        self.chain
            .owned
            .observe_retained_storage_v18(self.required, budget)
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.custody(budget)?;
        let source = self.chain.owned.source;
        self.chain.owned.check(budget)?;
        budget.charge_work(8)?;
        let output = self.chain.output(budget)?;
        let stages = argument_product_v1(output.rounds().len(), 2)?;
        if !self.memory.finalized
            || self.memory.initial_owner != std::ptr::from_ref(source.canonical(budget)?) as usize
            || self.memory.final_owner != std::ptr::from_ref(output.owner()) as usize
            || self.memory.next_stage != stages
            || self.memory.endpoint_count() != argument_sum_v1(&[stages, 1])?
            || !self.memory.endpoint_belongs_to(stages, output.owner())
        {
            return source.missing("scoped aggregate memory owner or stage census differs");
        }
        Ok(())
    }

    pub(crate) fn chain(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&ProductionAggregateSourceOutputHandoffV30<'_, '_>> {
        self.check(budget)?;
        Ok(self.chain)
    }

    pub(crate) fn memory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&ProductionAggregateMemoryChainV31> {
        self.check(budget)?;
        Ok(self.memory)
    }

    pub(crate) fn check_final_owner(
        &self,
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        if !std::ptr::eq(self.chain.output(budget)?.owner(), owner) {
            return self
                .chain
                .owned
                .source
                .missing("scoped aggregate memory final owner differs");
        }
        Ok(())
    }

    pub(crate) const fn runtime_requirements_are_discharged(&self) -> bool {
        false
    }

    pub(crate) const fn executed_memory_refinement_is_complete(&self) -> bool {
        false
    }

    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn aggregate_memory_visit_headers_v31<T, E, F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, 'w, T, E, F> = (
        ProductionScopedAggregateMemoryV31<'a, 'a, 'a>,
        ProductionAggregateMemoryChainV31,
        Option<ProductionAggregateMemoryChainV31>,
        Result<ProductionAggregateMemoryChainV31, ProductionAggregateSourceErrorV30>,
        &'a ProductionAggregateSourceOutputHandoffV30<'a, 'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        [usize; 8],
        [SourceOwnedResultV18<()>; 2],
        Result<(), ProductionAggregateSourceErrorV30>,
        Result<T, E>,
        std::thread::Result<Result<T, E>>,
        Option<SourceOwnedQueryFailureV18>,
        Result<
            (
                (ProductionAggregateMemoryChainV31, usize),
                SourceCallbackCustodyV29<F>,
            ),
            ProductionAggregateSourceErrorV30,
        >,
        SourceCallbackCustodyV29<F>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_, T, E, F>>(),
        std::mem::align_of::<Frame<'_, '_, T, E, F>>(),
        aggregate_memory_headers_v31()?,
    ])
}

impl ProductionAggregateSourceOutputHandoffV30<'_, '_> {
    // The callback cannot replace the chain, escape the scoped view, or turn
    // finalized definite-byte-init safety into a value/lifetime relation.
    pub(crate) fn with_private_memory_v31<'work, T, E, F>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: F,
    ) -> Result<T, E>
    where
        E: From<ProductionAggregateSourceErrorV30> + From<ProductionSourceOwnedViewErrorV18>,
        F: for<'scope> FnOnce(
            &ProductionScopedAggregateMemoryV31<'scope, '_, '_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    {
        self.with_private_memory_inner_v31(
            budget,
            consume,
            #[cfg(test)]
            None,
        )
    }

    fn with_private_memory_inner_v31<'work, T, E, F>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: F,
        #[cfg(test)] fault: Option<u8>,
    ) -> Result<T, E>
    where
        E: From<ProductionAggregateSourceErrorV30> + From<ProductionSourceOwnedViewErrorV18>,
        F: for<'scope> FnOnce(
            &ProductionScopedAggregateMemoryV31<'scope, '_, '_>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    {
        let consume = SourceCallbackCustodyV29::new(consume);
        self.owned.check(budget)?;
        let source = self.owned.source;
        let floor = budget.storage();
        let prepared = scoped_source_attempt_v29(source.cleanup, budget, floor, move |budget| {
            consume.prepare(|| {
                let result = (|| {
                    let entry = budget.storage();
                    let header = argument_sum_v1(&[
                        aggregate_memory_visit_headers_v31::<T, E, F>()?,
                        source_callback_custody_finish_preflight_v29::<T, E>(budget)?,
                        source_owned_finish_preflight_v26::<T, E>(budget)?,
                    ])?;
                    budget.reserve_storage(header)?;
                    let output = self.output(budget)?;
                    let mut memory = fold_aggregate_source_stages_v30(
                        self,
                        |stage, state: Option<ProductionAggregateMemoryChainV31>, budget| {
                            let memory = match state {
                                Some(memory) => memory,
                                None => ProductionAggregateMemoryChainV31::seed(stage, budget)?,
                            };
                            #[cfg(test)]
                            if stage.ordinal == 1 {
                                match fault {
                                    Some(0) => return Ok(memory),
                                    Some(1) => {
                                        return memory
                                            .advance(stage, budget)?
                                            .advance(stage, budget);
                                    }
                                    _ => (),
                                }
                            }
                            memory.advance(stage, budget)
                        },
                        budget,
                    )?;
                    #[cfg(test)]
                    match fault {
                        Some(2) => memory.next_stage = 0,
                        Some(3) => memory.final_owner = memory.initial_owner,
                        Some(4) => memory.endpoints[2].owner = memory.endpoints[1].owner,
                        _ => (),
                    }
                    memory.finish(source, output, budget)?;
                    let credit = memory.retained_storage()?;
                    let required = argument_sum_v1(&[entry, header, credit])?;
                    if budget.storage() != required {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    Ok((memory, argument_sum_v1(&[header, credit])?))
                })();
                source.retain_aggregate_result_v30(result)
            })
        });
        let ((memory, retained), mut consume) = prepared.map_err(E::from)?;
        let view = ProductionScopedAggregateMemoryV31 {
            chain: self,
            memory: &memory,
            required: budget.storage(),
            slot: std::ptr::from_ref(&*budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            view.check(budget)?;
            let Some(consume) = consume.take() else {
                source.cleanup.deny_refund();
                return Err(E::from(ProductionSourceOwnedViewErrorV18::from(
                    ArgumentResourceV1::Accounting,
                )));
            };
            consume(&view, budget)
        }));
        let callback_floor = budget.storage();
        let caught = consume.finish(caught);
        if budget.storage() < callback_floor {
            source.cleanup.deny_refund();
        }
        let prior = source.guard.first.get();
        let postflight = view.custody(budget);
        drop(view);
        drop(memory);
        // Only our census/header credit is settled. Paid callback output,
        // error, and panic payload storage is never inferred to be ours.
        source_owned_finish_callback_v18(
            caught,
            prior,
            postflight,
            source.cleanup,
            budget,
            retained,
        )
    }
}
