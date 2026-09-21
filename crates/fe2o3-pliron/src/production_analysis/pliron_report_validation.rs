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

use super::pliron_report_payload_receipt::{
    ProductionAnalysisReportPayloadReceiptV1, payload_limit_v1,
};
use crate::production_analysis::pliron_analysis_manager::PlironAnalysisManagerV1;
use crate::production_analysis::pliron_analysis_witness::preflight_production_analysis_witness_resource_upper_bound_v1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
    ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    ProductionAnalysisResourceUpperBoundV1,
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
    fn try_clone_payload_v1(
        &self,
        receipt: ProductionAnalysisReportPayloadReceiptV1,
    ) -> Result<Self, ProductionAnalysisResourceLimitV1> {
        receipt.require_pass(self.pass())?;
        if !receipt.is_exact() {
            return Ok(self.clone());
        }
        Ok(match self {
            Self::TensorLayout(report) => {
                Self::TensorLayout(report.try_clone_validation_payload_v1()?)
            }
            Self::Bounds(report) => Self::Bounds(report.try_clone_validation_payload_v1()?),
            Self::Atomic(report) => Self::Atomic(report.try_clone_validation_payload_v1()?),
            Self::Race(report) => Self::Race(report.try_clone_validation_payload_v1()?),
            Self::Barrier(report) => Self::Barrier(report.try_clone_validation_payload_v1()?),
            Self::Workgroup(report) => Self::Workgroup(report.try_clone_validation_payload_v1()?),
            Self::Semantic(report) => Self::Semantic(report.try_clone_validation_payload_v1()?),
            Self::Ownership(_) | Self::Pipeline(_) => {
                return Err(payload_limit_v1("unsupported exact report payload"));
            }
        })
    }

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
    payload_receipt: ProductionAnalysisReportPayloadReceiptV1,
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
    ResourceLimit {
        producing_pass: Option<KernelCheckPassKindV1>,
        resource: &'static str,
    },
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
            Self::ResourceLimit { .. } => "FE2O3-PRESERVE-045",
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
            Self::ResourceLimit { producing_pass, resource } => {
                formatter.write_str("report-validation resource limit was exceeded")?;
                if let Some(pass) = producing_pass {
                    write!(formatter, " for {}", pass.name())?;
                }
                write!(formatter, ": {resource}")
            }
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
    input_census: ProductionAnalysisInputCensusV1,
    next: usize,
    stages: Vec<ProductionAnalysisStageValidationV1>,
    setup_resource_upper_bound: ProductionAnalysisResourceUpperBoundV1,
    last_stage_resource_upper_bound: Option<ProductionAnalysisResourceUpperBoundV1>,
    _subject_borrow: PhantomData<(&'a Context, &'a FuncOp)>,
}

include!("pliron_report_validation/session_v1.rs");
#[path = "pliron_report_validation/conditional_v1.rs"]
pub(crate) mod conditional_validation_v1;
include!("pliron_report_validation/resource_tests.rs");
