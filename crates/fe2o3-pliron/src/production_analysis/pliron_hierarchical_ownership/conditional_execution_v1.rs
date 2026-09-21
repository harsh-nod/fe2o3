//! Selected graph coverage and ordinary residual ownership are separate results.

use super::*;

use super::{checked_ownership_product_v1 as mul, checked_ownership_sum_v1 as sum};
#[cfg(test)]
pub(crate) use crate::production_analysis::pliron_invocation_trace::{
    PlironTraceFailureV1 as TestTraceFailureV1,
    preflight_execution_layout_resource_upper_bound_v1 as test_layout_bound_v1,
    preflight_invocation_trace_resource_upper_bound_v1 as test_trace_bound_v1,
};
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::AdditionalObservationV1;
use crate::production_analysis::{
    pliron_function_inventory::BoundedPlironFunctionInventoryV1 as Inventory,
    pliron_ranked_coverage_v1::{
        RuleFactsV1, RuleRefusalV1, RuleSiteV1, ValueKeyV1,
        check_conditional_ownership_live_rule_with_observation_v1,
    },
};
use crate::{
    ProductionConditionalOwnershipSiteV1 as RecipeSite, ProductionRankedKernelV1 as Recipe,
    ProductionRankedOperationV1 as RecipeOp,
};
use pliron::context::Ptr;

type Manager = PlironAnalysisManagerV1;
type Census = ProductionAnalysisInputCensusV1;
type Bound = ProductionAnalysisResourceUpperBoundV1;
type Limit = ProductionAnalysisResourceLimitV1;
type Phase = ProductionAnalysisResourcePhaseV1;
type Site = HierarchicalOwnershipLocationV1;

