//! One fixed production-oriented verifier sequence for ranked PLIRON kernels.

use std::{error::Error, fmt};

use pliron::{builtin::ops::FuncOp, context::Context};

use crate::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::pliron_barrier::require_pliron_barrier_convergence_with_analyses_v1;
use crate::pliron_hierarchical_ownership::require_pliron_hierarchical_ownership_with_analyses_v1;
use crate::pliron_race::require_pliron_ranked_race_freedom_with_analyses_v1;
use crate::pliron_ranked_bounds::require_pliron_ranked_bounds_with_analyses_v1;
use crate::pliron_semantic_refinement::require_pliron_semantic_refinement_with_analyses_v1;
use crate::pliron_tensor_layout::require_pliron_tensor_layout_with_analyses_v1;
use crate::pliron_workgroup_memory::require_pliron_workgroup_memory_with_analyses_v1;
use crate::{
    HierarchicalOwnershipCheckErrorV1, HierarchicalOwnershipReportV1, KernelCheckPassKindV1,
    KernelCheckStatusV1, PlironAtomicLegalityCheckErrorV1, PlironAtomicLegalityReportV1,
    PlironAtomicTargetContextV1, PlironBarrierCheckErrorV1, PlironBarrierReportV1,
    PlironSemanticRefinementCheckErrorV1, PlironSemanticRefinementReportV1,
    PlironTensorLayoutCheckErrorV1, PlironTensorLayoutReportV1, PlironWorkgroupMemoryCheckErrorV1,
    PlironWorkgroupMemoryReportV1, RankedBoundsCheckErrorV1, RankedBoundsReportV1,
    RankedRaceCheckErrorV1, RankedRaceReportV1, require_pliron_atomic_legality_before_lowering_v1,
    require_pliron_atomic_legality_with_target_before_lowering_v1,
};

/// Complete mandatory production sequence. This is one indivisible production
/// pipeline: no lowering may occur between its passes.
pub const PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1: [KernelCheckPassKindV1; 7] = [
    KernelCheckPassKindV1::TensorLayout,
    KernelCheckPassKindV1::MemoryBounds,
    KernelCheckPassKindV1::AtomicLegality,
    KernelCheckPassKindV1::RaceFreedom,
    KernelCheckPassKindV1::BarrierConvergence,
    KernelCheckPassKindV1::WorkgroupMemory,
    KernelCheckPassKindV1::SemanticRefinement,
];

/// Exact reports produced by one uninterrupted execution of the mandatory
/// seven-pass production pipeline.
///
/// The fields are private and there is no constructor from individual reports,
/// so a consumer cannot manufacture this lineage by skipping tensor layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionPlironPreloweringReportV1 {
    tensor_layout: PlironTensorLayoutReportV1,
    bounds: RankedBoundsReportV1,
    atomics: PlironAtomicLegalityReportV1,
    race: RankedRaceReportV1,
    barriers: PlironBarrierReportV1,
    workgroup: PlironWorkgroupMemoryReportV1,
    semantics: PlironSemanticRefinementReportV1,
}

impl ProductionPlironPreloweringReportV1 {
    pub const fn pass_order(&self) -> &[KernelCheckPassKindV1; 7] {
        &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1
    }

