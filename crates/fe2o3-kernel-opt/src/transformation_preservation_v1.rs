//! Fail-closed preservation accounting for production Kernel IR transforms.
//!
//! This module does not create a shadow optimizer IR and does not grant
//! semantic-equivalence authority. It checks each mutation against the exact
//! canonical V13 graph, invalidates every affected pre-transform analysis, and
//! records the fresh analyses required before a later production admission may
//! rely on the transformed graph.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_kernel_analysis::{
    KernelCapabilityPreservationErrorV1, KernelCapabilityPreservationReplayV1,
    analyze_kernel_capability_preservation_v1,
};
use fe2o3_kernel_ir::{
    KernelIrDecodeError, Module, VerifiedCanonicalKernelIrErrorV13,
    VerifiedCanonicalKernelIrIdentityV13, VerifiedCanonicalKernelIrV13, decode_module_v13,
};
use fe2o3_pliron::{
    ContextBuildError, KirBridgeErrorV1, NameError, PlironOptimizationErrorV1,
    PlironOptimizationPassV1, PlironOptimizationPlanErrorV1, PlironOptimizationPlanV1,
    PlironSession,
};

use crate::{
    CheckedOptimizerQueryKindV1, CheckedOptimizerQuerySessionV1, CheckedOptimizerQueryV1,
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2, LoopMemoryEditV1,
    LoopMemoryTransformErrorV1, LoopMemoryTransformLimitsV1, LoopMemoryTransformReportV1,
    LoopMemoryTransformV1, OptimizerQueryDecisionV1, OptimizerQueryFailureV1,
    OptimizerQueryLimitsV1, TargetNeutralModuleTransformErrorV1,
    TargetNeutralModuleTransformLimitsV1, TargetNeutralModuleTransformReportV1,
    TargetNeutralModuleTransformV1, check_loop_memory_transform_relation_v1,
    check_target_neutral_module_transform_relation_v1, execute_loop_memory_transform_v1,
    execute_target_neutral_module_transform_v1, production_kernel_ir_pliron_optimization_limits_v2,
};

/// Closed audit roster for optimization families named by the production-SSA
/// architecture. An entry is executable only when it maps to a shipped Pliron
/// pass and every pass boundary receives a checked preservation record.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProductionTransformationV1 {
    SparseConditionalConstantPropagation,
    SimplifyControlFlow,
    SelectSameValueCanonicalization,
    DeadCodeElimination,
    LocalPureCommonSubexpressionElimination,
    GeneralTargetIndependentCanonicalization,
    GlobalValueNumbering,
    ScalarReplacementOfAggregates,
    SecondarySsaPromotion,
    HelperInlining,
    InterproceduralCleanup,
    LoopCanonicalization,
    InductionVariableSimplification,
    LoopInvariantCodeMotion,
    FullLoopUnrolling,
    PartialLoopUnrolling,
    MemoryEffectVersioning,
    MemorySimplification,
    InstructionScheduling,
}

impl ProductionTransformationV1 {
    pub const fn name(self) -> &'static str {
        match self {
            Self::SparseConditionalConstantPropagation => "sparse-conditional-constant-propagation",
            Self::SimplifyControlFlow => "simplify-control-flow",
            Self::SelectSameValueCanonicalization => "select-same-value-canonicalization",
            Self::DeadCodeElimination => "dead-code-elimination",
            Self::LocalPureCommonSubexpressionElimination => {
                "local-pure-common-subexpression-elimination"
            }
            Self::GeneralTargetIndependentCanonicalization => {
                "general-target-independent-canonicalization"
            }
            Self::GlobalValueNumbering => "global-value-numbering",
            Self::ScalarReplacementOfAggregates => "scalar-replacement-of-aggregates",
            Self::SecondarySsaPromotion => "secondary-ssa-promotion",
            Self::HelperInlining => "helper-inlining",
            Self::InterproceduralCleanup => "interprocedural-cleanup",
            Self::LoopCanonicalization => "loop-canonicalization",
            Self::InductionVariableSimplification => "induction-variable-simplification",
            Self::LoopInvariantCodeMotion => "loop-invariant-code-motion",
            Self::FullLoopUnrolling => "full-loop-unrolling",
            Self::PartialLoopUnrolling => "partial-loop-unrolling",
            Self::MemoryEffectVersioning => "memory-effect-versioning",
            Self::MemorySimplification => "memory-simplification",
            Self::InstructionScheduling => "instruction-scheduling",
        }
    }

    pub const fn from_pliron(pass: PlironOptimizationPassV1) -> Self {
        match pass {
            PlironOptimizationPassV1::SparseConditionalConstantPropagation => {
                Self::SparseConditionalConstantPropagation
            }
            PlironOptimizationPassV1::SimplifyControlFlow => Self::SimplifyControlFlow,
            PlironOptimizationPassV1::SelectSameValueCanonicalization => {
                Self::SelectSameValueCanonicalization
            }
            PlironOptimizationPassV1::DeadCodeElimination => Self::DeadCodeElimination,
            PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination => {
                Self::LocalPureCommonSubexpressionElimination
            }
            PlironOptimizationPassV1::GeneralTargetIndependentCanonicalization => {
                Self::GeneralTargetIndependentCanonicalization
            }
            PlironOptimizationPassV1::GlobalValueNumbering => Self::GlobalValueNumbering,
        }
    }

    pub const fn pliron(self) -> Option<PlironOptimizationPassV1> {
        match self {
            Self::SparseConditionalConstantPropagation => {
                Some(PlironOptimizationPassV1::SparseConditionalConstantPropagation)
            }
            Self::SimplifyControlFlow => Some(PlironOptimizationPassV1::SimplifyControlFlow),
            Self::SelectSameValueCanonicalization => {
                Some(PlironOptimizationPassV1::SelectSameValueCanonicalization)
            }
            Self::DeadCodeElimination => Some(PlironOptimizationPassV1::DeadCodeElimination),
            Self::LocalPureCommonSubexpressionElimination => {
                Some(PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination)
            }
            Self::GeneralTargetIndependentCanonicalization => {
                Some(PlironOptimizationPassV1::GeneralTargetIndependentCanonicalization)
            }
            Self::GlobalValueNumbering => Some(PlironOptimizationPassV1::GlobalValueNumbering),
            Self::ScalarReplacementOfAggregates
            | Self::SecondarySsaPromotion
            | Self::HelperInlining
            | Self::InterproceduralCleanup
            | Self::LoopCanonicalization
            | Self::InductionVariableSimplification
            | Self::LoopInvariantCodeMotion
            | Self::FullLoopUnrolling
            | Self::PartialLoopUnrolling
            | Self::MemoryEffectVersioning
            | Self::MemorySimplification
            | Self::InstructionScheduling => None,
        }
    }

    pub const fn module_transform(self) -> Option<TargetNeutralModuleTransformV1> {
        match self {
            Self::ScalarReplacementOfAggregates => {
                Some(TargetNeutralModuleTransformV1::ScalarReplacementOfAggregates)
            }
            Self::SecondarySsaPromotion => {
                Some(TargetNeutralModuleTransformV1::SecondarySsaPromotion)
            }
            Self::HelperInlining => Some(TargetNeutralModuleTransformV1::HelperInlining),
            Self::InterproceduralCleanup => {
                Some(TargetNeutralModuleTransformV1::InterproceduralCleanup)
            }
            _ => None,
        }
    }

    pub const fn loop_memory_transform(self) -> Option<LoopMemoryTransformV1> {
        match self {
            Self::LoopCanonicalization => Some(LoopMemoryTransformV1::LoopCanonicalization),
            Self::InductionVariableSimplification => {
                Some(LoopMemoryTransformV1::InductionVariableSimplification)
            }
            Self::LoopInvariantCodeMotion => Some(LoopMemoryTransformV1::LoopInvariantCodeMotion),
            Self::FullLoopUnrolling => Some(LoopMemoryTransformV1::FullLoopUnrolling),
            Self::PartialLoopUnrolling => Some(LoopMemoryTransformV1::PartialLoopUnrolling),
            Self::MemoryEffectVersioning => Some(LoopMemoryTransformV1::MemoryEffectVersioning),
            Self::MemorySimplification => Some(LoopMemoryTransformV1::MemorySimplification),
            _ => None,
        }
    }
}

