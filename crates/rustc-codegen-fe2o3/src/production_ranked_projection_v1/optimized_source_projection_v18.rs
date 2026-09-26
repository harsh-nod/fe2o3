//! Bounded shared constant preparation, not a complete optimized backend entry.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use source_ranked_consumer_resources_v18::{add, product, resource};
use std::mem::size_of;

/// Scoped inputs for the next output-CFG/effect consumer. These are original
/// invocation facts and a borrowed actual output, not an equivalence proof.
pub(crate) struct SourceInvocationProjectionV18<'a, 'g> {
    pub(super) invocation: &'a canonical_source_facts_v18::SourceInvocationV18<'a, 'g>,
    pub(super) function: &'a SemanticFunctionDeclV1,
    pub(super) constants: Option<&'a [Option<u64>]>,
    pub(super) capabilities: Option<&'a ProjectedCapabilityEffectsV1>,
    pub(super) reads: Option<&'a [Option<ProjectedCapabilityReadEffectV1>]>,
    pub(super) output: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'g>,
    pub(super) decisions: Option<&'a fe2o3_lower_mir_kernel::ProductionOptimizedSourceEffectsV18>,
    output_cfg: Option<&'a fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'a, 'g>>,
    arguments: SourceInvocationArgumentsV18<'a>,
}

/// Exact original caller operands. These remain in the caller's local domain;
/// they are not callee constants or proofs of an optimized physical value.
#[derive(Clone, Copy)]
pub(crate) enum SourceInvocationArgumentsV18<'a> {
    Root,
    Call {
        caller: &'a SemanticFunctionDeclV1,
        block: SemanticBlockIdV1,
        operands: &'a [SemanticOperandV1],
        tail: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceInvocationDispositionV18 { Active, OriginalInactive }

struct InactiveInvocationMeterV18<'a, 'g, 'b, 'w> {
    original: &'a fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'g>,
    budget: &'b mut Budget<'w>,
}

impl fe2o3_mir_model::SemanticAssertionMeterV1 for InactiveInvocationMeterV18<'_, '_, '_, '_> {
    type Error = ProductionRankedProjectionErrorV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.original.check_query_v18(self.budget).map_err(canonical_source_facts_v18::source_error)?;
        self.budget.charge_work(amount)
            .map_err(|error| canonical_source_facts_v18::retained_resource(self.original, error))
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.original.check_query_v18(self.budget).map_err(canonical_source_facts_v18::source_error)?;
        self.budget.reserve_storage(bytes)
            .map_err(|error| canonical_source_facts_v18::retained_resource(self.original, error))
    }
}

struct InvocationEffectsV18 {
    capabilities: ProjectedCapabilityEffectsV1,
    reads: Vec<Option<ProjectedCapabilityReadEffectV1>>,
}

#[cfg(test)]
std::thread_local! {
    static INVOCATION_RESOURCES_V18: std::cell::Cell<(usize, usize)> = const {
        std::cell::Cell::new((0, 0))
    };
}

#[cfg(test)]
impl Drop for InvocationEffectsV18 {
    fn drop(&mut self) {
        INVOCATION_RESOURCES_V18.with(|counts| {
            let (created, dropped) = counts.get();
            counts.set((created, dropped + 1));
        });
    }
}

#[cfg(test)]
pub(crate) fn invocation_resource_counts_for_test_v18() -> (usize, usize) {
    INVOCATION_RESOURCES_V18.with(std::cell::Cell::get)
}

impl SourceInvocationProjectionV18<'_, '_> {
    pub(crate) fn root(&self) -> usize { self.invocation.root }
    pub(crate) fn instance(&self) -> usize { self.invocation.instance }
    pub(crate) fn function(&self) -> SemanticFunctionIdV1 { self.invocation.function }
    pub(crate) fn caller(&self) -> Option<(usize, SemanticBlockIdV1)> { self.invocation.caller }
    pub(crate) fn disposition(&self) -> SourceInvocationDispositionV18 {
        if self.invocation.active { SourceInvocationDispositionV18::Active }
        else { SourceInvocationDispositionV18::OriginalInactive }
    }
    pub(crate) fn blocks(&self) -> usize { self.function.blocks().len() }
    pub(crate) fn original_function(&self) -> &SemanticFunctionDeclV1 { self.function }
    pub(crate) fn arguments(&self) -> SourceInvocationArgumentsV18<'_> { self.arguments }
    pub(crate) fn output(&self) -> &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_> { self.output }
    pub(crate) fn decisions(&self) -> Option<&fe2o3_lower_mir_kernel::ProductionOptimizedSourceEffectsV18> {
        self.decisions
    }

    pub(crate) fn visit_output_cfg_v18(
        &self,
        meter: &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = ProductionRankedProjectionErrorV1>,
        consume: impl FnMut(
            fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgEventV18<'_, '_>,
            &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = ProductionRankedProjectionErrorV1>,
        ) -> Result<(), ProductionRankedProjectionErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        meter.charge_work(2)?;
        let cfg = self.output_cfg.filter(|cfg| self.invocation.active
            && cfg.root() == self.invocation.root && std::ptr::eq(cfg.inventory(), self.output))
            .ok_or_else(|| canonical_source_facts_v18::source_error(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                    "invocation has no exact active output CFG")))?;
        cfg.visit(meter, consume)
    }

    #[cfg(test)]
    pub(crate) fn fact_presence_for_test_v18(&self) -> [bool; 4] {
        [self.constants.is_some(), self.capabilities.is_some(), self.reads.is_some(), self.decisions.is_some()]
    }

    pub(crate) fn check_effect_rows_v18(
        &self,
        meter: &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = ProductionRankedProjectionErrorV1>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        meter.charge_work(6)?;
        let (Some(constants), Some(capabilities), Some(reads), Some(_)) =
            (self.constants, self.capabilities, self.reads, self.decisions) else {
                if !self.invocation.active && self.constants.is_none() && self.capabilities.is_none()
                    && self.reads.is_none() && self.decisions.is_none() { return Ok(()); }
                return Err(canonical_source_facts_v18::source_error(
                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                        "invocation view has a missing or substituted disposition payload")));
            };
        let blocks = self.function.blocks().len();
        if !self.invocation.active || constants.len() != self.function.locals().len()
            || capabilities.layouts.len() != blocks
            || capabilities.global_reads.len() != blocks
            || capabilities.transpose_workgroups.len() != blocks
            || capabilities.read_views.len() != blocks
            || reads.len() != blocks
        {
            return Err(canonical_source_facts_v18::source_error(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                    "invocation effect view changed its exact source block roster")));
        }
        Ok(())
    }
}

