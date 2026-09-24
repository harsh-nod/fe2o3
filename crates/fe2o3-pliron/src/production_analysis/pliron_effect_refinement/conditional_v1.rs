use super::*;
use crate::production::ConditionalPipelineSubjectV1;
use crate::production_analysis::{
    conditional_execution_v1 as own, pliron_ranked_coverage_v1::RuleSiteV1,
};
use pliron::op::Op;

#[derive(Debug, Eq, PartialEq)]
struct BindingV1 {
    selection: usize,
    effect: EffectRefinementLocationV1,
    write: RuleSiteV1,
    obligation: [u64; 4],
}

#[derive(Debug, Eq, PartialEq)]
#[allow(
    clippy::large_enum_variant,
    reason = "retain the admitted nested report without allocating during ownership transfer"
)]
enum NestedOwnershipV1 {
    NotRun,
    NoEffectContracts,
    Executed(own::ReportV1),
}

#[derive(Debug, Eq, PartialEq)]
enum FailureV1 {
    InputMismatch,
    BoundsDependency(own::BoundsFailureV1),
    BindingMismatch,
    Resource(Limit),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReportV1 {
    common: Option<EffectBodyV1>,
    ownership: NestedOwnershipV1,
    bindings: Vec<BindingV1>,
    failure: Option<FailureV1>,
}

pub(crate) struct PreparedV1<'a> {
    input: &'a ConditionalPipelineSubjectV1<'a>,
    rows: Vec<own::RowV1>,
    bindings: Vec<BindingV1>,
}

impl PreparedV1<'_> {
    pub(crate) fn input(&self) -> &ConditionalPipelineSubjectV1<'_> {
        self.input
    }
}

fn limit(resource: &'static str) -> Limit {
    Limit {
        phase: Phase::EffectRefinement,
        resource,
    }
}

pub(crate) fn preflight_v1(census: Census, selections: usize) -> Result<Bound, Limit> {
    let contracts = census
        .effect_refinement_contracts
        .min(MAX_EFFECT_REFINEMENT_CONTRACTS_V1);
    let items = checked_effect_sum_v1(&[
        census.operations,
        census.blocks,
        census.results,
        census.block_arguments,
        contracts,
        selections,
        1,
    ])?;
    let work = checked_effect_sum_v1(&[
        own::BOUNDS_DEPENDENCY_VALIDATION_WORK_V1,
        checked_effect_product_v1(own::BOUNDS_DEPENDENCY_OBLIGATION_WORK_V1, census.operands)?,
        checked_effect_product_v1(
            own::BOUNDS_DEPENDENCY_ROSTER_WORK_V1,
            checked_effect_sum_v1(&[census.operations, census.ownership_contracts])?,
        )?,
        checked_effect_product_v1(
            128,
            checked_effect_product_v1(checked_effect_sum_v1(&[contracts, selections, 1])?, items)?,
        )?,
    ])?;
    let retained = checked_effect_sum_v1(&[
        std::mem::size_of::<ReportV1>(),
        checked_effect_product_v1(contracts, std::mem::size_of::<BindingV1>())?,
    ])?;
    Bound::checked_phase(Phase::EffectRefinement, work, retained, 0)
}

// The caller retains the effect-local and full nested ownership reservations.
pub(crate) fn prepare_preadmitted_v1<'a>(
    input: &'a ConditionalPipelineSubjectV1<'a>,
    rows: Vec<own::RowV1>,
) -> Result<PreparedV1<'a>, Limit> {
    if !rows.is_empty() || rows.capacity() != input.occurrences().len() {
        return Err(limit("conditional nested ownership row storage"));
    }
    let count = input
        .census()
        .effect_refinement_contracts
        .min(MAX_EFFECT_REFINEMENT_CONTRACTS_V1);
    let mut bindings = Vec::new();
    bindings
        .try_reserve_exact(count)
        .map_err(|_| limit("conditional effect binding allocation"))?;
    if bindings.capacity() != count {
        return Err(limit("conditional effect binding capacity"));
    }
    Ok(PreparedV1 {
        input,
        rows,
        bindings,
    })
}

struct ModeV1<'a, 'b, 'o, 'p, 'r> {
    additional:
        crate::production_analysis::invocation_receipt_v1::AdditionalObservationV1<'o, 'p, 'r>,
    input: &'a ConditionalPipelineSubjectV1<'a>,
    bounds: &'b own::BoundsSourceV1<'b>,
    rows: Vec<own::RowV1>,
    out: ReportV1,
}