/// Whether this checkout has one executable production path for a transform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionTransformationDispositionV1 {
    CheckedCanonicalV13Path,
    UnavailableFailClosed,
}

/// Mutation policy for operations, types, or interfaces carrying execution,
/// memory, synchronization, target, or source identity contracts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionSensitiveMutationPolicyV1 {
    ExactProtectedStructureAndAffectedAnalysisReplay,
    TransformUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionTransformationAuditEntryV1 {
    transformation: ProductionTransformationV1,
    disposition: ProductionTransformationDispositionV1,
    execution_sensitive_mutation: ExecutionSensitiveMutationPolicyV1,
}

impl ProductionTransformationAuditEntryV1 {
    pub const fn transformation(self) -> ProductionTransformationV1 {
        self.transformation
    }

    pub const fn disposition(self) -> ProductionTransformationDispositionV1 {
        self.disposition
    }

    pub const fn execution_sensitive_mutation(self) -> ExecutionSensitiveMutationPolicyV1 {
        self.execution_sensitive_mutation
    }
}

const fn checked(
    transformation: ProductionTransformationV1,
) -> ProductionTransformationAuditEntryV1 {
    ProductionTransformationAuditEntryV1 {
        transformation,
        disposition: ProductionTransformationDispositionV1::CheckedCanonicalV13Path,
        execution_sensitive_mutation:
            ExecutionSensitiveMutationPolicyV1::ExactProtectedStructureAndAffectedAnalysisReplay,
    }
}

const fn unavailable(
    transformation: ProductionTransformationV1,
) -> ProductionTransformationAuditEntryV1 {
    ProductionTransformationAuditEntryV1 {
        transformation,
        disposition: ProductionTransformationDispositionV1::UnavailableFailClosed,
        execution_sensitive_mutation: ExecutionSensitiveMutationPolicyV1::TransformUnavailable,
    }
}

pub const PRODUCTION_TRANSFORMATION_AUDIT_V1: [ProductionTransformationAuditEntryV1; 19] = [
    checked(ProductionTransformationV1::SparseConditionalConstantPropagation),
    checked(ProductionTransformationV1::SimplifyControlFlow),
    checked(ProductionTransformationV1::SelectSameValueCanonicalization),
    checked(ProductionTransformationV1::DeadCodeElimination),
    checked(ProductionTransformationV1::LocalPureCommonSubexpressionElimination),
    checked(ProductionTransformationV1::GeneralTargetIndependentCanonicalization),
    checked(ProductionTransformationV1::GlobalValueNumbering),
    checked(ProductionTransformationV1::ScalarReplacementOfAggregates),
    checked(ProductionTransformationV1::SecondarySsaPromotion),
    checked(ProductionTransformationV1::HelperInlining),
    checked(ProductionTransformationV1::InterproceduralCleanup),
    checked(ProductionTransformationV1::LoopCanonicalization),
    checked(ProductionTransformationV1::InductionVariableSimplification),
    checked(ProductionTransformationV1::LoopInvariantCodeMotion),
    checked(ProductionTransformationV1::FullLoopUnrolling),
    checked(ProductionTransformationV1::PartialLoopUnrolling),
    checked(ProductionTransformationV1::MemoryEffectVersioning),
    checked(ProductionTransformationV1::MemorySimplification),
    unavailable(ProductionTransformationV1::InstructionScheduling),
];

