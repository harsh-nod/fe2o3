//! One fixed production-oriented verifier sequence for ranked PLIRON kernels.
//!
//! The production API has one nine-stage route. Historical V1 report and
//! entry-point names are intentionally unavailable:
//!
//! ```compile_fail
//! use fe2o3_kernel_analysis::{
//!     ProductionPlironPreloweringErrorV1, ProductionPlironPreloweringReportV1,
//!     PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1,
//!     require_production_pliron_checks_before_lowering_v1,
//! };
//! ```

use std::{error::Error, fmt};

#[cfg(test)]
use std::cell::Cell;

use pliron::{builtin::ops::FuncOp, context::Context};

use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_atomic_legality::{
    preflight_atomic_legality_resource_upper_bound_v1,
    require_pliron_atomic_legality_with_analyses_v1,
};
use crate::production_analysis::pliron_barrier::{
    admit_barrier_progress_probe_v1, compose_barrier_dependencies_v1,
    preflight_barrier_convergence_resource_upper_bound_v1,
    require_pliron_barrier_with_scoped_input_v1,
};
use crate::production_analysis::pliron_effect_refinement::preflight_effect_refinement_resource_upper_bound_v1;
use crate::production_analysis::pliron_hierarchical_ownership::{
    preflight_hierarchical_ownership_resource_upper_bound_v1,
    require_pliron_hierarchical_ownership_with_analyses_v1,
};
use crate::production_analysis::pliron_invocation_trace::{
    preflight_execution_layout_resource_upper_bound_v1,
    preflight_invocation_trace_resource_upper_bound_v1,
};
use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
use crate::production_analysis::pliron_launch_contract::{
    preflight_launch_contract_resource_upper_bound_v1,
    require_pliron_launch_contract_with_analyses_v1,
};
use crate::production_analysis::pliron_memory_order::preflight_memory_order_attempt_resource_upper_bound_v1;
#[cfg(test)]
use crate::production_analysis::pliron_pass_contract::begin_production_pliron_pass_contract_session_v1;
use crate::production_analysis::pliron_pass_contract::{
    PlironPassContractSessionV1, PlironPassPreservationErrorV1, PlironPassPreservationReportV1,
    begin_production_pliron_pass_contract_session_with_resource_limits_v1,
};
use crate::production_analysis::pliron_pipeline_protocol::{
    preflight_pipeline_protocol_resource_upper_bound_v1,
    require_pliron_pipeline_protocol_with_analyses_v1,
};
use crate::production_analysis::pliron_presburger_adapter::preflight_presburger_resource_upper_bound_v1;
use crate::production_analysis::pliron_progress::preflight_scoped_progress_resource_upper_bound_v1;
use crate::production_analysis::pliron_provenance_alias::preflight_provenance_alias_resource_upper_bound_v1;
use crate::production_analysis::pliron_race::{
    preflight_race_resource_upper_bound_v1, require_pliron_ranked_race_freedom_with_analyses_v1,
};
use crate::production_analysis::pliron_ranked_bounds::{
    preflight_ranked_bounds_resource_upper_bound_v1, require_pliron_ranked_bounds_with_analyses_v1,
};
use crate::production_analysis::pliron_report_validation::{
    ProductionAnalysisReportEndpointV1, ProductionAnalysisReportValidationErrorV1,
    ProductionAnalysisReportValidationSessionV1, ProductionAnalysisReportValidationV1,
    SealedProductionAnalysisReportV1,
    begin_production_analysis_report_validation_with_resource_limits_v1,
};
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourceUpperBoundV1,
};
use crate::production_analysis::pliron_semantic_refinement::{
    preflight_semantic_refinement_resource_upper_bound_v1,
    require_pliron_semantic_refinement_with_scoped_input_v1,
};
use crate::production_analysis::pliron_simt_protocol::preflight_simt_protocol_resource_upper_bound_v1;
use crate::production_analysis::pliron_sparse_index::preflight_sparse_index_resource_upper_bound_v1;
use crate::production_analysis::pliron_tensor_layout::{
    preflight_tensor_layout_resource_upper_bound_v1, require_pliron_tensor_layout_with_analyses_v1,
};
use crate::production_analysis::pliron_workgroup_memory::{
    preflight_prepared_workgroup_memory_resource_upper_bound_v1,
    require_pliron_workgroup_memory_with_analyses_v1,
};
use crate::{
    HierarchicalOwnershipCheckErrorV1, HierarchicalOwnershipReportV1, KernelCheckPassKindV1,
    KernelCheckStatusV1, PlironAtomicLegalityCheckErrorV1, PlironAtomicLegalityReportV1,
    PlironAtomicTargetContextV1, PlironBarrierCheckErrorV1, PlironBarrierReportV1,
    PlironLaunchContractCheckErrorV1, PlironLaunchContractReportV1, PlironLaunchContractV1,
    PlironPipelineProtocolCheckErrorV1, PlironPipelineProtocolReportV1,
    PlironSemanticRefinementCheckErrorV1, PlironSemanticRefinementReportV1,
    PlironTensorLayoutCheckErrorV1, PlironTensorLayoutDataflowIssueV1, PlironTensorLayoutFindingV1,
    PlironTensorLayoutReportV1, PlironWorkgroupMemoryCheckErrorV1, PlironWorkgroupMemoryReportV1,
    RankedBoundsCheckErrorV1, RankedBoundsReportV1, RankedRaceCheckErrorV1, RankedRaceReportV1,
};

