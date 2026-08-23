//! One fixed production-oriented verifier sequence for ranked PLIRON kernels.

use std::{error::Error, fmt};

use pliron::{builtin::ops::FuncOp, context::Context};

use crate::{
    KernelCheckPassKindV1, PlironAtomicLegalityCheckErrorV1, PlironAtomicLegalityReportV1,
    PlironAtomicTargetContextV1, PlironBarrierCheckErrorV1, PlironBarrierReportV1,
    PlironSemanticRefinementCheckErrorV1, PlironSemanticRefinementReportV1,
    PlironTensorLayoutCheckErrorV1, PlironTensorLayoutReportV1, PlironWorkgroupMemoryCheckErrorV1,
    PlironWorkgroupMemoryReportV1, RankedBoundsCheckErrorV1, RankedBoundsReportV1,
    RankedRaceCheckErrorV1, RankedRaceReportV1, require_pliron_atomic_legality_before_lowering_v1,
    require_pliron_atomic_legality_with_target_before_lowering_v1,
    require_pliron_barrier_convergence_after_bounds_v1,
    require_pliron_ranked_bounds_before_lowering_v1,
    require_pliron_ranked_race_freedom_after_bounds_v1,
    require_pliron_semantic_refinement_after_bounds_v1,
    require_pliron_tensor_layout_before_lowering_v1,
    require_pliron_workgroup_memory_after_prerequisites_v1,
};

/// Complete mandatory production sequence. Tensor layout precedes the frozen
/// six-report V1 general analysis record and no lowering may occur between them.
pub const PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1: [KernelCheckPassKindV1; 7] = [
    KernelCheckPassKindV1::TensorLayout,
    KernelCheckPassKindV1::MemoryBounds,
    KernelCheckPassKindV1::AtomicLegality,
    KernelCheckPassKindV1::RaceFreedom,
    KernelCheckPassKindV1::BarrierConvergence,
    KernelCheckPassKindV1::WorkgroupMemory,
    KernelCheckPassKindV1::SemanticRefinement,
];

pub const GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1: [KernelCheckPassKindV1; 6] = [
    KernelCheckPassKindV1::MemoryBounds,
    KernelCheckPassKindV1::AtomicLegality,
    KernelCheckPassKindV1::RaceFreedom,
    KernelCheckPassKindV1::BarrierConvergence,
    KernelCheckPassKindV1::WorkgroupMemory,
    KernelCheckPassKindV1::SemanticRefinement,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionPlironPreloweringReportV1 {
    tensor_layout: PlironTensorLayoutReportV1,
    general: GeneralPlironKernelCheckReportV1,
}

impl ProductionPlironPreloweringReportV1 {
    pub const fn pass_order(&self) -> &[KernelCheckPassKindV1; 7] {
        &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V1
    }

    pub const fn tensor_layout(&self) -> &PlironTensorLayoutReportV1 {
        &self.tensor_layout
    }

    pub const fn general(&self) -> &GeneralPlironKernelCheckReportV1 {
        &self.general
    }

    pub fn is_clean(&self) -> bool {
        self.tensor_layout.is_clean() && self.general.is_clean()
    }

    pub fn into_parts(self) -> (PlironTensorLayoutReportV1, GeneralPlironKernelCheckReportV1) {
        (self.tensor_layout, self.general)
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
    General(GeneralPlironKernelCheckErrorV1),
}

impl fmt::Display for ProductionPlironPreloweringErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TensorLayout(error) => error.fmt(formatter),
            Self::General(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionPlironPreloweringErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TensorLayout(error) => Some(error),
            Self::General(error) => Some(error),
        }
    }
}

pub fn require_production_pliron_checks_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<ProductionPlironPreloweringReportV1, ProductionPlironPreloweringErrorV1> {
    let tensor_layout = require_pliron_tensor_layout_before_lowering_v1(context, function)
        .map_err(ProductionPlironPreloweringErrorV1::TensorLayout)?;
    let general = require_general_pliron_kernel_checks_before_lowering_v1(context, function)
        .map_err(ProductionPlironPreloweringErrorV1::General)?;
    Ok(ProductionPlironPreloweringReportV1 {
        tensor_layout,
        general,
    })
}