pub fn production_transformation_audit_v1() -> &'static [ProductionTransformationAuditEntryV1] {
    &PRODUCTION_TRANSFORMATION_AUDIT_V1
}

const fn disposition(
    transformation: ProductionTransformationV1,
) -> ProductionTransformationDispositionV1 {
    match transformation {
        ProductionTransformationV1::SparseConditionalConstantPropagation
        | ProductionTransformationV1::SimplifyControlFlow
        | ProductionTransformationV1::SelectSameValueCanonicalization
        | ProductionTransformationV1::DeadCodeElimination
        | ProductionTransformationV1::LocalPureCommonSubexpressionElimination
        | ProductionTransformationV1::GeneralTargetIndependentCanonicalization
        | ProductionTransformationV1::GlobalValueNumbering
        | ProductionTransformationV1::ScalarReplacementOfAggregates
        | ProductionTransformationV1::SecondarySsaPromotion
        | ProductionTransformationV1::HelperInlining
        | ProductionTransformationV1::InterproceduralCleanup
        | ProductionTransformationV1::LoopCanonicalization
        | ProductionTransformationV1::InductionVariableSimplification
        | ProductionTransformationV1::LoopInvariantCodeMotion
        | ProductionTransformationV1::FullLoopUnrolling
        | ProductionTransformationV1::PartialLoopUnrolling
        | ProductionTransformationV1::MemoryEffectVersioning
        | ProductionTransformationV1::MemorySimplification => {
            ProductionTransformationDispositionV1::CheckedCanonicalV13Path
        }
        ProductionTransformationV1::InstructionScheduling => {
            ProductionTransformationDispositionV1::UnavailableFailClosed
        }
    }
}

/// The transformation families selected by the actual frozen production
/// policy. This is derived from the policy roster rather than maintained as a
/// second hand-written schedule.
pub fn production_policy_transformations_v1() -> Vec<ProductionTransformationV1> {
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
        .into_iter()
        .map(|pass| ProductionTransformationV1::from_pliron(pass.pliron()))
        .collect()
}

/// A fail-closed invariant checked before any production optimizer execution.
pub fn production_policy_has_only_checked_transformations_v1() -> bool {
    production_policy_transformations_v1()
        .into_iter()
        .all(|transformation| {
            disposition(transformation)
                == ProductionTransformationDispositionV1::CheckedCanonicalV13Path
                && transformation.pliron().is_some()
        })
}

/// Analyses invalidated by any canonical graph mutation. The list is
/// deliberately conservative: a pre-transform result is never carried over
/// based on a pass-reported boolean.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OptimizerAnalysisV1 {
    CanonicalKirVerification,
    CapabilityPreservation,
    DefUseAndLiveness,
    Reachability,
    Dominance,
    PostDominance,
    LoopForest,
    CallGraphAndAbi,
    ConstantsAndRanges,
    PresburgerFacts,
    UniformityAndConvergence,
    ProvenanceAliasAndEffects,
    MemoryInitializationAndVersions,
    RankedShapeExtentAndStride,
    OwnershipBoundsAndRace,
    BarrierCollectiveAndPipeline,
    TargetRequirementsAndResources,
    SourceOperationRefinement,
    NumericalSemantics,
    ArtifactAndLaunch,
}

const INVALIDATED_BY_MUTATION: &[OptimizerAnalysisV1] = &[
    OptimizerAnalysisV1::CanonicalKirVerification,
    OptimizerAnalysisV1::CapabilityPreservation,
    OptimizerAnalysisV1::DefUseAndLiveness,
    OptimizerAnalysisV1::Reachability,
    OptimizerAnalysisV1::Dominance,
    OptimizerAnalysisV1::PostDominance,
    OptimizerAnalysisV1::LoopForest,
    OptimizerAnalysisV1::CallGraphAndAbi,
    OptimizerAnalysisV1::ConstantsAndRanges,
    OptimizerAnalysisV1::PresburgerFacts,
    OptimizerAnalysisV1::UniformityAndConvergence,
    OptimizerAnalysisV1::ProvenanceAliasAndEffects,
    OptimizerAnalysisV1::MemoryInitializationAndVersions,
    OptimizerAnalysisV1::RankedShapeExtentAndStride,
    OptimizerAnalysisV1::OwnershipBoundsAndRace,
    OptimizerAnalysisV1::BarrierCollectiveAndPipeline,
    OptimizerAnalysisV1::TargetRequirementsAndResources,
    OptimizerAnalysisV1::SourceOperationRefinement,
    OptimizerAnalysisV1::NumericalSemantics,
    OptimizerAnalysisV1::ArtifactAndLaunch,
];

const REPLAYED_AT_PASS_BOUNDARY: &[OptimizerAnalysisV1] = &[
    OptimizerAnalysisV1::CanonicalKirVerification,
    OptimizerAnalysisV1::CapabilityPreservation,
];

const REQUIRED_BEFORE_SEMANTIC_ADMISSION: &[OptimizerAnalysisV1] = &[
    OptimizerAnalysisV1::DefUseAndLiveness,
    OptimizerAnalysisV1::Reachability,
    OptimizerAnalysisV1::Dominance,
    OptimizerAnalysisV1::PostDominance,
    OptimizerAnalysisV1::LoopForest,
    OptimizerAnalysisV1::CallGraphAndAbi,
    OptimizerAnalysisV1::ConstantsAndRanges,
    OptimizerAnalysisV1::PresburgerFacts,
    OptimizerAnalysisV1::UniformityAndConvergence,
    OptimizerAnalysisV1::ProvenanceAliasAndEffects,
    OptimizerAnalysisV1::MemoryInitializationAndVersions,
    OptimizerAnalysisV1::RankedShapeExtentAndStride,
    OptimizerAnalysisV1::OwnershipBoundsAndRace,
    OptimizerAnalysisV1::BarrierCollectiveAndPipeline,
    OptimizerAnalysisV1::TargetRequirementsAndResources,
    OptimizerAnalysisV1::SourceOperationRefinement,
    OptimizerAnalysisV1::NumericalSemantics,
    OptimizerAnalysisV1::ArtifactAndLaunch,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformationPreservationModeV1 {
    ExactCanonicalIdentity,
    ExactProtectedStructureReplay,
}

/// Evidence that the output capability analysis was recomputed from the
/// post-transform canonical graph at its output epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FreshOutputAnalysisBindingV1 {
    graph_identity: VerifiedCanonicalKernelIrIdentityV13,
    graph_epoch: u64,
}