#[cfg(test)]
thread_local! {
    static PANIC_NEXT_PRODUCTION_ANALYSIS_V1: Cell<bool> = const { Cell::new(false) };
    static MUTATE_NEXT_PRODUCTION_ANALYSIS_V1: Cell<u8> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn panic_next_production_analysis_for_test_v1() {
    PANIC_NEXT_PRODUCTION_ANALYSIS_V1.with(|flag| flag.set(true));
}

#[cfg(test)]
pub(crate) fn transiently_mutate_next_production_analysis_for_test_v1() {
    MUTATE_NEXT_PRODUCTION_ANALYSIS_V1.with(|mode| mode.set(1));
}

#[cfg(test)]
pub(crate) fn structurally_mutate_next_production_analysis_for_test_v1() {
    MUTATE_NEXT_PRODUCTION_ANALYSIS_V1.with(|mode| mode.set(2));
}

#[cfg(test)]
fn maybe_panic_for_test_v1() {
    PANIC_NEXT_PRODUCTION_ANALYSIS_V1
        .with(|flag| assert!(!flag.replace(false), "injected production analysis panic"));
}

#[cfg(not(test))]
const fn maybe_panic_for_test_v1() {}

#[cfg(test)]
fn maybe_mutate_for_test_v1(context: &Context, function: &FuncOp) {
    use pliron::op::Op;

    MUTATE_NEXT_PRODUCTION_ANALYSIS_V1.with(|mode| match mode.replace(0) {
        0 => {}
        1 => {
            let operation = function.get_operation();
            let original = operation.deref(context).attributes.clone();
            operation.deref_mut(context).attributes = original.clone();
            operation.deref_mut(context).attributes = original;
        }
        2 => {
            function
                .get_operation()
                .deref_mut(context)
                .attributes
                .0
                .clear();
        }
        _ => unreachable!("test mutation mode is closed"),
    });
}

#[cfg(not(test))]
const fn maybe_mutate_for_test_v1(_context: &Context, _function: &FuncOp) {}

include!("pliron_pipeline/repairs_v1.rs");
/// Complete mandatory production sequence. This is one indivisible production
/// pipeline: no lowering may occur between its nine analysis stages.
pub const PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2: [KernelCheckPassKindV1; 9] = [
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[0].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[1].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[2].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[3].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[4].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[5].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[6].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[7].pass(),
    crate::PRODUCTION_PLIRON_PASS_CONTRACTS_V1[8].pass(),
];

/// Exact reports from one uninterrupted V2 production validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionPlironPreloweringReportV2 {
    target_contract: Option<PlironLaunchContractReportV1>,
    tensor_layout: PlironTensorLayoutReportV1,
    bounds: RankedBoundsReportV1,
    atomics: PlironAtomicLegalityReportV1,
    race: RankedRaceReportV1,
    ownership: HierarchicalOwnershipReportV1,
    barriers: PlironBarrierReportV1,
    pipeline_protocol: PlironPipelineProtocolReportV1,
    workgroup: PlironWorkgroupMemoryReportV1,
    semantics: PlironSemanticRefinementReportV1,
    preservation: PlironPassPreservationReportV1,
    report_validation: ProductionAnalysisReportValidationV1,
}

impl ProductionPlironPreloweringReportV2 {
    pub const fn pass_order(&self) -> &[KernelCheckPassKindV1; 9] {
        &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    }

    pub const fn tensor_layout(&self) -> &PlironTensorLayoutReportV1 {
        &self.tensor_layout
    }

    /// Target and host feasibility when the caller supplied compiler target
    /// inputs. The target-agnostic entry point leaves this absent and still
    /// grants no launch authority.
    pub const fn target_contract(&self) -> Option<&PlironLaunchContractReportV1> {
        self.target_contract.as_ref()
    }

    pub const fn bounds(&self) -> &RankedBoundsReportV1 {
        &self.bounds
    }