// The callback consumes the same solver results that the intrinsic projector
// consumes. It can retain owned non-authoritative output only by reporting its
// additional storage; borrowed function/CFG/effect views cannot escape.
pub(crate) fn with_optimized_source_invocations_v18<'g, F>(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'g>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'g>,
    budget: &mut Budget<'_>,
    mut consume: F,
) -> Result<(usize, usize), ProductionRankedProjectionErrorV1>
where F: FnMut(
        &SourceInvocationProjectionV18<'_, 'g>,
        &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = ProductionRankedProjectionErrorV1>,
    ) -> Result<usize, ProductionRankedProjectionErrorV1>,
{
    original.check_query_v18(budget).map_err(canonical_source_facts_v18::source_error)?;
    let scope_headers = invocation_scope_headers_v18::<F>()
        .map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
    budget.reserve_storage(scope_headers)
        .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
    let result = original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
        analyses.with_source_projection_facts(budget,
            |input_sparse, output_sparse, input_effects, output_effects, cleanup, budget| {
                let source = original.source(budget)?;
                let semantic = source.source_semantic(budget)?;
                let mut count = 0usize;
                let mut retained = 0usize;
                for root in 0..source.root_count(budget)? {
                    for instance in 0..source.instance_count(root, budget)? {
                        let floor = budget.storage();
                        let headers = invocation_headers_v18()
                            .map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
                        budget.reserve_storage(headers)
                            .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                        let invocation = canonical_source_facts_v18::SourceInvocationV18::new(
                            original, root, instance, budget)?;
                        let function = semantic.functions().get(invocation.function.index() as usize)
                            .ok_or_else(|| canonical_source_facts_v18::source_error(
                                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                                    "invocation projection function is absent")))?;
                        let arguments = if let Some((caller_instance, block)) = invocation.caller {
                            let caller_id = source.instance(root, caller_instance, budget)?.0;
                            let caller = semantic.functions().get(caller_id.index() as usize)
                                .ok_or_else(|| canonical_source_facts_v18::source_error(
                                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                                        "invocation caller function is absent")))?;
                            budget.charge_work(3)
                                .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                            let terminator = caller.blocks().get(block.index() as usize)
                                .ok_or_else(|| canonical_source_facts_v18::source_error(
                                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                                        "invocation caller block is absent")))?.terminator().kind();
                            let (callee, operands, tail) = match terminator {
                                SemanticTerminatorKindV1::Call(call) => (call.callee(), call.arguments(), false),
                                SemanticTerminatorKindV1::TailCall(call) => (call.callee(), call.arguments(), true),
                                _ => return Err(canonical_source_facts_v18::source_error(
                                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                                        "invocation caller is not a call"))),
                            };
                            if !matches!(semantic.callables().get(callee.index() as usize),
                                Some(SemanticCallableDeclV1::Defined { function }) if *function == invocation.function)
                            {
                                return Err(canonical_source_facts_v18::source_error(
                                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                                        "invocation argument callee differs from source instance")));
                            }
                            SourceInvocationArgumentsV18::Call { caller, block, operands, tail }
                        } else { SourceInvocationArgumentsV18::Root };
                        let kept = if invocation.active {
                        let mut pending = source_ranked_consumer_resources_v18::rows(
                            function.blocks().len(), budget)
                            .map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
                        budget.charge_work(function.blocks().len())
                            .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                        pending.resize(function.blocks().len(), None);
                        let project = |facts: &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>| {
                                facts.check_original_analysis_input_v18(semantic, invocation.function)?;
                                let enums = {
                                    let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
                                    SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(
                                        function, semantic.types(), &mut allocation).map_err(dominance_error_v18)?.0
                                };
                                with_optimized_source_constants_v18(facts, function, |constants, facts| {
                                    let resources = {
                                        let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
                                        let inventory = assertion_definition_inventory_with_allocation_v18(function, &mut allocation)?;
                                        let provenance = local_provenance_with_scalar_inventory_core_v18(
                                            semantic.callables(), semantic.types(), function,
                                            &inventory.counts, &inventory.address_escaped, &mut allocation)?;
                                        let contracts = local_allocation_contracts_core_v18(semantic.types(), function,
                                            &provenance.allocation_origins, &mut allocation)?;
                                        let effects = live_capability_driver_v18::project(semantic.callables(), function,
                                            &enums, &contracts, constants.values_for(function)?, &mut allocation)?;
                                        let reads = bind_capability_read_effects_core_v18(function,
                                            &effects.global_reads, &mut allocation)?;
                                        InvocationEffectsV18 { capabilities: effects, reads }
                                    };
                                    #[cfg(test)]
                                    INVOCATION_RESOURCES_V18.with(|counts| {
                                        let (created, dropped) = counts.get();
                                        counts.set((created + 1, dropped));
                                    });
                                    let decisions = optimized.instance_effects(root, instance, input_effects,
                                        output_effects, facts.constant_budget_v18()).map_err(canonical_source_facts_v18::source_error)?;
                                    let output = optimized.output_inventory(facts.constant_budget_v18())
                                        .map_err(canonical_source_facts_v18::source_error)?;
                                    let output_cfg = optimized.output_root_cfg_v18(root, facts.constant_budget_v18())
                                        .map_err(canonical_source_facts_v18::source_error)?;
                                    let view = SourceInvocationProjectionV18 { invocation: &invocation, function,
                                        constants: Some(constants.values_for(function)?), capabilities: Some(&resources.capabilities),
                                        reads: Some(&resources.reads), output, decisions: Some(&decisions),
                                        output_cfg: Some(&output_cfg), arguments };
                                    let before = facts.constant_budget_v18().storage();
                                    let kept = consume(&view, facts)?;
                                    let additional = facts.constant_budget_v18().storage().checked_sub(before)
                                        .ok_or_else(|| facts.retain_resource_error_v18(resource(Resource::Accounting)))?;
                                    if kept > additional {
                                        return Err(facts.retain_resource_error_v18(resource(Resource::Accounting)));
                                    }
                                    invocation.check(original, root, facts.constant_budget_v18())?;
                                    Ok(kept)
                                })
                            };
                        budget.reserve_storage(std::mem::size_of_val(&project))
                            .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                        let kept = optimized_source_facts_v18::with_optimized_source_projection_scope_v18(
                            original, optimized, input_sparse, output_sparse, input_effects, output_effects,
                            cleanup, root, canonical_source_facts_v18::SourceProjectionScopeV18::Invocation(&invocation),
                            &mut pending, budget, project)?;
                        drop(pending);
                        kept
                        } else {
                            invocation.check(original, root, budget)?;
                            let output = optimized.output_inventory(budget).map_err(canonical_source_facts_v18::source_error)?;
                            let view = SourceInvocationProjectionV18 { invocation: &invocation, function,
                                constants: None, capabilities: None, reads: None, output, decisions: None,
                                output_cfg: None, arguments };
                            let before = budget.storage();
                            let kept = consume(&view, &mut InactiveInvocationMeterV18 { original, budget })?;
                            let additional = budget.storage().checked_sub(before)
                                .ok_or_else(|| canonical_source_facts_v18::retained_resource(original, Resource::Accounting))?;
                            if kept > additional {
                                return Err(canonical_source_facts_v18::retained_resource(original, Resource::Accounting));
                            }
                            invocation.check(original, root, budget)?;
                            kept
                        };
                        drop(invocation);
                        original.check_query_v18(budget).map_err(canonical_source_facts_v18::source_error)?;
                        let scratch = budget.storage().checked_sub(floor).and_then(|bytes| bytes.checked_sub(kept))
                            .ok_or_else(|| canonical_source_facts_v18::retained_resource(original, Resource::Accounting))?;
                        budget.release_storage(scratch)
                            .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                        count = add(count, 1).map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
                        retained = add(retained, kept).map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
                    }
                }
                Ok((count, retained))
            })
    });
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            budget.release_storage(scope_headers)
                .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
            Ok(value)
        }
    }
}