impl FreshOutputAnalysisBindingV1 {
    pub const fn graph_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.graph_identity
    }

    pub const fn graph_epoch(self) -> u64 {
        self.graph_epoch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedModuleTransformRelationV1 {
    TargetNeutral(TargetNeutralModuleTransformReportV1),
    LoopMemory(Box<LoopMemoryTransformReportV1>),
}

/// Inert pass-boundary receipt. Consumers must execute every listed deferred
/// replay before granting semantic, target, artifact, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use = "a preservation record is not semantic-equivalence authority"]
pub struct CheckedTransformationPreservationRecordV1 {
    transformation: ProductionTransformationV1,
    mode: TransformationPreservationModeV1,
    input_identity: VerifiedCanonicalKernelIrIdentityV13,
    output_identity: VerifiedCanonicalKernelIrIdentityV13,
    input_epoch: u64,
    output_epoch: u64,
    capability_replay: KernelCapabilityPreservationReplayV1,
    fresh_output_analysis: FreshOutputAnalysisBindingV1,
    module_relation: Option<CheckedModuleTransformRelationV1>,
    optimizer_query_replays: Vec<CheckedOptimizerQueryV1>,
    invalidated_analyses: &'static [OptimizerAnalysisV1],
    completed_replays: &'static [OptimizerAnalysisV1],
    required_replays: &'static [OptimizerAnalysisV1],
}

impl CheckedTransformationPreservationRecordV1 {
    pub const fn transformation(&self) -> ProductionTransformationV1 {
        self.transformation
    }

    pub const fn mode(&self) -> TransformationPreservationModeV1 {
        self.mode
    }

    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.input_identity
    }

    pub const fn output_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.output_identity
    }

    pub const fn input_epoch(&self) -> u64 {
        self.input_epoch
    }

    pub const fn output_epoch(&self) -> u64 {
        self.output_epoch
    }

    pub fn changed(&self) -> bool {
        self.input_identity != self.output_identity
    }

    pub const fn capability_replay(&self) -> &KernelCapabilityPreservationReplayV1 {
        &self.capability_replay
    }

    pub const fn fresh_output_analysis(&self) -> FreshOutputAnalysisBindingV1 {
        self.fresh_output_analysis
    }

    pub const fn module_relation(&self) -> Option<&CheckedModuleTransformRelationV1> {
        self.module_relation.as_ref()
    }

    pub fn optimizer_query_replays(&self) -> &[CheckedOptimizerQueryV1] {
        &self.optimizer_query_replays
    }

    pub const fn invalidated_analyses(&self) -> &'static [OptimizerAnalysisV1] {
        self.invalidated_analyses
    }

    pub const fn completed_replays(&self) -> &'static [OptimizerAnalysisV1] {
        self.completed_replays
    }

    pub const fn required_replays(&self) -> &'static [OptimizerAnalysisV1] {
        self.required_replays
    }

    pub const fn requires_complete_final_graph_analysis_replay(&self) -> bool {
        !self.required_replays.is_empty()
    }

    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum TransformationPreservationErrorV1 {
    UnavailableFailClosed {
        transformation: ProductionTransformationV1,
    },
    InputRejected(VerifiedCanonicalKernelIrErrorV13),
    OutputRejected(VerifiedCanonicalKernelIrErrorV13),
    EpochOverflow {
        input_epoch: u64,
    },
    OutputEpochMismatch {
        input_epoch: u64,
        expected_output_epoch: u64,
        observed_output_epoch: u64,
    },
    ModuleContractChanged,
    ModuleTransform(TargetNeutralModuleTransformErrorV1),
    LoopMemoryTransform(LoopMemoryTransformErrorV1),
    OptimizerQueryReplay(OptimizerQueryFailureV1),
    OptimizerQueryReplayRefuted,
    CapabilityReplay(KernelCapabilityPreservationErrorV1),
    FreshOutputAnalysisMismatch,
}

impl fmt::Display for TransformationPreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnavailableFailClosed { transformation } => write!(
                formatter,
                "production transform '{}' is unavailable and remains fail-closed",
                transformation.name()
            ),
            Self::InputRejected(error) => {
                write!(formatter, "pre-transform canonical KIR V13 was rejected: {error}")
            }
            Self::OutputRejected(error) => {
                write!(formatter, "post-transform canonical KIR V13 was rejected: {error}")
            }
            Self::EpochOverflow { input_epoch } => write!(
                formatter,
                "transform mutation cannot advance graph epoch {input_epoch}"
            ),
            Self::OutputEpochMismatch {
                input_epoch,
                expected_output_epoch,
                observed_output_epoch,
            } => write!(
                formatter,
                "transform from epoch {input_epoch} requires output epoch {expected_output_epoch}, observed {observed_output_epoch}"
            ),
            Self::ModuleContractChanged => formatter.write_str(
                "transform changed a module, root, helper interface, launch, or target-requirement contract",
            ),
            Self::ModuleTransform(error) => {
                write!(formatter, "whole-module transform replay failed: {error}")
            }
            Self::LoopMemoryTransform(error) => {
                write!(formatter, "loop/memory transform replay failed: {error}")
            }
            Self::OptimizerQueryReplay(error) => {
                write!(formatter, "optimizer query replay failed: {error}")
            }
            Self::OptimizerQueryReplayRefuted => {
                formatter.write_str("optimizer query replay refuted a required transform fact")
            }
            Self::CapabilityReplay(error) => {
                write!(formatter, "V13 capability preservation replay failed: {error}")
            }
            Self::FreshOutputAnalysisMismatch => formatter.write_str(
                "fresh post-transform capability analysis did not bind the output graph and epoch",
            ),
        }
    }
}