// Only the pending owner's authenticated occurrence roster supplies these values.
pub(crate) type OccurrenceV1 = (RecipeSite, Ptr<Operation>, Value);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectedResultV1 {
    NotRun,
    Checked(RuleFactsV1),
    Refused(RuleRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RowV1 {
    pub(crate) recipe: RecipeSite,
    pub(crate) ownership: Site,
    pub(crate) view: ValueKeyV1,
    pub(crate) result: SelectedResultV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectionErrorV1 {
    Empty,
    Limit,
    RecipeSite,
    RecipeView,
    DuplicateSite,
    DuplicateOperation,
    DuplicateView,
    MissingOperation,
    NotContract,
    MissingCollectedContract,
    WrongView,
    ContractMismatch,
    ValueKey,
    RowStorage,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FailureV1 {
    Resource(Limit),
    CensusIdentity,
    Inventory {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    Census {
        resource: &'static str,
        actual: usize,
        expected: usize,
    },
    Epoch {
        expected: u64,
        actual: Option<u64>,
    },
    Selection {
        index: usize,
        reason: SelectionErrorV1,
    },
    Collection(HierarchicalOwnershipReportV1),
    Prerequisite(HierarchicalOwnershipReportV1),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReportV1 {
    pub(crate) selected: Vec<RowV1>,
    pub(crate) residual: Option<HierarchicalOwnershipReportV1>,
    pub(crate) failure: Option<FailureV1>,
    // Committed manager prefix only, not a complete return/unwind receipt.
    pub(crate) admitted: Bound,
}

impl From<Limit> for FailureV1 {
    fn from(error: Limit) -> Self {
        Self::Resource(error)
    }
}

fn resource(resource: &'static str) -> Limit {
    Limit {
        phase: Phase::HierarchicalOwnership,
        resource,
    }
}

fn selection(index: usize, reason: SelectionErrorV1) -> FailureV1 {
    FailureV1::Selection { index, reason }
}

fn count(
    resource: &'static str,
    actual: usize,
    expected: usize,
    complete: bool,
) -> Result<(), FailureV1> {
    if actual > expected || (complete && actual != expected) {
        Err(FailureV1::Census {
            resource,
            actual,
            expected,
        })
    } else {
        Ok(())
    }
}

fn check_epoch(ctx: &Context, expected: u64) -> Result<(), FailureV1> {
    let actual = ctx
        .ir_mutation_attempt_epoch()
        .ok()
        .map(|epoch| epoch.value());
    if actual == Some(expected) {
        Ok(())
    } else {
        Err(FailureV1::Epoch { expected, actual })
    }
}

fn check_input(am: &Manager, census: Census, selections: usize) -> Result<(), FailureV1> {
    if am.input_census() != Some(census) {
        return Err(FailureV1::CensusIdentity);
    }
    if selections == 0 {
        return Err(selection(0, SelectionErrorV1::Empty));
    }
    if selections > MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1 {
        return Err(selection(
            MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1,
            SelectionErrorV1::Limit,
        ));
    }
    Ok(())
}

pub(crate) fn value_key_v1(ctx: &Context, inv: &Inventory, value: Value) -> Option<ValueKeyV1> {
    if let Some(pointer) = value.defining_op() {
        let site = inv
            .operations()
            .iter()
            .find(|site| site.pointer() == pointer)?;
        let raw = site.pointer().try_deref(ctx).ok()?;
        let index = (0..raw.get_num_results()).find(|&index| raw.get_result(index) == value)?;
        Some(ValueKeyV1::Result {
            site: RuleSiteV1 {
                block: site.block(),
                operation: site.operation(),
            },
            index,
        })
    } else {
        let pointer = value.defining_block()?;
        let block = inv
            .blocks()
            .iter()
            .position(|candidate| *candidate == pointer)?;
        let raw = inv.blocks()[block].try_deref(ctx).ok()?;
        let index = (0..raw.get_num_arguments()).find(|&index| raw.get_argument(index) == value)?;
        Some(ValueKeyV1::Argument { block, index })
    }
}

fn census_names(ctx: &Context, inv: &Inventory, census: Census) -> Result<usize, FailureV1> {
    count("blocks", inv.blocks().len(), census.blocks, true)?;
    count(
        "operations",
        inv.operations().len(),
        census.operations,
        true,
    )?;
    let (mut arguments, mut results, mut contracts, mut names) = (0, 0, 0, 0);
    for pointer in inv.blocks() {
        let raw = pointer.deref(ctx);
        arguments = sum(&[arguments, raw.get_num_arguments()])?;
        count("block arguments", arguments, census.block_arguments, false)?;
        for index in 0..raw.get_num_arguments() {
            names = sum(&[
                names,
                raw.get_argument(index)
                    .unique_name_byte_len(ctx)
                    .ok_or_else(|| resource("conditional name arithmetic"))?,
            ])?;
        }
    }
    for site in inv.operations() {
        let raw = site.pointer().deref(ctx);
        results = sum(&[results, raw.get_num_results()])?;
        count("results", results, census.results, false)?;
        for index in 0..raw.get_num_results() {
            names = sum(&[
                names,
                raw.get_result(index)
                    .unique_name_byte_len(ctx)
                    .ok_or_else(|| resource("conditional name arithmetic"))?,
            ])?;
        }
        if Operation::is_op::<OwnershipContractOp>(site.pointer(), ctx) {
            contracts = sum(&[contracts, 1])?;
            count(
                "ownership contracts",
                contracts,
                census.ownership_contracts,
                false,
            )?;
        }
    }
    count("block arguments", arguments, census.block_arguments, true)?;
    count("results", results, census.results, true)?;
    count(
        "ownership contracts",
        contracts,
        census.ownership_contracts,
        true,
    )?;
    Ok(names)
}

fn overhead(census: Census, selections: usize) -> Result<Bound, Limit> {
    let contracts = census
        .ownership_contracts
        .min(MAX_HIERARCHICAL_OWNERSHIP_CONTRACTS_V1);
    let values = sum(&[census.results, census.block_arguments])?;
    let items = sum(&[
        census.operations,
        census.blocks,
        values,
        census.attributes,
        1,
    ])?;
    let work = mul(
        128,
        sum(&[
            mul(sum(&[selections, contracts, 1])?, items)?,
            mul(values, sum(&[values, census.attributes, 1])?)?,
            mul(selections, selections)?,
            1,
        ])?,
    )?;
    let slots = contracts.max(4);
    let retained = sum(&[
        std::mem::size_of::<ReportV1>(),
        mul(selections, std::mem::size_of::<RowV1>())?,
        mul(mul(2, slots)?, std::mem::size_of::<ContractV1>())?,
        mul(mul(16, slots)?, std::mem::size_of::<usize>())?,
    ])?;
    Bound::checked_phase(Phase::HierarchicalOwnership, work, retained, 0)
}

pub(crate) struct PreparedRowsV1 {
    pub(crate) rows: Vec<RowV1>,
    // Used only by ownership preflight. The live query uses the original census.
    pub(crate) named_census: Census,
}

#[cfg(test)]
pub(crate) fn prepare_rows_v1(
    ctx: &Context,
    function: &FuncOp,
    am: &mut Manager,
    census: Census,
    expected_epoch: u64,
    selections: usize,
) -> Result<PreparedRowsV1, FailureV1> {
    prepare_rows_with_observation_v1(ctx, function, am, census, expected_epoch, selections, None)
        .map(|(prepared, _)| prepared)
}

pub(crate) fn prepare_rows_with_observation_v1(
    ctx: &Context,
    function: &FuncOp,
    am: &mut Manager,
    census: Census,
    expected_epoch: u64,
    selections: usize,
    observer: OwnershipObserverV1<'_, '_, '_>,
) -> Result<(PreparedRowsV1, Option<Bound>), FailureV1> {
    match observer {
        None => prepare_rows_observed_inner_v1(
            ctx,
            function,
            am,
            census,
            expected_epoch,
            selections,
            None,
        ),
        Some(observer) => observer.with_projection(&Ok, |nested| {
            prepare_rows_observed_inner_v1(
                ctx,
                function,
                am,
                census,
                expected_epoch,
                selections,
                Some(nested),
            )
        }),
    }
}

fn prepare_rows_observed_inner_v1(
    ctx: &Context,
    function: &FuncOp,
    am: &mut Manager,
    census: Census,
    expected_epoch: u64,
    selections: usize,
    observer: OwnershipObserverV1<'_, '_, '_>,
) -> Result<(PreparedRowsV1, Option<Bound>), FailureV1> {
    check_epoch(ctx, expected_epoch)?;
    check_input(am, census, selections)?;
    let phase = Phase::HierarchicalOwnership;
    let quota = |error: Limit| {
        if let Some(observer) = observer {
            observer.deny(error);
        }
        FailureV1::Resource(error)
    };
    let overhead_bound = overhead(census, selections).map_err(&quota)?;
    if let Some(observer) = observer {
        let limits = am.remaining_resource_limits(phase).map_err(&quota)?;
        observer
            .require(limits, phase, Ok(overhead_bound))
            .map_err(&quota)?;
    }
    am.admit_retained_resource_upper_bound(phase, overhead_bound)
        .map_err(&quota)?;
    am.prepare_function_inventory(ctx, function);
    let inv = am.function_inventory_handle().map_err(|error| {
        if let Some(observer) = observer {
            observer.deny(Limit {
                phase: Phase::FunctionInventory,
                resource: error.resource(),
            });
        }
        FailureV1::Inventory {
            resource: error.resource(),
            actual: error.actual(),
            limit: error.limit(),
        }
    })?;
    // This traversal is already covered by the admitted overhead.
    let names = census_names(ctx, &inv, census).map_err(|failure| match failure {
        FailureV1::Resource(error) => quota(error),
        failure => failure,
    })?;
    let names_bound =
        (|| Bound::checked_phase(phase, mul(names, 4)?, mul(names, 4)?, 0))().map_err(&quota)?;
    let prepared_bound = match observer {
        None => None,
        Some(observer) => {
            let limits = am.remaining_resource_limits(phase).map_err(&quota)?;
            Some(
                observer
                    .with_projection(
                        &|local| overhead_bound.checked_then_retain(local, phase),
                        |nested| {
                            nested.require(limits, phase, Ok(names_bound))?;
                            overhead_bound.checked_then_retain(names_bound, phase)
                        },
                    )
                    .map_err(&quota)?,
            )
        }
    };
    am.admit_retained_resource_upper_bound(phase, names_bound)
        .map_err(&quota)?;
    let mut named_census = census;
    named_census.identifier_bytes = sum(&[census.identifier_bytes, names]).map_err(&quota)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(selections)
        .map_err(|_| quota(resource("conditional row allocation")))?;
    if rows.capacity() != selections {
        return Err(quota(resource("conditional row capacity")));
    }
    check_epoch(ctx, expected_epoch)?;
    Ok((PreparedRowsV1 { rows, named_census }, prepared_bound))
}

fn resolve(
    ctx: &Context,
    recipe: &Recipe,
    prepared: &PreparedOwnershipContractsV1,
    occurrences: &[OccurrenceV1],
    index: usize,
) -> Result<RowV1, FailureV1> {
    use SelectionErrorV1 as E;
    let occurrence = occurrences[index];
    let fail = |reason| selection(index, reason);
    let Some(RecipeOp::OwnershipContract {
        view,
        coverage,
        partition,
    }) = recipe
        .blocks()
        .get(occurrence.0.block as usize)
        .and_then(|block| block.operations().get(occurrence.0.operation as usize))
    else {
        return Err(fail(E::RecipeSite));
    };
    if *view != occurrence.0.view {
        return Err(fail(E::RecipeView));
    }
    for previous in &occurrences[..index] {
        if (previous.0.block, previous.0.operation) == (occurrence.0.block, occurrence.0.operation)
        {
            return Err(fail(E::DuplicateSite));
        }
        if previous.1 == occurrence.1 {
            return Err(fail(E::DuplicateOperation));
        }
        if previous.2 == occurrence.2 {
            return Err(fail(E::DuplicateView));
        }
    }
    let site = prepared
        .inventory
        .operations()
        .iter()
        .find(|site| site.pointer() == occurrence.1)
        .ok_or_else(|| fail(E::MissingOperation))?;
    let location = Site {
        block: site.block(),
        operation: site.operation(),
    };
    let dynamic = Operation::get_op_dyn(site.pointer(), ctx);
    let live = dynamic
        .downcast_ref::<OwnershipContractOp>()
        .ok_or_else(|| fail(E::NotContract))?;
    if site.pointer().deref(ctx).get_num_operands() != 1 {
        return Err(fail(E::NotContract));
    }
    if live.view(ctx) != occurrence.2 {
        return Err(fail(E::WrongView));
    }
    let contract = prepared
        .contracts
        .iter()
        .find(|contract| contract.location == location)
        .ok_or_else(|| fail(E::MissingCollectedContract))?;
    if contract.view != occurrence.2
        || contract.coverage != *coverage
        || contract.partition != *partition
        || live.coverage(ctx) != Some(contract.coverage)
        || live.partition(ctx) != Some(contract.partition)
    {
        return Err(fail(E::ContractMismatch));
    }
    let view =
        value_key_v1(ctx, &prepared.inventory, occurrence.2).ok_or_else(|| fail(E::ValueKey))?;
    Ok(RowV1 {
        recipe: occurrence.0,
        ownership: location,
        view,
        result: SelectedResultV1::NotRun,
    })
}

pub(crate) enum RaceSourceV1<'a> {
    // This must be the validated pass4 result from the same pipeline invocation.
    SameInvocation(&'a crate::RankedRaceReportV1),
    FreshNested,
}

// The pipeline admits the full-roster ownership/prerequisite bound and prepares
// dependency/trace caches once. It retains stage scratch across self-admitting
// live queries. This entry does not repeat those admissions or refund them.
#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "preserve independently varied preadmission test inputs and the existing observer adapter"
)]
pub(crate) fn run_preadmitted_v1(
    ctx: &Context,
    function: &FuncOp,
    am: &mut Manager,
    census: Census,
    expected_epoch: u64,
    recipe: &Recipe,
    occurrences: &[OccurrenceV1],
    race: RaceSourceV1<'_>,
    rows: Vec<RowV1>,
) -> ReportV1 {
    run_preadmitted_with_observation_v1(
        ctx,
        function,
        am,
        census,
        expected_epoch,
        recipe,
        occurrences,
        race,
        rows,
        None,
    )
}

#[cfg(test)]
#[allow(
    clippy::too_many_arguments,
    reason = "preserve independently varied preadmission test inputs and the existing observer adapter"
)]
pub(crate) fn run_preadmitted_with_observation_v1(
    ctx: &Context,
    function: &FuncOp,
    am: &mut Manager,
    census: Census,
    expected_epoch: u64,
    recipe: &Recipe,
    occurrences: &[OccurrenceV1],
    race: RaceSourceV1<'_>,
    rows: Vec<RowV1>,
    observer: OwnershipObserverV1<'_, '_, '_>,
) -> ReportV1 {
    run_preadmitted_with_admissions_v1(
        ExecutionInputV1 {
            ctx,
            function,
            census,
            expected_epoch,
            recipe,
            occurrences,
        },
        am,
        race,
        rows,
        (observer, None),
    )
}

pub(crate) struct ExecutionInputV1<'a> {
    pub(crate) ctx: &'a Context,
    pub(crate) function: &'a FuncOp,
    pub(crate) census: Census,
    pub(crate) expected_epoch: u64,
    pub(crate) recipe: &'a Recipe,
    pub(crate) occurrences: &'a [OccurrenceV1],
}

pub(crate) fn run_preadmitted_with_admissions_v1(
    input: ExecutionInputV1<'_>,
    am: &mut Manager,
    race: RaceSourceV1<'_>,
    rows: Vec<RowV1>,
    observations: (
        OwnershipObserverV1<'_, '_, '_>,
        AdditionalObservationV1<'_, '_, '_>,
    ),
) -> ReportV1 {
    let ExecutionInputV1 {
        ctx,
        function,
        census,
        expected_epoch,
        recipe,
        occurrences,
    } = input;
    let (observer, additional) = observations;
    let run = || {
        let mut out = ReportV1 {
            selected: rows,
            residual: None,
            failure: None,
            admitted: am.resource_upper_bound(),
        };
        let result = (|| -> Result<(), FailureV1> {
            check_epoch(ctx, expected_epoch)?;
            check_input(am, census, occurrences.len())?;
            if !out.selected.is_empty() || out.selected.capacity() != occurrences.len() {
                return Err(selection(0, SelectionErrorV1::RowStorage));
            }
            let mut prepared =
                prepare_ownership_contracts_with_observation_v1(ctx, function, am, observer)
                    .map_err(FailureV1::Collection)?;
            count(
                "blocks",
                prepared.inventory.blocks().len(),
                census.blocks,
                true,
            )?;
            count(
                "operations",
                prepared.inventory.operations().len(),
                census.operations,
                true,
            )?;
            count(
                "ownership contracts",
                prepared.contracts.len(),
                census.ownership_contracts,
                true,
            )?;
            let needs_effect_domain = prepared
                .contracts
                .iter()
                .any(|contract| contract.coverage == OwnershipCoverageAttr::ExactEffectDomain);
            let mut prerequisites = prepare_ownership_prerequisites_with_observation_v1(
                ctx, function, am, &prepared, observer,
            )
            .map_err(FailureV1::Prerequisite)?;
            if let Some(detail) = prerequisites.mandatory_bounds_failure.take() {
                return Err(FailureV1::Prerequisite(one_with_summary(
                    HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail },
                    prepared.coverage_summary,
                )));
            }
            let fresh;
            let race_report = match race {
                RaceSourceV1::SameInvocation(report) => Some(report),
                RaceSourceV1::FreshNested if !needs_effect_domain => {
                    fresh = run_pliron_ranked_race_with_observation_v1(ctx, function, am, observer);
                    Some(&fresh)
                }
                RaceSourceV1::FreshNested => None,
            };
            if let Some(report) = race_report.filter(|report| !report.is_clean()) {
                let detail = bounded_nested_findings_detail_v1(
                    "mandatory ranked race failed: ",
                    report.findings(),
                );
                return Err(FailureV1::Prerequisite(one_with_summary(
                    HierarchicalOwnershipFindingV1::EffectDomainIncomplete { detail },
                    prepared.coverage_summary,
                )));
            }
            check_epoch(ctx, expected_epoch)?;
            for index in 0..occurrences.len() {
                out.selected
                    .push(resolve(ctx, recipe, &prepared, occurrences, index)?);
            }
            for (index, row) in out.selected.iter_mut().enumerate() {
                let contract = prepared
                    .contracts
                    .iter()
                    .find(|contract| contract.location == row.ownership)
                    .ok_or_else(|| selection(index, SelectionErrorV1::MissingCollectedContract))?;
                let supported = contract.coverage == OwnershipCoverageAttr::TotalView
                    && contract.partition == OwnershipPartitionAttr::ExactSets
                    && contract.view_op.memory_space(ctx) == Some(MemorySpaceAttr::Global)
                    && contract
                        .view_op
                        .view_type(ctx)
                        .is_some_and(|ty| ty.deref(ctx).shape() == [DYNAMIC_EXTENT]);
                let checked = if supported {
                    check_conditional_ownership_live_rule_with_observation_v1(
                        (ctx, function),
                        &prepared.inventory,
                        census,
                        expected_epoch,
                        (occurrences[index].1, occurrences[index].2),
                        am,
                        additional,
                    )?
                } else {
                    Err(RuleRefusalV1::UnsupportedProfile)
                };
                row.result = match checked {
                    Ok(facts) if facts.view == row.view => SelectedResultV1::Checked(facts),
                    Ok(_) => SelectedResultV1::Refused(RuleRefusalV1::Coordinate),
                    Err(error) => SelectedResultV1::Refused(error),
                };
            }
            prepared.contracts.retain(|contract| {
                !out.selected
                    .iter()
                    .any(|row| row.ownership == contract.location)
            });
            let summary = declared_coverage_summary(&prepared.contracts);
            out.residual = Some(run_prepared_ownership_with_observation_v1(
                ctx,
                function,
                am,
                &prepared.inventory,
                prepared.contracts,
                summary,
                (prerequisites, observer),
            ));
            check_epoch(ctx, expected_epoch)
        })();
        out.failure = result.err();
        if let (Some(observer), Some(FailureV1::Resource(error))) = (observer, &out.failure) {
            observer.deny(*error);
        }
        out.admitted = am.resource_upper_bound();
        out
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}

impl ReportV1 {
    pub(crate) fn is_clean(&self) -> bool {
        self.failure.is_none()
            && !self.selected.is_empty()
            && self
                .selected
                .iter()
                .all(|row| matches!(row.result, SelectedResultV1::Checked(_)))
            && self
                .residual
                .as_ref()
                .is_some_and(HierarchicalOwnershipReportV1::is_clean)
    }
}