fn invocation_scope_headers_v18<F>() -> Result<usize, ProductionRankedProjectionErrorV1> {
    // Both nested closures capture the original/optimized references and the
    // mutable callback borrow; the callback itself remains live until return.
    add(size_of::<F>(), add(product(6, size_of::<&()>())?,
        size_of::<Result<(usize, usize), ProductionRankedProjectionErrorV1>>())?)
}

fn invocation_headers_v18() -> Result<usize, ProductionRankedProjectionErrorV1> {
    let mut bytes = 0;
    for header in [
        size_of::<canonical_source_facts_v18::SourceInvocationV18<'_, '_>>(),
        size_of::<canonical_source_facts_v18::SourceProjectionScopeV18<'_, '_>>(),
        size_of::<SourceInvocationProjectionV18<'_, '_>>(),
        size_of::<(usize, usize, usize, usize)>(),
        size_of::<InvocationEffectsV18>(),
        size_of::<Result<usize, ProductionRankedProjectionErrorV1>>(),
        size_of::<Result<(usize, usize), ProductionRankedProjectionErrorV1>>(),
        size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceEffectsV18>(),
        size_of::<InactiveInvocationMeterV18<'_, '_, '_, '_>>(),
        size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>>(),
        size_of::<SourceInvocationArgumentsV18<'_>>(),
    ] { bytes = add(bytes, header)?; }
    Ok(bytes)
}

pub(super) enum ConstantAllocationV18<'b, 'w> {
    Legacy,
    Source(&'b mut Budget<'w>),
}

impl ConstantAllocationV18<'_, '_> {
    pub(super) fn charge(&mut self, work: usize) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy => Ok(()),
            Self::Source(budget) => budget.charge_work(work).map_err(resource),
        }
    }

    pub(super) fn charge_linear_resolution(
        &mut self,
        locals: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy => Ok(()),
            Self::Source(budget) => budget.charge_work(product(locals, 4)?).map_err(resource),
        }
    }

    pub(super) fn rows<T>(
        &mut self,
        count: usize,
        legacy_error: &'static str,
    ) -> Result<Vec<T>, ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy => {
                let mut rows = Vec::new();
                rows.try_reserve_exact(count).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(legacy_error)
                })?;
                Ok(rows)
            }
            Self::Source(budget) => source_ranked_consumer_resources_v18::rows(count, budget),
        }
    }

    pub(super) fn drop_rows<T>(
        &mut self,
        rows: Vec<T>,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        match self {
            Self::Legacy => {
                drop(rows);
                Ok(())
            }
            Self::Source(budget) => {
                let bytes = row_storage_v18(&rows)?;
                drop(rows);
                budget.release_storage(bytes).map_err(resource)
            }
        }
    }
}

fn row_storage_v18<T>(rows: &Vec<T>) -> Result<usize, ProductionRankedProjectionErrorV1> {
    add(size_of::<Vec<T>>(), product(rows.capacity(), size_of::<T>())?)
}

fn derive_original_induction_v18(
    facts: &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>,
    semantic: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
) -> Result<fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1, ProductionRankedProjectionErrorV1> {
    let result = derive_original_induction_core_v18(facts, semantic, function);
    result.map_err(|error| facts.retain_resource_error_v18(error))
}

fn derive_original_induction_core_v18(
    facts: &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>,
    semantic: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
) -> Result<fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1, ProductionRankedProjectionErrorV1> {
    facts.check_original_analysis_input_v18(semantic, function)?;
    let floor = facts.constant_budget_v18().storage();
    facts.constant_budget_v18().reserve_storage(
        add(size_of::<source_ranked_consumer_resources_v18::SourceAssertionMeterV18<'_, '_>>(),
            size_of::<Result<fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1, ProductionRankedProjectionErrorV1>>())?
    ).map_err(resource)?;
    let report = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_with_meter_v18(
        semantic, function, fe2o3_mir_model::SemanticU32InductionAnalysisLimitsV1::default(),
        &mut source_ranked_consumer_resources_v18::SourceAssertionMeterV18(facts.constant_budget_v18()),
    ).map_err(|error| match error {
        fe2o3_mir_model::SemanticU32InductionBoundSnapshotErrorV1::Analysis(error) =>
            ProductionRankedProjectionErrorV1::SemanticU32Induction(error),
        fe2o3_mir_model::SemanticU32InductionBoundSnapshotErrorV1::Meter(error) => error,
    })?;
    let retained = report.retained_storage_v18()
        .map_err(ProductionRankedProjectionErrorV1::SemanticU32Induction)?;
    facts.check_original_analysis_input_v18(semantic, function)?;
    let scratch = facts.constant_budget_v18().storage().checked_sub(floor)
        .and_then(|bytes| bytes.checked_sub(retained))
        .ok_or_else(|| resource(Resource::Accounting))?;
    facts.constant_budget_v18().release_storage(scratch).map_err(resource)?;
    Ok(report)
}

pub(super) fn dominance_error_v18(
    error: fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18<ProductionRankedProjectionErrorV1>,
) -> ProductionRankedProjectionErrorV1 {
    match error {
        fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18::Analysis(error) =>
            ProductionRankedProjectionErrorV1::Unsupported(error.detail()),
        fe2o3_mir_model::SemanticOptionDominanceMeteredErrorV18::Meter(error) => error,
    }
}