impl Error for TransformationPreservationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InputRejected(error) | Self::OutputRejected(error) => Some(error),
            Self::ModuleTransform(error) => Some(error),
            Self::LoopMemoryTransform(error) => Some(error),
            Self::OptimizerQueryReplay(error) => Some(error),
            Self::CapabilityReplay(error) => Some(error),
            Self::UnavailableFailClosed { .. }
            | Self::EpochOverflow { .. }
            | Self::OutputEpochMismatch { .. }
            | Self::ModuleContractChanged
            | Self::OptimizerQueryReplayRefuted
            | Self::FreshOutputAnalysisMismatch => None,
        }
    }
}

/// Checks one production transform directly against its pre- and
/// post-transform canonical KIR V13 modules.
pub fn check_transformation_preservation_v1(
    transformation: ProductionTransformationV1,
    before: &Module,
    after: &Module,
    input_epoch: u64,
    output_epoch: u64,
) -> Result<CheckedTransformationPreservationRecordV1, TransformationPreservationErrorV1> {
    if disposition(transformation) == ProductionTransformationDispositionV1::UnavailableFailClosed {
        return Err(TransformationPreservationErrorV1::UnavailableFailClosed { transformation });
    }

    let input_analysis = analyze_kernel_capability_preservation_v1(before, input_epoch)
        .map_err(TransformationPreservationErrorV1::InputRejected)?;
    let fresh_output_analysis = analyze_kernel_capability_preservation_v1(after, output_epoch)
        .map_err(TransformationPreservationErrorV1::OutputRejected)?;
    let changed = input_analysis.canonical_identity() != fresh_output_analysis.canonical_identity();
    let expected_output_epoch = if changed {
        input_epoch
            .checked_add(1)
            .ok_or(TransformationPreservationErrorV1::EpochOverflow { input_epoch })?
    } else {
        input_epoch
    };
    if output_epoch != expected_output_epoch {
        return Err(TransformationPreservationErrorV1::OutputEpochMismatch {
            input_epoch,
            expected_output_epoch,
            observed_output_epoch: output_epoch,
        });
    }

    let (module_relation, optimizer_query_replays) =
        if let Some(module_transform) = transformation.module_transform() {
            (
                Some(CheckedModuleTransformRelationV1::TargetNeutral(
                    check_target_neutral_module_transform_relation_v1(
                        module_transform,
                        before,
                        after,
                        TargetNeutralModuleTransformLimitsV1::default(),
                    )
                    .map_err(TransformationPreservationErrorV1::ModuleTransform)?,
                )),
                Vec::new(),
            )
        } else if let Some(loop_memory_transform) = transformation.loop_memory_transform() {
            let report = check_loop_memory_transform_relation_v1(
                loop_memory_transform,
                before,
                after,
                LoopMemoryTransformLimitsV1::default(),
            )
            .map_err(TransformationPreservationErrorV1::LoopMemoryTransform)?;
            let query_replays = replay_loop_optimizer_queries(before, input_epoch, &report)?;
            (
                Some(CheckedModuleTransformRelationV1::LoopMemory(Box::new(
                    report,
                ))),
                query_replays,
            )
        } else {
            (None, Vec::new())
        };
    check_module_contract(transformation, before, after)?;
    let capability_replay = input_analysis
        .replay_candidate(input_epoch, output_epoch, u64::from(changed), after)
        .map_err(TransformationPreservationErrorV1::CapabilityReplay)?;
    if fresh_output_analysis.graph_epoch() != output_epoch
        || fresh_output_analysis.canonical_identity() != capability_replay.output_identity()
    {
        return Err(TransformationPreservationErrorV1::FreshOutputAnalysisMismatch);
    }

    let (mode, invalidated_analyses, completed_replays, required_replays) = if changed {
        (
            TransformationPreservationModeV1::ExactProtectedStructureReplay,
            INVALIDATED_BY_MUTATION,
            REPLAYED_AT_PASS_BOUNDARY,
            REQUIRED_BEFORE_SEMANTIC_ADMISSION,
        )
    } else {
        (
            TransformationPreservationModeV1::ExactCanonicalIdentity,
            &[][..],
            &[][..],
            &[][..],
        )
    };

    Ok(CheckedTransformationPreservationRecordV1 {
        transformation,
        mode,
        input_identity: *input_analysis.canonical_identity(),
        output_identity: *fresh_output_analysis.canonical_identity(),
        input_epoch,
        output_epoch,
        capability_replay,
        fresh_output_analysis: FreshOutputAnalysisBindingV1 {
            graph_identity: *fresh_output_analysis.canonical_identity(),
            graph_epoch: fresh_output_analysis.graph_epoch(),
        },
        module_relation,
        optimizer_query_replays,
        invalidated_analyses,
        completed_replays,
        required_replays,
    })
}

