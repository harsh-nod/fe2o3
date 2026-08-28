//! Sealed provenance and independent-checking boundary for production reports.
//!
//! A clean analysis report is not, by itself, evidence that the analysis
//! visited every relevant operation or discharged every obligation. This
//! module binds each actual report result to the exact compiler-owned PLIRON
//! checkpoint and records the independently replayed witness, or the exact
//! evidence gap that keeps the supported-fragment result `Incomplete`.
//!
//! Compact structural labels are diagnostic lineage only. Session decisions
//! consume private tokens minted after retained canonical-byte comparison.

use std::{fmt, marker::PhantomData};

use pliron::{
    builtin::ops::FuncOp,
    context::{Context, Ptr},
    op::Op,
    operation::Operation,
};

use crate::{
    HierarchicalOwnershipReportV1, KernelCheckPassKindV1, KernelCheckStatusV1,
    PRODUCTION_PLIRON_PASS_CONTRACTS_V1, PlironAtomicLegalityReportV1,
    PlironAtomicTargetCapabilityV1, PlironAtomicTargetContextV1, PlironBarrierReportV1,
    PlironPassCheckpointTokenV1, PlironPassPreservationReportV1, PlironPassValidationHandleV1,
    PlironPipelineProtocolReportV1, PlironSemanticRefinementReportV1,
    PlironStructuralIdentityLabelV1, PlironTensorLayoutReportV1, PlironWorkgroupMemoryReportV1,
    ProductionAnalysisWitnessEnvelopeV1, ProductionAnalysisWitnessValidationErrorV1,
    RankedBoundsReportV1, RankedRaceReportV1, issue_and_validate_production_analysis_witness_v1,
};

/// Number of reports in the fixed production analysis sequence.
pub const PRODUCTION_ANALYSIS_REPORT_COUNT_V1: usize = 9;

/// Exact implementation identity bound into a sealed stage result.
///
/// A variant change is an analysis-version change. Fixed resource limits and
/// decision rules are part of the named implementation. Runtime target
/// capabilities are retained separately in the typed configuration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionAnalysisImplementationV1 {
    PlironTensorLayoutV1,
    PlironRankedBoundsV1,
    PlironAtomicLegalityV1,
    PlironRankedRaceV1,
    PlironHierarchicalOwnershipV1,
    PlironBarrierConvergenceV1,
    PlironPipelineProtocolV1,
    PlironWorkgroupMemoryV1,
    PlironSemanticRefinementV1,
}

impl ProductionAnalysisImplementationV1 {
    pub const fn pass(self) -> KernelCheckPassKindV1 {
        match self {
            Self::PlironTensorLayoutV1 => KernelCheckPassKindV1::TensorLayout,
            Self::PlironRankedBoundsV1 => KernelCheckPassKindV1::MemoryBounds,
            Self::PlironAtomicLegalityV1 => KernelCheckPassKindV1::AtomicLegality,
            Self::PlironRankedRaceV1 => KernelCheckPassKindV1::RaceFreedom,
            Self::PlironHierarchicalOwnershipV1 => KernelCheckPassKindV1::HierarchicalOwnership,
            Self::PlironBarrierConvergenceV1 => KernelCheckPassKindV1::BarrierConvergence,
            Self::PlironPipelineProtocolV1 => KernelCheckPassKindV1::PipelineProtocol,
            Self::PlironWorkgroupMemoryV1 => KernelCheckPassKindV1::WorkgroupMemory,
            Self::PlironSemanticRefinementV1 => KernelCheckPassKindV1::SemanticRefinement,
        }
    }

    pub const fn version(self) -> &'static str {
        match self {
            Self::PlironTensorLayoutV1 => "fe2o3.pliron.tensor-layout.analysis.v1",
            Self::PlironRankedBoundsV1 => "fe2o3.pliron.ranked-bounds.analysis.v1",
            Self::PlironAtomicLegalityV1 => "fe2o3.pliron.atomic-legality.analysis.v1",
            Self::PlironRankedRaceV1 => "fe2o3.pliron.ranked-race.analysis.v1",
            Self::PlironHierarchicalOwnershipV1 => {
                "fe2o3.pliron.hierarchical-ownership.analysis.v1"
            }
            Self::PlironBarrierConvergenceV1 => "fe2o3.pliron.barrier-convergence.analysis.v1",
            Self::PlironPipelineProtocolV1 => "fe2o3.pliron.pipeline-protocol.analysis.v1",
            Self::PlironWorkgroupMemoryV1 => "fe2o3.pliron.workgroup-memory.analysis.v1",
            Self::PlironSemanticRefinementV1 => "fe2o3.pliron.semantic-refinement.analysis.v1",
        }
    }
}

fn implementation_for(pass: KernelCheckPassKindV1) -> ProductionAnalysisImplementationV1 {
    match pass {
        KernelCheckPassKindV1::TensorLayout => {
            ProductionAnalysisImplementationV1::PlironTensorLayoutV1
        }
        KernelCheckPassKindV1::MemoryBounds => {
            ProductionAnalysisImplementationV1::PlironRankedBoundsV1
        }
        KernelCheckPassKindV1::AtomicLegality => {
            ProductionAnalysisImplementationV1::PlironAtomicLegalityV1
        }
        KernelCheckPassKindV1::RaceFreedom => {
            ProductionAnalysisImplementationV1::PlironRankedRaceV1
        }
        KernelCheckPassKindV1::HierarchicalOwnership => {
            ProductionAnalysisImplementationV1::PlironHierarchicalOwnershipV1
        }
        KernelCheckPassKindV1::BarrierConvergence => {
            ProductionAnalysisImplementationV1::PlironBarrierConvergenceV1
        }
        KernelCheckPassKindV1::PipelineProtocol => {
            ProductionAnalysisImplementationV1::PlironPipelineProtocolV1
        }
        KernelCheckPassKindV1::WorkgroupMemory => {
            ProductionAnalysisImplementationV1::PlironWorkgroupMemoryV1
        }
        KernelCheckPassKindV1::SemanticRefinement => {
            ProductionAnalysisImplementationV1::PlironSemanticRefinementV1
        }
        KernelCheckPassKindV1::Structural | KernelCheckPassKindV1::ControlFlow => {
            unreachable!("non-production analysis stage")
        }
    }
}