fn derive_original_dominance_v18(
    facts: &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>,
    semantic: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
) -> Result<(SemanticOptionDominanceV1, SemanticEnumPayloadDominanceV1, usize), ProductionRankedProjectionErrorV1> {
    let result = derive_original_dominance_core_v18(facts, semantic, function);
    result.map_err(|error| facts.retain_resource_error_v18(error))
}

fn derive_original_dominance_core_v18(
    facts: &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>,
    semantic: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
) -> Result<(SemanticOptionDominanceV1, SemanticEnumPayloadDominanceV1, usize), ProductionRankedProjectionErrorV1> {
    facts.check_original_analysis_input_v18(semantic, function)?;
    let original = semantic.functions().get(function.index() as usize)
        .ok_or(ProductionRankedProjectionErrorV1::Unsupported("the original dominance function is absent"))?;
    let floor = facts.constant_budget_v18().storage();
    facts.constant_budget_v18().reserve_storage(
        add(size_of::<source_ranked_consumer_resources_v18::SourceAssertionMeterV18<'_, '_>>(),
            size_of::<Result<(SemanticOptionDominanceV1, SemanticEnumPayloadDominanceV1, usize), ProductionRankedProjectionErrorV1>>())?
    ).map_err(resource)?;
    let mut meter = source_ranked_consumer_resources_v18::SourceAssertionMeterV18(facts.constant_budget_v18());
    let (producers, _) = fe2o3_mir_model::semantic_option_producers_with_meter_v18(
        original, semantic.callables(), &mut meter).map_err(dominance_error_v18)?;
    let (option, option_bytes) = SemanticOptionDominanceV1::analyze_with_meter_v18(
        original, &producers, &mut meter).map_err(dominance_error_v18)?;
    let (enums, enum_bytes) = SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(
        original, semantic.types(), &mut meter).map_err(dominance_error_v18)?;
    drop(producers);
    let tuple_header = size_of::<(SemanticOptionDominanceV1, SemanticEnumPayloadDominanceV1, usize)>()
        .checked_sub(size_of::<SemanticOptionDominanceV1>() + size_of::<SemanticEnumPayloadDominanceV1>())
        .ok_or_else(|| resource(Resource::Accounting))?;
    let retained = add(add(option_bytes, enum_bytes)?, tuple_header)?;
    facts.check_original_analysis_input_v18(semantic, function)?;
    let scratch = facts.constant_budget_v18().storage().checked_sub(floor)
        .and_then(|bytes| bytes.checked_sub(retained))
        .ok_or_else(|| resource(Resource::Accounting))?;
    facts.constant_budget_v18().release_storage(scratch).map_err(resource)?;
    Ok((option, enums, retained))
}

pub(super) fn observe_constant_custody_v18(
    budget: &Budget<'_>,
    cleanup: &fe2o3_pliron::CanonicalAnalysisCleanupV1<'_>,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if cleanup.refund_denied()
        || slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        cleanup.deny_refund();
        return Err(resource(Resource::Accounting));
    }
    Ok(())
}

// The constructor is private to this leaf. It runs only after the nominal
// optimized facts reader has authenticated its exact original source function.
pub(super) struct SourceConstantLocalsV18<'f> {
    function: &'f SemanticFunctionDeclV1,
    values: Vec<Option<u64>>,
}

impl SourceConstantLocalsV18<'_> {
    pub(super) fn values_for(
        &self,
        function: &SemanticFunctionDeclV1,
    ) -> Result<&[Option<u64>], ProductionRankedProjectionErrorV1> {
        if !std::ptr::eq(self.function, function) || self.values.len() != function.locals().len() {
            return Err(canonical_source_facts_v18::source_error(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                    "constant projection substituted its exact original function",
                ),
            ));
        }
        Ok(&self.values)
    }
}

#[cfg(test)]
pub(super) fn check_constant_owner_for_test_v18(
    function: &SemanticFunctionDeclV1,
    substituted: &SemanticFunctionDeclV1,
    values: Vec<Option<u64>>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let view = SourceConstantLocalsV18 { function, values };
    view.values_for(substituted).map(|_| ())
}

type ConstantScopeFrameV18<'s, 'f, 'a, 'r, 'i, 'g, 'b, 'w, F> = (
    &'s mut optimized_source_facts_v18::OptimizedSourceFactsV18<'a, 'r, 'i, 'g, 'b, 'w>,
    &'f SemanticFunctionDeclV1,
    F,
    &'s mut Option<usize>,
    usize,
    fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    usize,
    usize,
);

fn constant_scope_headers_v18<T, F>() -> Result<usize, ProductionRankedProjectionErrorV1> {
    let mut bytes = 0;
    for header in [
        size_of::<SourceConstantLocalsV18<'_>>(),
        size_of::<ConstantAllocationV18<'_, '_>>(),
        size_of::<Option<usize>>(),
        size_of::<(usize, fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1, usize)>(),
        size_of::<Result<Vec<Option<u64>>, ProductionRankedProjectionErrorV1>>(),
        size_of::<Result<T, ProductionRankedProjectionErrorV1>>(),
        size_of::<std::thread::Result<Result<T, ProductionRankedProjectionErrorV1>>>(),
        size_of::<std::panic::AssertUnwindSafe<
            ConstantScopeFrameV18<'_, '_, '_, '_, '_, '_, '_, '_, F>,
        >>(),
    ] {
        bytes = add(bytes, header)?;
    }
    Ok(bytes)
}

pub(super) fn with_optimized_source_constants_v18<'a, 'r, 'i, 'g, 'b, 'w, T, F>(
    facts: &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'a, 'r, 'i, 'g, 'b, 'w>,
    function: &SemanticFunctionDeclV1,
    consume: F,
) -> Result<T, ProductionRankedProjectionErrorV1>
where
    F: FnOnce(
        &SourceConstantLocalsV18<'_>,
        &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'a, 'r, 'i, 'g, 'b, 'w>,
    ) -> Result<T, ProductionRankedProjectionErrorV1>,
{
    facts.check_constant_function_v18(function)?;
    let headers = constant_scope_headers_v18::<T, F>()
        .map_err(|error| facts.retain_resource_error_v18(error))?;
    let reservation = facts.constant_budget_v18().reserve_storage(headers).map_err(resource);
    reservation.map_err(|error| facts.retain_resource_error_v18(error))?;
    let budget = facts.constant_budget_v18();
    let slot = budget as *const Budget<'_> as usize;
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let mut active_floor = None;
    // The unwind closure moves one complete tuple into a named function, so
    // its actual capture header is exactly the prepaid frame type above.
    let frame = (&mut *facts, function, consume, &mut active_floor, slot, ledger, floor, headers);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        run_constant_scope_v18(frame)
    }));
    match caught {
        Ok(result) => result,
        Err(payload) => {
            // Also observe underpayment during a raw unwind before any outer
            // source scope considers releasing its own scratch reservation.
            let _ = facts.constant_custody_v18(slot, ledger, active_floor.unwrap_or(floor));
            std::panic::resume_unwind(payload)
        }
    }
}