impl ModeV1<'_, '_, '_, '_, '_> {
    fn binding(
        &self,
        ctx: &Context,
        inventory: &Inventory,
        contract: &EffectContractV1,
        write: EffectRefinementLocationV1,
        selection: usize,
    ) -> Option<BindingV1> {
        let NestedOwnershipV1::Executed(ownership) = &self.out.ownership else {
            return None;
        };
        let occurrence = self.input.occurrences().get(selection)?;
        let row = ownership.selected.get(selection)?;
        let own::SelectedResultV1::Checked(facts) = row.result else {
            return None;
        };
        let site = inventory
            .operations()
            .iter()
            .find(|site| site.pointer() == occurrence.1)?;
        if row.recipe != occurrence.0
            || row.ownership.block() != site.block()
            || row.ownership.operation() != site.operation()
            || row.view != facts.view
            || own::value_key_v1(ctx, inventory, contract.view)? != facts.view
            || contract.indices.len() != 1
            || own::value_key_v1(ctx, inventory, contract.indices[0])? != facts.index
            || facts.write
                != (RuleSiteV1 {
                    block: write.block(),
                    operation: write.operation(),
                })
        {
            return None;
        }
        Some(BindingV1 {
            selection,
            effect: contract.location,
            write: facts.write,
            obligation: contract.obligation,
        })
    }
}

impl EffectModeV1 for ModeV1<'_, '_, '_, '_, '_> {
    fn no_contracts(&mut self) {
        self.out.ownership = NestedOwnershipV1::NoEffectContracts;
    }

    fn ownership(
        &mut self,
        execution: EffectExecutionV1<'_>,
        _: &[EffectContractV1],
        _: &mut Vec<PlironEffectRefinementFindingV1>,
        observer: EffectObserverV1<'_, '_, '_>,
    ) -> bool {
        let EffectExecutionV1 {
            context: ctx,
            function,
            analyses,
        } = execution;
        let ownership = own::run_preadmitted_with_bounds_v1(
            own::ExecutionInputV1 {
                ctx,
                function,
                census: self.input.census(),
                expected_epoch: self.input.epoch(),
                recipe: self.input.recipe(),
                occurrences: self.input.occurrences(),
            },
            analyses,
            own::RaceSourceV1::FreshNested,
            self.bounds,
            std::mem::take(&mut self.rows),
            (observer, self.additional),
        );
        let clean = ownership.is_clean();
        self.out.ownership = NestedOwnershipV1::Executed(ownership);
        clean
    }

    fn credit(
        &mut self,
        candidate: EffectWriteV1<'_>,
        observer: EffectObserverV1<'_, '_, '_>,
    ) -> bool {
        let EffectWriteV1 {
            context: ctx,
            inventory,
            contract,
            location: write,
        } = candidate;
        // Classify by the actual selected view before attempting a binding.
        let Some(selection) = self
            .input
            .occurrences()
            .iter()
            .position(|occurrence| occurrence.2 == contract.view)
        else {
            return true;
        };
        match self.binding(ctx, inventory, contract, write, selection) {
            Some(binding)
                if !self.out.bindings.iter().any(|old| {
                    old.selection == binding.selection || old.write == binding.write
                }) =>
            {
                if self.out.bindings.len() == self.out.bindings.capacity() {
                    let error = limit("conditional binding capacity exhausted");
                    if let Some(observer) = observer {
                        observer.deny(error);
                    }
                    self.out.failure.get_or_insert(FailureV1::Resource(error));
                } else {
                    self.out.bindings.push(binding);
                }
            }
            _ => {
                self.out.failure.get_or_insert(FailureV1::BindingMismatch);
            }
        }
        false
    }
}

impl ReportV1 {
    #[cfg(all(test, feature = "internal-proof-staging"))]
    pub(crate) fn test_parts(&self) -> ((usize, usize, usize), &[PlironEffectRefinementFindingV1]) {
        let common = self.common.as_ref().expect("effect body ran");
        (
            (
                common.contracts,
                common.proved_contracts,
                self.bindings.len(),
            ),
            &common.findings,
        )
    }

    #[cfg(all(test, feature = "internal-proof-staging"))]
    pub(crate) fn test_no_contracts(&self) -> bool {
        matches!(&self.ownership, NestedOwnershipV1::NoEffectContracts)
    }

    #[cfg(all(test, feature = "internal-proof-staging"))]
    pub(crate) fn test_binding_refusals(
        mut self,
        input: &ConditionalPipelineSubjectV1<'_>,
        analyses: &Manager,
    ) {
        use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
            InvocationReceiptFailureV1, InvocationReceiptV1,
        };

        assert!(self.is_clean());
        let binding = self.bindings.pop().expect("selected binding exists");
        assert!(!self.is_clean());
        self.bindings.push(binding);
        let inventory = analyses.function_inventory().unwrap();
        let (contracts, _, _, _, _) = collect(input.context(), inventory);
        let contract = &contracts[0];
        let mut mode = ModeV1 {
            input,
            bounds: &own::BoundsSourceV1::Ordinary,
            rows: Vec::new(),
            additional: None,
            out: self,
        };
        let credited = mode.credit(
            EffectWriteV1 {
                context: input.context(),
                inventory,
                contract,
                location: EffectRefinementLocationV1 {
                    block: usize::MAX,
                    operation: 0,
                },
            },
            None,
        );
        assert!(!credited);
        assert_eq!(mode.out.failure, Some(FailureV1::BindingMismatch));
        assert_eq!(mode.out.common.as_ref().unwrap().proved_contracts, 0);
        assert!(!mode.out.is_clean());

