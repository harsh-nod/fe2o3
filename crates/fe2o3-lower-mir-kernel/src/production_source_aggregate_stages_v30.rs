// One actual source-owned traversal is shared by occurrence transport and proof
// construction. Consumers receive a scoped checked pair, never a replacement map.
include!("production_source_aggregate_transport_v30.rs");
include!("production_source_aggregate_coordinates_v30.rs");
include!("production_source_aggregate_roles_v30.rs");

trait AggregateStageStateV30 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1>;
}

impl AggregateStageStateV30 for () {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        Ok(0)
    }
}

enum AggregateSourceStageRelationV30<'scope> {
    Scalar(fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'scope, 'scope, 'scope, 'scope>),
    Aggregate(fe2o3_kernel_analysis::CheckedCanonicalKirAggregateSsaV18<'scope>),
}

struct AggregateSourceStageV30<'scope> {
    source: &'scope ProductionSourceOwnedViewV18<'scope>,
    chain: &'scope fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
    ordinal: usize,
    input: &'scope fe2o3_kernel_analysis::CanonicalKirInventoryV18<'scope>,
    output: &'scope fe2o3_kernel_analysis::CanonicalKirInventoryV18<'scope>,
    relation: &'scope AggregateSourceStageRelationV30<'scope>,
    aggregate_index: Option<
        &'scope fe2o3_kernel_analysis::CheckedCanonicalKirAggregateOccurrencesV30<'scope, 'scope>,
    >,
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl AggregateSourceStageV30<'_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.source.check_query_v18(budget)?;
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            self.source.cleanup.deny_refund();
            return self
                .source
                .retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        let round = self.chain.rounds().get(self.ordinal / 2).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("actual aggregate stage ordinal"),
        )?;
        let (input, output) = if self.ordinal % 2 == 0 {
            (
                if self.ordinal == 0 {
                    self.source.canonical(budget)?
                } else {
                    self.chain.rounds()[self.ordinal / 2 - 1]
                        .aggregate()
                        .output()
                },
                round.scalar().owner(),
            )
        } else {
            (round.scalar().owner(), round.aggregate().output())
        };
        if !std::ptr::eq(self.input.owner(), input) || !std::ptr::eq(self.output.owner(), output) {
            return self.source.missing("actual aggregate stage endpoint owner");
        }
        let same_pair = match self.relation {
            AggregateSourceStageRelationV30::Scalar(pair) => {
                self.ordinal % 2 == 0
                    && std::ptr::eq(pair.input(), self.input)
                    && std::ptr::eq(pair.output(), self.output)
                    && self.aggregate_index.is_none()
            }
            AggregateSourceStageRelationV30::Aggregate(pair) => {
                self.ordinal % 2 == 1
                    && std::ptr::eq(pair.input(), input)
                    && std::ptr::eq(pair.output(), output)
                    && std::ptr::eq(pair.witness(), round.aggregate().witness())
                    && self.aggregate_index.is_some_and(|index| {
                        std::ptr::eq(index.checked(), pair)
                            && std::ptr::eq(index.input(), self.input)
                            && std::ptr::eq(index.output(), self.output)
                    })
            }
        };
        if !same_pair {
            return self.source.missing("actual aggregate stage relation owner");
        }
        Ok(())
    }
}

fn aggregate_stage_headers_v30<T, F>() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, T, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1,
        fe2o3_kernel_analysis::CanonicalKirTransitionStorageV1,
        fe2o3_kernel_analysis::CanonicalKirAggregateSsaStorageV18,
        Option<fe2o3_kernel_analysis::CheckedCanonicalKirAggregateOccurrencesV30<'a, 'a>>,
        Result<
            (
                fe2o3_kernel_analysis::CheckedCanonicalKirAggregateOccurrencesV30<'a, 'a>,
                fe2o3_kernel_analysis::CanonicalKirAggregateSsaStorageV18,
            ),
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
        AggregateSourceStageRelationV30<'a>,
        AggregateSourceStageV30<'a>,
        Option<T>,
        T,
        F,
        &'a mut ArgumentBudgetV1<'a>,
        [usize; 10],
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, T, F>>(),
        std::mem::align_of::<Frame<'_, T, F>>(),
        size_of::<Result<T, ProductionAggregateSourceErrorV30>>(),
        size_of::<Result<(), ProductionAggregateSourceErrorV30>>(),
        aggregate_source_headers_v30()?,
        aggregate_definition_headers_v30()?,
        aggregate_coordinate_headers_v30()?,
    ])
}