fn run_constant_scope_v18<'a, 'r, 'i, 'g, 'b, 'w, T, F>(
    frame: ConstantScopeFrameV18<'_, '_, 'a, 'r, 'i, 'g, 'b, 'w, F>,
) -> Result<T, ProductionRankedProjectionErrorV1>
where
    F: FnOnce(
        &SourceConstantLocalsV18<'_>,
        &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'a, 'r, 'i, 'g, 'b, 'w>,
    ) -> Result<T, ProductionRankedProjectionErrorV1>,
{
        let (facts, function, consume, active_floor, slot, ledger, floor, headers) = frame;
        let values = constant_locals_core_v18(
            function,
            &mut ConstantAllocationV18::Source(facts.constant_budget_v18()),
        );
        let values = values.map_err(|error| facts.retain_resource_error_v18(error))?;
        let retained = row_storage_v18(&values)
            .map_err(|error| facts.retain_resource_error_v18(error))?;
        let view = SourceConstantLocalsV18 { function, values };
        *active_floor = Some(facts.constant_budget_v18().storage());
        let result = consume(&view, facts);
        let custody = facts.constant_custody_v18(slot, ledger, active_floor.unwrap_or(floor));
        // On refusal the enclosing source scope still owns the credits, after
        // this value vector has dropped. An earlier error is never replaced.
        drop(view);
        match result {
            Err(error) => Err(error),
            Ok(value) => match custody {
                Err(error) => {
                    drop(value);
                    Err(facts.retain_resource_error_v18(error))
                }
                Ok(()) => {
                    let released = add(retained, headers)
                        .map_err(|error| facts.retain_resource_error_v18(error))?;
                    if let Err(error) = facts.constant_budget_v18()
                        .release_storage(released)
                        .map_err(resource)
                    {
                        facts.deny_constant_refund_v18();
                        drop(value);
                        return Err(facts.retain_resource_error_v18(error));
                    }
                    *active_floor = None;
                    Ok(value)
                }
            },
        }
}

#[cfg(test)]
pub(crate) fn inspect_actual_source_constants_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    inspect_actual_source_components_for_test_v18(original, optimized, budget, 0).map(|result| result.0)
}

#[cfg(test)]
pub(crate) fn inspect_actual_source_capability_collections_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    inspect_actual_source_components_for_test_v18(original, optimized, budget, 1).map(|result| result.1)
}

#[cfg(test)]
pub(crate) fn inspect_actual_source_capabilities_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    inspect_actual_source_components_for_test_v18(original, optimized, budget, 2).map(|result| result.1)
}

#[cfg(test)]
pub(crate) fn refuse_actual_source_capabilities_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    inspect_actual_source_components_for_test_v18(original, optimized, budget, 3).map(|result| result.1)
}

#[cfg(test)]
pub(crate) fn inspect_actual_source_typed_capabilities_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<(usize, bool), ProductionRankedProjectionErrorV1> {
    inspect_actual_source_invocation_capabilities_for_test_v18(original, optimized, budget, false)
}

#[cfg(test)]
pub(crate) fn fail_actual_source_constants_after_custody_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>, work_limit: usize,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    inspect_actual_source_components_with_limit_for_test_v18(original, optimized, budget, 5, Some(work_limit)).map(|result| result.1)
}

#[cfg(test)]
pub(crate) fn refuse_actual_source_capability_phase_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>, normalize: bool,
) -> (ProductionRankedProjectionErrorV1, capability_refusal_phase_v18::Observation) {
    use capability_refusal_phase_v18::{Phase, Run};
    let run = Run::start(if normalize { Phase::Normalize } else { Phase::Clear });
    let result = inspect_actual_source_invocation_capabilities_for_test_v18(original, optimized, budget, true);
    let observed = run.observation();
    assert!(observed.injected, "no real partial capability phase reached its next paid work step");
    assert!(observed.created > 0);
    assert_eq!(observed.created, observed.dropped, "the refused live driver retained its private workspace");
    (result.err().expect("the phase-refused driver returned a successful value"), observed)
}