/// Runtime analysis configuration included in the sealed stage identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionAnalysisConfigurationV1 {
    /// All configuration is fixed by the named implementation version.
    FixedByImplementation,
    /// Atomics were checked conservatively without a bound target capability.
    AtomicTargetAgnostic,
    /// Exact sorted target capabilities supplied to atomic legality.
    AtomicTarget {
        capabilities: Vec<PlironAtomicTargetCapabilityV1>,
    },
}

/// Exact named checkpoint for one production report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionAnalysisCheckpointV1 {
    position: usize,
    pass: KernelCheckPassKindV1,
    identity: PlironStructuralIdentityLabelV1,
    mutation_epoch: u64,
}

impl ProductionAnalysisCheckpointV1 {
    pub const fn position(self) -> usize {
        self.position
    }

    pub const fn pass(self) -> KernelCheckPassKindV1 {
        self.pass
    }

    /// Diagnostic label only. Validation never accepts label equality.
    pub const fn identity_label(self) -> PlironStructuralIdentityLabelV1 {
        self.identity
    }

    /// Monotonic context epoch sealed into this exact pass checkpoint.
    pub const fn mutation_epoch(self) -> u64 {
        self.mutation_epoch
    }
}

/// The exhaustive evidence absent from a current V1 report.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionAnalysisWitnessGapV1 {
    TensorLayoutExhaustiveDataflow,
    BoundsPresburgerProofTranscript,
    AtomicEnumerationCapabilityAndProvenance,
    RaceEffectsAliasAndHappensBefore,
    OwnershipDomainDisjointnessAndCoverage,
    BarrierReachabilityUniformityAndPostdominance,
    PipelineEpochLifecycleAndSlotReuse,
    WorkgroupAllocationLifetimeAndConflict,
    SemanticRootsControlEffectsAndNumerics,
}

impl ProductionAnalysisWitnessGapV1 {
    pub const fn pass(self) -> KernelCheckPassKindV1 {
        match self {
            Self::TensorLayoutExhaustiveDataflow => KernelCheckPassKindV1::TensorLayout,
            Self::BoundsPresburgerProofTranscript => KernelCheckPassKindV1::MemoryBounds,
            Self::AtomicEnumerationCapabilityAndProvenance => KernelCheckPassKindV1::AtomicLegality,
            Self::RaceEffectsAliasAndHappensBefore => KernelCheckPassKindV1::RaceFreedom,
            Self::OwnershipDomainDisjointnessAndCoverage => {
                KernelCheckPassKindV1::HierarchicalOwnership
            }
            Self::BarrierReachabilityUniformityAndPostdominance => {
                KernelCheckPassKindV1::BarrierConvergence
            }
            Self::PipelineEpochLifecycleAndSlotReuse => KernelCheckPassKindV1::PipelineProtocol,
            Self::WorkgroupAllocationLifetimeAndConflict => KernelCheckPassKindV1::WorkgroupMemory,
            Self::SemanticRootsControlEffectsAndNumerics => {
                KernelCheckPassKindV1::SemanticRefinement
            }
        }
    }

    pub const fn required_evidence(self) -> &'static str {
        match self {
            Self::TensorLayoutExhaustiveDataflow => {
                "an exhaustive operation/value layout fact map with propagation and consumer-compatibility witnesses"
            }
            Self::BoundsPresburgerProofTranscript => {
                "every ranked access dimension, admitted path domain, normalized affine inequality, and independently checkable Presburger certificate"
            }
            Self::AtomicEnumerationCapabilityAndProvenance => {
                "an exhaustive atomic-operation enumeration joined to exact target capabilities, memory provenance, scope, and ordering witnesses"
            }
            Self::RaceEffectsAliasAndHappensBefore => {
                "every instantiated memory effect plus independently checkable alias, disjointness, ownership, and happens-before witnesses"
            }
            Self::OwnershipDomainDisjointnessAndCoverage => {
                "every contracted output/write domain plus range, injectivity, partition-disjointness, and total-coverage certificates"
            }
            Self::BarrierReachabilityUniformityAndPostdominance => {
                "every barrier and participant domain plus reachability, uniform-control, phase, and postdominance certificates"
            }
            Self::PipelineEpochLifecycleAndSlotReuse => {
                "every staged epoch transition, ring-slot identity, loop invariant, prime/drain obligation, and release-before-reuse witness"
            }
            Self::WorkgroupAllocationLifetimeAndConflict => {
                "every workgroup allocation/effect with byte layout, lifetime, epoch, alias, and conflict-freedom witnesses"
            }
            Self::SemanticRootsControlEffectsAndNumerics => {
                "a complete reference-root, output, control/loop, effect, arithmetic, and numerical proof object rather than commitments alone"
            }
        }
    }
}