fn replay_loop_optimizer_queries(
    input: &Module,
    input_epoch: u64,
    report: &LoopMemoryTransformReportV1,
) -> Result<Vec<CheckedOptimizerQueryV1>, TransformationPreservationErrorV1> {
    let session =
        CheckedOptimizerQuerySessionV1::new(input, input_epoch, OptimizerQueryLimitsV1::default())
            .map_err(TransformationPreservationErrorV1::OptimizerQueryReplay)?;
    let mut receipts = Vec::new();
    for proof in report.dynamic_trip_proofs() {
        let fact = report
            .induction_facts()
            .iter()
            .find(|fact| fact.function() == proof.function() && fact.header() == proof.header())
            .ok_or(TransformationPreservationErrorV1::OptimizerQueryReplayRefuted)?;
        let used = report.edits().iter().any(|edit| match edit {
            LoopMemoryEditV1::LoopFullyUnrolled {
                function, header, ..
            }
            | LoopMemoryEditV1::LoopPartiallyUnrolled {
                function, header, ..
            } => function == proof.function() && *header == proof.header(),
            LoopMemoryEditV1::OperationMoved {
                source,
                destination_block,
            } => {
                report.transform() == LoopMemoryTransformV1::LoopInvariantCodeMotion
                    && source.function() == proof.function()
                    && *destination_block == fact.preheader()
            }
            _ => false,
        });
        if !used {
            continue;
        }
        match session.prove_dominating_index_equals_constant(
            proof.function(),
            fact.preheader(),
            proof.bound_value(),
        ) {
            OptimizerQueryDecisionV1::Proved(receipt)
                if matches!(
                    receipt.kind(),
                    CheckedOptimizerQueryKindV1::DominatingIndexEqualsConstant {
                        constant,
                        ..
                    } if *constant == proof.exact_bound()
                ) =>
            {
                receipts.push(receipt);
            }
            OptimizerQueryDecisionV1::Incomplete(error) => {
                return Err(TransformationPreservationErrorV1::OptimizerQueryReplay(
                    error,
                ));
            }
            OptimizerQueryDecisionV1::Proved(_) | OptimizerQueryDecisionV1::Refuted { .. } => {
                return Err(TransformationPreservationErrorV1::OptimizerQueryReplayRefuted);
            }
        }
    }
    if report.transform() != LoopMemoryTransformV1::LoopInvariantCodeMotion {
        return Ok(receipts);
    }
    let mut requirements = BTreeSet::new();
    for fact in report
        .induction_facts()
        .iter()
        .filter(|fact| fact.trip_count().is_none())
    {
        let used = report.edits().iter().any(|edit| {
            matches!(
                edit,
                LoopMemoryEditV1::OperationMoved {
                    source,
                    destination_block,
                } if source.function() == fact.function()
                    && *destination_block == fact.preheader()
            )
        });
        if used {
            requirements.insert((
                fact.function().clone(),
                fact.preheader(),
                fact.start_value(),
                fact.bound_value(),
            ));
        }
    }
    for (function, destination, lhs, rhs) in requirements {
        match session.prove_dominating_unsigned_less_than(&function, destination, lhs, rhs) {
            OptimizerQueryDecisionV1::Proved(receipt) => receipts.push(receipt),
            OptimizerQueryDecisionV1::Incomplete(error) => {
                return Err(TransformationPreservationErrorV1::OptimizerQueryReplay(
                    error,
                ));
            }
            OptimizerQueryDecisionV1::Refuted { .. } => {
                return Err(TransformationPreservationErrorV1::OptimizerQueryReplayRefuted);
            }
        }
    }
    Ok(receipts)
}

/// Result of one checked, fresh-session V13 transform. This value carries no
/// semantic, publication, artifact, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct CheckedCanonicalTransformationV13V1 {
    module: Module,
    canonical: VerifiedCanonicalKernelIrV13,
    preservation: CheckedTransformationPreservationRecordV1,
}

impl CheckedCanonicalTransformationV13V1 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.canonical
    }

    pub const fn preservation(&self) -> &CheckedTransformationPreservationRecordV1 {
        &self.preservation
    }

    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }
}

pub const TARGET_NEUTRAL_SCALAR_CLEANUP_REPLAY_ORDER_V1: [ProductionTransformationV1; 7] = [
    ProductionTransformationV1::GeneralTargetIndependentCanonicalization,
    ProductionTransformationV1::SparseConditionalConstantPropagation,
    ProductionTransformationV1::SimplifyControlFlow,
    ProductionTransformationV1::GlobalValueNumbering,
    ProductionTransformationV1::LocalPureCommonSubexpressionElimination,
    ProductionTransformationV1::DeadCodeElimination,
    ProductionTransformationV1::SimplifyControlFlow,
];
pub const TARGET_NEUTRAL_SCALAR_CLEANUP_MAX_ITERATIONS_V1: usize = 8;

#[derive(Debug, Eq, PartialEq)]
pub struct CheckedScalarCleanupReplayV1 {
    module: Module,
    final_epoch: u64,
    iterations: usize,
    pass_records: Vec<CheckedTransformationPreservationRecordV1>,
}

impl CheckedScalarCleanupReplayV1 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub const fn iterations(&self) -> usize {
        self.iterations
    }

    pub fn pass_records(&self) -> &[CheckedTransformationPreservationRecordV1] {
        &self.pass_records
    }

    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum CheckedScalarCleanupReplayErrorV1 {
    Transform(CheckedCanonicalTransformationErrorV1),
    IterationLimitExceeded { limit: usize },
}

impl fmt::Display for CheckedScalarCleanupReplayErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transform(error) => error.fmt(formatter),
            Self::IterationLimitExceeded { limit } => write!(
                formatter,
                "target-neutral scalar cleanup did not converge within {limit} iterations"
            ),
        }
    }
}

impl Error for CheckedScalarCleanupReplayErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transform(error) => Some(error),
            Self::IterationLimitExceeded { .. } => None,
        }
    }
}