// State is constructed at the first actual pair, inside the owning attempt.
// This lets a reducing stage release old backing without crossing the attempt's
// entry floor. Errors/unwinds drop all state before that attempt can refund.
fn fold_aggregate_source_stages_v30<T, F>(
    handoff: &ProductionAggregateSourceOutputHandoffV30<'_, '_>,
    mut fold: F,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<T, ProductionAggregateSourceErrorV30>
where
    T: AggregateStageStateV30,
    F: for<'stage, 'work> FnMut(
        &AggregateSourceStageV30<'stage>,
        Option<T>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, ProductionAggregateSourceErrorV30>,
{
    use ProductionAggregateSourceErrorV30 as E;
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryV18 as Inventory, check_canonical_kir_aggregate_ssa_v18,
        check_canonical_kir_transition_v18,
    };
    let source = handoff.owned.source;
    let floor = budget.storage();
    scoped_source_attempt_v29(source.cleanup, budget, floor, move |budget| {
        let result = (|| {
            handoff.owned.check(budget)?;
            let header = aggregate_stage_headers_v30::<T, F>()?;
            budget.reserve_storage(header)?;
            handoff.replay(budget)?;
            let chain = handoff.output(budget)?;
            let mut current = source.canonical(budget)?;
            let mut state: Option<T> = None;
            for (round_index, round) in chain.rounds().iter().enumerate() {
                for aggregate in [false, true] {
                    budget.charge_work(32)?;
                    let endpoint = if aggregate {
                        round.aggregate().output()
                    } else {
                        round.scalar().owner()
                    };
                    let (input, input_receipt) =
                        Inventory::derive_v18(current, budget).map_err(E::Inventory)?;
                    budget.reserve_storage(input_receipt.retained_storage())?;
                    let (output, output_receipt) =
                        Inventory::derive_v18(endpoint, budget).map_err(E::Inventory)?;
                    budget.reserve_storage(output_receipt.retained_storage())?;
                    let (relation, credit) = if aggregate {
                        let (pair, receipt) = check_canonical_kir_aggregate_ssa_v18(
                            current,
                            endpoint,
                            round.aggregate().witness(),
                            budget,
                        )
                        .map_err(|error| {
                            E::Optimization(
                                fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18::Aggregate(
                                    fe2o3_kernel_opt::OwnedAggregateSsaErrorV18::Check(error),
                                ),
                            )
                        })?;
                        (
                            AggregateSourceStageRelationV30::Aggregate(pair),
                            receipt.retained_storage(),
                        )
                    } else {
                        let (pair, receipt) = check_canonical_kir_transition_v18(
                            &input,
                            &output,
                            round.scalar().occurrences().candidate(),
                            budget,
                        )
                        .map_err(E::Transition)?;
                        (
                            AggregateSourceStageRelationV30::Scalar(pair),
                            receipt.retained_storage(),
                        )
                    };
                    budget.reserve_storage(credit)?;
                    let (aggregate_index, index_credit) = match &relation {
                        AggregateSourceStageRelationV30::Scalar(_) => (None, 0),
                        AggregateSourceStageRelationV30::Aggregate(pair) => {
                            let (index, receipt) =
                                fe2o3_kernel_analysis::CheckedCanonicalKirAggregateOccurrencesV30::derive(
                                    pair, &input, &output, budget,
                                )
                                .map_err(|error| E::Optimization(
                                    fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18::Aggregate(
                                        fe2o3_kernel_opt::OwnedAggregateSsaErrorV18::Check(error),
                                    ),
                                ))?;
                            (Some(index), receipt.retained_storage())
                        }
                    };
                    budget.reserve_storage(index_credit)?;
                    let required = budget
                        .storage()
                        .checked_sub(
                            state
                                .as_ref()
                                .map(AggregateStageStateV30::retained_storage)
                                .transpose()?
                                .unwrap_or(0),
                        )
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let stage = AggregateSourceStageV30 {
                        source,
                        chain,
                        ordinal: round_index * 2 + usize::from(aggregate),
                        input: &input,
                        output: &output,
                        relation: &relation,
                        aggregate_index: aggregate_index.as_ref(),
                        required,
                        slot: std::ptr::from_ref(&*budget) as usize,
                        ledger: budget.work_ledger_identity_v1(),
                    };
                    stage.check(budget)?;
                    let next = fold(&stage, state.take(), budget)
                        .inspect_err(|error| source.deny_aggregate_accounting_v30(error))?;
                    stage.check(budget)?;
                    if budget.storage() < argument_sum_v1(&[required, next.retained_storage()?])? {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    state = Some(next);
                    drop(stage);
                    drop(aggregate_index);
                    drop(relation);
                    drop(output);
                    drop(input);
                    budget.release_storage(argument_sum_v1(&[
                        credit,
                        index_credit,
                        input_receipt.retained_storage(),
                        output_receipt.retained_storage(),
                    ])?)?;
                    current = endpoint;
                }
            }
            if !std::ptr::eq(current, chain.owner()) {
                return source
                    .missing("complete aggregate terminal endpoint")
                    .map_err(Into::into);
            }
            handoff.owned.check(budget)?;
            drop(fold);
            budget.release_storage(header)?;
            state.ok_or_else(|| {
                ProductionSourceOwnedViewErrorV18::Binding("empty aggregate stage fold").into()
            })
        })();
        source.retain_aggregate_result_v30(result)
    })
}