pub fn require_production_pliron_checks_with_atomic_target_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
    atomic_target: &PlironAtomicTargetContextV1,
) -> Result<ProductionPlironPreloweringReportV1, ProductionPlironPreloweringErrorV1> {
    let tensor_layout = require_pliron_tensor_layout_before_lowering_v1(context, function)
        .map_err(ProductionPlironPreloweringErrorV1::TensorLayout)?;
    let general = require_general_pliron_kernel_checks_with_atomic_target_before_lowering_v1(
        context,
        function,
        atomic_target,
    )
    .map_err(ProductionPlironPreloweringErrorV1::General)?;
    Ok(ProductionPlironPreloweringReportV1 {
        tensor_layout,
        general,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralPlironKernelCheckReportV1 {
    bounds: RankedBoundsReportV1,
    atomics: PlironAtomicLegalityReportV1,
    race: RankedRaceReportV1,
    barriers: PlironBarrierReportV1,
    workgroup: PlironWorkgroupMemoryReportV1,
    semantics: PlironSemanticRefinementReportV1,
}

impl GeneralPlironKernelCheckReportV1 {
    pub const fn pass_order(&self) -> &[KernelCheckPassKindV1; 6] {
        &GENERAL_PLIRON_KERNEL_CHECK_PASS_ORDER_V1
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

    pub fn into_parts(
        self,
    ) -> (
        RankedBoundsReportV1,
        PlironAtomicLegalityReportV1,
        RankedRaceReportV1,
        PlironBarrierReportV1,
        PlironWorkgroupMemoryReportV1,
        PlironSemanticRefinementReportV1,
    ) {
        (
            self.bounds,
            self.atomics,
            self.race,
            self.barriers,
            self.workgroup,
            self.semantics,
        )
    }

    pub fn is_clean(&self) -> bool {
        self.bounds.is_clean()
            && self.atomics.is_clean()
            && self.race.is_clean()
            && self.barriers.is_clean()
            && self.workgroup.is_clean()
            && self.semantics.is_clean()
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GeneralPlironKernelCheckErrorV1 {
    Bounds(RankedBoundsCheckErrorV1),
    Atomic(PlironAtomicLegalityCheckErrorV1),
    Race(RankedRaceCheckErrorV1),
    Barrier(PlironBarrierCheckErrorV1),
    Workgroup(PlironWorkgroupMemoryCheckErrorV1),
    Semantic(PlironSemanticRefinementCheckErrorV1),
}

impl fmt::Display for GeneralPlironKernelCheckErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bounds(error) => error.fmt(formatter),
            Self::Atomic(error) => error.fmt(formatter),
            Self::Race(error) => error.fmt(formatter),
            Self::Barrier(error) => error.fmt(formatter),
            Self::Workgroup(error) => error.fmt(formatter),
            Self::Semantic(error) => error.fmt(formatter),
        }
    }
}

impl Error for GeneralPlironKernelCheckErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Bounds(error) => Some(error),
            Self::Atomic(error) => Some(error),
            Self::Race(error) => Some(error),
            Self::Barrier(error) => Some(error),
            Self::Workgroup(error) => Some(error),
            Self::Semantic(error) => Some(error),
        }
    }
}

pub fn require_general_pliron_kernel_checks_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<GeneralPlironKernelCheckReportV1, GeneralPlironKernelCheckErrorV1> {
    require_general_pliron_kernel_checks(context, function, None)
}

pub fn require_general_pliron_kernel_checks_with_atomic_target_before_lowering_v1(
    context: &Context,
    function: &FuncOp,
    atomic_target: &PlironAtomicTargetContextV1,
) -> Result<GeneralPlironKernelCheckReportV1, GeneralPlironKernelCheckErrorV1> {
    require_general_pliron_kernel_checks(context, function, Some(atomic_target))
}

fn require_general_pliron_kernel_checks(
    context: &Context,
    function: &FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
) -> Result<GeneralPlironKernelCheckReportV1, GeneralPlironKernelCheckErrorV1> {
    let bounds = require_pliron_ranked_bounds_before_lowering_v1(context, function)
        .map_err(GeneralPlironKernelCheckErrorV1::Bounds)?;
    let atomics = match atomic_target {
        Some(target) => {
            require_pliron_atomic_legality_with_target_before_lowering_v1(context, function, target)
        }
        None => require_pliron_atomic_legality_before_lowering_v1(context, function),
    }
    .map_err(GeneralPlironKernelCheckErrorV1::Atomic)?;
    let race = require_pliron_ranked_race_freedom_after_bounds_v1(context, function)
        .map_err(GeneralPlironKernelCheckErrorV1::Race)?;
    let barriers = require_pliron_barrier_convergence_after_bounds_v1(context, function)
        .map_err(GeneralPlironKernelCheckErrorV1::Barrier)?;
    let workgroup = require_pliron_workgroup_memory_after_prerequisites_v1(context, function)
        .map_err(GeneralPlironKernelCheckErrorV1::Workgroup)?;
    let semantics = require_pliron_semantic_refinement_after_bounds_v1(context, function)
        .map_err(GeneralPlironKernelCheckErrorV1::Semantic)?;
    Ok(GeneralPlironKernelCheckReportV1 {
        bounds,
        atomics,
        race,
        barriers,
        workgroup,
        semantics,
    })
}