    pub const fn atomics(&self) -> &PlironAtomicLegalityReportV1 {
        &self.atomics
    }

    pub const fn race(&self) -> &RankedRaceReportV1 {
        &self.race
    }

    pub const fn ownership(&self) -> &HierarchicalOwnershipReportV1 {
        &self.ownership
    }

    pub const fn barriers(&self) -> &PlironBarrierReportV1 {
        &self.barriers
    }

    pub const fn pipeline_protocol(&self) -> &PlironPipelineProtocolReportV1 {
        &self.pipeline_protocol
    }

    pub const fn workgroup(&self) -> &PlironWorkgroupMemoryReportV1 {
        &self.workgroup
    }

    pub const fn semantics(&self) -> &PlironSemanticRefinementReportV1 {
        &self.semantics
    }

    /// Exact structural lineage around every analysis-only production pass.
    pub const fn preservation(&self) -> &PlironPassPreservationReportV1 {
        &self.preservation
    }

    /// Provenance/integrity result and the explicit independent-witness gaps
    /// for the nine reports. This is separate from [`Self::status`]: a clean
    /// policy result is useful diagnostics but is not proof authority.
    pub const fn report_validation(&self) -> &ProductionAnalysisReportValidationV1 {
        &self.report_validation
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.target_contract
            .as_ref()
            .map_or(
                KernelCheckStatusV1::Clean,
                PlironLaunchContractReportV1::status,
            )
            .join(
                self.tensor_layout
                    .status()
                    .join(self.bounds.status())
                    .join(self.atomics.status())
                    .join(self.race.status())
                    .join(self.ownership.status())
                    .join(self.barriers.status())
                    .join(self.pipeline_protocol.status())
                    .join(self.workgroup.status())
                    .join(self.semantics.status()),
            )
    }

    pub fn is_clean(&self) -> bool {
        self.status() == KernelCheckStatusV1::Clean
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionPlironPreloweringErrorV2 {
    ResourceLimit {
        phase: crate::ProductionAnalysisResourcePhaseV1,
        producing_pass: Option<KernelCheckPassKindV1>,
        resource: &'static str,
    },
    TargetContract(PlironLaunchContractCheckErrorV1),
    TensorLayout(PlironTensorLayoutCheckErrorV1),
    Bounds(RankedBoundsCheckErrorV1),
    Atomic(PlironAtomicLegalityCheckErrorV1),
    Race(RankedRaceCheckErrorV1),
    Ownership(HierarchicalOwnershipCheckErrorV1),
    Barrier(PlironBarrierCheckErrorV1),
    PipelineProtocol(PlironPipelineProtocolCheckErrorV1),
    Workgroup(PlironWorkgroupMemoryCheckErrorV1),
    Semantic(PlironSemanticRefinementCheckErrorV1),
    Preservation(PlironPassPreservationErrorV1),
    ReportValidation(ProductionAnalysisReportValidationErrorV1),
}

impl ProductionPlironPreloweringErrorV2 {
    /// Every error from the unified production pass pipeline has at least one
    /// actionable repair. Suggestions remain non-authoritative and are never
    /// applied silently.
    pub fn repair_hints(&self) -> Vec<KernelCheckRepairV1> {
        let repair = match self {
            Self::ResourceLimit { .. } => KernelCheckPassKindV1::Structural,
            Self::TargetContract(_) => return vec![launch_contract_repair_v1()],
            Self::TensorLayout(error) => return vec![tensor_layout_repair_for_error_v1(error)],
            Self::Bounds(_) => KernelCheckPassKindV1::MemoryBounds,
            Self::Atomic(_) => KernelCheckPassKindV1::AtomicLegality,
            Self::Race(_) => KernelCheckPassKindV1::RaceFreedom,
            Self::Ownership(_) => KernelCheckPassKindV1::HierarchicalOwnership,
            Self::Barrier(_) => KernelCheckPassKindV1::BarrierConvergence,
            Self::PipelineProtocol(_) => KernelCheckPassKindV1::PipelineProtocol,
            Self::Workgroup(_) => KernelCheckPassKindV1::WorkgroupMemory,
            Self::Semantic(_) => KernelCheckPassKindV1::SemanticRefinement,
            Self::Preservation(error) => {
                return vec![pass_preservation_repair_for_error_v1(error)];
            }
            Self::ReportValidation(error) => {
                return vec![report_validation_repair_for_error_v1(error)];
            }
        };
        vec![kernel_check_repair_for_pass_v1(repair)]
    }
}

impl fmt::Display for ProductionPlironPreloweringErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                phase,
                producing_pass,
                resource,
            } => {
                write!(
                    formatter,
                    "production PLIRON resource limit was exceeded [{}]: {resource}",
                    phase.code(),
                )?;
                if let Some(pass) = producing_pass {
                    write!(formatter, " (producing pass: {})", pass.name())?;
                }
                Ok(())
            }
            Self::TargetContract(error) => error.fmt(formatter),
            Self::TensorLayout(error) => error.fmt(formatter),
            Self::Bounds(error) => error.fmt(formatter),
            Self::Atomic(error) => error.fmt(formatter),
            Self::Race(error) => error.fmt(formatter),
            Self::Ownership(error) => error.fmt(formatter),
            Self::Barrier(error) => error.fmt(formatter),
            Self::PipelineProtocol(error) => error.fmt(formatter),
            Self::Workgroup(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
            Self::Preservation(error) => error.fmt(formatter),
            Self::ReportValidation(error) => error.fmt(formatter),
        }?;
        write_repairs(formatter, &self.repair_hints())
    }
}

impl Error for ProductionPlironPreloweringErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ResourceLimit { .. } => None,
            Self::TargetContract(error) => Some(error),
            Self::TensorLayout(error) => Some(error),
            Self::Bounds(error) => Some(error),
            Self::Atomic(error) => Some(error),
            Self::Race(error) => Some(error),
            Self::Ownership(error) => Some(error),
            Self::Barrier(error) => Some(error),
            Self::PipelineProtocol(error) => Some(error),
            Self::Workgroup(error) => Some(error),
            Self::Semantic(error) => Some(error),
            Self::Preservation(error) => Some(error),
            Self::ReportValidation(error) => Some(error),
        }
    }
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn require_production_pliron_checks_before_lowering_v2(
    context: &Context,
    function: &FuncOp,
) -> Result<ProductionPlironPreloweringReportV2, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(
        context,
        function,
        None,
        None,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .map(|outcome| outcome.report)
}

#[allow(clippy::result_large_err)]
pub(crate) fn require_production_pliron_checks_before_lowering_with_resource_limits_v1(
    context: &Context,
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionPlironPreloweringOutcomeV1, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(context, function, None, None, limits)
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn require_production_pliron_checks_with_atomic_target_before_lowering_v2(
    context: &Context,
    function: &FuncOp,
    atomic_target: &PlironAtomicTargetContextV1,
) -> Result<ProductionPlironPreloweringReportV2, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(
        context,
        function,
        Some(atomic_target),
        None,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .map(|outcome| outcome.report)
}

#[allow(clippy::result_large_err)]
pub(crate) fn require_production_pliron_checks_with_atomic_target_and_resource_limits_v1(
    context: &Context,
    function: &FuncOp,
    atomic_target: &PlironAtomicTargetContextV1,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionPlironPreloweringOutcomeV1, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(context, function, Some(atomic_target), None, limits)
}

/// Runs the same fixed nine-stage policy pipeline with compiler-supplied
/// target and host-allocation preconditions checked before those stages.
#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn require_production_pliron_checks_with_target_before_lowering_v2(
    context: &Context,
    function: &FuncOp,
    target_contract: &PlironLaunchContractV1,
) -> Result<ProductionPlironPreloweringReportV2, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(
        context,
        function,
        None,
        Some(target_contract),
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .map(|outcome| outcome.report)
}

#[allow(clippy::result_large_err)]
#[cfg(test)]
pub(crate) fn require_production_pliron_checks_with_atomic_and_target_before_lowering_v2(
    context: &Context,
    function: &FuncOp,
    atomic_target: &PlironAtomicTargetContextV1,
    target_contract: &PlironLaunchContractV1,
) -> Result<ProductionPlironPreloweringReportV2, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(
        context,
        function,
        Some(atomic_target),
        Some(target_contract),
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .map(|outcome| outcome.report)
}

struct ProductionAnalysisStageV1 {
    pass: KernelCheckPassKindV1,
    producing_phase: crate::production_analysis::ProductionAnalysisResourcePhaseV1,
    producing_phase_upper_bound: ProductionAnalysisResourceUpperBoundV1,
}

include!("pliron_pipeline/scoped_stage_v67.rs");
include!("pliron_pipeline/barrier_resources_v1.rs");

