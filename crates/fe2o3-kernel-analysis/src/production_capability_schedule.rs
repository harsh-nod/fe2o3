//! Frozen ordering and dependency inventory for production capability checks.
//!
//! There is one production schedule. Each executor stage owns one or more
//! logical obligations, and W4 retains independently checked evidence for
//! every obligation after executing this exact schedule.

use crate::KernelCheckPassKindV1;

pub const PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_VERSION_V1: u16 = 1;
pub const PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1: usize = 19;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionCapabilityAnalysisKindV1 {
    CanonicalTyping,
    CapabilityProvenance,
    ResourceLegality,
    Uniformity,
    TensorLayout,
    MemoryBounds,
    AtomicLegality,
    RaceFreedom,
    HierarchicalOwnership,
    BarrierConvergence,
    PipelineProtocol,
    Initialization,
    MemoryVisibility,
    WorkgroupMemoryEpochs,
    EffectRefinement,
    SemanticRefinement,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionW4AnalysisObligationKindV1 {
    CanonicalTyping,
    CapabilityProvenance,
    ResourceLegality,
    Uniformity,
    TensorLayout,
    MemoryBounds,
    AtomicLegality,
    HappensBefore,
    RaceFreedom,
    HierarchicalOwnership,
    BarrierConvergence,
    BarrierOrder,
    PipelineProtocol,
    Initialization,
    MemoryVisibility,
    WorkgroupMemoryEpochs,
    CollectiveParticipation,
    EffectRefinement,
    SemanticRefinement,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionW4AnalysisObligationStageV1 {
    kind: ProductionW4AnalysisObligationKindV1,
    dependencies: &'static [ProductionW4AnalysisObligationKindV1],
    owner: ProductionCapabilityAnalysisKindV1,
}

impl ProductionW4AnalysisObligationStageV1 {
    const fn new(
        kind: ProductionW4AnalysisObligationKindV1,
        dependencies: &'static [ProductionW4AnalysisObligationKindV1],
        owner: ProductionCapabilityAnalysisKindV1,
    ) -> Self {
        Self {
            kind,
            dependencies,
            owner,
        }
    }

    pub const fn kind(self) -> ProductionW4AnalysisObligationKindV1 {
        self.kind
    }
    pub const fn dependencies(self) -> &'static [ProductionW4AnalysisObligationKindV1] {
        self.dependencies
    }
    pub const fn owner(self) -> ProductionCapabilityAnalysisKindV1 {
        self.owner
    }
    pub const fn grants_evidence_or_compiler_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionCapabilityAnalysisExecutorV1 {
    CanonicalKirVerifierV13,
    KernelCapabilityAffectedAnalysisV1,
    MandatoryPlironPass(KernelCheckPassKindV1),
    EmbeddedInPlironPass(KernelCheckPassKindV1),
    MandatoryTargetCapabilityClosureV1,
}

impl ProductionCapabilityAnalysisExecutorV1 {
    /// Compatibility query for the frozen manifest. This is scheduling policy,
    /// never evidence that an executor ran.
    pub const fn is_mandatory_final_graph_gate(self) -> bool {
        matches!(
            self,
            Self::CanonicalKirVerifierV13
                | Self::KernelCapabilityAffectedAnalysisV1
                | Self::MandatoryPlironPass(_)
                | Self::EmbeddedInPlironPass(_)
                | Self::MandatoryTargetCapabilityClosureV1
        )
    }
    pub const fn grants_evidence_authority(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionCapabilityAnalysisStageV1 {
    kind: ProductionCapabilityAnalysisKindV1,
    dependencies: &'static [ProductionCapabilityAnalysisKindV1],
    executor: ProductionCapabilityAnalysisExecutorV1,
    obligations: &'static [ProductionW4AnalysisObligationStageV1],
}

impl ProductionCapabilityAnalysisStageV1 {
    const fn new(
        kind: ProductionCapabilityAnalysisKindV1,
        dependencies: &'static [ProductionCapabilityAnalysisKindV1],
        executor: ProductionCapabilityAnalysisExecutorV1,
        obligations: &'static [ProductionW4AnalysisObligationStageV1],
    ) -> Self {
        Self {
            kind,
            dependencies,
            executor,
            obligations,
        }
    }
    pub const fn kind(self) -> ProductionCapabilityAnalysisKindV1 {
        self.kind
    }
    pub const fn dependencies(self) -> &'static [ProductionCapabilityAnalysisKindV1] {
        self.dependencies
    }
    pub const fn executor(self) -> ProductionCapabilityAnalysisExecutorV1 {
        self.executor
    }
    pub const fn obligations(self) -> &'static [ProductionW4AnalysisObligationStageV1] {
        self.obligations
    }
    pub const fn grants_compiler_refinement_authority(self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(self) -> bool {
        false
    }
}

use KernelCheckPassKindV1 as Pass;
use ProductionCapabilityAnalysisExecutorV1::{
    CanonicalKirVerifierV13, EmbeddedInPlironPass, KernelCapabilityAffectedAnalysisV1,
    MandatoryPlironPass, MandatoryTargetCapabilityClosureV1,
};
use ProductionCapabilityAnalysisKindV1::{
    AtomicLegality, BarrierConvergence, CanonicalTyping, CapabilityProvenance, EffectRefinement,
    HierarchicalOwnership, Initialization, MemoryBounds, MemoryVisibility, PipelineProtocol,
    RaceFreedom, ResourceLegality, SemanticRefinement, TensorLayout, Uniformity,
    WorkgroupMemoryEpochs,
};
use ProductionW4AnalysisObligationKindV1 as Obligation;

const CANONICAL: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::CanonicalTyping,
        &[],
        CanonicalTyping,
    )];
const PROVENANCE: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::CapabilityProvenance,
        &[Obligation::CanonicalTyping],
        CapabilityProvenance,
    )];