/// Replays the closed scalar cleanup order to an exact bounded fixed point.
/// Each pass independently reconstructs canonical mutation and capability
/// preservation; convergence is derived from canonical identity equality.
pub fn execute_checked_scalar_cleanup_replay_v1(
    input: &Module,
    input_epoch: u64,
) -> Result<CheckedScalarCleanupReplayV1, CheckedScalarCleanupReplayErrorV1> {
    let mut module = input.clone();
    let mut epoch = input_epoch;
    let mut pass_records = Vec::new();
    for iteration in 1..=TARGET_NEUTRAL_SCALAR_CLEANUP_MAX_ITERATIONS_V1 {
        let mut changed = false;
        for transformation in TARGET_NEUTRAL_SCALAR_CLEANUP_REPLAY_ORDER_V1 {
            let output =
                execute_checked_canonical_transformation_v13_v1(transformation, &module, epoch)
                    .map_err(CheckedScalarCleanupReplayErrorV1::Transform)?;
            changed |= output.preservation().changed();
            epoch = output.preservation().output_epoch();
            pass_records.push(output.preservation().clone());
            module = output.module;
        }
        if !changed {
            return Ok(CheckedScalarCleanupReplayV1 {
                module,
                final_epoch: epoch,
                iterations: iteration,
                pass_records,
            });
        }
    }
    Err(CheckedScalarCleanupReplayErrorV1::IterationLimitExceeded {
        limit: TARGET_NEUTRAL_SCALAR_CLEANUP_MAX_ITERATIONS_V1,
    })
}

#[derive(Debug)]
pub enum CheckedCanonicalTransformationErrorV1 {
    UnavailableFailClosed {
        transformation: ProductionTransformationV1,
    },
    Input(VerifiedCanonicalKernelIrErrorV13),
    Output(VerifiedCanonicalKernelIrErrorV13),
    DialectRegistration(NameError),
    Session(ContextBuildError),
    Import(KirBridgeErrorV1),
    Plan(PlironOptimizationPlanErrorV1),
    Transform(PlironOptimizationErrorV1),
    Export(KirBridgeErrorV1),
    Decode(KernelIrDecodeError),
    ModuleTransform(TargetNeutralModuleTransformErrorV1),
    LoopMemoryTransform(LoopMemoryTransformErrorV1),
    EpochOverflow,
    MutationAccountingMismatch {
        reported_changed: bool,
        canonical_changed: bool,
    },
    Preservation(TransformationPreservationErrorV1),
}

impl fmt::Display for CheckedCanonicalTransformationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnavailableFailClosed { transformation } => write!(
                formatter,
                "production transform '{}' is unavailable and remains fail-closed",
                transformation.name()
            ),
            Self::Input(error) => write!(formatter, "canonical V13 input was rejected: {error}"),
            Self::Output(error) => write!(formatter, "canonical V13 output was rejected: {error}"),
            Self::DialectRegistration(error) => {
                write!(
                    formatter,
                    "GPU dialect registration was rejected: {error:?}"
                )
            }
            Self::Session(error) => write!(formatter, "fresh Pliron session failed: {error:?}"),
            Self::Import(error) => write!(formatter, "canonical V13 import failed: {error}"),
            Self::Plan(error) => write!(formatter, "one-pass transform plan failed: {error}"),
            Self::Transform(error) => write!(formatter, "typed transform failed: {error}"),
            Self::Export(error) => write!(formatter, "canonical V13 export failed: {error}"),
            Self::Decode(error) => {
                write!(formatter, "canonical V13 output did not decode: {error}")
            }
            Self::ModuleTransform(error) => {
                write!(
                    formatter,
                    "whole-module canonical transform failed: {error}"
                )
            }
            Self::LoopMemoryTransform(error) => {
                write!(formatter, "loop/memory canonical transform failed: {error}")
            }
            Self::EpochOverflow => formatter.write_str("transform output epoch overflowed"),
            Self::MutationAccountingMismatch {
                reported_changed,
                canonical_changed,
            } => write!(
                formatter,
                "pass reported changed={reported_changed}, but canonical identities observed changed={canonical_changed}"
            ),
            Self::Preservation(error) => error.fmt(formatter),
        }
    }
}

impl Error for CheckedCanonicalTransformationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) | Self::Output(error) => Some(error),
            Self::Import(error) | Self::Export(error) => Some(error),
            Self::Plan(error) => Some(error),
            Self::Transform(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::ModuleTransform(error) => Some(error),
            Self::LoopMemoryTransform(error) => Some(error),
            Self::Preservation(error) => Some(error),
            Self::UnavailableFailClosed { .. }
            | Self::DialectRegistration(_)
            | Self::Session(_)
            | Self::EpochOverflow
            | Self::MutationAccountingMismatch { .. } => None,
        }
    }
}