#[allow(clippy::result_large_err)]
fn run_and_record_production_analysis_invocation_v1<T, E>(
    context: &Context,
    function: &FuncOp,
    analyses: &mut PlironAnalysisManagerV1,
    preservation: &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
    validation: &mut ProductionAnalysisReportValidationSessionV1<'_>,
    stage: ProductionAnalysisStageV1,
    invoke: impl FnOnce(
        &mut PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'_>>,
        KernelCheckPassKindV1,
        crate::production_analysis::pliron_resource_envelope::ProductionAnalysisReplacementLimitsV1,
        &mut PlironAnalysisManagerV1,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1>,
) -> Result<Result<T, E>, ProductionPlironPreloweringErrorV2>
where
    T: SealedProductionAnalysisReportV1,
{
    let ProductionAnalysisStageV1 {
        pass,
        producing_phase,
        producing_phase_upper_bound,
    } = stage;
    analyses
        .admit_retained_resource_upper_bound(producing_phase, producing_phase_upper_bound)
        .map_err(|error| stage_resource_upper_bound_error_v1(error, pass))?;
    let replaced_identity_storage = preservation
        .lineage_identity_resource_upper_bound_v1()
        .retained_storage_upper_bound();
    let preservation_limits = analyses
        .remaining_identity_replacement_resource_limits_v1(replaced_identity_storage)
        .map_err(|error| stage_resource_upper_bound_error_v1(error, pass))?;
    let result = invoke(preservation, pass, preservation_limits, analyses)
        .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
    let checkpoint_upper_bound = preservation
        .last_checkpoint_resource_upper_bound_v1()
        .ok_or(ProductionPlironPreloweringErrorV2::ResourceLimit {
            phase: crate::ProductionAnalysisResourcePhaseV1::PassPreservation,
            producing_pass: Some(pass),
            resource: "pass checkpoint resource upper bound",
        })?;
    analyses
        .resource_contract_replace_retained_v1(
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::PassPreservation,
            replaced_identity_storage,
            checkpoint_upper_bound,
            preservation
                .lineage_identity_resource_upper_bound_v1()
                .retained_storage_upper_bound(),
        )
        .map_err(|error| stage_resource_upper_bound_error_v1(error, pass))?;
    if let Ok(report) = &result {
        let checkpoint = preservation
            .last_checkpoint()
            .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
        let validation_limits = analyses
            .remaining_resource_limits(
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::ReportValidation,
            )
            .map_err(|error| stage_resource_upper_bound_error_v1(error, pass))?;
        validation
            .record_with_resource_limits_v1(
                ProductionAnalysisReportEndpointV1 { context, function },
                checkpoint,
                report,
                producing_phase_upper_bound,
                validation_limits,
                analyses,
            )
            .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
        let validation_upper_bound = validation.last_stage_resource_upper_bound_v1().ok_or(
            ProductionPlironPreloweringErrorV2::ResourceLimit {
                phase: crate::ProductionAnalysisResourcePhaseV1::ReportValidation,
                producing_pass: Some(pass),
                resource: "report-validation resource upper bound",
            },
        )?;
        analyses
            .admit_retained_resource_upper_bound(
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::ReportValidation,
                validation_upper_bound,
            )
            .map_err(|error| stage_resource_upper_bound_error_v1(error, pass))?;
    }
    Ok(result)
}

include!("pliron_pipeline/ownership_resources_v1.rs");

fn compose_effect_refinement_resource_upper_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    effect_local: ProductionAnalysisResourceUpperBoundV1,
    ownership: ProductionAnalysisResourceUpperBoundV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    if census.effect_refinement_contracts == 0 {
        // The effect checker returns its allocation-free clean report before
        // invoking ownership when the authenticated inventory has no effect
        // contract. Do not reserve a nested execution that cannot occur.
        Ok(effect_local)
    } else {
        effect_local.checked_with_nested_sequence_discard(
            &[ownership],
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::EffectRefinement,
        )
    }
}

fn remaining_resource_limits_v1(
    analyses: &PlironAnalysisManagerV1,
    phase: crate::production_analysis::ProductionAnalysisResourcePhaseV1,
) -> Result<ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourceLimitV1> {
    analyses.remaining_resource_limits(phase)
}

include!("pliron_pipeline/cache_release_v1.rs");

fn resource_upper_bound_error_v1(
    error: crate::production_analysis::ProductionAnalysisResourceLimitV1,
) -> ProductionPlironPreloweringErrorV2 {
    ProductionPlironPreloweringErrorV2::ResourceLimit {
        phase: error.phase,
        producing_pass: None,
        resource: error.resource,
    }
}

fn stage_resource_upper_bound_error_v1(
    error: crate::production_analysis::ProductionAnalysisResourceLimitV1,
    pass: KernelCheckPassKindV1,
) -> ProductionPlironPreloweringErrorV2 {
    ProductionPlironPreloweringErrorV2::ResourceLimit {
        phase: error.phase,
        producing_pass: Some(pass),
        resource: error.resource,
    }
}