        let write = mode.out.bindings[0].write;
        mode.out.bindings = Vec::new();
        let mut receipt = InvocationReceiptV1::new(
            Default::default(),
            crate::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let phase = receipt
            .phase(
                crate::ProductionAnalysisResourcePhaseV1::EffectRefinement,
                0,
            )
            .unwrap();
        assert!(!mode.credit(
            EffectWriteV1 {
                context: input.context(),
                inventory,
                contract,
                location: EffectRefinementLocationV1 {
                    block: write.block,
                    operation: write.operation,
                },
            },
            Some(&phase.observer(&Ok)),
        ));
        drop(phase);
        // Keep the original semantic refusal, but do not lose the later quota.
        assert_eq!(mode.out.failure, Some(FailureV1::BindingMismatch));
        assert!(mode.out.bindings.is_empty());
        let error = limit("conditional binding capacity exhausted");
        assert_eq!(receipt.snapshot().first_denial, Some(error));
        assert!(!receipt.snapshot().caught_panic);
        assert_eq!(
            receipt.complete(),
            Err(InvocationReceiptFailureV1::Denied(error))
        );
    }

    pub(crate) fn is_clean(&self) -> bool {
        let (Some(common), NestedOwnershipV1::Executed(ownership)) =
            (&self.common, &self.ownership)
        else {
            return false;
        };
        self.failure.is_none()
            && common.findings.is_empty()
            && ownership.is_clean()
            && self.bindings.len() == ownership.selected.len()
            && common.proved_contracts.checked_add(self.bindings.len()) == Some(common.contracts)
    }
}

#[cfg(test)]
pub(crate) fn run_preadmitted_v1(
    ctx: &Context,
    function: &FuncOp,
    analyses: &mut Manager,
    prepared: PreparedV1<'_>,
) -> ReportV1 {
    run_preadmitted_with_observation_v1(ctx, function, analyses, prepared, None)
}

#[cfg(test)]
pub(crate) fn run_preadmitted_with_observation_v1(
    ctx: &Context,
    function: &FuncOp,
    analyses: &mut Manager,
    prepared: PreparedV1<'_>,
    observer: EffectObserverV1<'_, '_, '_>,
) -> ReportV1 {
    run_preadmitted_with_admissions_v1(ctx, function, analyses, prepared, (observer, None))
}

#[cfg(test)]
pub(crate) fn run_preadmitted_with_admissions_v1(
    ctx: &Context,
    function: &FuncOp,
    analyses: &mut Manager,
    prepared: PreparedV1<'_>,
    observations: (
        EffectObserverV1<'_, '_, '_>,
        crate::production_analysis::invocation_receipt_v1::AdditionalObservationV1<'_, '_, '_>,
    ),
) -> ReportV1 {
    run_preadmitted_with_bounds_v1(
        ctx,
        function,
        analyses,
        prepared,
        &own::BoundsSourceV1::Ordinary,
        observations,
    )
}

pub(crate) fn run_preadmitted_with_bounds_v1(
    ctx: &Context,
    function: &FuncOp,
    analyses: &mut Manager,
    prepared: PreparedV1<'_>,
    bounds: &own::BoundsSourceV1<'_>,
    observations: (
        EffectObserverV1<'_, '_, '_>,
        crate::production_analysis::invocation_receipt_v1::AdditionalObservationV1<'_, '_, '_>,
    ),
) -> ReportV1 {
    let (observer, additional) = observations;
    let run = || {
        let PreparedV1 {
            input,
            rows,
            bindings,
        } = prepared;
        let mut mode = ModeV1 {
            additional,
            input,
            bounds,
            rows,
            out: ReportV1 {
                common: None,
                ownership: NestedOwnershipV1::NotRun,
                bindings,
                failure: None,
            },
        };
        if !std::ptr::eq(ctx, input.context())
            || function.get_operation() != input.function().get_operation()
            || analyses.input_census() != Some(input.census())
            || ctx
                .ir_mutation_attempt_epoch()
                .ok()
                .map(|epoch| epoch.value())
                != Some(input.epoch())
        {
            mode.out.failure = Some(FailureV1::InputMismatch);
            return mode.out;
        }
        if let Err(error) = bounds.validate_v1(input, analyses) {
            mode.out.failure = Some(FailureV1::BoundsDependency(error));
            return mode.out;
        }
        mode.out.common = Some(run_effect_core_v1(
            ctx, function, analyses, &mut mode, observer,
        ));
        if ctx
            .ir_mutation_attempt_epoch()
            .ok()
            .map(|epoch| epoch.value())
            != Some(input.epoch())
        {
            mode.out.failure.get_or_insert(FailureV1::InputMismatch);
        }
        mode.out
    };
    match observer {
        None => run(),
        Some(observer) => observer.with_projection(&Ok, |_| run()),
    }
}