/// Executes one real checked transform against canonical V13 in a fresh owner
/// session, then derives mutation and epoch state from canonical identities.
pub fn execute_checked_canonical_transformation_v13_v1(
    transformation: ProductionTransformationV1,
    input: &Module,
    input_epoch: u64,
) -> Result<CheckedCanonicalTransformationV13V1, CheckedCanonicalTransformationErrorV1> {
    if let Some(module_transform) = transformation.module_transform() {
        let canonical = VerifiedCanonicalKernelIrV13::from_module(input.clone())
            .map_err(CheckedCanonicalTransformationErrorV1::Input)?;
        let (module, report) = execute_target_neutral_module_transform_v1(
            module_transform,
            input,
            TargetNeutralModuleTransformLimitsV1::default(),
        )
        .map_err(CheckedCanonicalTransformationErrorV1::ModuleTransform)?;
        let output = VerifiedCanonicalKernelIrV13::from_module(module.clone())
            .map_err(CheckedCanonicalTransformationErrorV1::Output)?;
        let canonical_changed = output.identity() != canonical.identity();
        if report.changed() != canonical_changed {
            return Err(
                CheckedCanonicalTransformationErrorV1::MutationAccountingMismatch {
                    reported_changed: report.changed(),
                    canonical_changed,
                },
            );
        }
        let output_epoch = if canonical_changed {
            input_epoch
                .checked_add(1)
                .ok_or(CheckedCanonicalTransformationErrorV1::EpochOverflow)?
        } else {
            input_epoch
        };
        let preservation = check_transformation_preservation_v1(
            transformation,
            input,
            &module,
            input_epoch,
            output_epoch,
        )
        .map_err(CheckedCanonicalTransformationErrorV1::Preservation)?;
        return Ok(CheckedCanonicalTransformationV13V1 {
            module,
            canonical: output,
            preservation,
        });
    }
    if let Some(loop_memory_transform) = transformation.loop_memory_transform() {
        let canonical = VerifiedCanonicalKernelIrV13::from_module(input.clone())
            .map_err(CheckedCanonicalTransformationErrorV1::Input)?;
        let (module, report) = execute_loop_memory_transform_v1(
            loop_memory_transform,
            input,
            LoopMemoryTransformLimitsV1::default(),
        )
        .map_err(CheckedCanonicalTransformationErrorV1::LoopMemoryTransform)?;
        let output = VerifiedCanonicalKernelIrV13::from_module(module.clone())
            .map_err(CheckedCanonicalTransformationErrorV1::Output)?;
        let canonical_changed = output.identity() != canonical.identity();
        if report.changed() != canonical_changed {
            return Err(
                CheckedCanonicalTransformationErrorV1::MutationAccountingMismatch {
                    reported_changed: report.changed(),
                    canonical_changed,
                },
            );
        }
        let output_epoch = if canonical_changed {
            input_epoch
                .checked_add(1)
                .ok_or(CheckedCanonicalTransformationErrorV1::EpochOverflow)?
        } else {
            input_epoch
        };
        let preservation = check_transformation_preservation_v1(
            transformation,
            input,
            &module,
            input_epoch,
            output_epoch,
        )
        .map_err(CheckedCanonicalTransformationErrorV1::Preservation)?;
        return Ok(CheckedCanonicalTransformationV13V1 {
            module,
            canonical: output,
            preservation,
        });
    }
    let pass = transformation
        .pliron()
        .ok_or(CheckedCanonicalTransformationErrorV1::UnavailableFailClosed { transformation })?;
    let canonical = VerifiedCanonicalKernelIrV13::from_module(input.clone())
        .map_err(CheckedCanonicalTransformationErrorV1::Input)?;
    let limits = production_kernel_ir_pliron_optimization_limits_v2();
    let registration = dialect_gpu::dialect_registration()
        .map_err(CheckedCanonicalTransformationErrorV1::DialectRegistration)?;
    let mut session = PlironSession::new(limits.shell(), [registration])
        .map_err(CheckedCanonicalTransformationErrorV1::Session)?;
    let graph = session
        .import_canonical_kir_v13_o0(&canonical)
        .map_err(CheckedCanonicalTransformationErrorV1::Import)?;
    let plan = PlironOptimizationPlanV1::new(vec![pass], limits.pliron())
        .map_err(CheckedCanonicalTransformationErrorV1::Plan)?;
    let report = session
        .execute_optimization_v1(graph.root(), &plan)
        .map_err(CheckedCanonicalTransformationErrorV1::Transform)?;
    let (output, _) = session
        .extract_optimized_canonical_kir_v13_v1(&graph)
        .map_err(CheckedCanonicalTransformationErrorV1::Export)?;
    let module = decode_module_v13(output.canonical_bytes())
        .map_err(CheckedCanonicalTransformationErrorV1::Decode)?;
    let canonical_changed = output.identity() != canonical.identity();
    let [pass_report] = report.passes() else {
        return Err(
            CheckedCanonicalTransformationErrorV1::MutationAccountingMismatch {
                reported_changed: false,
                canonical_changed,
            },
        );
    };
    if pass_report.pass() != pass || pass_report.changed() != canonical_changed {
        return Err(
            CheckedCanonicalTransformationErrorV1::MutationAccountingMismatch {
                reported_changed: pass_report.changed(),
                canonical_changed,
            },
        );
    }
    let output_epoch = if canonical_changed {
        input_epoch
            .checked_add(1)
            .ok_or(CheckedCanonicalTransformationErrorV1::EpochOverflow)?
    } else {
        input_epoch
    };
    let preservation = check_transformation_preservation_v1(
        transformation,
        input,
        &module,
        input_epoch,
        output_epoch,
    )
    .map_err(CheckedCanonicalTransformationErrorV1::Preservation)?;
    Ok(CheckedCanonicalTransformationV13V1 {
        module,
        canonical: output,
        preservation,
    })
}

pub fn check_pliron_transformation_preservation_v1(
    pass: PlironOptimizationPassV1,
    before: &Module,
    after: &Module,
    input_epoch: u64,
    output_epoch: u64,
) -> Result<CheckedTransformationPreservationRecordV1, TransformationPreservationErrorV1> {
    check_transformation_preservation_v1(
        ProductionTransformationV1::from_pliron(pass),
        before,
        after,
        input_epoch,
        output_epoch,
    )
}

fn check_module_contract(
    transformation: ProductionTransformationV1,
    before: &Module,
    after: &Module,
) -> Result<(), TransformationPreservationErrorV1> {
    if before.id != after.id
        || before.kernels != after.kernels
        || before.required_capabilities != after.required_capabilities
    {
        return Err(TransformationPreservationErrorV1::ModuleContractChanged);
    }
    if transformation == ProductionTransformationV1::InterproceduralCleanup {
        return Ok(());
    }
    let interfaces_match = before.functions.len() == after.functions.len()
        && before
            .functions
            .iter()
            .zip(&after.functions)
            .all(|(lhs, rhs)| {
                lhs.id == rhs.id
                    && lhs.signature == rhs.signature
                    && lhs.role == rhs.role
                    && lhs.required_capabilities == rhs.required_capabilities
                    && lhs.body.is_some() == rhs.body.is_some()
                    && lhs.body.as_ref().map(|body| &body.parameters)
                        == rhs.body.as_ref().map(|body| &body.parameters)
            });
    if !interfaces_match {
        return Err(TransformationPreservationErrorV1::ModuleContractChanged);
    }
    Ok(())
}