pub(crate) fn witness_gap(pass: KernelCheckPassKindV1) -> ProductionAnalysisWitnessGapV1 {
    match pass {
        KernelCheckPassKindV1::TensorLayout => {
            ProductionAnalysisWitnessGapV1::TensorLayoutExhaustiveDataflow
        }
        KernelCheckPassKindV1::MemoryBounds => {
            ProductionAnalysisWitnessGapV1::BoundsPresburgerProofTranscript
        }
        KernelCheckPassKindV1::AtomicLegality => {
            ProductionAnalysisWitnessGapV1::AtomicEnumerationCapabilityAndProvenance
        }
        KernelCheckPassKindV1::RaceFreedom => {
            ProductionAnalysisWitnessGapV1::RaceEffectsAliasAndHappensBefore
        }
        KernelCheckPassKindV1::HierarchicalOwnership => {
            ProductionAnalysisWitnessGapV1::OwnershipDomainDisjointnessAndCoverage
        }
        KernelCheckPassKindV1::BarrierConvergence => {
            ProductionAnalysisWitnessGapV1::BarrierReachabilityUniformityAndPostdominance
        }
        KernelCheckPassKindV1::PipelineProtocol => {
            ProductionAnalysisWitnessGapV1::PipelineEpochLifecycleAndSlotReuse
        }
        KernelCheckPassKindV1::WorkgroupMemory => {
            ProductionAnalysisWitnessGapV1::WorkgroupAllocationLifetimeAndConflict
        }
        KernelCheckPassKindV1::SemanticRefinement => {
            ProductionAnalysisWitnessGapV1::SemanticRootsControlEffectsAndNumerics
        }
        KernelCheckPassKindV1::Structural | KernelCheckPassKindV1::ControlFlow => {
            unreachable!("non-production analysis stage")
        }
    }
}

/// Non-authoritative replay result from one successfully sealed report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionAnalysisStageValidationV1 {
    checkpoint: ProductionAnalysisCheckpointV1,
    implementation: ProductionAnalysisImplementationV1,
    configuration: ProductionAnalysisConfigurationV1,
    analysis_status: KernelCheckStatusV1,
    witness: ProductionAnalysisWitnessEnvelopeV1,
}

impl ProductionAnalysisStageValidationV1 {
    pub const fn checkpoint(&self) -> ProductionAnalysisCheckpointV1 {
        self.checkpoint
    }

    pub const fn implementation(&self) -> ProductionAnalysisImplementationV1 {
        self.implementation
    }

    pub const fn configuration(&self) -> &ProductionAnalysisConfigurationV1 {
        &self.configuration
    }

    pub const fn analysis_status(&self) -> KernelCheckStatusV1 {
        self.analysis_status
    }

    pub const fn remaining_witness_gap(&self) -> Option<ProductionAnalysisWitnessGapV1> {
        self.witness.coverage().gap()
    }

    pub const fn witness(&self) -> &ProductionAnalysisWitnessEnvelopeV1 {
        &self.witness
    }

    pub const fn independent_validation_status(&self) -> KernelCheckStatusV1 {
        self.analysis_status.join(self.witness.status())
    }
}

/// Diagnostic summary for the fixed nine sealed results.
///
/// This type deliberately carries no private seal or canonical bytes and
/// cannot be converted into proof authority. Complete supported-fragment
/// replay remains separate from compiler-refinement and lowering authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionAnalysisReportValidationV1 {
    stages: Vec<ProductionAnalysisStageValidationV1>,
}

impl ProductionAnalysisReportValidationV1 {
    pub fn stages(&self) -> &[ProductionAnalysisStageValidationV1] {
        &self.stages
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.stages
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, stage| {
                status.join(stage.independent_validation_status())
            })
    }

    pub fn all_reports_independently_validated(&self) -> bool {
        self.stages
            .iter()
            .all(|stage| stage.witness.coverage().is_complete())
    }

    /// Diagnostic metadata and compact labels never mint refinement authority.
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    /// Report integrity is not artifact, lowering, publication, or launch
    /// authority.
    pub const fn grants_lowering_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CapturedProductionAnalysisReportV1 {
    TensorLayout(PlironTensorLayoutReportV1),
    Bounds(RankedBoundsReportV1),
    Atomic(PlironAtomicLegalityReportV1),
    Race(RankedRaceReportV1),
    Ownership(HierarchicalOwnershipReportV1),
    Barrier(PlironBarrierReportV1),
    Pipeline(PlironPipelineProtocolReportV1),
    Workgroup(PlironWorkgroupMemoryReportV1),
    Semantic(PlironSemanticRefinementReportV1),
}

impl CapturedProductionAnalysisReportV1 {
    pub(crate) const fn pass(&self) -> KernelCheckPassKindV1 {
        match self {
            Self::TensorLayout(report) => report.pass(),
            Self::Bounds(report) => report.pass(),
            Self::Atomic(report) => report.pass(),
            Self::Race(report) => report.pass(),
            Self::Ownership(report) => report.pass(),
            Self::Barrier(report) => report.pass(),
            Self::Pipeline(report) => report.pass(),
            Self::Workgroup(report) => report.pass(),
            Self::Semantic(report) => report.pass(),
        }
    }

    pub(crate) fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::TensorLayout(report) => report.status(),
            Self::Bounds(report) => report.status(),
            Self::Atomic(report) => report.status(),
            Self::Race(report) => report.status(),
            Self::Ownership(report) => report.status(),
            Self::Barrier(report) => report.status(),
            Self::Pipeline(report) => report.status(),
            Self::Workgroup(report) => report.status(),
            Self::Semantic(report) => report.status(),
        }
    }
}

struct BoundProductionAnalysisReportV1 {
    checkpoint_token: PlironPassCheckpointTokenV1,
    context_address: usize,
    function: Ptr<Operation>,
    submitted_checkpoint: ProductionAnalysisCheckpointV1,
    implementation: ProductionAnalysisImplementationV1,
    configuration: ProductionAnalysisConfigurationV1,
    claimed_status: KernelCheckStatusV1,
    issued_report: CapturedProductionAnalysisReportV1,
    submitted_report: CapturedProductionAnalysisReportV1,
}

/// Stable fail-closed diagnostics for the sealed session itself.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionAnalysisReportValidationErrorV1 {
    CounterfeitOrCrossSessionSeal {
        position: usize,
    },
    CrossContextReport {
        position: usize,
    },
    CrossFunctionReport {
        position: usize,
    },
    StageOrderMismatch {
        position: usize,
        expected: KernelCheckPassKindV1,
        observed: KernelCheckPassKindV1,
    },
    ReplayedReport {
        issued_position: usize,
        current_position: usize,
    },
    CheckpointMetadataTampered {
        position: usize,
    },
    ImplementationTampered {
        position: usize,
    },
    ConfigurationTampered {
        position: usize,
    },
    ReportPayloadTampered {
        position: usize,
    },
    ReportStatusTampered {
        position: usize,
    },
    PreservationManifestInconsistent,
    OmittedReport {
        position: usize,
        pass: KernelCheckPassKindV1,
    },
    WitnessValidation {
        position: usize,
        error: ProductionAnalysisWitnessValidationErrorV1,
    },
}

impl ProductionAnalysisReportValidationErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::CounterfeitOrCrossSessionSeal { .. } => "FE2O3-PRESERVE-031",
            Self::CrossContextReport { .. } => "FE2O3-PRESERVE-032",
            Self::CrossFunctionReport { .. } => "FE2O3-PRESERVE-033",
            Self::StageOrderMismatch { .. } => "FE2O3-PRESERVE-035",
            Self::ReplayedReport { .. } => "FE2O3-PRESERVE-044",
            Self::CheckpointMetadataTampered { .. } => "FE2O3-PRESERVE-036",
            Self::ImplementationTampered { .. } => "FE2O3-PRESERVE-037",
            Self::ConfigurationTampered { .. } => "FE2O3-PRESERVE-038",
            Self::ReportPayloadTampered { .. } => "FE2O3-PRESERVE-039",
            Self::ReportStatusTampered { .. } => "FE2O3-PRESERVE-040",
            Self::PreservationManifestInconsistent => "FE2O3-PRESERVE-041",
            Self::OmittedReport { .. } => "FE2O3-PRESERVE-043",
            Self::WitnessValidation { error, .. } => error.code(),
        }
    }
}

impl fmt::Display for ProductionAnalysisReportValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::CounterfeitOrCrossSessionSeal { position } => write!(
                formatter,
                "analysis report at position {position} was not issued by this exact preservation session"
            ),
            Self::CrossContextReport { position } => write!(
                formatter,
                "analysis report at position {position} belongs to a different PLIRON context"
            ),
            Self::CrossFunctionReport { position } => write!(
                formatter,
                "analysis report at position {position} belongs to a different PLIRON function"
            ),
            Self::StageOrderMismatch {
                position,
                expected,
                observed,
            } => write!(
                formatter,
                "analysis report {observed:?} appears at position {position}; expected {expected:?}"
            ),
            Self::ReplayedReport {
                issued_position,
                current_position,
            } => write!(
                formatter,
                "analysis report issued for position {issued_position} was replayed at position {current_position}"
            ),
            Self::CheckpointMetadataTampered { position } => write!(
                formatter,
                "analysis checkpoint metadata was modified at position {position}"
            ),
            Self::ImplementationTampered { position } => write!(
                formatter,
                "analysis implementation/version was modified at position {position}"
            ),
            Self::ConfigurationTampered { position } => write!(
                formatter,
                "analysis configuration was modified at position {position}"
            ),
            Self::ReportPayloadTampered { position } => write!(
                formatter,
                "analysis facts or findings were modified after sealing at position {position}"
            ),
            Self::ReportStatusTampered { position } => write!(
                formatter,
                "analysis status is inconsistent with the sealed report at position {position}"
            ),
            Self::PreservationManifestInconsistent => formatter.write_str(
                "the custody-bound exact-preservation manifest does not contain the same fixed nine checkpoints",
            ),
            Self::OmittedReport { position, pass } => write!(
                formatter,
                "analysis report {pass:?} is omitted from required position {position}"
            ),
            Self::WitnessValidation { position, error } => {
                write!(formatter, "analysis witness at position {position} failed: {error}")
            }
        }
    }
}

impl std::error::Error for ProductionAnalysisReportValidationErrorV1 {}

pub(crate) struct ProductionAnalysisReportValidationSessionV1<'a> {
    preservation: PlironPassValidationHandleV1,
    context_address: usize,
    function: Ptr<Operation>,
    atomic_configuration: ProductionAnalysisConfigurationV1,
    next: usize,
    stages: Vec<ProductionAnalysisStageValidationV1>,
    _subject_borrow: PhantomData<(&'a Context, &'a FuncOp)>,
}

impl<'a> ProductionAnalysisReportValidationSessionV1<'a> {
    fn new(
        context: &'a Context,
        function: &'a FuncOp,
        atomic_target: Option<&PlironAtomicTargetContextV1>,
        preservation: PlironPassValidationHandleV1,
    ) -> Self {
        let atomic_configuration = atomic_target.map_or(
            ProductionAnalysisConfigurationV1::AtomicTargetAgnostic,
            |target| ProductionAnalysisConfigurationV1::AtomicTarget {
                capabilities: target.capabilities().iter().copied().collect(),
            },
        );
        Self {
            preservation,
            context_address: context as *const Context as usize,
            function: function.get_operation(),
            atomic_configuration,
            next: 0,
            stages: Vec::with_capacity(PRODUCTION_ANALYSIS_REPORT_COUNT_V1),
            _subject_borrow: PhantomData,
        }
    }

    fn expected_configuration(
        &self,
        pass: KernelCheckPassKindV1,
    ) -> ProductionAnalysisConfigurationV1 {
        if pass == KernelCheckPassKindV1::AtomicLegality {
            self.atomic_configuration.clone()
        } else {
            ProductionAnalysisConfigurationV1::FixedByImplementation
        }
    }