#[allow(clippy::result_large_err)]
fn require_production_pliron_checks_v2(
    context: &Context,
    function: &FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    target_contract: Option<&PlironLaunchContractV1>,
    resource_limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionPlironPreloweringOutcomeV1, ProductionPlironPreloweringErrorV2> {
    maybe_panic_for_test_v1();
    let provider = LivePlironStructuralIdentityProviderV1::new(context, function);
    let mut preservation = begin_production_pliron_pass_contract_session_with_resource_limits_v1(
        provider,
        resource_limits,
    )
    .map_err(|error| {
        if target_contract.is_some()
            && matches!(
                &error,
                PlironPassPreservationErrorV1::IdentityUnavailable { source_code, .. }
                    if *source_code == "FE2O3-PRESERVE-000"
            )
        {
            ProductionPlironPreloweringErrorV2::TargetContract(
                PlironLaunchContractCheckErrorV1::structural_prerequisite_v1(),
            )
        } else {
            ProductionPlironPreloweringErrorV2::Preservation(error)
        }
    })?;
    let input_census = preservation.input_census_v1();
    let mut analyses = PlironAnalysisManagerV1::new_with_resource_contract(
        function,
        input_census,
        preservation.initial_identity_resource_upper_bound_v1(),
        preservation
            .lineage_identity_resource_upper_bound_v1()
            .retained_storage_upper_bound(),
        resource_limits,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let target_contract_upper_bound = target_contract
        .map(|_| {
            preflight_launch_contract_resource_upper_bound_v1(
                input_census,
                remaining_resource_limits_v1(
                    &analyses,
                    crate::production_analysis::ProductionAnalysisResourcePhaseV1::LaunchContract,
                )
                .map_err(resource_upper_bound_error_v1)?,
            )
            .map_err(resource_upper_bound_error_v1)
        })
        .transpose()?;
    if let Some(upper_bound) = target_contract_upper_bound {
        retain_cache_resource_upper_bound_v1(
            &mut analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::LaunchContract,
            upper_bound,
        )
        .map_err(resource_upper_bound_error_v1)?;
    }
    let target_contract = target_contract
        .map(|target| {
            require_pliron_launch_contract_with_analyses_v1(
                context,
                function,
                target,
                &mut analyses,
            )
        })
        .transpose()
        .map_err(ProductionPlironPreloweringErrorV2::TargetContract)?;

    let sparse_upper_bound = preflight_sparse_index_resource_upper_bound_v1(
        context,
        function,
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::SparseIndex,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::SparseIndex,
        sparse_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    analyses.prepare_sparse_indices(context, function);

    let presburger_limits = remaining_resource_limits_v1(
        &analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::Presburger,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let presburger_upper_bound = preflight_presburger_resource_upper_bound_v1(
        analyses.sparse_indices().ok(),
        presburger_limits,
    )
    .map_err(resource_upper_bound_error_v1)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::Presburger,
        presburger_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    analyses.prepare_presburger(context, function);

    let execution_layout_upper_bound = preflight_execution_layout_resource_upper_bound_v1(
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::LaunchContract,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::LaunchContract,
        execution_layout_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    analyses.prepare_execution_layout(context, function);
    let execution_layout = analyses.execution_layout().map_err(|_| {
        ProductionPlironPreloweringErrorV2::ResourceLimit {
            phase: crate::ProductionAnalysisResourcePhaseV1::LaunchContract,
            producing_pass: None,
            resource: "execution-layout analysis",
        }
    })?;

    let trace_limits = remaining_resource_limits_v1(
        &analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::InvocationTrace,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let trace_preflight = preflight_invocation_trace_resource_upper_bound_v1(
        context,
        analyses.function_inventory().ok(),
        input_census,
        analyses.sparse_indices().ok(),
        execution_layout,
        trace_limits,
    )
    .map_err(resource_upper_bound_error_v1)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::InvocationTrace,
        trace_preflight.attempt_upper_bound(),
    )
    .map_err(resource_upper_bound_error_v1)?;
    analyses.prepare_exact_trace(context, function);
    let trace_admission = trace_preflight
        .exact_admission(analyses.exact_trace())
        .map_err(resource_upper_bound_error_v1)?;

    let report_validation_limits = remaining_resource_limits_v1(
        &analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::ReportValidation,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let mut report_validation =
        begin_production_analysis_report_validation_with_resource_limits_v1(
            context,
            function,
            atomic_target,
            preservation.validation_handle(),
            input_census,
            report_validation_limits,
        )
        .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::ReportValidation,
        report_validation.setup_resource_upper_bound_v1(),
    )
    .map_err(resource_upper_bound_error_v1)?;
    let tensor_layout_upper_bound = preflight_tensor_layout_resource_upper_bound_v1(
        input_census,
        trace_admission,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::TensorLayout,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let tensor_layout = run_and_record_production_analysis_stage_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::TensorLayout,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::TensorLayout,
            producing_phase_upper_bound: tensor_layout_upper_bound,
        },
        |analyses| {
            let report = require_pliron_tensor_layout_with_analyses_v1(context, function, analyses);
            maybe_mutate_for_test_v1(context, function);
            report
        },
    )?
    .map_err(ProductionPlironPreloweringErrorV2::TensorLayout)?;
    let ranked_bounds_upper_bound = preflight_ranked_bounds_resource_upper_bound_v1(
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::MemoryBounds,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let bounds = run_and_record_production_analysis_stage_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::MemoryBounds,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::MemoryBounds,
            producing_phase_upper_bound: ranked_bounds_upper_bound,
        },
        |analyses| require_pliron_ranked_bounds_with_analyses_v1(context, function, analyses),
    )?
    .map_err(ProductionPlironPreloweringErrorV2::Bounds)?;
    let atomic_upper_bound = preflight_atomic_legality_resource_upper_bound_v1(
        input_census,
        atomic_target,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::AtomicLegality,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let atomics = run_and_record_production_analysis_stage_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::AtomicLegality,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::AtomicLegality,
            producing_phase_upper_bound: atomic_upper_bound,
        },
        |analyses| {
            require_pliron_atomic_legality_with_analyses_v1(
                context,
                function,
                atomic_target,
                analyses,
            )
        },
    )?
    .map_err(ProductionPlironPreloweringErrorV2::Atomic)?;
    let provenance_upper_bound = preflight_provenance_alias_resource_upper_bound_v1(
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::ProvenanceAlias,
        provenance_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    analyses.prepare_provenance_alias(context, function);
    let sparse = analyses.sparse_indices().map_err(|_| {
        ProductionPlironPreloweringErrorV2::ResourceLimit {
            phase: crate::ProductionAnalysisResourcePhaseV1::SparseIndex,
            producing_pass: None,
            resource: "sparse-index analysis",
        }
    })?;
    let race_upper_bound = preflight_race_resource_upper_bound_v1(
        context,
        function,
        analyses.function_inventory().ok(),
        input_census,
        sparse,
        execution_layout,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::RaceFreedom,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let race = run_and_record_production_analysis_stage_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::RaceFreedom,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::RaceFreedom,
            producing_phase_upper_bound: race_upper_bound,
        },
        |analyses| require_pliron_ranked_race_freedom_with_analyses_v1(context, function, analyses),
    )?
    .map_err(ProductionPlironPreloweringErrorV2::Race)?;
    let ownership_local_upper_bound = preflight_hierarchical_ownership_resource_upper_bound_v1(
        input_census,
        trace_admission,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let ownership_upper_bound = compose_hierarchical_ownership_resource_upper_bound_v1(
        input_census,
        ownership_local_upper_bound,
        ranked_bounds_upper_bound,
        race_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let ownership = run_and_record_production_analysis_stage_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::HierarchicalOwnership,
            producing_phase: crate::production_analysis::ProductionAnalysisResourcePhaseV1::HierarchicalOwnership,
            producing_phase_upper_bound: ownership_upper_bound,
        },
        |analyses| {
            require_pliron_hierarchical_ownership_with_analyses_v1(context, function, analyses)
        },
    )?
    .map_err(ProductionPlironPreloweringErrorV2::Ownership)?;
    let simt_protocol_upper_bound = preflight_simt_protocol_resource_upper_bound_v1(
        trace_admission,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::SimtProtocol,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::SimtProtocol,
        simt_protocol_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    analyses.prepare_simt_protocol(context, function);
    let pipeline_protocol_upper_bound = preflight_pipeline_protocol_resource_upper_bound_v1(
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::PipelineProtocol,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let (barrier_upper_bound, barrier_progress_needed) = preflight_barrier_stage_v1(
        context,
        &mut analyses,
        &input_census,
        trace_admission,
        pipeline_protocol_upper_bound,
    )?;
    let barriers = run_and_record_scoped_barrier_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        barrier_upper_bound,
        barrier_progress_needed,
    )?
    .map_err(ProductionPlironPreloweringErrorV2::Barrier)?;
    let pipeline_protocol = run_and_record_production_analysis_stage_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::PipelineProtocol,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::PipelineProtocol,
            producing_phase_upper_bound: pipeline_protocol_upper_bound,
        },
        |analyses| require_pliron_pipeline_protocol_with_analyses_v1(context, function, analyses),
    )?
    .map_err(ProductionPlironPreloweringErrorV2::PipelineProtocol)?;
    let memory_order_admission = preflight_memory_order_attempt_resource_upper_bound_v1(
        input_census,
        trace_admission,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::MemoryOrder,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    retain_cache_resource_upper_bound_v1(
        &mut analyses,
        crate::production_analysis::ProductionAnalysisResourcePhaseV1::MemoryOrder,
        memory_order_admission.upper_bound(),
    )
    .map_err(resource_upper_bound_error_v1)?;
    analyses.prepare_memory_order(context, function);
    let workgroup_local_upper_bound = preflight_prepared_workgroup_memory_resource_upper_bound_v1(
        input_census,
        memory_order_admission,
        analyses
            .memory_order()
            .map(|memory_order| memory_order.issues()),
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let workgroup_upper_bound = workgroup_local_upper_bound
        .checked_with_nested_sequence_discard(
            &[pipeline_protocol_upper_bound],
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
        )
        .map_err(resource_upper_bound_error_v1)?;
    let workgroup = run_and_record_production_analysis_stage_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        ProductionAnalysisStageV1 {
            pass: KernelCheckPassKindV1::WorkgroupMemory,
            producing_phase:
                crate::production_analysis::ProductionAnalysisResourcePhaseV1::WorkgroupMemory,
            producing_phase_upper_bound: workgroup_upper_bound,
        },
        |analyses| require_pliron_workgroup_memory_with_analyses_v1(context, function, analyses),
    )?
    .map_err(ProductionPlironPreloweringErrorV2::Workgroup)?;
    let effect_local_upper_bound = preflight_effect_refinement_resource_upper_bound_v1(
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::EffectRefinement,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let effect_upper_bound = compose_effect_refinement_resource_upper_bound_v1(
        input_census,
        effect_local_upper_bound,
        ownership_upper_bound,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let semantic_local_upper_bound = preflight_semantic_refinement_resource_upper_bound_v1(
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::SemanticRefinement,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let progress_upper_bound = preflight_scoped_progress_resource_upper_bound_v1(
        input_census,
        remaining_resource_limits_v1(
            &analyses,
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::Progress,
        )
        .map_err(resource_upper_bound_error_v1)?,
    )
    .map_err(resource_upper_bound_error_v1)?;
    let semantic_upper_bound = semantic_local_upper_bound
        .checked_with_nested_sequence_retain(
            &[progress_upper_bound, effect_upper_bound],
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::SemanticRefinement,
        )
        .map_err(resource_upper_bound_error_v1)?;
    let semantics = run_and_record_scoped_semantic_refinement_v1(
        context,
        function,
        &mut analyses,
        &mut preservation,
        &mut report_validation,
        semantic_upper_bound,
    )?
    .map_err(|error| ProductionPlironPreloweringErrorV2::Semantic(*error))?;
    let replaced_identity_storage = preservation
        .lineage_identity_resource_upper_bound_v1()
        .retained_storage_upper_bound();
    let preservation_limits = analyses
        .remaining_identity_replacement_resource_limits_v1(replaced_identity_storage)
        .map_err(resource_upper_bound_error_v1)?;
    let preservation = preservation
        .finish_with_resource_limits_v1(preservation_limits.output)
        .map_err(ProductionPlironPreloweringErrorV2::Preservation)?;
    analyses
        .resource_contract_replace_retained_v1(
            crate::production_analysis::ProductionAnalysisResourcePhaseV1::PassPreservation,
            replaced_identity_storage,
            preservation.resource_upper_bound_v1(),
            preservation.output_identity().canonical_len(),
        )
        .map_err(resource_upper_bound_error_v1)?;
    let report_validation = report_validation
        .finish_validation(&preservation)
        .map_err(ProductionPlironPreloweringErrorV2::ReportValidation)?;
    let report = ProductionPlironPreloweringReportV2 {
        target_contract,
        tensor_layout,
        bounds,
        atomics,
        race,
        ownership,
        barriers,
        pipeline_protocol,
        workgroup,
        semantics,
        preservation,
        report_validation,
    };
    let resource_upper_bound = finish_exclusive_analysis_caches_v1(
        analyses,
        ExclusiveAnalysisCacheReservationsV1 {
            sparse: sparse_upper_bound,
            presburger: presburger_upper_bound,
            execution_layout: execution_layout_upper_bound,
            invocation_trace: trace_preflight.attempt_upper_bound(),
            provenance: provenance_upper_bound,
            simt: simt_protocol_upper_bound,
            memory_order: memory_order_admission.upper_bound(),
        },
    )
    .map_err(resource_upper_bound_error_v1)?;
    Ok(ProductionPlironPreloweringOutcomeV1 {
        report,
        resource_upper_bound,
    })
}

include!("pliron_pipeline/resource_tests.rs");
include!("pliron_pipeline/progress_scoped_pipeline_v67_tests.rs");