// Kernel-context entries are physical wrappers, not the logical source body.
// Observe every authentic active invocation using the same scope constructor as
// the production invocation driver; a source function number alone is not a key.
#[cfg(test)]
fn inspect_actual_source_invocation_capabilities_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
    refuse_phase: bool,
) -> Result<(usize, bool), ProductionRankedProjectionErrorV1> {
    original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
        analyses.with_source_projection_facts(budget,
            |input_sparse, output_sparse, input_effects, output_effects, cleanup, budget| {
                let source = original.source(budget)?;
                let semantic = source.source_semantic(budget)?;
                let mut total = (0usize, false);
                for root in 0..source.root_count(budget)? {
                    for instance in 0..source.instance_count(root, budget)? {
                        let floor = budget.storage();
                        // The shared invocation allowance covers its owner,
                        // pending scope, fixed observation and result envelopes.
                        let headers = invocation_headers_v18()
                            .map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
                        budget.reserve_storage(headers)
                            .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                        let invocation = canonical_source_facts_v18::SourceInvocationV18::new(
                            original, root, instance, budget)?;
                        if invocation.active {
                            let function = semantic.functions().get(invocation.function.index() as usize)
                                .ok_or_else(|| canonical_source_facts_v18::source_error(
                                    fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Binding(
                                        "capability observer invocation function is absent")))?;
                            let mut pending = source_ranked_consumer_resources_v18::rows(
                                function.blocks().len(), budget)
                                .map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
                            budget.charge_work(function.blocks().len())
                                .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                            pending.resize(function.blocks().len(), None);
                            let project = |facts: &mut optimized_source_facts_v18::OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_>| {
                                facts.check_original_analysis_input_v18(semantic, invocation.function)?;
                                let enums = {
                                    let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
                                    SemanticEnumPayloadDominanceV1::analyze_with_meter_v18(
                                        function, semantic.types(), &mut allocation).map_err(dominance_error_v18)?.0
                                };
                                with_optimized_source_constants_v18(facts, function, |constants, facts| {
                                    let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
                                    let inventory = assertion_definition_inventory_with_allocation_v18(function, &mut allocation)?;
                                    let provenance = local_provenance_with_scalar_inventory_core_v18(
                                        semantic.callables(), semantic.types(), function,
                                        &inventory.counts, &inventory.address_escaped, &mut allocation)?;
                                    let contracts = local_allocation_contracts_core_v18(semantic.types(), function,
                                        &provenance.allocation_origins, &mut allocation)?;
                                    if refuse_phase {
                                        drop(allocation);
                                        let mut meter = PhaseRefusalMeterV18 { inner: facts };
                                        let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::Source(&mut meter);
                                        allocation.header::<PhaseRefusalMeterV18<'_>>()?;
                                        let effects = live_capability_driver_v18::project(semantic.callables(), function,
                                            &enums, &contracts, constants.values_for(function)?, &mut allocation)?;
                                        drop(effects);
                                        // A wrapper with no live capability need not
                                        // reach a partial phase. Continue to its child.
                                        Ok((0, false))
                                    } else {
                                        live_capability_driver_v18::count_typed_entries_for_test(
                                            semantic.callables(), function, &enums, &contracts,
                                            constants.values_for(function)?, &mut allocation)
                                    }
                                })
                            };
                            budget.reserve_storage(std::mem::size_of_val(&project))
                                .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                            let observed = optimized_source_facts_v18::with_optimized_source_projection_scope_v18(
                                original, optimized, input_sparse, output_sparse, input_effects, output_effects,
                                cleanup, root, canonical_source_facts_v18::SourceProjectionScopeV18::Invocation(&invocation),
                                &mut pending, budget, project)?;
                            total.0 = add(total.0, observed.0)
                                .map_err(|error| canonical_source_facts_v18::retain_resource_error(original, error))?;
                            total.1 |= observed.1;
                            drop(pending);
                        }
                        invocation.check(original, root, budget)?;
                        drop(invocation);
                        original.check_query_v18(budget).map_err(canonical_source_facts_v18::source_error)?;
                        let scratch = budget.storage().checked_sub(floor)
                            .ok_or_else(|| canonical_source_facts_v18::retained_resource(original, Resource::Accounting))?;
                        budget.release_storage(scratch)
                            .map_err(|error| canonical_source_facts_v18::retained_resource(original, error))?;
                    }
                }
                Ok(total)
            })
    })
}

#[cfg(test)]
trait PhaseBudgetV18: fe2o3_mir_model::SemanticAssertionMeterV1<Error = ProductionRankedProjectionErrorV1> {
    fn observe(&mut self) -> (usize, usize, Option<usize>, Option<usize>);
}

#[cfg(test)]
impl PhaseBudgetV18 for optimized_source_facts_v18::OptimizedSourceFactsV18<'_, '_, '_, '_, '_, '_> {
    fn observe(&mut self) -> (usize, usize, Option<usize>, Option<usize>) {
        let budget = self.constant_budget_v18();
        (budget.work(), budget.storage(), budget.failed_work(), budget.failed_storage())
    }
}

#[cfg(test)]
struct PhaseRefusalMeterV18<'a> { inner: &'a mut dyn PhaseBudgetV18 }

#[cfg(test)]
impl fe2o3_mir_model::SemanticAssertionMeterV1 for PhaseRefusalMeterV18<'_> {
    type Error = ProductionRankedProjectionErrorV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        if amount > 0 && capability_refusal_phase_v18::should_refuse() {
            let (work_before, storage_before, failed_work, failed_storage) = self.inner.observe();
            assert_eq!((failed_work, failed_storage), (None, None));
            let result = self.inner.charge_work(usize::MAX);
            let (work_after, storage_after, first_failed_work, first_failed_storage) = self.inner.observe();
            assert!(result.is_err());
            capability_refusal_phase_v18::record(capability_refusal_phase_v18::Observation {
                injected: true, work_before, work_after, storage_before, storage_after,
                first_failed_work, first_failed_storage, requested_work: amount,
                ..capability_refusal_phase_v18::Observation::default()
            });
            result
        } else { self.inner.charge_work(amount) }
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> { self.inner.reserve_storage(bytes) }
}

#[cfg(test)]
struct DriverRefusalMeterV18<'a> {
    inner: &'a mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = ProductionRankedProjectionErrorV1>,
    work_calls: usize,
    refuse_at: usize,
}

#[cfg(test)]
impl fe2o3_mir_model::SemanticAssertionMeterV1 for DriverRefusalMeterV18<'_> {
    type Error = ProductionRankedProjectionErrorV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.work_calls = add(self.work_calls, 1)?;
        // Refuse through the genuine source meter, leaving ample accepted
        // budget for later small charges. This tests custody, not exhaustion.
        self.inner.charge_work(if self.work_calls == self.refuse_at { usize::MAX } else { amount })
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.inner.reserve_storage(bytes)
    }
}

#[cfg(test)]
fn capability_driver_probe_v18(callables: &[SemanticCallableDeclV1], function: &SemanticFunctionDeclV1,
    enums: &SemanticEnumPayloadDominanceV1, contracts: &[Option<AllocationContractV1>], constants: &[Option<u64>],
    allocation: &mut source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>, refuse: bool,
) -> Result<ProjectedCapabilityEffectsV1, ProductionRankedProjectionErrorV1> {
    use source_ranked_consumer_resources_v18::ProjectionAllocationV18 as Allocation;
    if !refuse { return live_capability_driver_v18::project(callables, function, enums, contracts, constants, allocation); }
    allocation.header::<DriverRefusalMeterV18<'_>>()?;
    allocation.header::<usize>()?;
    let Allocation::Source(inner) = allocation else { return Err(resource(Resource::Accounting)); };
    let work_calls = {
        let mut meter = DriverRefusalMeterV18 { inner: &mut **inner, work_calls: 0, refuse_at: usize::MAX };
        let effects = live_capability_driver_v18::project(callables, function, enums, contracts, constants, &mut Allocation::Source(&mut meter))?;
        drop(effects);
        meter.work_calls
    };
    let mut meter = DriverRefusalMeterV18 { inner: &mut **inner, work_calls: 0, refuse_at: work_calls };
    let result = live_capability_driver_v18::project(callables, function, enums, contracts, constants, &mut Allocation::Source(&mut meter));
    assert!(result.is_err(), "the real driver did not propagate its final injected work refusal");
    result
}