    fn issue(
        &self,
        context: &Context,
        function: &FuncOp,
        checkpoint_token: PlironPassCheckpointTokenV1,
        report: CapturedProductionAnalysisReportV1,
    ) -> Result<BoundProductionAnalysisReportV1, ProductionAnalysisReportValidationErrorV1> {
        self.require_subject_handles(context, function, self.next)?;
        if !self.preservation.same_custody(&checkpoint_token) {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CounterfeitOrCrossSessionSeal {
                    position: self.next,
                },
            );
        }
        let expected = PRODUCTION_PLIRON_PASS_CONTRACTS_V1
            .get(self.next)
            .ok_or(ProductionAnalysisReportValidationErrorV1::OmittedReport {
                position: self.next,
                pass: report.pass(),
            })?
            .pass();
        let submitted_checkpoint = ProductionAnalysisCheckpointV1 {
            position: checkpoint_token.position(),
            pass: checkpoint_token.pass(),
            identity: checkpoint_token.identity(),
            mutation_epoch: checkpoint_token.mutation_epoch(),
        };
        Ok(BoundProductionAnalysisReportV1 {
            checkpoint_token,
            context_address: self.context_address,
            function: self.function,
            submitted_checkpoint,
            implementation: implementation_for(expected),
            configuration: self.expected_configuration(expected),
            claimed_status: report.status(),
            issued_report: report.clone(),
            submitted_report: report,
        })
    }

    fn accept(
        &mut self,
        context: &Context,
        function: &FuncOp,
        bound: &BoundProductionAnalysisReportV1,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        let position = self.next;
        self.require_subject_handles(context, function, position)?;
        if !self.preservation.same_custody(&bound.checkpoint_token) {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CounterfeitOrCrossSessionSeal {
                    position,
                },
            );
        }
        let expected = PRODUCTION_PLIRON_PASS_CONTRACTS_V1
            .get(position)
            .ok_or(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position,
                    expected: KernelCheckPassKindV1::SemanticRefinement,
                    observed: bound.submitted_report.pass(),
                },
            )?
            .pass();
        let issued_position = bound.checkpoint_token.position();
        if issued_position < position {
            return Err(ProductionAnalysisReportValidationErrorV1::ReplayedReport {
                issued_position,
                current_position: position,
            });
        }
        if issued_position != position || bound.checkpoint_token.pass() != expected {
            return Err(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position,
                    expected,
                    observed: bound.checkpoint_token.pass(),
                },
            );
        }
        if bound.context_address != self.context_address {
            return Err(ProductionAnalysisReportValidationErrorV1::CrossContextReport { position });
        }
        if bound.function != self.function {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CrossFunctionReport { position },
            );
        }
        if bound.submitted_checkpoint.position != issued_position
            || bound.submitted_checkpoint.pass != bound.checkpoint_token.pass()
            || bound.submitted_checkpoint.identity != bound.checkpoint_token.identity()
            || bound.submitted_checkpoint.mutation_epoch != bound.checkpoint_token.mutation_epoch()
        {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CheckpointMetadataTampered { position },
            );
        }
        if bound.implementation != implementation_for(expected)
            || bound.implementation.pass() != expected
        {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ImplementationTampered { position },
            );
        }
        if bound.configuration != self.expected_configuration(expected) {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ConfigurationTampered { position },
            );
        }
        if bound.issued_report != bound.submitted_report {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ReportPayloadTampered { position },
            );
        }
        let observed = bound.submitted_report.pass();
        if observed != expected {
            return Err(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position,
                    expected,
                    observed,
                },
            );
        }
        if bound.claimed_status != bound.submitted_report.status() {
            return Err(
                ProductionAnalysisReportValidationErrorV1::ReportStatusTampered { position },
            );
        }
        let witness =
            issue_and_validate_production_analysis_witness_v1(
                context,
                function,
                bound.submitted_checkpoint,
                bound.implementation,
                bound.configuration.clone(),
                bound.submitted_report.clone(),
            )
            .map_err(|error| {
                ProductionAnalysisReportValidationErrorV1::WitnessValidation { position, error }
            })?;
        self.stages.push(ProductionAnalysisStageValidationV1 {
            checkpoint: bound.submitted_checkpoint,
            implementation: bound.implementation,
            configuration: bound.configuration.clone(),
            analysis_status: bound.claimed_status,
            witness,
        });
        self.next += 1;
        Ok(())
    }

    fn require_subject_handles(
        &self,
        context: &Context,
        function: &FuncOp,
        position: usize,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        if context as *const Context as usize != self.context_address {
            return Err(ProductionAnalysisReportValidationErrorV1::CrossContextReport { position });
        }
        if function.get_operation() != self.function {
            return Err(
                ProductionAnalysisReportValidationErrorV1::CrossFunctionReport { position },
            );
        }
        Ok(())
    }

    fn finish(
        self,
        preservation: &PlironPassPreservationReportV1,
    ) -> Result<ProductionAnalysisReportValidationV1, ProductionAnalysisReportValidationErrorV1>
    {
        if self.next != PRODUCTION_ANALYSIS_REPORT_COUNT_V1 {
            let pass = PRODUCTION_PLIRON_PASS_CONTRACTS_V1[self.next].pass();
            return Err(ProductionAnalysisReportValidationErrorV1::OmittedReport {
                position: self.next,
                pass,
            });
        }
        let manifest_matches = self.preservation.same_report_custody(preservation)
            && self.preservation.input_identity() == preservation.input_identity()
            && self.preservation.input_mutation_epoch() == preservation.input_mutation_epoch()
            && preservation.certificates().len() == PRODUCTION_ANALYSIS_REPORT_COUNT_V1
            && preservation.is_exact_identity()
            && self
                .stages
                .iter()
                .zip(preservation.certificates())
                .enumerate()
                .all(|(position, (stage, certificate))| {
                    stage.checkpoint.position == position
                        && stage.checkpoint.pass == certificate.pass()
                        && stage.checkpoint.identity == certificate.identity()
                        && stage.checkpoint.mutation_epoch == certificate.mutation_epoch()
                })
            && preservation
                .certificates()
                .iter()
                .zip(PRODUCTION_PLIRON_PASS_CONTRACTS_V1)
                .all(|(certificate, contract)| certificate.pass() == contract.pass());
        if !manifest_matches {
            return Err(
                ProductionAnalysisReportValidationErrorV1::PreservationManifestInconsistent,
            );
        }
        Ok(ProductionAnalysisReportValidationV1 {
            stages: self.stages,
        })
    }
}