    pub const fn tensor_layout(&self) -> &PlironTensorLayoutReportV1 {
        &self.tensor_layout
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

    pub const fn barriers(&self) -> &PlironBarrierReportV1 {
        &self.barriers
    }

    pub const fn workgroup(&self) -> &PlironWorkgroupMemoryReportV1 {
        &self.workgroup
    }

    pub const fn semantics(&self) -> &PlironSemanticRefinementReportV1 {
        &self.semantics
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.tensor_layout
            .status()
            .join(self.bounds.status())
            .join(self.atomics.status())
            .join(self.race.status())
            .join(self.barriers.status())
            .join(self.workgroup.status())
            .join(self.semantics.status())
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
pub enum ProductionPlironPreloweringErrorV1 {
    TensorLayout(PlironTensorLayoutCheckErrorV1),
    Bounds(RankedBoundsCheckErrorV1),
    Atomic(PlironAtomicLegalityCheckErrorV1),
    Race(RankedRaceCheckErrorV1),
    Barrier(PlironBarrierCheckErrorV1),
    Workgroup(PlironWorkgroupMemoryCheckErrorV1),
    Semantic(PlironSemanticRefinementCheckErrorV1),
}

impl fmt::Display for ProductionPlironPreloweringErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TensorLayout(error) => error.fmt(formatter),
            Self::Bounds(error) => error.fmt(formatter),
            Self::Atomic(error) => error.fmt(formatter),
            Self::Race(error) => error.fmt(formatter),
            Self::Barrier(error) => error.fmt(formatter),
            Self::Workgroup(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionPlironPreloweringErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TensorLayout(error) => Some(error),
            Self::Bounds(error) => Some(error),
            Self::Atomic(error) => Some(error),
            Self::Race(error) => Some(error),
            Self::Barrier(error) => Some(error),
            Self::Workgroup(error) => Some(error),
            Self::Semantic(error) => Some(error),
        }
    }
}

pub fn require_production_pliron_checks_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<ProductionPlironPreloweringReportV1, ProductionPlironPreloweringErrorV1> {
    require_production_pliron_checks(context, function, None)
}

pub fn require_production_pliron_checks_with_atomic_target_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
    atomic_target: &PlironAtomicTargetContextV1,
) -> Result<ProductionPlironPreloweringReportV1, ProductionPlironPreloweringErrorV1> {
    require_production_pliron_checks(context, function, Some(atomic_target))
}

fn require_production_pliron_checks(
    context: &Context,
    function: &FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
) -> Result<ProductionPlironPreloweringReportV1, ProductionPlironPreloweringErrorV1> {
    // A fresh manager is part of the transaction boundary. A later pipeline
    // invocation, including post-lowering revalidation, necessarily recomputes
    // facts from that invocation's immutable IR.
    let mut analyses = PlironAnalysisManagerV1::new(function);
    require_production_pliron_checks_with_analyses(context, function, atomic_target, &mut analyses)
}

fn require_production_pliron_checks_with_analyses(
    context: &Context,
    function: &FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    analyses: &mut PlironAnalysisManagerV1,
) -> Result<ProductionPlironPreloweringReportV1, ProductionPlironPreloweringErrorV1> {
    let tensor_layout = require_pliron_tensor_layout_with_analyses_v1(context, function, analyses)
        .map_err(ProductionPlironPreloweringErrorV1::TensorLayout)?;
    let bounds = require_pliron_ranked_bounds_with_analyses_v1(context, function, analyses)
        .map_err(ProductionPlironPreloweringErrorV1::Bounds)?;
    let atomics = match atomic_target {
        Some(target) => {
            require_pliron_atomic_legality_with_target_before_lowering_v1(context, function, target)
        }
        None => require_pliron_atomic_legality_before_lowering_v1(context, function),
    }
    .map_err(ProductionPlironPreloweringErrorV1::Atomic)?;
    let race = require_pliron_ranked_race_freedom_with_analyses_v1(context, function, analyses)
        .map_err(ProductionPlironPreloweringErrorV1::Race)?;
    let barriers = require_pliron_barrier_convergence_with_analyses_v1(context, function, analyses)
        .map_err(ProductionPlironPreloweringErrorV1::Barrier)?;
    let workgroup = require_pliron_workgroup_memory_with_analyses_v1(context, function, analyses)
        .map_err(ProductionPlironPreloweringErrorV1::Workgroup)?;
    let semantics =
        require_pliron_semantic_refinement_with_analyses_v1(context, function, analyses)
            .map_err(ProductionPlironPreloweringErrorV1::Semantic)?;
    Ok(ProductionPlironPreloweringReportV1 {
        tensor_layout,
        bounds,
        atomics,
        race,
        barriers,
        workgroup,
        semantics,
    })
}

/// Production sequence with hierarchy-level ownership reconstruction.
///
/// V1 remains frozen because its seven reports are encoded by the existing V4
/// middle-end evidence schema. V2 adds ownership as an explicit stage instead
/// of changing the meaning of those historical bytes.
pub const PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2: [KernelCheckPassKindV1; 8] = [
    KernelCheckPassKindV1::TensorLayout,
    KernelCheckPassKindV1::MemoryBounds,
    KernelCheckPassKindV1::AtomicLegality,
    KernelCheckPassKindV1::RaceFreedom,
    KernelCheckPassKindV1::HierarchicalOwnership,
    KernelCheckPassKindV1::BarrierConvergence,
    KernelCheckPassKindV1::WorkgroupMemory,
    KernelCheckPassKindV1::SemanticRefinement,
];