#[cfg(test)]
fn inspect_actual_source_components_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
    inspect_capabilities: u8,
) -> Result<(usize, usize, bool), ProductionRankedProjectionErrorV1> {
    inspect_actual_source_components_with_limit_for_test_v18(original, optimized, budget, inspect_capabilities, None)
}

#[cfg(test)]
fn inspect_actual_source_components_with_limit_for_test_v18(
    original: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    optimized: &fe2o3_lower_mir_kernel::ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>, inspect_capabilities: u8, work_limit: Option<usize>,
) -> Result<(usize, usize, bool), ProductionRankedProjectionErrorV1> {
    original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
        analyses.with_source_projection_facts(budget,
            |input_sparse, output_sparse, input_effects, output_effects, cleanup, budget| {
                let source = original.source(budget)?;
                let semantic = source.source_semantic(budget)?;
                let root = source.root(0, budget)?.0;
                let selection = semantic.select_kernel_body_for_root_v1(root)
                    .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                        "constant test fixture has no selected original body"))?;
                let function = &semantic.functions()[selection.body().index() as usize];
                let mut pending = source_ranked_consumer_resources_v18::rows(
                    function.blocks().len(), budget)?;
                pending.resize(function.blocks().len(), None);
                let count = optimized_source_facts_v18::with_optimized_source_projection_facts_v18(
                    original, optimized, input_sparse, output_sparse, input_effects, output_effects,
                    cleanup, 0, selection, &mut pending, budget, |facts| {
                        let floor = facts.constant_budget_v18().storage();
                        let induction = derive_original_induction_v18(facts, semantic, selection.body())?;
                        assert_eq!(induction.semantic_mir_sha256(), semantic.semantic_sha256());
                        assert_eq!(induction.function(), selection.body());
                        assert!(!induction.grants_authority());
                        let (option, enums, dominance_bytes) = derive_original_dominance_v18(
                            facts, semantic, selection.body())?;
                        assert!(!option.grants_authority());
                        assert!(!enums.grants_authority());
                        let mut allocation=source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
                        assert!(matches!(allocation,source_ranked_consumer_resources_v18::ProjectionAllocationV18::Source(_)));
                        let scratch=allocation.copy_slice(&[1_u64,2,3])?;
                        let scratch_bytes=add(row_storage_v18(&scratch)?,size_of::<source_ranked_consumer_resources_v18::ProjectionAllocationV18<'_>>())?;
                        drop(allocation);
                        drop(scratch);
                        let released = facts.constant_budget_v18().release_storage(scratch_bytes).map_err(resource);
                        released.map_err(|error| facts.retain_resource_error_v18(error))?;
                        let (count, capability_blocks, typed_together) = with_optimized_source_constants_v18(facts, function, |view, facts| {
                            let constants = view.values_for(function)?;
                            if inspect_capabilities == 5 {
                                let budget = facts.constant_budget_v18();
                                let remaining = work_limit.expect("chronology fixture requires its owned-account limit")
                                    .checked_sub(budget.work()).expect("fixture exceeded its owned-account limit");
                                budget.charge_work(remaining).unwrap();
                                budget.release_storage(1).unwrap();
                                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                                    "selected constant consumer before custody postflight"));
                            }
                            if inspect_capabilities < 2 { return Ok((constants.len(), 0, false)); }
                            facts.check_constant_function_v18(function)?;
                            let budget = facts.constant_budget_v18();
                            let slot = budget as *const Budget<'_> as usize;
                            let ledger = budget.work_ledger_identity_v1();
                            let before = budget.storage();
                            let (blocks, together) = {
                                let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
                                let inventory = assertion_definition_inventory_with_allocation_v18(function, &mut allocation)?;
                                let provenance = local_provenance_with_scalar_inventory_core_v18(
                                    semantic.callables(), semantic.types(), function,
                                    &inventory.counts, &inventory.address_escaped, &mut allocation)?;
                                let contracts = local_allocation_contracts_core_v18(semantic.types(), function,
                                    &provenance.allocation_origins, &mut allocation)?;
                                let effects = capability_driver_probe_v18(semantic.callables(), function,
                                    &enums, &contracts, constants, &mut allocation, inspect_capabilities == 3)?;
                                let blocks = function.blocks().len();
                                assert_eq!(effects.layouts.len(), blocks);
                                assert_eq!(effects.global_reads.len(), blocks);
                                assert_eq!(effects.transpose_workgroups.len(), blocks);
                                assert_eq!(effects.read_views.len(), blocks);
                                (blocks, false)
                            };
                            facts.constant_custody_v18(slot, ledger, before)
                                .map_err(|error| facts.retain_resource_error_v18(error))?;
                            let scratch = facts.constant_budget_v18().storage().checked_sub(before)
                                .ok_or_else(|| facts.retain_resource_error_v18(resource(Resource::Accounting)))?;
                            let released = facts.constant_budget_v18().release_storage(scratch).map_err(resource);
                            released.map_err(|error| facts.retain_resource_error_v18(error))?;
                            Ok((constants.len(), blocks, together))
                        })?;
                        let keys = if inspect_capabilities == 1 {
                            facts.check_constant_function_v18(function)?;
                            let before = facts.constant_budget_v18().storage();
                            let mut allocation = source_ranked_consumer_resources_v18::ProjectionAllocationV18::from_facts(facts)?;
                            // This probe exercises source-key collections only. No
                            // pipeline aliases or capability values are asserted.
                            let owner_seeds = allocation.filled(function.locals().len(), None)?;
                            let domain = live_capability_collections_v18::Domain::prepare(function, &owner_seeds, &mut allocation)?;
                            let workspace = live_capability_collections_v18::Workspace::new(&domain, &mut allocation)?;
                            let empty = workspace.snapshot(&mut allocation)?;
                            assert!(empty.rows.is_empty());
                            for &key in &domain.keys {
                                assert!(workspace.get(function, key, &mut allocation)?.is_none());
                            }
                            let count = domain.keys.len();
                            drop(empty);
                            drop(workspace);
                            drop(domain);
                            drop(owner_seeds);
                            drop(allocation);
                            let scratch = facts.constant_budget_v18().storage().checked_sub(before)
                                .ok_or_else(|| facts.retain_resource_error_v18(resource(Resource::Accounting)))?;
                            let released = facts.constant_budget_v18().release_storage(scratch).map_err(resource);
                            released.map_err(|error| facts.retain_resource_error_v18(error))?;
                            count
                        } else { capability_blocks };
                        let retained = induction.retained_storage_v18()
                            .map_err(ProductionRankedProjectionErrorV1::SemanticU32Induction)?;
                        drop(induction);
                        drop(option);
                        drop(enums);
                        let released = add(retained, dominance_bytes)
                            .map_err(|error| facts.retain_resource_error_v18(error))?;
                        let released = facts.constant_budget_v18().release_storage(released).map_err(resource);
                        released.map_err(|error| facts.retain_resource_error_v18(error))?;
                        assert_eq!(facts.constant_budget_v18().storage(), floor);
                        Ok((count, keys, typed_together))
                    })?;
                let bytes = row_storage_v18(&pending)?;
                drop(pending);
                budget.release_storage(bytes).map_err(resource)?;
                Ok(count)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn invocation_headers_have_independent_exact_and_one_short_storage_oracles() {
        type Callback = [u128; 7];
        let scope = size_of::<Callback>() + 6 * size_of::<&()>()
            + size_of::<Result<(usize, usize), ProductionRankedProjectionErrorV1>>();
        let invocation = size_of::<canonical_source_facts_v18::SourceInvocationV18<'_, '_>>()
            + size_of::<canonical_source_facts_v18::SourceProjectionScopeV18<'_, '_>>()
            + size_of::<SourceInvocationProjectionV18<'_, '_>>()
            + 4 * size_of::<usize>()
            + size_of::<InvocationEffectsV18>()
            + size_of::<Result<usize, ProductionRankedProjectionErrorV1>>()
            + size_of::<Result<(usize, usize), ProductionRankedProjectionErrorV1>>()
            + size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceEffectsV18>()
            + size_of::<InactiveInvocationMeterV18<'_, '_, '_, '_>>()
            + size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>>()
            + size_of::<SourceInvocationArgumentsV18<'_>>();
        assert_eq!(invocation_scope_headers_v18::<Callback>().unwrap(), scope);
        assert_eq!(invocation_headers_v18().unwrap(), invocation);
        for expected in [scope, invocation] {
            for short in [false, true] {
                let mut work = Work::new(0);
                let mut budget = Budget::new(&mut work, 29 + expected - usize::from(short));
                budget.reserve_storage(29).unwrap();
                let result = budget.reserve_storage(expected);
                if short {
                    let Err(Resource::Storage(error)) = result else { panic!("missing header refusal"); };
                    assert_eq!((error.actual(), error.limit()), (29 + expected, 28 + expected));
                    assert_eq!((budget.storage(), budget.peak_storage()), (29, 29));
                    assert_eq!(budget.failed_storage(), Some(29 + expected));
                } else {
                    result.unwrap();
                    assert_eq!((budget.storage(), budget.peak_storage()), (29 + expected, 29 + expected));
                    budget.release_storage(expected).unwrap();
                    assert_eq!(budget.storage(), 29);
                    assert_eq!(budget.failed_storage(), None);
                }
                assert_eq!((budget.work(), budget.failed_work()), (0, None));
            }
        }
    }

    #[test]
    fn constant_custody_denial_is_sticky_and_keeps_first_resource_refusals() {
        for hostile in 0..4 {
            let cleanup = fe2o3_pliron::CanonicalAnalysisCleanupV1::new();
            let mut work = Work::new(3);
            let mut other_work = Work::new(3);
            let mut budget = Budget::new(&mut work, 31);
            budget.reserve_storage(31).unwrap();
            let slot = &budget as *const Budget<'_> as usize;
            let ledger = budget.work_ledger_identity_v1();
            observe_constant_custody_v18(&budget, &cleanup, slot, ledger, 31).unwrap();
            budget.charge_work(3).unwrap();
            assert!(budget.charge_work(1).is_err());
            assert!(budget.reserve_storage(1).is_err());
            let other = Budget::new(&mut other_work, 31);
            let (checked_slot, checked_ledger) = match hostile {
                0 => { budget.release_storage(1).unwrap(); (slot, ledger) }
                1 => (slot ^ 1, ledger),
                2 => (slot, other.work_ledger_identity_v1()),
                3 => { cleanup.deny_refund(); (slot, ledger) }
                _ => unreachable!(),
            };
            let before = (budget.work(), budget.storage(), budget.peak_storage(), budget.failed_storage());
            assert!(observe_constant_custody_v18(&budget, &cleanup, checked_slot, checked_ledger, 31).is_err());
            assert!(cleanup.refund_denied());
            assert!(observe_constant_custody_v18(&budget, &cleanup, slot, ledger, 0).is_err());
            assert_eq!((budget.work(), budget.storage(), budget.peak_storage(), budget.failed_storage()), before);
            drop(budget);
            assert_eq!(work.failed_work(), Some(4));
        }
    }

    #[test]
    fn constant_scope_header_has_independent_exact_and_short_oracles() {
        type Payload = [u128; 3];
        type Callback = [u128; 7];
        type Facts<'a, 'r, 'i, 'g, 'b, 'w> =
            optimized_source_facts_v18::OptimizedSourceFactsV18<'a, 'r, 'i, 'g, 'b, 'w>;
        type Frame<'a> = (
            &'a mut Facts<'a, 'a, 'a, 'a, 'a, 'a>,
            &'a SemanticFunctionDeclV1,
            Callback,
            &'a mut Option<usize>,
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
        );
        let expected = size_of::<SourceConstantLocalsV18<'_>>()
            + size_of::<ConstantAllocationV18<'_, '_>>()
            + size_of::<Option<usize>>()
            + size_of::<(usize, fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1, usize)>()
            + size_of::<Result<Vec<Option<u64>>, ProductionRankedProjectionErrorV1>>()
            + size_of::<Result<Payload, ProductionRankedProjectionErrorV1>>()
            + size_of::<std::thread::Result<Result<Payload, ProductionRankedProjectionErrorV1>>>()
            + size_of::<std::panic::AssertUnwindSafe<Frame<'_>>>();
        assert_eq!(constant_scope_headers_v18::<Payload, Callback>().unwrap(), expected);
        for short in [false, true] {
            let mut work = Work::new(0);
            let limit = 11 + expected - usize::from(short);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(11).unwrap();
            let result = budget.reserve_storage(constant_scope_headers_v18::<Payload, Callback>().unwrap());
            if short {
                let Err(Resource::Storage(error)) = result else { panic!("lost header refusal"); };
                assert_eq!((error.actual(), error.limit()), (11 + expected, limit));
                assert_eq!(budget.storage(), 11);
                assert_eq!(budget.failed_storage(), Some(11 + expected));
            } else {
                result.unwrap();
                assert_eq!(budget.storage(), limit);
                assert_eq!(budget.failed_storage(), None);
            }
            assert_eq!(budget.work(), 0);
        }
    }
}