pub(crate) fn begin_production_analysis_report_validation_v1<'a>(
    context: &'a Context,
    function: &'a FuncOp,
    atomic_target: Option<&PlironAtomicTargetContextV1>,
    preservation: PlironPassValidationHandleV1,
) -> ProductionAnalysisReportValidationSessionV1<'a> {
    ProductionAnalysisReportValidationSessionV1::new(context, function, atomic_target, preservation)
}

pub(crate) trait SealedProductionAnalysisReportV1: Clone {
    fn capture_for_sealed_validation(&self) -> CapturedProductionAnalysisReportV1;
}

macro_rules! impl_sealed_report {
    ($report:ty, $variant:ident) => {
        impl SealedProductionAnalysisReportV1 for $report {
            fn capture_for_sealed_validation(&self) -> CapturedProductionAnalysisReportV1 {
                CapturedProductionAnalysisReportV1::$variant(self.clone())
            }
        }
    };
}

impl_sealed_report!(PlironTensorLayoutReportV1, TensorLayout);
impl_sealed_report!(RankedBoundsReportV1, Bounds);
impl_sealed_report!(PlironAtomicLegalityReportV1, Atomic);
impl_sealed_report!(RankedRaceReportV1, Race);
impl_sealed_report!(HierarchicalOwnershipReportV1, Ownership);
impl_sealed_report!(PlironBarrierReportV1, Barrier);
impl_sealed_report!(PlironPipelineProtocolReportV1, Pipeline);
impl_sealed_report!(PlironWorkgroupMemoryReportV1, Workgroup);
impl_sealed_report!(PlironSemanticRefinementReportV1, Semantic);

impl<'a> ProductionAnalysisReportValidationSessionV1<'a> {
    pub(crate) fn record<R: SealedProductionAnalysisReportV1>(
        &mut self,
        context: &Context,
        function: &FuncOp,
        checkpoint: PlironPassCheckpointTokenV1,
        report: &R,
    ) -> Result<(), ProductionAnalysisReportValidationErrorV1> {
        let bound = self.issue(
            context,
            function,
            checkpoint,
            report.capture_for_sealed_validation(),
        )?;
        self.accept(context, function, &bound)
    }