const RESOURCES: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::ResourceLegality,
        &[
            Obligation::CanonicalTyping,
            Obligation::CapabilityProvenance,
        ],
        ResourceLegality,
    )];
const UNIFORM: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::Uniformity,
        &[
            Obligation::CanonicalTyping,
            Obligation::CapabilityProvenance,
        ],
        Uniformity,
    )];
const LAYOUT: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::TensorLayout,
        &[Obligation::CanonicalTyping, Obligation::Uniformity],
        TensorLayout,
    )];
const BOUNDS: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::MemoryBounds,
        &[Obligation::CanonicalTyping, Obligation::TensorLayout],
        MemoryBounds,
    )];
const ATOMICS: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::AtomicLegality,
        &[
            Obligation::CapabilityProvenance,
            Obligation::ResourceLegality,
            Obligation::MemoryBounds,
        ],
        AtomicLegality,
    )];
const RACES: &[ProductionW4AnalysisObligationStageV1] = &[
    ProductionW4AnalysisObligationStageV1::new(
        Obligation::HappensBefore,
        &[
            Obligation::Uniformity,
            Obligation::MemoryBounds,
            Obligation::AtomicLegality,
        ],
        RaceFreedom,
    ),
    ProductionW4AnalysisObligationStageV1::new(
        Obligation::RaceFreedom,
        &[
            Obligation::MemoryBounds,
            Obligation::AtomicLegality,
            Obligation::HappensBefore,
        ],
        RaceFreedom,
    ),
];
const OWNERSHIP: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::HierarchicalOwnership,
        &[Obligation::MemoryBounds, Obligation::RaceFreedom],
        HierarchicalOwnership,
    )];
const BARRIERS: &[ProductionW4AnalysisObligationStageV1] = &[
    ProductionW4AnalysisObligationStageV1::new(
        Obligation::BarrierConvergence,
        &[Obligation::Uniformity, Obligation::MemoryBounds],
        BarrierConvergence,
    ),
    ProductionW4AnalysisObligationStageV1::new(
        Obligation::BarrierOrder,
        &[Obligation::BarrierConvergence],
        BarrierConvergence,
    ),
    ProductionW4AnalysisObligationStageV1::new(
        Obligation::CollectiveParticipation,
        &[
            Obligation::Uniformity,
            Obligation::HierarchicalOwnership,
            Obligation::BarrierConvergence,
            Obligation::BarrierOrder,
        ],
        BarrierConvergence,
    ),
];
const PIPELINES: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::PipelineProtocol,
        &[Obligation::BarrierOrder],
        PipelineProtocol,
    )];
const INITIALIZED: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::Initialization,
        &[Obligation::MemoryBounds, Obligation::PipelineProtocol],
        Initialization,
    )];
const VISIBLE: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::MemoryVisibility,
        &[
            Obligation::AtomicLegality,
            Obligation::HappensBefore,
            Obligation::BarrierOrder,
        ],
        MemoryVisibility,
    )];
const EPOCHS: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::WorkgroupMemoryEpochs,
        &[
            Obligation::Initialization,
            Obligation::MemoryVisibility,
            Obligation::PipelineProtocol,
        ],
        WorkgroupMemoryEpochs,
    )];
const EFFECTS: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::EffectRefinement,
        &[
            Obligation::RaceFreedom,
            Obligation::HierarchicalOwnership,
            Obligation::MemoryVisibility,
            Obligation::CollectiveParticipation,
        ],
        EffectRefinement,
    )];
const SEMANTICS: &[ProductionW4AnalysisObligationStageV1] =
    &[ProductionW4AnalysisObligationStageV1::new(
        Obligation::SemanticRefinement,
        &[Obligation::EffectRefinement],
        SemanticRefinement,
    )];