/// Exact reports from one uninterrupted V2 production validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionPlironPreloweringReportV2 {
    tensor_layout: PlironTensorLayoutReportV1,
    bounds: RankedBoundsReportV1,
    atomics: PlironAtomicLegalityReportV1,
    race: RankedRaceReportV1,
    ownership: HierarchicalOwnershipReportV1,
    barriers: PlironBarrierReportV1,
    workgroup: PlironWorkgroupMemoryReportV1,
    semantics: PlironSemanticRefinementReportV1,
}

impl ProductionPlironPreloweringReportV2 {
    pub const fn pass_order(&self) -> &[KernelCheckPassKindV1; 8] {
        &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
    }

    pub const fn tensor_layout(&self) -> &PlironTensorLayoutReportV1 {
        &self.tensor_layout
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

    pub const fn workgroup(&self) -> &PlironWorkgroupMemoryReportV1 {
        &self.workgroup
    }

    pub const fn semantics(&self) -> &PlironSemanticRefinementReportV1 {
        &self.semantics
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.tensor_layout
            .status()
            .join(self.bounds.status())
            .join(self.atomics.status())
            .join(self.race.status())
            .join(self.ownership.status())
            .join(self.barriers.status())
            .join(self.workgroup.status())
            .join(self.semantics.status())
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
    TensorLayout(PlironTensorLayoutCheckErrorV1),
    Bounds(RankedBoundsCheckErrorV1),
    Atomic(PlironAtomicLegalityCheckErrorV1),
    Race(RankedRaceCheckErrorV1),
    Ownership(HierarchicalOwnershipCheckErrorV1),
    Barrier(PlironBarrierCheckErrorV1),
    Workgroup(PlironWorkgroupMemoryCheckErrorV1),
    Semantic(PlironSemanticRefinementCheckErrorV1),
}

impl fmt::Display for ProductionPlironPreloweringErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TensorLayout(error) => error.fmt(formatter),
            Self::Bounds(error) => error.fmt(formatter),
            Self::Atomic(error) => error.fmt(formatter),
            Self::Race(error) => error.fmt(formatter),
            Self::Ownership(error) => error.fmt(formatter),
            Self::Barrier(error) => error.fmt(formatter),
            Self::Workgroup(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionPlironPreloweringErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TensorLayout(error) => Some(error),
            Self::Bounds(error) => Some(error),
            Self::Atomic(error) => Some(error),
            Self::Race(error) => Some(error),
            Self::Ownership(error) => Some(error),
            Self::Barrier(error) => Some(error),
            Self::Workgroup(error) => Some(error),
            Self::Semantic(error) => Some(error),
        }
    }
}

pub fn require_production_pliron_checks_before_lowering_v2(
    context: &Context,
    function: &FuncOp,
) -> Result<ProductionPlironPreloweringReportV2, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(context, function, None)
}

pub fn require_production_pliron_checks_with_atomic_target_before_lowering_v2(
    context: &Context,
    function: &FuncOp,
    atomic_target: &PlironAtomicTargetContextV1,
) -> Result<ProductionPlironPreloweringReportV2, ProductionPlironPreloweringErrorV2> {
    require_production_pliron_checks_v2(context, function, Some(atomic_target))
}

fn require_production_pliron_checks_v2(
    context: &Context,
    function: &FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
) -> Result<ProductionPlironPreloweringReportV2, ProductionPlironPreloweringErrorV2> {
    let mut analyses = PlironAnalysisManagerV1::new(function);
    let tensor_layout =
        require_pliron_tensor_layout_with_analyses_v1(context, function, &mut analyses)
            .map_err(ProductionPlironPreloweringErrorV2::TensorLayout)?;
    let bounds = require_pliron_ranked_bounds_with_analyses_v1(context, function, &mut analyses)
        .map_err(ProductionPlironPreloweringErrorV2::Bounds)?;
    let atomics = match atomic_target {
        Some(target) => {
            require_pliron_atomic_legality_with_target_before_lowering_v1(context, function, target)
        }
        None => require_pliron_atomic_legality_before_lowering_v1(context, function),
    }
    .map_err(ProductionPlironPreloweringErrorV2::Atomic)?;
    let race =
        require_pliron_ranked_race_freedom_with_analyses_v1(context, function, &mut analyses)
            .map_err(ProductionPlironPreloweringErrorV2::Race)?;
    let ownership =
        require_pliron_hierarchical_ownership_with_analyses_v1(context, function, &mut analyses)
            .map_err(ProductionPlironPreloweringErrorV2::Ownership)?;
    let barriers =
        require_pliron_barrier_convergence_with_analyses_v1(context, function, &mut analyses)
            .map_err(ProductionPlironPreloweringErrorV2::Barrier)?;
    let workgroup =
        require_pliron_workgroup_memory_with_analyses_v1(context, function, &mut analyses)
            .map_err(ProductionPlironPreloweringErrorV2::Workgroup)?;
    let semantics =
        require_pliron_semantic_refinement_with_analyses_v1(context, function, &mut analyses)
            .map_err(ProductionPlironPreloweringErrorV2::Semantic)?;
    Ok(ProductionPlironPreloweringReportV2 {
        tensor_layout,
        bounds,
        atomics,
        race,
        ownership,
        barriers,
        workgroup,
        semantics,
    })
}