    pub(crate) fn finish_validation(
        self,
        preservation: &PlironPassPreservationReportV1,
    ) -> Result<ProductionAnalysisReportValidationV1, ProductionAnalysisReportValidationErrorV1>
    {
        self.finish(preservation)
    }
}
#[cfg(test)]
mod tests {
    use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
    use dialect_kernel::{
        DIALECT_NAME, InvocationIndexOp, ReturnOp, TensorConvergenceAttr, TensorLayoutOp,
        register_dialect,
    };
    use fe2o3_kernel_ir::TensorLayoutContractV1;
    use fe2o3_pliron_owner_core::ensure_context_identity;
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
    };
    use std::{cell::Cell, rc::Rc, sync::Arc};

    use super::*;
    use crate::pliron_ir_identity::BuiltIdentityV1;
    use crate::pliron_pass_contract::{
        IdentityCaptureFailureV1, IdentityComparisonFailureV1, MutationEpochCaptureFailureV1,
        PlironStructuralIdentityProviderV1,
    };
    use crate::{
        LivePlironStructuralIdentityProviderV1, PlironPassContractSessionV1,
        begin_production_pliron_pass_contract_session_v1,
        require_production_pliron_checks_before_lowering_v2, run_pliron_atomic_legality_check_v1,
        run_pliron_ranked_bounds_check_v1, run_pliron_ranked_race_check_v1,
        run_pliron_tensor_layout_check_v1,
    };

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(
            &mut context,
            &DialectName::try_new(DIALECT_NAME).expect("valid kernel dialect name"),
        )
        .expect("register kernel dialect");
        dialect_gpu::register_dialect(&mut context).expect("register gpu dialect");
        ensure_context_identity(&mut context).expect("context identity");
        context
    }

    fn valid_function(context: &mut Context, name: &str) -> FuncOp {
        let function = FuncOp::new(
            context,
            name.try_into().expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        );
        let entry = function.get_entry_block(context);
        for operation in [
            ExecutionLayoutOp::new_with_domain(
                context,
                7,
                [64, 1, 1],
                [64, 1, 1],
                64,
                ExecutionDomainAttr::FullPhysicalWorkgroups,
            )
            .get_operation(),
            InvocationIndexOp::new(context, 0, 64).get_operation(),
            TensorLayoutOp::new(
                context,
                &TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                TensorConvergenceAttr::UniformSubgroup,
                64,
            )
            .get_operation(),
            ReturnOp::new(context).get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        function
    }

    fn bare_function(context: &mut Context, name: &str) -> FuncOp {
        let function = FuncOp::new(
            context,
            name.try_into().expect("valid function name"),
            FunctionType::get(context, vec![], vec![]),
        );
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(function.get_entry_block(context), context);
        function
    }

    type LivePassSession<'a> =
        PlironPassContractSessionV1<LivePlironStructuralIdentityProviderV1<'a>>;

    fn first_bound<'a>(
        context: &'a Context,
        function: &'a FuncOp,
    ) -> (
        LivePassSession<'a>,
        ProductionAnalysisReportValidationSessionV1<'a>,
        BoundProductionAnalysisReportV1,
    ) {
        let provider = LivePlironStructuralIdentityProviderV1::new(context, function);
        let mut preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("identity session");
        let validation_handle = preservation.validation_handle();
        let validation = begin_production_analysis_report_validation_v1(
            context,
            function,
            None,
            validation_handle,
        );
        let report = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                Ok::<_, ()>(run_pliron_tensor_layout_check_v1(context, function))
            })
            .expect("preserved pass")
            .expect("analysis result");
        let checkpoint = preservation.last_checkpoint().expect("checkpoint");
        let bound = validation
            .issue(
                context,
                function,
                checkpoint,
                CapturedProductionAnalysisReportV1::TensorLayout(report),
            )
            .expect("sealed first report");
        (preservation, validation, bound)
    }

    #[test]
    fn production_happy_path_completes_only_supported_witness_fragments() {
        let context = &mut setup();
        let function = valid_function(context, "validation_happy");
        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("ordinary policy pipeline remains usable");
        let validation = report.report_validation();

        assert_eq!(validation.stages().len(), 9);
        assert_eq!(validation.status(), KernelCheckStatusV1::Incomplete);
        assert!(!validation.all_reports_independently_validated());
        assert!(!validation.grants_compiler_refinement_authority());
        assert!(!validation.grants_lowering_or_launch_authority());
        for (position, stage) in validation.stages().iter().enumerate() {
            assert_eq!(stage.checkpoint().position(), position);
            assert_eq!(
                stage.checkpoint().pass(),
                PRODUCTION_PLIRON_PASS_CONTRACTS_V1[position].pass()
            );
            assert_eq!(stage.implementation().pass(), stage.checkpoint().pass());
            assert_eq!(stage.analysis_status(), KernelCheckStatusV1::Clean);
            let expected_status =
                if stage.checkpoint().pass() == KernelCheckPassKindV1::MemoryBounds {
                    KernelCheckStatusV1::Clean
                } else {
                    KernelCheckStatusV1::Incomplete
                };
            assert_eq!(stage.independent_validation_status(), expected_status);
            if expected_status == KernelCheckStatusV1::Clean {
                assert_eq!(stage.remaining_witness_gap(), None);
            } else {
                let gap = stage
                    .remaining_witness_gap()
                    .expect("remaining witness gap");
                assert_eq!(gap.pass(), stage.checkpoint().pass());
                assert!(!gap.required_evidence().is_empty());
            }
            assert!(!stage.witness().grants_compiler_refinement_authority());
            assert!(!stage.witness().grants_lowering_or_launch_authority());
        }
        assert_eq!(
            validation.stages()[2].configuration(),
            &ProductionAnalysisConfigurationV1::AtomicTargetAgnostic
        );
    }

    #[test]
    fn unchanged_report_and_checkpoint_are_accepted_without_a_second_snapshot() {
        let context = &mut setup();
        let function = valid_function(context, "validation_noop");
        let (_preservation, mut validation, bound) = first_bound(context, &function);
        validation
            .accept(context, &function, &bound)
            .expect("unaltered compiler-owned seal");
        assert_eq!(validation.next, 1);
        assert_eq!(
            validation.stages[0].analysis_status(),
            KernelCheckStatusV1::Clean
        );
    }

    #[test]
    fn cross_session_and_stale_seals_are_rejected() {
        let context = &mut setup();
        let function = valid_function(context, "validation_stale");
        let (_first_preservation, _first_validation, bound) = first_bound(context, &function);

        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let second_preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("second session");
        let mut second_validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            second_preservation.validation_handle(),
        );
        assert!(matches!(
            second_validation.accept(context, &function, &bound),
            Err(
                ProductionAnalysisReportValidationErrorV1::CounterfeitOrCrossSessionSeal {
                    position: 0
                }
            )
        ));
    }

    #[test]
    fn cross_context_and_cross_function_use_are_rejected() {
        let first_context = &mut setup();
        let first_function = valid_function(first_context, "same_text");
        let other_function = valid_function(first_context, "other_function");
        let (_preservation, mut validation, bound) = first_bound(first_context, &first_function);

        let second_context = &mut setup();
        let second_function = valid_function(second_context, "same_text");
        assert!(matches!(
            validation.accept(second_context, &second_function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::CrossContextReport { position: 0 })
        ));

        assert!(matches!(
            validation.accept(first_context, &other_function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::CrossFunctionReport { position: 0 })
        ));
    }

    #[test]
    fn replay_and_cross_stage_report_swaps_are_rejected() {
        let context = &mut setup();
        let function = valid_function(context, "validation_replay");
        let (mut preservation, mut validation, first) = first_bound(context, &function);
        let wrong_report = first.issued_report.clone();
        validation
            .accept(context, &function, &first)
            .expect("first use accepted");
        assert!(matches!(
            validation.accept(context, &function, &first),
            Err(ProductionAnalysisReportValidationErrorV1::ReplayedReport {
                issued_position: 0,
                current_position: 1
            })
        ));

        preservation
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || Ok::<_, ()>(()))
            .expect("preserved bounds checkpoint")
            .expect("pass result");
        let swapped = validation
            .issue(
                context,
                &function,
                preservation.last_checkpoint().expect("bounds checkpoint"),
                wrong_report,
            )
            .expect("seal records the actual submitted report kind");
        assert!(matches!(
            validation.accept(context, &function, &swapped),
            Err(
                ProductionAnalysisReportValidationErrorV1::StageOrderMismatch {
                    position: 1,
                    expected: KernelCheckPassKindV1::MemoryBounds,
                    observed: KernelCheckPassKindV1::TensorLayout,
                }
            )
        ));
    }

    #[test]
    fn checkpoint_implementation_configuration_payload_and_status_tampering_are_rejected() {
        let context = &mut setup();
        let function = valid_function(context, "validation_tamper");

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.submitted_checkpoint.pass = KernelCheckPassKindV1::MemoryBounds;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(
                ProductionAnalysisReportValidationErrorV1::CheckpointMetadataTampered {
                    position: 0
                }
            )
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.submitted_checkpoint.mutation_epoch =
            bound.submitted_checkpoint.mutation_epoch.wrapping_add(1);
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(
                ProductionAnalysisReportValidationErrorV1::CheckpointMetadataTampered {
                    position: 0
                }
            )
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.implementation = ProductionAnalysisImplementationV1::PlironRankedBoundsV1;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ImplementationTampered { position: 0 })
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.configuration = ProductionAnalysisConfigurationV1::AtomicTargetAgnostic;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ConfigurationTampered { position: 0 })
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.submitted_report = CapturedProductionAnalysisReportV1::Bounds(
            run_pliron_ranked_bounds_check_v1(context, &function),
        );
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ReportPayloadTampered { position: 0 })
        ));

        let (_preservation, mut validation, mut bound) = first_bound(context, &function);
        bound.claimed_status = KernelCheckStatusV1::Incomplete;
        assert!(matches!(
            validation.accept(context, &function, &bound),
            Err(ProductionAnalysisReportValidationErrorV1::ReportStatusTampered { position: 0 })
        ));
    }

    #[test]
    fn omitted_report_is_terminal_before_manifest_admission() {
        let context = &mut setup();
        let function = valid_function(context, "validation_omitted");
        let completed = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("completed reference run");
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("new session");
        let validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            preservation.validation_handle(),
        );
        assert!(matches!(
            validation.finish(completed.preservation()),
            Err(ProductionAnalysisReportValidationErrorV1::OmittedReport {
                position: 0,
                pass: KernelCheckPassKindV1::TensorLayout,
            })
        ));
    }

    #[test]
    fn non_clean_report_remains_non_clean_and_independently_incomplete() {
        let context = &mut setup();
        let function = bare_function(context, "validation_negative");
        let provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let mut preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("identity session");
        let mut validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            preservation.validation_handle(),
        );

        let tensor = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                Ok::<_, ()>(run_pliron_tensor_layout_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &tensor,
            )
            .unwrap();
        let bounds = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || {
                Ok::<_, ()>(run_pliron_ranked_bounds_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &bounds,
            )
            .unwrap();
        let atomics = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::AtomicLegality, || {
                Ok::<_, ()>(run_pliron_atomic_legality_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &atomics,
            )
            .unwrap();
        let race = preservation
            .run_contiguous_pass(KernelCheckPassKindV1::RaceFreedom, || {
                Ok::<_, ()>(run_pliron_ranked_race_check_v1(context, &function))
            })
            .unwrap()
            .unwrap();
        let observed_status = race.status();
        validation
            .record(
                context,
                &function,
                preservation.last_checkpoint().unwrap(),
                &race,
            )
            .unwrap();
        assert_eq!(validation.stages[3].analysis_status(), observed_status);
        let mut rejected = validation.stages[3].clone();
        rejected.analysis_status = KernelCheckStatusV1::Rejected;
        assert_eq!(
            rejected.independent_validation_status(),
            KernelCheckStatusV1::Rejected
        );
        assert_eq!(
            validation.stages[3].independent_validation_status(),
            KernelCheckStatusV1::Incomplete
        );
    }

    struct CountingProvider<'a> {
        inner: LivePlironStructuralIdentityProviderV1<'a>,
        captures: Rc<Cell<usize>>,
    }

    impl PlironStructuralIdentityProviderV1 for CountingProvider<'_> {
        type Snapshot = BuiltIdentityV1;

        fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
            self.inner.mutation_epoch()
        }

        fn capture(&mut self) -> Result<Self::Snapshot, IdentityCaptureFailureV1> {
            self.captures.set(self.captures.get() + 1);
            self.inner.capture()
        }

        fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
            self.inner.label(snapshot)
        }

        fn require_exact_identity(
            &self,
            expected: &Self::Snapshot,
            observed: &Self::Snapshot,
        ) -> Result<(), IdentityComparisonFailureV1> {
            self.inner.require_exact_identity(expected, observed)
        }

        fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]> {
            self.inner.retain_exact_identity(snapshot)
        }
    }

    #[test]
    fn complete_report_validation_keeps_structural_capture_count_at_nine() {
        let context = &mut setup();
        let function = valid_function(context, "validation_capture_count");
        let reports = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("source reports");
        let captures = Rc::new(Cell::new(0));
        let provider = CountingProvider {
            inner: LivePlironStructuralIdentityProviderV1::new(context, &function),
            captures: Rc::clone(&captures),
        };
        let mut preservation =
            begin_production_pliron_pass_contract_session_v1(provider).expect("identity session");
        let mut validation = begin_production_analysis_report_validation_v1(
            context,
            &function,
            None,
            preservation.validation_handle(),
        );

        macro_rules! checkpoint_report {
            ($pass:expr, $report:expr) => {{
                preservation
                    .run_contiguous_pass($pass, || Ok::<_, ()>(()))
                    .expect("exact checkpoint")
                    .expect("pass result");
                validation
                    .record(
                        context,
                        &function,
                        preservation.last_checkpoint().expect("checkpoint token"),
                        $report,
                    )
                    .expect("sealed report");
            }};
        }
        checkpoint_report!(KernelCheckPassKindV1::TensorLayout, reports.tensor_layout());
        checkpoint_report!(KernelCheckPassKindV1::MemoryBounds, reports.bounds());
        checkpoint_report!(KernelCheckPassKindV1::AtomicLegality, reports.atomics());
        checkpoint_report!(KernelCheckPassKindV1::RaceFreedom, reports.race());
        checkpoint_report!(
            KernelCheckPassKindV1::HierarchicalOwnership,
            reports.ownership()
        );
        checkpoint_report!(
            KernelCheckPassKindV1::BarrierConvergence,
            reports.barriers()
        );
        checkpoint_report!(
            KernelCheckPassKindV1::PipelineProtocol,
            reports.pipeline_protocol()
        );
        checkpoint_report!(KernelCheckPassKindV1::WorkgroupMemory, reports.workgroup());
        checkpoint_report!(
            KernelCheckPassKindV1::SemanticRefinement,
            reports.semantics()
        );

        let preservation = preservation.finish().expect("complete preservation");
        validation
            .finish_validation(&preservation)
            .expect("custody-bound validation");
        assert_eq!(
            captures.get(),
            1 + PRODUCTION_ANALYSIS_REPORT_COUNT_V1,
            "report sealing must not add structural identity captures"
        );
    }
}