/// The one production schedule. Mandatory PLIRON stages retain the exact
/// existing nine-pass order; obligation order is its flattened nested order.
pub const PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1: [ProductionCapabilityAnalysisStageV1; 16] = [
    ProductionCapabilityAnalysisStageV1::new(
        CanonicalTyping,
        &[],
        CanonicalKirVerifierV13,
        CANONICAL,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        CapabilityProvenance,
        &[CanonicalTyping],
        KernelCapabilityAffectedAnalysisV1,
        PROVENANCE,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        ResourceLegality,
        &[CanonicalTyping, CapabilityProvenance],
        MandatoryTargetCapabilityClosureV1,
        RESOURCES,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        Uniformity,
        &[CanonicalTyping, CapabilityProvenance],
        EmbeddedInPlironPass(Pass::BarrierConvergence),
        UNIFORM,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        TensorLayout,
        &[CanonicalTyping, Uniformity],
        MandatoryPlironPass(Pass::TensorLayout),
        LAYOUT,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        MemoryBounds,
        &[CanonicalTyping, TensorLayout],
        MandatoryPlironPass(Pass::MemoryBounds),
        BOUNDS,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        AtomicLegality,
        &[CanonicalTyping, CapabilityProvenance],
        MandatoryPlironPass(Pass::AtomicLegality),
        ATOMICS,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        RaceFreedom,
        &[MemoryBounds, AtomicLegality],
        MandatoryPlironPass(Pass::RaceFreedom),
        RACES,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        HierarchicalOwnership,
        &[MemoryBounds],
        MandatoryPlironPass(Pass::HierarchicalOwnership),
        OWNERSHIP,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        BarrierConvergence,
        &[Uniformity, MemoryBounds],
        MandatoryPlironPass(Pass::BarrierConvergence),
        BARRIERS,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        PipelineProtocol,
        &[BarrierConvergence],
        MandatoryPlironPass(Pass::PipelineProtocol),
        PIPELINES,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        Initialization,
        &[MemoryBounds, PipelineProtocol],
        EmbeddedInPlironPass(Pass::WorkgroupMemory),
        INITIALIZED,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        MemoryVisibility,
        &[AtomicLegality, BarrierConvergence],
        EmbeddedInPlironPass(Pass::WorkgroupMemory),
        VISIBLE,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        WorkgroupMemoryEpochs,
        &[Initialization, MemoryVisibility, PipelineProtocol],
        MandatoryPlironPass(Pass::WorkgroupMemory),
        EPOCHS,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        EffectRefinement,
        &[RaceFreedom, HierarchicalOwnership],
        EmbeddedInPlironPass(Pass::SemanticRefinement),
        EFFECTS,
    ),
    ProductionCapabilityAnalysisStageV1::new(
        SemanticRefinement,
        &[EffectRefinement],
        MandatoryPlironPass(Pass::SemanticRefinement),
        SEMANTICS,
    ),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionW4AnalysisScheduleErrorV1 {
    Length { expected: usize, observed: usize },
    Dependency { position: usize },
    Stage { position: usize },
    ObligationCount { expected: usize, observed: usize },
    ObligationDependency { position: usize },
    ObligationOwner { position: usize },
}

/// Validates executor and obligation metadata in the one production schedule.
/// Success does not imply that any named check ran.
pub fn validate_production_w4_analysis_obligation_schedule_v1(
    schedule: &[ProductionCapabilityAnalysisStageV1],
) -> Result<(), ProductionW4AnalysisScheduleErrorV1> {
    if schedule.len() != PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len() {
        return Err(ProductionW4AnalysisScheduleErrorV1::Length {
            expected: PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len(),
            observed: schedule.len(),
        });
    }
    for (position, (observed, expected)) in schedule
        .iter()
        .zip(PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1)
        .enumerate()
    {
        if observed != &expected {
            return Err(ProductionW4AnalysisScheduleErrorV1::Stage { position });
        }
        if observed.dependencies().iter().any(|dependency| {
            !schedule[..position]
                .iter()
                .any(|prior| prior.kind() == *dependency)
        }) {
            return Err(ProductionW4AnalysisScheduleErrorV1::Dependency { position });
        }
    }

    let obligations = schedule
        .iter()
        .flat_map(|stage| {
            stage
                .obligations()
                .iter()
                .map(move |obligation| (stage, obligation))
        })
        .collect::<Vec<_>>();
    if obligations.len() != PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1 {
        return Err(ProductionW4AnalysisScheduleErrorV1::ObligationCount {
            expected: PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1,
            observed: obligations.len(),
        });
    }
    for (position, (stage, obligation)) in obligations.iter().enumerate() {
        if obligation.owner() != stage.kind() {
            return Err(ProductionW4AnalysisScheduleErrorV1::ObligationOwner { position });
        }
        if obligation.dependencies().iter().any(|dependency| {
            !obligations[..position]
                .iter()
                .any(|(_, prior)| prior.kind() == *dependency)
        }) {
            return Err(ProductionW4AnalysisScheduleErrorV1::ObligationDependency { position });
        }
    }
    Ok(())
}

pub const fn production_capability_schedule_grants_compiler_refinement_authority_v1() -> bool {
    false
}

pub const fn production_capability_schedule_grants_artifact_or_launch_authority_v1() -> bool {
    false
}