#[cfg(test)]
mod tests {
    use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
    use dialect_kernel::{
        DIALECT_NAME, ReturnOp, TensorConvergenceAttr, TensorLayoutOp, register_dialect,
    };
    use fe2o3_kernel_ir::TensorLayoutContractV1;
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
    };

    use super::*;
    use crate::pliron_analysis_manager::PlironAnalysisComputationCountsV1;

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(
            &mut context,
            &DialectName::try_new(DIALECT_NAME).expect("valid kernel dialect name"),
        )
        .expect("register kernel dialect");
        dialect_gpu::register_dialect(&mut context).expect("register gpu dialect");
        context
    }

    fn valid_tensor_function(context: &mut Context, name: &str) -> (FuncOp, ReturnOp) {
        let function = FuncOp::new(
            context,
            name.try_into().expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        );
        let entry = function.get_entry_block(context);
        let layout = ExecutionLayoutOp::new_with_domain(
            context,
            7,
            [64, 1, 1],
            [64, 1, 1],
            64,
            ExecutionDomainAttr::FullPhysicalWorkgroups,
        );
        let tensor = TensorLayoutOp::new(
            context,
            &TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            TensorConvergenceAttr::UniformSubgroup,
            64,
        );
        let ret = ReturnOp::new(context);
        for operation in [
            layout.get_operation(),
            tensor.get_operation(),
            ret.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        (function, ret)
    }

    #[test]
    fn production_run_reuses_each_analysis_root_exactly_once() {
        let context = &mut setup();
        let (function, _) = valid_tensor_function(context, "analysis_reuse");
        let mut analyses = PlironAnalysisManagerV1::new(&function);

        let report =
            require_production_pliron_checks_with_analyses(context, &function, None, &mut analyses)
                .expect("valid tensor function passes the production pipeline");

        assert!(report.is_clean());
        assert_eq!(report.status(), KernelCheckStatusV1::Clean);
        assert_eq!(
            analyses.computation_counts(),
            PlironAnalysisComputationCountsV1 {
                sparse_indices: 1,
                execution_layout: 1,
                exact_trace: 1,
            }
        );
        assert_eq!(
            analyses.cached_entries(),
            super::super::pliron_analysis_manager::MAX_PLIRON_ANALYSIS_CACHE_SLOTS_V1
        );
    }

    #[test]
    fn revalidation_uses_a_fresh_manager_after_ir_changes() {
        let context = &mut setup();
        let (function, ret) = valid_tensor_function(context, "fresh_revalidation");
        let mut first = PlironAnalysisManagerV1::new(&function);
        require_production_pliron_checks_with_analyses(context, &function, None, &mut first)
            .expect("initial IR passes");
        assert_eq!(first.computation_counts().execution_layout, 1);
        drop(first);

        let conflicting_layout = ExecutionLayoutOp::new_with_domain(
            context,
            7,
            [128, 1, 1],
            [64, 1, 1],
            64,
            ExecutionDomainAttr::FullPhysicalWorkgroups,
        );
        conflicting_layout
            .get_operation()
            .insert_before(context, ret.get_operation());

        let mut revalidation = PlironAnalysisManagerV1::new(&function);
        let error = require_production_pliron_checks_with_analyses(
            context,
            &function,
            None,
            &mut revalidation,
        )
        .expect_err("fresh analysis rejects conflicting execution layouts");

        assert!(matches!(
            error,
            ProductionPlironPreloweringErrorV1::TensorLayout(_)
        ));
        assert_eq!(revalidation.computation_counts().execution_layout, 1);
    }
}
