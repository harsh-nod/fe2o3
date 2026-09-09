//! Single production-pipeline transaction shell.
//!
//! This module owns the one integration point for issue #175. It deliberately
//! contains no workload recognition. The sole semantic-MIR importer owns the
//! consuming target-authentication boundary and moves an admitted request into
//! a typed stage before the mandatory generic kernel-verification pipeline.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::marker::PhantomData;
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};

use rustc_middle::ty::TyCtxt;
use sha2::{Digest as _, Sha256};

use crate::artifact_transaction::{BuildAttempt, ProducerIdentity};
use crate::collector::AuthenticatedCollectedKernelClosureV1;
use crate::protected_compiler_execution::{
    AdmittedProtectedCompilerExecutionV1, ProtectedCompilerExecutionErrorV1,
};
use crate::protected_rustc_invocation::{
    AdmittedProtectedRustcInvocationV1, ProtectedRustcInvocationErrorV1,
};

#[path = "production_bundle_transaction_v8.rs"]
mod production_bundle_transaction_v8;

#[path = "production_pipeline_tutorial_transaction_v1.rs"]
mod tutorial_transaction_v1;

pub use tutorial_transaction_v1::{
    TutorialProductionTransactionErrorCodeV1, TutorialProductionTransactionErrorV1,
    TutorialProductionTransactionReceiptV1,
    prepare_tutorial_capability_qualification_transaction_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionDisposition {
    HostOnly,
    DeviceTransaction,
}

pub(crate) const fn disposition(device_candidate_count: usize) -> ProductionDisposition {
    if device_candidate_count == 0 {
        ProductionDisposition::HostOnly
    } else {
        ProductionDisposition::DeviceTransaction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProductionFinalGraphVerificationErrorV13 {
    MissingResult {
        stage: &'static str,
    },
    NonProductionOptimizer,
    StaleResult {
        stage: &'static str,
        expected_epoch: u64,
        observed_epoch: u64,
    },
    SubjectMismatch {
        stage: &'static str,
    },
    EmptyTargetCapabilityClosure,
}

impl fmt::Display for ProductionFinalGraphVerificationErrorV13 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingResult { stage } => {
                write!(formatter, "missing required {stage} result")
            }
            Self::NonProductionOptimizer => formatter
                .write_str("KIR V13 optimization did not use the exact fixed production V6 policy"),
            Self::StaleResult {
                stage,
                expected_epoch,
                observed_epoch,
            } => write!(
                formatter,
                "{stage} names stale graph epoch {observed_epoch}; expected {expected_epoch}",
            ),
            Self::SubjectMismatch { stage } => {
                write!(
                    formatter,
                    "{stage} does not name the exact transaction graph"
                )
            }
            Self::EmptyTargetCapabilityClosure => {
                formatter.write_str("target-capability closure contains no exact target decisions")
            }
        }
    }
}

impl std::error::Error for ProductionFinalGraphVerificationErrorV13 {}

#[derive(Debug)]
pub(crate) enum ProductionPipelineError {
    CustomLlvmConfiguration,
    EmptyCollectedDeviceClosure,
    SemanticImport(crate::collector::ProductionSemanticImportErrorV1),
    SemanticMiddleEnd(fe2o3_pliron::ProductionSemanticMirErrorV1),
    SemanticSsa(fe2o3_pliron::ProductionSemanticSsaErrorV1),
    RankedProjection(crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1),
    RankedVerification(crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1),
    TargetNeutralLowering(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1),
    MissingMirPlironTranslationValidation,
    SimulationKernelIrV7(fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV7),
    SimulationBundle(fe2o3_kernel_ir::SimulationBundleErrorV1),
    SimulationDebugMap(fe2o3_kernel_ir::DebugSourceMapErrorV1),
    SimulationBundleV2(fe2o3_kernel_ir::SimulationBundleErrorV2),
    SimulationBundleV3(fe2o3_kernel_ir::SimulationBundleErrorV3),
    SimulationBundleV4(fe2o3_kernel_ir::SimulationBundleErrorV4),
    SimulationBundleV5(fe2o3_kernel_ir::SimulationBundleErrorV5),
    SimulationBundleV6(fe2o3_kernel_ir::SimulationBundleErrorV6),
    SimulationBundleV8(fe2o3_kernel_ir::SimulationBundleErrorV8),
    ProductionBundleTransactionV8(
        production_bundle_transaction_v8::ProductionBundleTransactionErrorV8,
    ),
    SimulationDebugMapV2(fe2o3_kernel_ir::DebugSourceMapErrorV2),
    SimulationDebugSourceCaptureUnavailable(fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1),
    SimulationDebugMapCorrespondence(&'static str),
    SemanticDebugMap(fe2o3_kernel_ir::SemanticDebugMapErrorV1),
    SemanticDebugFragment(fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1),
    SimulationSourceLineage(fe2o3_compiler_lineage::LineageErrorV3),
    SimulationProductionKirV9,
    SimulationProductionKirV11,
    SimulationProductionKirV12,
    SimulationProductionKirV13,
    FormalMemoryAdmission(fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1),
    Geometry(crate::production_geometry_v1::ProductionGeometryErrorV1),
    TargetBackend(crate::production_backend_v1::ProductionBackendErrorV1),
    TargetOptimizationV6(fe2o3_kernel_opt::KernelIrTargetNeutralOptimizationErrorV6),
    TargetOptimizationReplayV6(
        fe2o3_kernel_opt::KernelIrTargetNeutralStructuralReplayAdmissionErrorV6,
    ),
    TargetCapabilityAnalysis(fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV13),
    FinalGraphVerificationV13(ProductionFinalGraphVerificationErrorV13),
    FinalGraphExecution(fe2o3_pliron::ProductionFinalGraphVerificationErrorV1),
    FinalGraphCapabilityNonClean(Box<fe2o3_kernel_analysis::ProductionW4NonCleanResultV1>),
    DescriptorEvidence(crate::compiler_descriptor::CompilerDescriptorError),
    SemanticLineage(crate::production_semantic_lineage_v3::ProductionSemanticLineageErrorV3),
    FunctionalRefinementCustody(&'static str),
    FinalGraphFunctionalRuntime(fe2o3_verifier::FunctionalRefinementRuntimeErrorV1),
    FinalGraphFunctionalRefinement(fe2o3_verifier::ProductionFinalGraphFunctionalRefinementErrorV1),
    RustcLineageMismatch,
    ProtectedRustcInvocation(ProtectedRustcInvocationErrorV1),
    ProtectedCompilerExecution(ProtectedCompilerExecutionErrorV1),
    ExtractionCannotPublish,
    WorkerHandoffExtractionRequiresExtractionCustody,
    WorkerHandoff(crate::production_worker_handoff::ProductionWorkerHandoffError),
    CapabilityV5Publication(fe2o3_artifact_transaction::CompilerCapabilityHandoffErrorV5),
    CompilerExecutionSubject(fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV1),
    CompilerExecutionReceiptTransport(
        fe2o3_artifact_transaction::CompilerExecutionReceiptTransportErrorV1,
    ),
    CompilerExecutionReceiptTransportBindingMismatch,
    ProtectedCompilerCompletionPackage(String),
}

impl fmt::Display for ProductionPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CustomLlvmConfiguration => formatter.write_str(
                "production compilation rejects caller-selected LLVM arguments or passes before transaction construction",
            ),
            Self::EmptyCollectedDeviceClosure => formatter.write_str(
                "production compilation requires a nonempty collector-sealed device closure",
            ),
            Self::SemanticImport(error) => write!(formatter, "production compilation {error}"),
            Self::SemanticMiddleEnd(error) => {
                write!(formatter, "production compilation exact semantic middle end failed: {error}")
            }
            Self::SemanticSsa(error) => {
                write!(formatter, "production compilation semantic SSA planning failed: {error}")
            }
            Self::RankedProjection(error) => {
                write!(formatter, "production compilation general kernel verification failed: {error}")
            }
            Self::RankedVerification(error) => {
                write!(formatter, "production compilation ranked verification failed: {error}")
            }
            Self::TargetNeutralLowering(error) => {
                write!(formatter, "production compilation target-neutral lowering failed: {error}")
            }
            Self::MissingMirPlironTranslationValidation => formatter.write_str(
                "production compilation reached target-neutral custody without independent MIR-to-PLIRON translation validation",
            ),
            Self::SimulationKernelIrV7(error) => write!(
                formatter,
                "production compilation cannot project the already-lowered module to exact simulation Kernel IR V7: {error}"
            ),
            Self::SimulationBundle(error) => {
                write!(formatter, "production compilation simulation bundle failed: {error}")
            }
            Self::SimulationDebugMap(error) => write!(
                formatter,
                "production compilation simulation debug map failed: {error}"
            ),
            Self::SimulationBundleV2(error) => write!(
                formatter,
                "production compilation simulation bundle V2 failed: {error}"
            ),
            Self::SimulationBundleV3(error) => write!(
                formatter,
                "production compilation simulation bundle V3 failed: {error}"
            ),
            Self::SimulationBundleV4(error) => write!(
                formatter,
                "production compilation simulation bundle V4 failed: {error}"
            ),
            Self::SimulationBundleV5(error) => write!(
                formatter,
                "production compilation simulation bundle V5 failed: {error}"
            ),
            Self::SimulationBundleV6(error) => write!(
                formatter,
                "production compilation simulation bundle V6 failed: {error}"
            ),
            Self::SimulationBundleV8(error) => write!(
                formatter,
                "production compilation simulation bundle V8 failed: {error}"
            ),
            Self::ProductionBundleTransactionV8(error) => write!(
                formatter,
                "production compilation Bundle V8 transaction binding failed: {error}"
            ),
            Self::SimulationDebugMapV2(error) => write!(
                formatter,
                "production compilation simulation debug map V2 failed: {error}"
            ),
            Self::SimulationDebugSourceCaptureUnavailable(gap) => write!(
                formatter,
                "production compilation simulation bundle V2 source-variable names and observations are incomplete ({gap:?}); explicit V2 export fails closed"
            ),
            Self::SimulationDebugMapCorrespondence(detail) => write!(
                formatter,
                "production compilation simulation debug-map correspondence failed: {detail}"
            ),
            Self::SemanticDebugMap(error) => write!(
                formatter,
                "production semantic debug map construction failed: {error}"
            ),
            Self::SemanticDebugFragment(error) => write!(
                formatter,
                "production semantic debug fragment construction failed: {error}"
            ),
            Self::SimulationSourceLineage(error) => write!(
                formatter,
                "production compilation simulation source-lineage receipt failed: {error}"
            ),
            Self::SimulationProductionKirV9 => formatter.write_str(
                "production Kernel IR V9 is not representable by the exact V7 CPU simulator; no downgrade or hardware fallback was attempted",
            ),
            Self::SimulationProductionKirV11 => formatter.write_str(
                "production Kernel IR V11 is not representable by the exact V7/V10 simulation projections; no downgrade or hardware fallback was attempted",
            ),
            Self::SimulationProductionKirV12 => formatter.write_str(
                "production Kernel IR V12 is not representable by a legacy simulation projection; no downgrade or hardware fallback was attempted",
            ),
            Self::SimulationProductionKirV13 => formatter.write_str(
                "production Kernel IR V13 is not representable by a legacy simulation projection; no downgrade or hardware fallback was attempted",
            ),
            Self::FormalMemoryAdmission(error) => {
                write!(formatter, "production compilation formal memory admission failed: {error}")
            }
            Self::Geometry(error) => {
                write!(formatter, "production compilation geometry validation failed: {error}")
            }
            Self::TargetBackend(error) => {
                write!(formatter, "production compilation target backend failed: {error}")
            }
            Self::TargetOptimizationV6(error) => write!(
                formatter,
                "production compilation canonical KIR V13 V6 optimization failed: {error}"
            ),
            Self::TargetOptimizationReplayV6(error) => write!(
                formatter,
                "production compilation canonical KIR V13 V6 structural replay failed: {error}"
            ),
            Self::TargetCapabilityAnalysis(error) => write!(
                formatter,
                "production compilation target-binding capability analysis failed: {error}"
            ),
            Self::FinalGraphVerificationV13(error) => write!(
                formatter,
                "production compilation canonical KIR V13 final-graph verification failed: {error}"
            ),
            Self::FinalGraphExecution(error) => write!(
                formatter,
                "production compilation canonical KIR V13 PLIRON verification failed: {error}"
            ),
            Self::FinalGraphCapabilityNonClean(result) => write!(
                formatter,
                "production compilation W4 capability analysis failed closed: {result}"
            ),
            Self::DescriptorEvidence(error) => {
                write!(formatter, "production compilation descriptor evidence failed: {error}")
            }
            Self::SemanticLineage(error) => write!(formatter, "production compilation {error}"),
            Self::FunctionalRefinementCustody(detail) => write!(
                formatter,
                "production functional-refinement custody is incomplete: {detail}",
            ),
            Self::FinalGraphFunctionalRuntime(error) => write!(
                formatter,
                "production final-graph functional-refinement runtime failed: {error}",
            ),
            Self::FinalGraphFunctionalRefinement(error) => write!(
                formatter,
                "production final-graph functional-refinement proof failed: {error}",
            ),
            Self::RustcLineageMismatch => formatter.write_str(
                "production compilation rustc preflight plan is not bound to the retained identity inventory",
            ),
            Self::ProtectedRustcInvocation(error) => write!(
                formatter,
                "production compilation final protected rustc invocation validation failed: {error}"
            ),
            Self::ProtectedCompilerExecution(error) => write!(
                formatter,
                "production compilation protected compiler execution failed: {error}"
            ),
            Self::ExtractionCannotPublish => formatter.write_str(
                "production extraction custody cannot publish a compiler-module handoff",
            ),
            Self::WorkerHandoffExtractionRequiresExtractionCustody => formatter.write_str(
                "inert compiler-module extraction requires extraction-only custody",
            ),
            Self::WorkerHandoff(error) => {
                write!(formatter, "production compilation compiler-module handoff failed: {error}")
            }
            Self::CapabilityV5Publication(error) => {
                write!(formatter, "production compilation protected V5 publication failed: {error}")
            }
            Self::CompilerExecutionSubject(error) => write!(
                formatter,
                "production compilation compiler-execution subject failed: {error}"
            ),
            Self::CompilerExecutionReceiptTransport(error) => write!(
                formatter,
                "production compilation compiler-execution receipt transport failed: {error}"
            ),
            Self::CompilerExecutionReceiptTransportBindingMismatch => formatter.write_str(
                "production compilation compiler-execution receipt transport changed its exact subject or byte length",
            ),
            Self::ProtectedCompilerCompletionPackage(error) => write!(
                formatter,
                "production compilation protected compiler-completion package failed: {error}",
            ),
        }
    }
}

impl std::error::Error for ProductionPipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticImport(error) => Some(error),
            Self::SemanticMiddleEnd(error) => Some(error),
            Self::SemanticSsa(error) => Some(error),
            Self::RankedProjection(error) => Some(error),
            Self::RankedVerification(error) => Some(error),
            Self::TargetNeutralLowering(error) => Some(error),
            Self::SimulationKernelIrV7(error) => Some(error),
            Self::SimulationBundle(error) => Some(error),
            Self::SimulationDebugMap(error) => Some(error),
            Self::SimulationBundleV2(error) => Some(error),
            Self::SimulationBundleV3(error) => Some(error),
            Self::SimulationBundleV4(error) => Some(error),
            Self::SimulationBundleV5(error) => Some(error),
            Self::SimulationBundleV6(error) => Some(error),
            Self::SimulationBundleV8(error) => Some(error),
            Self::ProductionBundleTransactionV8(error) => Some(error),
            Self::SimulationDebugMapV2(error) => Some(error),
            Self::SemanticDebugMap(error) => Some(error),
            Self::SemanticDebugFragment(error) => Some(error),
            Self::SimulationSourceLineage(error) => Some(error),
            Self::FormalMemoryAdmission(error) => Some(error),
            Self::Geometry(error) => Some(error),
            Self::TargetBackend(error) => Some(error),
            Self::TargetOptimizationV6(error) => Some(error),
            Self::TargetOptimizationReplayV6(error) => Some(error),
            Self::TargetCapabilityAnalysis(error) => Some(error),
            Self::FinalGraphVerificationV13(error) => Some(error),
            Self::FinalGraphExecution(error) => Some(error),
            Self::DescriptorEvidence(error) => Some(error),
            Self::SemanticLineage(error) => Some(error),
            Self::FinalGraphFunctionalRuntime(error) => Some(error),
            Self::FinalGraphFunctionalRefinement(error) => Some(error),
            Self::ProtectedRustcInvocation(error) => Some(error),
            Self::ProtectedCompilerExecution(error) => Some(error),
            Self::WorkerHandoff(error) => Some(error),
            Self::CapabilityV5Publication(error) => Some(error),
            Self::CompilerExecutionSubject(error) => Some(error),
            Self::CompilerExecutionReceiptTransport(error) => Some(error),
            Self::CustomLlvmConfiguration
            | Self::EmptyCollectedDeviceClosure
            | Self::MissingMirPlironTranslationValidation
            | Self::RustcLineageMismatch
            | Self::SimulationProductionKirV9
            | Self::SimulationProductionKirV11
            | Self::SimulationProductionKirV12
            | Self::SimulationProductionKirV13
            | Self::SimulationDebugSourceCaptureUnavailable(_)
            | Self::SimulationDebugMapCorrespondence(_)
            | Self::FinalGraphCapabilityNonClean(_)
            | Self::FunctionalRefinementCustody(_)
            | Self::ProtectedCompilerCompletionPackage(_)
            | Self::ExtractionCannotPublish
            | Self::CompilerExecutionReceiptTransportBindingMismatch
            | Self::WorkerHandoffExtractionRequiresExtractionCustody => None,
        }
    }
}

pub(crate) fn reject_custom_llvm_configuration(
    has_custom_llvm_configuration: bool,
) -> Result<(), ProductionPipelineError> {
    if has_custom_llvm_configuration {
        Err(ProductionPipelineError::CustomLlvmConfiguration)
    } else {
        Ok(())
    }
}

pub(super) struct CollectedRustStage<'tcx> {
    tcx: TyCtxt<'tcx>,
    closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
    typed_descriptor_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    debug_source_capture: crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2,
    transaction: ProductionTransactionBindings,
}

struct ProductionTransactionBindings {
    producer: ProducerIdentity,
    output_dir: PathBuf,
    compiler_ffi_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    compiler_custody: ProductionCompilerCustody,
}

enum ProductionCompilerCustody {
    ProtectedV3 {
        invocation: Box<AdmittedProtectedRustcInvocationV1>,
        compiler_execution: Box<AdmittedProtectedCompilerExecutionV1>,
        attempt: BuildAttempt,
    },
    ExtractionOnly,
}

impl ProductionCompilerCustody {
    fn protected(
        invocation: AdmittedProtectedRustcInvocationV1,
        compiler_execution: AdmittedProtectedCompilerExecutionV1,
        attempt: BuildAttempt,
    ) -> Self {
        Self::ProtectedV3 {
            invocation: Box::new(invocation),
            compiler_execution: Box::new(compiler_execution),
            attempt,
        }
    }

    const fn extraction_only() -> Self {
        Self::ExtractionOnly
    }

    fn retained_protected_binding_count(&self) -> usize {
        match self {
            Self::ProtectedV3 { .. } => 2,
            Self::ExtractionOnly => 0,
        }
    }

    fn is_extraction_only(&self) -> bool {
        matches!(self, Self::ExtractionOnly)
    }

    fn into_publication_custody(
        self,
    ) -> Result<ProtectedProductionPublicationCustody, ProductionPipelineError> {
        match self {
            Self::ProtectedV3 {
                invocation,
                compiler_execution,
                attempt,
            } => Ok(ProtectedProductionPublicationCustody {
                attempt,
                invocation,
                compiler_execution,
            }),
            Self::ExtractionOnly => Err(ProductionPipelineError::ExtractionCannotPublish),
        }
    }
}

struct ProtectedProductionPublicationCustody {
    attempt: BuildAttempt,
    invocation: Box<AdmittedProtectedRustcInvocationV1>,
    compiler_execution: Box<AdmittedProtectedCompilerExecutionV1>,
}

struct AuthenticatedProductionBindings {
    rustc_identity_inventory: crate::collector::AuthenticatedRustcIdentityInventoryV3,
    rustc_preflight_plan: crate::collector::AuthenticatedRustcPreflightPlanV3,
    rustc_target: crate::production_target_v1::AuthenticatedProductionTargetV1,
    kernel_contexts: Option<crate::collector::AuthenticatedProductionKernelContextsV1>,
    reference_effect_bindings: crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    debug_source_files: Box<[fe2o3_kernel_ir::DebugSourceMapFileV1]>,
    debug_source_scopes: Box<[crate::rustc_semantic_plan_v1::RetainedDebugSourceScopeV2]>,
    debug_source_variables: Box<[crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableV2]>,
    debug_capture_gap: Option<fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1>,
    typed_descriptor_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    transaction: ProductionTransactionBindings,
}

pub(super) struct AdmittedSemanticMirStage {
    semantic_mir: fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    bindings: AuthenticatedProductionBindings,
}

pub(super) struct EquivalentSemanticMirStage {
    semantic_mir: fe2o3_pliron::ProductionSemanticMirOwnerV1,
    bindings: AuthenticatedProductionBindings,
}

pub(super) struct SsaSemanticMirStage {
    semantic_ssa: fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only owner of one production compilation stage.
///
/// Its fields and stage types stay private so no caller can synthesize or
/// bypass a transition. The transaction carries no artifact, publication,
/// load, launch, or runtime authority.
pub(crate) struct ProductionCompilation<'tcx, Stage> {
    stage: Stage,
    invariant_session: PhantomData<fn(TyCtxt<'tcx>) -> TyCtxt<'tcx>>,
}

/// Move-only production stage retaining rustc identities, transaction
/// bindings, admitted semantic MIR, and the owner-held verified PLIRON graph.
pub(crate) struct RankedVerifiedProductionCompilation {
    ranked: crate::production_ranked_projection_v1::ProductionRankedSemanticProgramV1,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only production stage retaining exact semantic ownership, verified
/// Kernel IR, correspondence evidence, and the original transaction bindings.
pub(crate) struct TargetNeutralProductionCompilation {
    lowered: fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only production stage retaining exact semantic ownership, verified
/// Kernel IR, composed formal/ranked memory evidence, and transaction bindings.
pub(crate) struct FormalMemoryAdmittedProductionCompilation {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
}

/// Move-only production stage retaining formal admission, exact target-bound
/// Kernel IR, deterministic exact-target LLVM text, and transaction bindings.
pub(crate) struct TargetLoweredProductionCompilation {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    target_module: fe2o3_kernel_ir::Module,
    target_optimization: fe2o3_kernel_opt::KernelIrTargetNeutralOptimizationReportV6,
    target_verification: ProductionV13TargetVerificationCustody,
    workgroups: Box<[(String, fe2o3_kernel_ir::WorkgroupSize)]>,
    llvm_ir: String,
    bindings: AuthenticatedProductionBindings,
}

#[derive(Clone, Copy)]
struct ProductionV13CapabilityResults<'a> {
    optimizer: Option<&'a fe2o3_kernel_opt::KernelIrTargetNeutralOptimizationReportV6>,
    structural_replay:
        Option<&'a fe2o3_kernel_opt::KernelIrTargetNeutralStructuralReplayAdmissionV6>,
    target_closure: Option<&'a crate::production_backend_v1::ProductionBackendCapabilityClosureV1>,
}

/// Move-only custody for the exact optimized V13 graph and every result needed
/// before final-graph verification.
struct PreparedV13FinalGraphVerification {
    pre_optimization_canonical: fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
    neutral_identity: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    optimized_identity: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_canonical: fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    target_module: fe2o3_kernel_ir::Module,
    kernel_ids: Box<[fe2o3_kernel_ir::KernelId]>,
    optimizer: fe2o3_kernel_opt::KernelIrTargetNeutralOptimizationReportV6,
    structural_replay: fe2o3_kernel_opt::KernelIrTargetNeutralStructuralReplayAdmissionV6,
    target_closure: crate::production_backend_v1::ProductionBackendCapabilityClosureV1,
}

struct VerifiedV13TargetLoweringInput {
    target_module: fe2o3_kernel_ir::Module,
    kernel_ids: Box<[fe2o3_kernel_ir::KernelId]>,
    target_optimization: fe2o3_kernel_opt::KernelIrTargetNeutralOptimizationReportV6,
    target_verification: ProductionV13TargetVerificationCustody,
}

const FINAL_GRAPH_FUNCTIONAL_RUNTIME_ROOT_V1: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
const FINAL_GRAPH_FUNCTIONAL_TIMEOUT_SECONDS_V1: u32 = 120;
const FINAL_GRAPH_FUNCTIONAL_ROSTER_DOMAIN_V1: &[u8] =
    b"FE2O3/RUSTC-CODEGEN/FINAL-GRAPH-FUNCTIONAL-ROSTER/V1\0";

/// Exact W5 closure and W4 verification result retained as one move-only V13
/// owner. The report is evidence only; custody grants no lowering, publication,
/// artifact, load, or launch authority.
struct ProductionV13TargetVerificationCustody {
    verified_graph: fe2o3_pliron::ProductionVerifiedFinalGraphV1,
    pre_optimization_canonical: fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
    pre_optimization_epoch: u64,
    final_epoch: u64,
    target_closure: crate::production_backend_v1::ProductionBackendCapabilityClosureV1,
}

impl ProductionV13TargetVerificationCustody {
    fn try_new(
        mut verified_graph: fe2o3_pliron::ProductionVerifiedFinalGraphV1,
        pre_optimization_canonical: fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
        pre_optimization_epoch: u64,
        final_epoch: u64,
        target_closure: crate::production_backend_v1::ProductionBackendCapabilityClosureV1,
    ) -> Result<Self, ProductionFinalGraphVerificationErrorV13> {
        verified_graph.revalidate_live().map_err(|_| {
            ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "retained live final KIR V13 owner",
            }
        })?;
        let custody = Self {
            verified_graph,
            pre_optimization_canonical,
            pre_optimization_epoch,
            final_epoch,
            target_closure,
        };
        custody.validate_subject(custody.final_canonical().identity(), final_epoch)?;
        if custody.pre_optimization_canonical.identity() == custody.final_canonical().identity()
            && custody.pre_optimization_epoch != final_epoch
        {
            return Err(ProductionFinalGraphVerificationErrorV13::StaleResult {
                stage: "retained pre-optimization canonical graph",
                expected_epoch: final_epoch,
                observed_epoch: custody.pre_optimization_epoch,
            });
        }
        if custody.pre_optimization_epoch > final_epoch {
            return Err(ProductionFinalGraphVerificationErrorV13::StaleResult {
                stage: "retained pre-optimization graph",
                expected_epoch: final_epoch,
                observed_epoch: custody.pre_optimization_epoch,
            });
        }
        Ok(custody)
    }

    fn validate_subject(
        &self,
        final_graph: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
        final_epoch: u64,
    ) -> Result<(), ProductionFinalGraphVerificationErrorV13> {
        let report = self.verified_graph.report();
        let resources = report.resources();
        let capability = report.capability();
        let bridge = report.bridge();
        let bridge_input = bridge.input();
        let bridge_output = bridge.output();
        let closure_subject = self.target_closure.subject();
        if self.final_canonical().identity() != final_graph
            || self.final_epoch != final_epoch
            || report.final_graph() != final_graph
            || report.final_epoch() != final_epoch
            || resources.final_graph() != final_graph
            || resources.final_epoch() != final_epoch
            || resources.closure_identity() != self.target_closure().identity()
            || closure_subject.semantic_operation_version() != 1
            || closure_subject.canonical_graph_version() != 13
            || closure_subject.digest() != *final_graph.digest()
            || closure_subject.canonical_length() != final_graph.canonical_length()
            || closure_subject.epoch() != final_epoch
            || !self.target_closure.launch_subject_matches()
            || capability.canonical_identity() != final_graph
            || capability.graph_epoch() != final_epoch
            || !bridge.is_exact()
            || bridge_input != bridge_output
            || bridge_input.canonical_bytes() != final_graph.canonical_length()
            || bridge_output.canonical_bytes() != final_graph.canonical_length()
        {
            return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "retained W5 closure and W4 final-graph report",
            });
        }
        Ok(())
    }

    fn validate_target_module(
        &self,
        target_module: &fe2o3_kernel_ir::Module,
    ) -> Result<(), ProductionFinalGraphVerificationErrorV13> {
        let canonical =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(target_module.clone())
                .map_err(
                    |_| ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                        stage: "retained V13 final module",
                    },
                )?;
        self.validate_subject(canonical.identity(), self.final_epoch)
    }

    fn validate_live_owner(&mut self) -> Result<(), ProductionFinalGraphVerificationErrorV13> {
        self.verified_graph.revalidate_live().map_err(|_| {
            ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "retained live final KIR V13 owner",
            }
        })?;
        self.validate_subject(self.final_canonical().identity(), self.final_epoch)
    }

    const fn final_canonical(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13 {
        self.verified_graph.canonical()
    }

    const fn target_closure(
        &self,
    ) -> &crate::production_backend_v1::ProductionBackendCapabilityClosureV1 {
        &self.target_closure
    }

    const fn final_graph_report(&self) -> &fe2o3_pliron::ProductionFinalGraphVerificationReportV1 {
        self.verified_graph.report()
    }

    const fn final_v13_epoch(&self) -> Option<u64> {
        Some(self.final_epoch)
    }

    const fn retained_record_count(&self) -> usize {
        3
    }

    fn validate_retained_records(
        &mut self,
    ) -> Result<(), ProductionFinalGraphVerificationErrorV13> {
        self.validate_live_owner()
    }

    fn execute_w4_capability_witness(
        &mut self,
        subject: fe2o3_kernel_analysis::ProductionW4FinalGraphSubjectV1,
        target_context: &fe2o3_kernel_analysis::ProductionW4TargetResourceInputV1,
    ) -> Result<
        fe2o3_kernel_analysis::ProductionW4FinalGraphCapabilityWitnessV1,
        ProductionPipelineError,
    > {
        let target = {
            let evidence = self
                .target_closure
                .validated_resource_evidence_v1()
                .map_err(ProductionPipelineError::TargetBackend)?;
            if subject.final_graph() != self.final_canonical().identity()
                || subject.final_epoch() != self.final_epoch
                || subject.target_closure_identity() != &evidence.closure_identity()
                || target_context.final_graph() != subject.final_graph()
                || target_context.final_epoch() != subject.final_epoch()
                || target_context.target_identity() != subject.target_identity()
                || target_context.launch_identity() != subject.launch_identity()
                || target_context.closure_identity() != subject.target_closure_identity()
            {
                return Err(ProductionPipelineError::FinalGraphVerificationV13(
                    ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                        stage: "replay-validated W4 target resource context",
                    },
                ));
            }
            crate::production_worker_handoff::retain_production_w4_target_resource_input_v5(
                *subject.final_graph(),
                subject.final_epoch(),
                *subject.target_identity(),
                *subject.launch_identity(),
                evidence.closure_identity(),
                evidence.canonical_closure().to_vec(),
                evidence.decisions().to_vec(),
                target_context.atomic_target().clone(),
                target_context.launch_contract().clone(),
            )
            .map_err(ProductionPipelineError::WorkerHandoff)?
        };
        let execution = self
            .verified_graph
            .execute_w4_capability_witness(subject.clone(), target)
            .map_err(ProductionPipelineError::FinalGraphExecution)?;
        let witness = match execution {
            fe2o3_kernel_analysis::ProductionW4FinalGraphExecutionV1::Complete(witness) => witness,
            fe2o3_kernel_analysis::ProductionW4FinalGraphExecutionV1::NonClean(result) => {
                result.require_exact_subject_v1(&subject).map_err(|_| {
                    ProductionPipelineError::FinalGraphVerificationV13(
                        ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                            stage: "non-clean production W4 diagnostic witness",
                        },
                    )
                })?;
                return Err(ProductionPipelineError::FinalGraphCapabilityNonClean(
                    Box::new(result),
                ));
            }
        };
        self.validate_live_owner()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        Ok(witness)
    }

    fn prepare_simulation_bundle_v8_graph(
        &mut self,
        target_module: &fe2o3_kernel_ir::Module,
        production_identity: fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrIdentityV1,
        target_optimization: &fe2o3_kernel_opt::KernelIrTargetNeutralOptimizationReportV6,
    ) -> Result<(fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13, u64), ProductionPipelineError> {
        self.validate_live_owner()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        self.validate_target_module(target_module)
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        if production_identity.version()
            != fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V13
            || production_identity.digest() != self.pre_optimization_canonical.identity().digest()
            || production_identity.canonical_length()
                != self
                    .pre_optimization_canonical
                    .identity()
                    .canonical_length()
            || target_optimization.initial_epoch() != self.pre_optimization_epoch
            || target_optimization.final_epoch() != self.final_epoch
        {
            return Err(ProductionPipelineError::FinalGraphVerificationV13(
                ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                    stage: "V8 production graph and optimization epoch custody",
                },
            ));
        }

        let canonical = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_canonical_bytes(
            self.final_canonical().canonical_bytes().to_vec(),
        )
        .map_err(ProductionPipelineError::TargetCapabilityAnalysis)?;
        let report = self.verified_graph.report();
        let resources = report.resources();
        let closure_subject = self.target_closure.subject();
        if self.verified_graph.module() != target_module
            || report.final_graph() != canonical.identity()
            || report.final_epoch() != self.final_epoch
            || resources.final_graph() != canonical.identity()
            || resources.final_epoch() != self.final_epoch
            || resources.closure_identity() != self.target_closure.identity()
            || closure_subject.semantic_operation_version() != 1
            || closure_subject.canonical_graph_version() != 13
            || closure_subject.digest() != *canonical.identity().digest()
            || closure_subject.canonical_length() != canonical.identity().canonical_length()
            || closure_subject.epoch() != self.final_epoch
            || !self.target_closure.launch_subject_matches()
        {
            return Err(ProductionPipelineError::FinalGraphVerificationV13(
                ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                    stage: "retained V8 target closure and W4 final-graph result",
                },
            ));
        }
        self.validate_live_owner()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        Ok((canonical, self.final_epoch))
    }
}

/// Private handoff input that can only be constructed by the exact production
/// target-lowering stage. It grants no publication or artifact authority.
pub(crate) struct AuthenticatedProductionTargetModule {
    admitted: fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    target_module: fe2o3_kernel_ir::Module,
    llvm_ir: String,
    typed_descriptor_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
    compiler_ffi_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
}

fn exact_target_workgroup_roster_v1(
    module: &fe2o3_kernel_ir::Module,
) -> Result<Box<[(String, fe2o3_kernel_ir::WorkgroupSize)]>, ProductionPipelineError> {
    if module.kernels.is_empty() {
        return Err(ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
        ));
    }
    module
        .kernels
        .iter()
        .map(|kernel| {
            kernel
                .workgroup_size
                .map(|workgroup| (kernel.id.as_str().to_owned(), workgroup))
                .ok_or(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
                ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn require_exact_v13_capability_results(
    neutral_identity: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    optimized_identity: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_identity: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    results: ProductionV13CapabilityResults<'_>,
) -> Result<(), ProductionFinalGraphVerificationErrorV13> {
    let optimizer =
        results
            .optimizer
            .ok_or(ProductionFinalGraphVerificationErrorV13::MissingResult {
                stage: "V6 optimizer preservation chain",
            })?;
    if !optimizer.is_exact_fixed_policy_replay()
        || optimizer.grants_semantic_preservation_authority()
    {
        return Err(ProductionFinalGraphVerificationErrorV13::NonProductionOptimizer);
    }
    if optimizer.input_identity() != neutral_identity {
        return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
            stage: "V6 optimizer input",
        });
    }
    if optimizer.output_identity() != optimized_identity {
        return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
            stage: "V6 optimizer output",
        });
    }
    let mut expected_identity = neutral_identity;
    let mut expected_epoch = optimizer.initial_epoch();
    for record in optimizer.transformations() {
        let capability = record.capability_replay();
        let fresh = record.fresh_output_analysis();
        if record.input_identity() != expected_identity
            || record.input_epoch() != expected_epoch
            || capability.input_identity() != record.input_identity()
            || capability.output_identity() != record.output_identity()
            || capability.input_epoch() != record.input_epoch()
            || capability.output_epoch() != record.output_epoch()
            || fresh.graph_identity() != record.output_identity()
            || fresh.graph_epoch() != record.output_epoch()
            || record.grants_semantic_preservation_authority()
        {
            return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "V6 per-transform preservation record",
            });
        }
        let replay_shape_is_exact = if record.changed() {
            record.mode()
                == fe2o3_kernel_opt::TransformationPreservationModeV1::ExactProtectedStructureReplay
                && record.requires_complete_final_graph_analysis_replay()
                && !record.invalidated_analyses().is_empty()
                && !record.required_replays().is_empty()
        } else {
            record.mode()
                == fe2o3_kernel_opt::TransformationPreservationModeV1::ExactCanonicalIdentity
                && record.invalidated_analyses().is_empty()
                && record.required_replays().is_empty()
        };
        if !replay_shape_is_exact {
            return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "V6 affected-analysis invalidation",
            });
        }
        expected_identity = record.output_identity();
        expected_epoch = record.output_epoch();
    }
    if expected_identity != optimized_identity || expected_epoch != optimizer.final_epoch() {
        return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
            stage: "V6 terminal preservation record",
        });
    }

    let structural_replay = results.structural_replay.ok_or(
        ProductionFinalGraphVerificationErrorV13::MissingResult {
            stage: "independent V6 structural replay",
        },
    )?;
    if structural_replay.input_identity() != neutral_identity
        || structural_replay.output_identity() != optimized_identity
        || structural_replay.report() != optimizer
        || !structural_replay.establishes_exact_closed_replay()
        || structural_replay.establishes_semantic_preservation()
    {
        return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
            stage: "independent V6 structural replay",
        });
    }

    let target_closure =
        results
            .target_closure
            .ok_or(ProductionFinalGraphVerificationErrorV13::MissingResult {
                stage: "V13 target-capability closure",
            })?;
    let subject = target_closure.subject();
    if subject.semantic_operation_version() != 1
        || subject.canonical_graph_version() != 13
        || subject.digest() != *optimized_identity.digest()
        || subject.canonical_length() != optimized_identity.canonical_length()
        || !target_closure.launch_subject_matches()
    {
        return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
            stage: "target-capability closure",
        });
    }
    if subject.epoch() != optimizer.final_epoch() {
        return Err(ProductionFinalGraphVerificationErrorV13::StaleResult {
            stage: "target-capability closure",
            expected_epoch: optimizer.final_epoch(),
            observed_epoch: subject.epoch(),
        });
    }
    if target_closure.decision_count() == 0 {
        return Err(ProductionFinalGraphVerificationErrorV13::EmptyTargetCapabilityClosure);
    }
    if final_identity != optimized_identity {
        return Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
            stage: "immutable optimized final graph",
        });
    }
    if final_epoch != optimizer.final_epoch() {
        return Err(ProductionFinalGraphVerificationErrorV13::StaleResult {
            stage: "optimized final graph",
            expected_epoch: optimizer.final_epoch(),
            observed_epoch: final_epoch,
        });
    }
    Ok(())
}

fn prepare_v13_final_graph_verification(
    neutral_module: &fe2o3_kernel_ir::Module,
    neutral_canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13,
    target_backend: &crate::production_backend_v1::ProductionBackendTargetV1,
) -> Result<PreparedV13FinalGraphVerification, ProductionPipelineError> {
    let reconstructed =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(neutral_module.clone())
            .map_err(ProductionPipelineError::TargetCapabilityAnalysis)?;
    if reconstructed.identity() != neutral_canonical.identity() {
        return Err(ProductionPipelineError::FinalGraphVerificationV13(
            ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "lowered canonical KIR V13 owner",
            },
        ));
    }

    let neutral_identity = *reconstructed.identity();
    let optimized = fe2o3_kernel_opt::optimize_production_kernel_ir_module_v6(neutral_module)
        .map_err(ProductionPipelineError::TargetOptimizationV6)?;
    let structural_replay = fe2o3_kernel_opt::admit_production_kernel_ir_structural_replay_v6(
        neutral_module,
        optimized.module(),
        optimized.report(),
    )
    .map_err(ProductionPipelineError::TargetOptimizationReplayV6)?;
    let (optimized_module, optimized_canonical, optimizer) = optimized.into_parts();
    let optimized_identity = *optimized_canonical.identity();
    let optimizer_final_epoch = optimizer.final_epoch();

    let target_closure = target_backend
        .close_semantic_capabilities_v1(&optimized_canonical, optimizer_final_epoch)
        .map_err(ProductionPipelineError::TargetBackend)?;
    let final_epoch = optimizer_final_epoch;
    let final_canonical = optimized_canonical;
    let target_module = optimized_module;
    let kernel_ids = target_module
        .kernels
        .iter()
        .map(|kernel| kernel.id.clone())
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let prepared = PreparedV13FinalGraphVerification {
        pre_optimization_canonical: reconstructed,
        neutral_identity,
        optimized_identity,
        final_canonical,
        final_epoch,
        target_module,
        kernel_ids,
        optimizer,
        structural_replay,
        target_closure,
    };
    prepared.validate_capability_results()?;
    Ok(prepared)
}

impl PreparedV13FinalGraphVerification {
    fn capability_results(&self) -> ProductionV13CapabilityResults<'_> {
        ProductionV13CapabilityResults {
            optimizer: Some(&self.optimizer),
            structural_replay: Some(&self.structural_replay),
            target_closure: Some(&self.target_closure),
        }
    }

    fn validate_capability_results(&self) -> Result<(), ProductionPipelineError> {
        if self.kernel_ids.len() != self.target_module.kernels.len()
            || self
                .kernel_ids
                .iter()
                .zip(&self.target_module.kernels)
                .any(|(expected, kernel)| expected != &kernel.id)
        {
            return Err(ProductionPipelineError::FinalGraphVerificationV13(
                ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                    stage: "target-bound kernel roster",
                },
            ));
        }
        require_exact_v13_capability_results(
            &self.neutral_identity,
            &self.optimized_identity,
            self.final_canonical.identity(),
            self.final_epoch,
            self.capability_results(),
        )
        .map_err(ProductionPipelineError::FinalGraphVerificationV13)
    }

    fn require_final_graph_pliron_schedule(
        self,
    ) -> Result<VerifiedV13TargetLoweringInput, ProductionPipelineError> {
        self.validate_capability_results()?;
        let target_contract = self
            .target_closure
            .final_graph_target_contract_v1(
                &self.final_canonical,
                self.final_epoch,
                self.neutral_identity,
                self.optimizer.initial_epoch(),
            )
            .map_err(ProductionPipelineError::TargetBackend)?;
        let mut verified = fe2o3_pliron::ProductionFinalGraphOwnerV1::try_new(
            self.final_canonical,
            self.final_epoch,
            target_contract,
        )
        .map_err(ProductionPipelineError::FinalGraphExecution)?
        .verify()
        .map_err(ProductionPipelineError::FinalGraphExecution)?;
        verified
            .revalidate_live()
            .map_err(ProductionPipelineError::FinalGraphExecution)?;
        if verified.module() != &self.target_module
            || self.kernel_ids.len() != verified.module().kernels.len()
            || self
                .kernel_ids
                .iter()
                .zip(&verified.module().kernels)
                .any(|(expected, kernel)| expected != &kernel.id)
            || verified.report().final_graph() != verified.canonical().identity()
            || verified.report().final_epoch() != self.final_epoch
        {
            return Err(ProductionPipelineError::FinalGraphVerificationV13(
                ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                    stage: "owner-held final-graph release",
                },
            ));
        }
        let target_verification = ProductionV13TargetVerificationCustody::try_new(
            verified,
            self.pre_optimization_canonical,
            self.optimizer.initial_epoch(),
            self.final_epoch,
            self.target_closure,
        )
        .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        Ok(VerifiedV13TargetLoweringInput {
            target_module: self.target_module,
            kernel_ids: self.kernel_ids,
            target_optimization: self.optimizer,
            target_verification,
        })
    }
}

struct ProductionFinalGraphFunctionalRootCustodyV1 {
    logical_name: String,
    export_symbol: Box<[u8]>,
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    source_rank: u8,
    execution: fe2o3_verifier::ProductionFinalGraphFunctionalRefinementExecutionV2,
}

/// Exact final-graph functional proofs and their source reference bindings are
/// retained beside the sole live V13/PLIRON owner. This owner grants no
/// publication authority until W4 is executed for the same subject.
struct ProductionFinalGraphFunctionalCustodyV1 {
    target: ProductionV13TargetVerificationCustody,
    reference_effect_bindings: crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    roots: Box<[ProductionFinalGraphFunctionalRootCustodyV1]>,
    canonical_roster_identity:
        crate::production_ranked_projection_v1::ProductionRankedKernelRosterIdentityV1,
    canonical_kernel_order: Box<[usize]>,
    identity: [u8; 32],
}

impl ProductionFinalGraphFunctionalCustodyV1 {
    fn try_new(
        admitted: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
        mut target: ProductionV13TargetVerificationCustody,
        ranked: crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
        reference_effect_bindings: crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    ) -> Result<Self, ProductionPipelineError> {
        require_exact_protected_reference_roster_v1(&reference_effect_bindings, &ranked)?;
        target
            .validate_live_owner()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        let runtime = fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1::open(
            FINAL_GRAPH_FUNCTIONAL_RUNTIME_ROOT_V1,
        )
        .map_err(ProductionPipelineError::FinalGraphFunctionalRuntime)?;
        let (ranked_roots, canonical_roster_identity, canonical_kernel_order) = ranked
            .into_final_graph_functional_roster()
            .map_err(ProductionPipelineError::RankedVerification)?
            .into_parts();
        if ranked_roots.len() != target.verified_graph.module().kernels.len()
            || ranked_roots.len() != reference_effect_bindings.as_slice().len()
            || canonical_kernel_order.len() != ranked_roots.len()
        {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "final-graph proof, reference, and executable root rosters differ",
            ));
        }

        let source_graph = *target.pre_optimization_canonical.identity();
        let final_graph = *target.final_canonical().identity();
        let final_epoch = target.final_epoch;
        let target_closure = target.target_closure().identity();
        let mut consumed_bindings = BTreeSet::new();
        let mut roots = Vec::with_capacity(ranked_roots.len());
        for (ordinal, root) in ranked_roots.into_vec().into_iter().enumerate() {
            let kernel = target.verified_graph.module().kernels.get(ordinal).ok_or(
                ProductionPipelineError::FunctionalRefinementCustody(
                    "final executable root disappeared",
                ),
            )?;
            let mut matches = reference_effect_bindings
                .as_slice()
                .iter()
                .enumerate()
                .filter(|(_, binding)| {
                    binding.logical_kernel_name == root.logical_name()
                        && binding.kernel.function_sha256 == *root.kernel_binding()
                });
            let Some((binding_ordinal, binding)) = matches.next() else {
                return Err(ProductionPipelineError::FunctionalRefinementCustody(
                    "final-graph proof root has no exact reference-effect binding",
                ));
            };
            if matches.next().is_some()
                || !consumed_bindings.insert(binding_ordinal)
                || kernel.id.as_str().as_bytes() != root.export_symbol()
                || kernel.entry.as_str().as_bytes() != root.export_symbol()
                || kernel.domain.rank() != root.source_rank()
            {
                return Err(ProductionPipelineError::FunctionalRefinementCustody(
                    "final-graph proof root was reordered or cross-wired",
                ));
            }
            let expected_ranked_kernel = root.verified().derivation().ranked_kernel();
            let logical_name = root.logical_name().to_owned();
            let export_symbol = root.export_symbol().to_vec().into_boxed_slice();
            let semantic_root = root.semantic_root().index();
            let semantic_root_identity = *root.semantic_root_identity().as_bytes();
            let kernel_binding = *root.kernel_binding();
            let source_rank = root.source_rank();
            let execution = fe2o3_verifier::bind_effect_ir_derived_functional_refinement_to_borrowed_final_graph_v2(
                &runtime,
                admitted.semantic_kir(),
                &mut target.verified_graph,
                root.into_verified(),
                FINAL_GRAPH_FUNCTIONAL_TIMEOUT_SECONDS_V1,
            )
            .map_err(ProductionPipelineError::FinalGraphFunctionalRefinement)?;
            validate_final_graph_functional_root_v1(
                &execution,
                binding,
                expected_ranked_kernel,
                source_graph,
                final_graph,
                final_epoch,
                target_closure,
            )?;
            roots.push(ProductionFinalGraphFunctionalRootCustodyV1 {
                logical_name,
                export_symbol,
                semantic_root,
                semantic_root_identity,
                kernel_binding,
                source_rank,
                execution,
            });
        }
        if consumed_bindings.len() != reference_effect_bindings.as_slice().len() {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "reference-effect roster contains an unconsumed binding",
            ));
        }
        runtime
            .revalidate()
            .map_err(ProductionPipelineError::FinalGraphFunctionalRuntime)?;
        target
            .validate_live_owner()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        let roots = roots.into_boxed_slice();
        let identity = derive_final_graph_functional_roster_identity_v1(
            source_graph,
            final_graph,
            final_epoch,
            target_closure,
            canonical_roster_identity,
            &canonical_kernel_order,
            &roots,
        )?;
        Ok(Self {
            target,
            reference_effect_bindings,
            roots,
            canonical_roster_identity,
            canonical_kernel_order,
            identity,
        })
    }

    fn validate_live_owner(&mut self) -> Result<(), ProductionPipelineError> {
        self.target
            .validate_live_owner()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        if self.roots.is_empty()
            || self.roots.len() != self.reference_effect_bindings.as_slice().len()
            || self.roots.len() != self.canonical_kernel_order.len()
        {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "retained final-graph functional roster changed shape",
            ));
        }
        let source_graph = *self.target.pre_optimization_canonical.identity();
        let final_graph = *self.target.final_canonical().identity();
        let final_epoch = self.target.final_epoch;
        let target_closure = self.target.target_closure().identity();
        let mut seen = BTreeSet::new();
        for root in &self.roots {
            let mut matches = self
                .reference_effect_bindings
                .as_slice()
                .iter()
                .enumerate()
                .filter(|(_, binding)| {
                    binding.logical_kernel_name == root.logical_name
                        && binding.kernel.function_sha256 == root.kernel_binding
                });
            let Some((ordinal, binding)) = matches.next() else {
                return Err(ProductionPipelineError::FunctionalRefinementCustody(
                    "retained final-graph proof lost its reference binding",
                ));
            };
            if matches.next().is_some() || !seen.insert(ordinal) {
                return Err(ProductionPipelineError::FunctionalRefinementCustody(
                    "retained final-graph proof has ambiguous reference custody",
                ));
            }
            validate_final_graph_functional_root_v1(
                &root.execution,
                binding,
                root.execution.report().ranked_kernel(),
                source_graph,
                final_graph,
                final_epoch,
                target_closure,
            )?;
        }
        let identity = derive_final_graph_functional_roster_identity_v1(
            source_graph,
            final_graph,
            final_epoch,
            target_closure,
            self.canonical_roster_identity,
            &self.canonical_kernel_order,
            &self.roots,
        )?;
        if identity != self.identity {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "retained final-graph functional roster identity changed",
            ));
        }
        Ok(())
    }

    const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    fn execute_w4_capability_witness(
        &mut self,
        subject: fe2o3_kernel_analysis::ProductionW4FinalGraphSubjectV1,
        target: &fe2o3_kernel_analysis::ProductionW4TargetResourceInputV1,
    ) -> Result<
        fe2o3_kernel_analysis::ProductionW4FinalGraphCapabilityWitnessV1,
        ProductionPipelineError,
    > {
        self.validate_live_owner()?;
        if subject.functional_refinement_identity() != self.identity() {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "W4 subject does not retain Hilbert's exact final-graph functional roster",
            ));
        }
        let witness = self.target.execute_w4_capability_witness(subject, target)?;
        self.validate_live_owner()?;
        Ok(witness)
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_final_graph_functional_root_v1(
    execution: &fe2o3_verifier::ProductionFinalGraphFunctionalRefinementExecutionV2,
    binding: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1,
    expected_ranked_kernel: fe2o3_proof_contracts::DigestV1,
    source_graph: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_graph: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    target_closure: [u8; 32],
) -> Result<(), ProductionPipelineError> {
    let report = execution.report();
    let proof_binding = report.binding();
    if report.safe_reference_mir().as_bytes() != &binding.reference.rustc_mir_body_sha256
        || report.kernel_mir().as_bytes() != &binding.kernel.rustc_mir_body_sha256
        || report.ranked_kernel() != expected_ranked_kernel
        || report.source_graph() != source_graph
        || report.final_graph() != final_graph
        || report.final_epoch() != final_epoch
        || report.target_closure() != target_closure
        || report.output_writes() == 0
        || proof_binding.safe_reference_identity().as_bytes() != &binding.reference.function_sha256
        || proof_binding.safe_reference_mir_hash() != report.safe_reference_mir()
        || proof_binding.kernel_subject_identity().as_bytes() != final_graph.digest()
        || proof_binding.kernel_mir_hash() != report.kernel_mir()
        || proof_binding.normalized_obligation_effect_ir_hash() != report.obligation()
        || execution.final_signed_receipt_wire().is_empty()
        || execution.final_receipt_verifying_key() == &[0; 32]
        || !execution.retains_strictly_imported_final_graph_receipt()
        || execution.grants_llvm_or_later_authority()
    {
        return Err(ProductionPipelineError::FunctionalRefinementCustody(
            "final-graph functional receipt names stale or cross-wired subjects",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn derive_final_graph_functional_roster_identity_v1(
    source_graph: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_graph: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    target_closure: [u8; 32],
    canonical_roster_identity:
        crate::production_ranked_projection_v1::ProductionRankedKernelRosterIdentityV1,
    canonical_kernel_order: &[usize],
    roots: &[ProductionFinalGraphFunctionalRootCustodyV1],
) -> Result<[u8; 32], ProductionPipelineError> {
    if roots.is_empty() || roots.len() != canonical_kernel_order.len() {
        return Err(ProductionPipelineError::FunctionalRefinementCustody(
            "cannot identify an empty or incomplete final-graph functional roster",
        ));
    }
    let mut digest = Sha256::new();
    digest.update((FINAL_GRAPH_FUNCTIONAL_ROSTER_DOMAIN_V1.len() as u64).to_le_bytes());
    digest.update(FINAL_GRAPH_FUNCTIONAL_ROSTER_DOMAIN_V1);
    digest.update(source_graph.digest());
    digest.update(source_graph.canonical_length().to_le_bytes());
    digest.update(final_graph.digest());
    digest.update(final_graph.canonical_length().to_le_bytes());
    digest.update(final_epoch.to_le_bytes());
    digest.update(target_closure);
    digest.update(canonical_roster_identity.as_bytes());
    digest.update((canonical_kernel_order.len() as u64).to_le_bytes());
    for index in canonical_kernel_order {
        let index = u64::try_from(*index).map_err(|_| {
            ProductionPipelineError::FunctionalRefinementCustody(
                "canonical final-graph root ordinal cannot be represented",
            )
        })?;
        digest.update(index.to_le_bytes());
    }
    digest.update((roots.len() as u64).to_le_bytes());
    for root in roots {
        digest.update((root.logical_name.len() as u64).to_le_bytes());
        digest.update(root.logical_name.as_bytes());
        digest.update((root.export_symbol.len() as u64).to_le_bytes());
        digest.update(&root.export_symbol);
        digest.update(root.semantic_root.to_le_bytes());
        digest.update(root.semantic_root_identity);
        digest.update(root.kernel_binding);
        digest.update([root.source_rank]);
        let report = root.execution.report();
        for identity in [
            report.safe_reference_mir(),
            report.kernel_mir(),
            report.ranked_kernel(),
            report.parallel_contract(),
            report.output_expression_product(),
            report.generated_source(),
            report.obligation(),
        ] {
            digest.update(identity.as_bytes());
        }
        digest.update(report.output_writes().to_le_bytes());
        digest.update(root.execution.final_receipt_verifying_key());
        digest.update((root.execution.final_signed_receipt_wire().len() as u64).to_le_bytes());
        digest.update(root.execution.final_signed_receipt_wire());
    }
    Ok(digest.finalize().into())
}

/// Producer-side W1-W6 typestate. The post-Worker #213 verifier remains the
/// sole authority gate; this owner ensures the compiler cannot publish its
/// input without retaining both final functional and exact W4 schedule custody.
struct ProductionW1W6PublicationOwnerV1 {
    functional: ProductionFinalGraphFunctionalCustodyV1,
    w4_schedule: fe2o3_kernel_analysis::ProductionW4AnalysisScheduleWitnessV1,
}

impl ProductionW1W6PublicationOwnerV1 {
    fn try_bind(
        mut functional: ProductionFinalGraphFunctionalCustodyV1,
        w4_schedule: fe2o3_kernel_analysis::ProductionW4AnalysisScheduleWitnessV1,
    ) -> Result<Self, ProductionPipelineError> {
        functional.validate_live_owner()?;
        let final_graph = functional.target.final_canonical().identity();
        let final_epoch = functional.target.final_epoch;
        let target_closure = functional.target.target_closure().identity();
        if w4_schedule.final_graph() != final_graph
            || w4_schedule.final_epoch() != final_epoch
            || w4_schedule.target_closure_identity() != &target_closure
            || w4_schedule.identity() == &[0; 32]
            || w4_schedule.checker_identity() == &[0; 32]
            || w4_schedule.obligations().is_empty()
            || w4_schedule.obligations().iter().any(|obligation| {
                let evidence = obligation.independent_evidence();
                evidence.final_graph() != final_graph
                    || evidence.final_epoch() != final_epoch
                    || evidence.pliron_epoch() != w4_schedule.pliron_epoch()
                    || obligation.evidence_identity() == &[0; 32]
            })
            || w4_schedule.grants_compiler_refinement_authority()
            || w4_schedule.grants_lowering_artifact_or_launch_authority()
        {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "W4 schedule is stale or names a different final functional subject",
            ));
        }
        Ok(Self {
            functional,
            w4_schedule,
        })
    }

    fn prepare_capability_handoff(
        &mut self,
        legacy_handoff: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
        capability_inputs: crate::production_worker_handoff::ProductionCapabilityCarriageInputsV5,
    ) -> Result<fe2o3_compiler_ffi::InertProductionCapabilityHandoffV5, ProductionPipelineError>
    {
        self.functional.validate_live_owner()?;
        if self.w4_schedule.final_graph() != self.functional.target.final_canonical().identity()
            || self.w4_schedule.final_epoch() != self.functional.target.final_epoch
            || self.w4_schedule.target_closure_identity()
                != &self.functional.target.target_closure().identity()
        {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "W1-W6 publication owner changed before handoff construction",
            ));
        }
        crate::production_worker_handoff::prepare_production_capability_handoff_v5(
            legacy_handoff,
            self.functional.target.target_closure(),
            self.functional.target.final_graph_report(),
            capability_inputs,
        )
        .map_err(ProductionPipelineError::WorkerHandoff)
    }

    fn validate_for_publication(&mut self) -> Result<(), ProductionPipelineError> {
        self.functional.validate_live_owner()?;
        if self.w4_schedule.final_graph() != self.functional.target.final_canonical().identity()
            || self.w4_schedule.final_epoch() != self.functional.target.final_epoch
            || self.w4_schedule.target_closure_identity()
                != &self.functional.target.target_closure().identity()
        {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "W1-W6 publication owner changed before durable publication",
            ));
        }
        Ok(())
    }
}

struct PreparedProductionWorkerPublication {
    producer: ProducerIdentity,
    output_dir: PathBuf,
    attempt: BuildAttempt,
    invocation: Box<AdmittedProtectedRustcInvocationV1>,
    compiler_execution: Box<AdmittedProtectedCompilerExecutionV1>,
    semantic_lineage: crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3,
    rustc_target: crate::production_target_v1::AuthenticatedProductionTargetV1,
    final_graph_functional: ProductionFinalGraphFunctionalCustodyV1,
    w4_diagnostic_source_map_v2: Box<[u8]>,
    prepared: crate::production_worker_handoff::PreparedProductionWorkerHandoff,
    bundle_v8: production_bundle_transaction_v8::PreparedProductionBundleTransactionV8,
}

fn require_exact_protected_reference_roster_v1(
    bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    ranked: &crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
) -> Result<(), ProductionPipelineError> {
    if bindings.as_slice().len() != ranked.root_count() {
        return Err(ProductionPipelineError::FunctionalRefinementCustody(
            "reference-effect and ranked-root rosters have different lengths",
        ));
    }
    let mut names = BTreeSet::new();
    for root in ranked.roots() {
        let mut matches = bindings.as_slice().iter().filter(|binding| {
            binding.logical_kernel_name == root.logical_name()
                && &binding.kernel.function_sha256 == root.kernel_binding()
        });
        let Some(binding) = matches.next() else {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "ranked root has no exact authenticated reference-effect binding",
            ));
        };
        if matches.next().is_some()
            || !names.insert(binding.logical_kernel_name.as_str())
            || binding.reference.rustc_mir_body_sha256 == [0; 32]
            || binding.kernel.rustc_mir_body_sha256 == [0; 32]
            || binding.effect_ir_sha256 == [0; 32]
            || binding.observable_output_writes.is_empty()
            || root.verification().aggregate_verus_execution().is_none()
        {
            return Err(ProductionPipelineError::FunctionalRefinementCustody(
                "reference-effect binding or retained Verus execution is incomplete",
            ));
        }
    }
    Ok(())
}

/// Complete authority-free output of the sole protected compiler publication.
pub(crate) struct PublishedProductionWorkerTransaction {
    subject: fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1,
    bundle_v8: production_bundle_transaction_v8::ProductionBoundBundleV8,
}

impl PublishedProductionWorkerTransaction {
    pub(crate) fn outer_handoff(
        &self,
    ) -> fe2o3_artifact_transaction::InertCompilerExecutionContentBindingV1 {
        debug_assert!(self.bundle_v8.remains_bound_to(&self.subject));
        self.subject.outer_handoff()
    }
}

impl AuthenticatedProductionTargetModule {
    pub(crate) fn into_parts(
        self,
    ) -> (
        fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
        fe2o3_compiler_ffi::DeviceTargetV1,
        fe2o3_kernel_ir::Module,
        String,
        Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
        Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    ) {
        (
            self.admitted,
            self.target,
            self.target_module,
            self.llvm_ir,
            self.typed_descriptor_roots,
            self.compiler_ffi_envelope,
        )
    }
}

impl TargetNeutralProductionCompilation {
    fn into_prepared_simulation_bundle_v1(
        self,
        compiler_execution_binding: fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1,
    ) -> Result<
        (
            fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
            AuthenticatedProductionBindings,
            fe2o3_kernel_ir::PreparedSimulationBundleV1,
        ),
        ProductionPipelineError,
    > {
        let Self {
            lowered,
            ranked_verification: _,
            bindings,
        } = self;
        lowered
            .verify_equivalence()
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        if bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != bindings.rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        let production_identity = lowered.canonical_kernel_ir_identity();
        let production_identity = match production_identity.version() {
            fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V8 => {
                fe2o3_kernel_ir::SimulationProductionKirIdentityV1::v8(
                    *production_identity.digest(),
                    production_identity.canonical_length(),
                )
                .map_err(ProductionPipelineError::SimulationBundle)?
            }
            fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V9 => {
                return Err(ProductionPipelineError::SimulationProductionKirV9);
            }
            fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V11 => {
                return Err(ProductionPipelineError::SimulationProductionKirV11);
            }
            fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V12 => {
                return Err(ProductionPipelineError::SimulationProductionKirV12);
            }
            fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V13 => {
                return Err(ProductionPipelineError::SimulationProductionKirV13);
            }
        };
        let canonical_v7 =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7::from_module(lowered.module().clone())
                .map_err(ProductionPipelineError::SimulationKernelIrV7)?;
        let inventory_receipt =
            fe2o3_compiler_lineage::InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
                bindings.rustc_identity_inventory.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let preflight_receipt =
            fe2o3_compiler_lineage::InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
                bindings.rustc_preflight_plan.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let inventory_identity = inventory_receipt.identity();
        let preflight_identity = preflight_receipt.identity();
        let lineage = fe2o3_kernel_ir::SimulationSourceLineageV1::new(
            *inventory_identity.sha256(),
            inventory_identity.byte_len(),
            *preflight_identity.sha256(),
            preflight_identity.byte_len(),
        )
        .map_err(ProductionPipelineError::SimulationBundle)?;
        let prepared = fe2o3_kernel_ir::PreparedSimulationBundleV1::new(
            compiler_execution_binding,
            lineage,
            production_identity,
            bindings
                .rustc_target
                .backend()
                .contract()
                .canonical_target(),
            canonical_v7,
        )
        .map_err(ProductionPipelineError::SimulationBundle)?;
        Ok((lowered, bindings, prepared))
    }

    fn into_simulation_bundle_v1(
        self,
        compiler_execution_binding: fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV1, ProductionPipelineError> {
        let (lowered, bindings, prepared) =
            self.into_prepared_simulation_bundle_v1(compiler_execution_binding)?;
        let debug_map = compiler_debug_source_map_v1(
            &lowered,
            &bindings.debug_source_files,
            prepared.debug_source_map_binding(),
        )?;
        prepared
            .finalize_with_source_map(debug_map)
            .map_err(ProductionPipelineError::SimulationBundle)
    }

    fn into_simulation_bundle_v2(
        self,
        compiler_execution_binding: fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV2, ProductionPipelineError> {
        let (lowered, bindings, prepared) =
            self.into_prepared_simulation_bundle_v1(compiler_execution_binding)?;
        require_complete_simulation_debug_source_capture_v2(bindings.debug_capture_gap)?;
        let debug_map = compiler_debug_source_map_v2(
            &lowered,
            &bindings.debug_source_files,
            &bindings.debug_source_scopes,
            &bindings.debug_source_variables,
            prepared.debug_source_map_binding(),
        )?;
        let inner = prepared
            .finalize_without_source_map()
            .map_err(ProductionPipelineError::SimulationBundle)?;
        fe2o3_kernel_ir::VerifiedSimulationBundleV2::new(inner, debug_map)
            .map_err(ProductionPipelineError::SimulationBundleV2)
    }

    fn into_prepared_simulation_bundle_v3(
        self,
        compiler_execution_binding: fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1,
    ) -> Result<
        (
            fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
            AuthenticatedProductionBindings,
            fe2o3_kernel_ir::VerifiedSimulationBundleV3,
        ),
        ProductionPipelineError,
    > {
        let (lowered, bindings, prepared) =
            self.into_prepared_simulation_bundle_v1(compiler_execution_binding)?;
        require_complete_simulation_debug_source_capture_v2(bindings.debug_capture_gap)?;
        let debug_map = compiler_debug_source_map_v2(
            &lowered,
            &bindings.debug_source_files,
            &bindings.debug_source_scopes,
            &bindings.debug_source_variables,
            prepared.debug_source_map_binding(),
        )?;
        let inner_v1 = prepared
            .finalize_without_source_map()
            .map_err(ProductionPipelineError::SimulationBundle)?;
        let inner_v2 = fe2o3_kernel_ir::VerifiedSimulationBundleV2::new(inner_v1, debug_map)
            .map_err(ProductionPipelineError::SimulationBundleV2)?;
        let semantic = lowered.semantic().semantic();
        let mut semantic_mir = Vec::new();
        semantic_mir
            .try_reserve_exact(semantic.canonical_encoding().len())
            .map_err(|_| {
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "semantic MIR bundle allocation failed",
                )
            })?;
        semantic_mir.extend_from_slice(semantic.canonical_encoding());
        let storage_map = compiler_semantic_storage_map_v1(
            &lowered,
            &bindings.debug_source_variables,
            SemanticStorageMapBindingInputV1 {
                container_identity: *inner_v2.identity().as_bytes(),
                subject_identity: *inner_v2.subject_identity(),
                canonical_kir_digest: *inner_v2.canonical_kir_v7_identity().digest(),
                canonical_kir_bytes: inner_v2.canonical_kir_v7_identity().canonical_length(),
            },
        )?;
        let bundle =
            fe2o3_kernel_ir::VerifiedSimulationBundleV3::new(inner_v2, semantic_mir, storage_map)
                .map_err(ProductionPipelineError::SimulationBundleV3)?;
        Ok((lowered, bindings, bundle))
    }

    fn into_simulation_bundle_v3(
        self,
        compiler_execution_binding: fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV3, ProductionPipelineError> {
        let (_, _, bundle) = self.into_prepared_simulation_bundle_v3(compiler_execution_binding)?;
        Ok(bundle)
    }

    fn into_simulation_bundle_v4(
        self,
        compiler_execution_binding: fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV4, ProductionPipelineError> {
        let (lowered, _, inner) =
            self.into_prepared_simulation_bundle_v3(compiler_execution_binding)?;
        let storage_map = compiler_semantic_storage_map_v2(&lowered, *inner.identity().as_bytes())?;
        fe2o3_kernel_ir::VerifiedSimulationBundleV4::new(inner, storage_map)
            .map_err(ProductionPipelineError::SimulationBundleV4)
    }

    fn into_simulation_bundle_v5(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV5, ProductionPipelineError> {
        let Self {
            lowered,
            ranked_verification: _,
            bindings,
        } = self;
        lowered
            .verify_equivalence()
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        require_complete_simulation_debug_source_capture_v2(bindings.debug_capture_gap)?;
        if bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != bindings.rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        let production = lowered.canonical_kernel_ir_identity();
        let production = fe2o3_kernel_ir::SimulationProductionKirIdentityV5::new(
            match production.version() {
                fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V8 => 8,
                fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V9 => 9,
                fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V11 => {
                    return Err(ProductionPipelineError::SimulationProductionKirV11);
                }
                fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V12 => {
                    return Err(ProductionPipelineError::SimulationProductionKirV12);
                }
                fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V13 => {
                    return Err(ProductionPipelineError::SimulationProductionKirV13);
                }
            },
            *production.digest(),
            production.canonical_length(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV5)?;
        let canonical_v10 =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV10::from_module(lowered.module().clone())
                .map_err(|error| {
                    ProductionPipelineError::SimulationBundleV5(
                        fe2o3_kernel_ir::SimulationBundleErrorV5::CanonicalKir(error),
                    )
                })?;
        let inventory_receipt =
            fe2o3_compiler_lineage::InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
                bindings.rustc_identity_inventory.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let preflight_receipt =
            fe2o3_compiler_lineage::InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
                bindings.rustc_preflight_plan.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let inventory_identity = inventory_receipt.identity();
        let preflight_identity = preflight_receipt.identity();
        let lineage = fe2o3_kernel_ir::SimulationSourceLineageV1::new(
            *inventory_identity.sha256(),
            inventory_identity.byte_len(),
            *preflight_identity.sha256(),
            preflight_identity.byte_len(),
        )
        .map_err(ProductionPipelineError::SimulationBundle)?;
        let prepared = fe2o3_kernel_ir::PreparedSimulationBundleV5::new(
            lineage,
            production,
            bindings
                .rustc_target
                .backend()
                .contract()
                .canonical_target(),
            canonical_v10,
        )
        .map_err(ProductionPipelineError::SimulationBundleV5)?;
        let debug_map = compiler_debug_source_map_v2(
            &lowered,
            &bindings.debug_source_files,
            &bindings.debug_source_scopes,
            &bindings.debug_source_variables,
            prepared.debug_source_map_binding(),
        )?;
        let semantic = lowered.semantic().semantic();
        let mut semantic_mir = Vec::new();
        semantic_mir
            .try_reserve_exact(semantic.canonical_encoding().len())
            .map_err(|_| {
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "semantic MIR V5 bundle allocation failed",
                )
            })?;
        semantic_mir.extend_from_slice(semantic.canonical_encoding());
        let subject = *prepared.subject_identity();
        let kir_digest = *prepared.canonical_kir_v10_digest();
        let kir_bytes = prepared.canonical_kir_v10_length();
        let legacy_storage = compiler_semantic_storage_map_v1(
            &lowered,
            &bindings.debug_source_variables,
            SemanticStorageMapBindingInputV1 {
                container_identity: subject,
                subject_identity: subject,
                canonical_kir_digest: kir_digest,
                canonical_kir_bytes: kir_bytes,
            },
        )?;
        let storage = fe2o3_kernel_ir::SemanticStorageMapV5::new(
            subject,
            semantic.wire_version().as_u16(),
            *semantic.semantic_sha256().as_bytes(),
            semantic.canonical_encoding().len() as u64,
            *semantic.target_layout_identity().as_bytes(),
            kir_digest,
            kir_bytes,
            legacy_storage.kernels().to_vec(),
            legacy_storage.variables().to_vec(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV5)?;
        let legacy_aggregate = compiler_semantic_storage_map_v2(&lowered, subject)?;
        let aggregate = fe2o3_kernel_ir::SemanticAggregateStorageMapV5::new(
            subject,
            kir_digest,
            kir_bytes,
            legacy_aggregate.kernels().to_vec(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV5)?;
        prepared
            .finalize(debug_map, semantic_mir, storage, aggregate)
            .map_err(ProductionPipelineError::SimulationBundleV5)
    }

    fn into_simulation_bundle_v6(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV6, ProductionPipelineError> {
        let Self {
            lowered,
            ranked_verification: _,
            bindings,
        } = self;
        lowered
            .verify_equivalence()
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        require_complete_simulation_debug_source_capture_v2(bindings.debug_capture_gap)?;
        if bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != bindings.rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        if matches!(
            lowered.canonical_kernel_ir_identity().version(),
            fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V12
        ) {
            return Err(ProductionPipelineError::SimulationProductionKirV12);
        }
        let canonical_v11 =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV11::from_module(lowered.module().clone())
                .map_err(|error| {
                    ProductionPipelineError::SimulationBundleV6(
                        fe2o3_kernel_ir::SimulationBundleErrorV6::CanonicalKir(error),
                    )
                })?;
        let production = fe2o3_kernel_ir::SimulationProductionKirIdentityV6::new(
            11,
            *canonical_v11.identity().digest(),
            canonical_v11.identity().canonical_length(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV6)?;
        let inventory_receipt =
            fe2o3_compiler_lineage::InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
                bindings.rustc_identity_inventory.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let preflight_receipt =
            fe2o3_compiler_lineage::InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
                bindings.rustc_preflight_plan.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let inventory_identity = inventory_receipt.identity();
        let preflight_identity = preflight_receipt.identity();
        let lineage = fe2o3_kernel_ir::SimulationSourceLineageV1::new(
            *inventory_identity.sha256(),
            inventory_identity.byte_len(),
            *preflight_identity.sha256(),
            preflight_identity.byte_len(),
        )
        .map_err(ProductionPipelineError::SimulationBundle)?;
        let prepared = fe2o3_kernel_ir::PreparedSimulationBundleV6::new(
            lineage,
            production,
            bindings
                .rustc_target
                .backend()
                .contract()
                .canonical_target(),
            canonical_v11,
        )
        .map_err(ProductionPipelineError::SimulationBundleV6)?;
        let debug_map = compiler_debug_source_map_v2(
            &lowered,
            &bindings.debug_source_files,
            &bindings.debug_source_scopes,
            &bindings.debug_source_variables,
            prepared.debug_source_map_binding(),
        )?;
        let semantic = lowered.semantic().semantic();
        let mut semantic_mir = Vec::new();
        semantic_mir
            .try_reserve_exact(semantic.canonical_encoding().len())
            .map_err(|_| {
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "semantic MIR V6 bundle allocation failed",
                )
            })?;
        semantic_mir.extend_from_slice(semantic.canonical_encoding());
        let subject = *prepared.subject_identity();
        let kir_digest = *prepared.canonical_kir_v11_digest();
        let kir_bytes = prepared.canonical_kir_v11_length();
        let legacy_storage = compiler_semantic_storage_map_v1(
            &lowered,
            &bindings.debug_source_variables,
            SemanticStorageMapBindingInputV1 {
                container_identity: subject,
                subject_identity: subject,
                canonical_kir_digest: kir_digest,
                canonical_kir_bytes: kir_bytes,
            },
        )?;
        let storage = fe2o3_kernel_ir::SemanticStorageMapV6::new(
            subject,
            semantic.wire_version().as_u16(),
            *semantic.semantic_sha256().as_bytes(),
            semantic.canonical_encoding().len() as u64,
            *semantic.target_layout_identity().as_bytes(),
            kir_digest,
            kir_bytes,
            legacy_storage.kernels().to_vec(),
            legacy_storage.variables().to_vec(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV6)?;
        let legacy_aggregate = compiler_semantic_storage_map_v2(&lowered, subject)?;
        let aggregate = fe2o3_kernel_ir::SemanticAggregateStorageMapV6::new(
            subject,
            kir_digest,
            kir_bytes,
            legacy_aggregate.kernels().to_vec(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV6)?;
        prepared
            .finalize(debug_map, semantic_mir, storage, aggregate)
            .map_err(ProductionPipelineError::SimulationBundleV6)
    }

    fn admit_formal_memory(
        self,
    ) -> Result<FormalMemoryAdmittedProductionCompilation, ProductionPipelineError> {
        let Self {
            lowered,
            ranked_verification,
            bindings,
        } = self;
        let admitted = fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1::try_admit(lowered)
            .map_err(ProductionPipelineError::FormalMemoryAdmission)?;
        Ok(FormalMemoryAdmittedProductionCompilation {
            admitted,
            ranked_verification,
            bindings,
        })
    }
}

impl FormalMemoryAdmittedProductionCompilation {
    fn lower_production_target(
        self,
    ) -> Result<TargetLoweredProductionCompilation, ProductionPipelineError> {
        let Self {
            admitted,
            ranked_verification,
            bindings,
        } = self;
        let target_backend = bindings.rustc_target.backend();
        let semantic = admitted.semantic_kir().semantic().semantic();
        if semantic.roots().is_empty()
            || semantic.roots().len() != bindings.typed_descriptor_roots.len()
            || semantic.roots().len() != admitted.semantic_kir().module().kernels.len()
            || semantic.roots().len() != admitted.kernels().len()
        {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        }
        for (((typed_root, semantic_root), kernel), formal) in bindings
            .typed_descriptor_roots
            .iter()
            .zip(semantic.roots())
            .zip(admitted.semantic_kir().module().kernels.iter())
            .zip(admitted.kernels())
        {
            let semantic_function = semantic
                .functions()
                .get(semantic_root.index() as usize)
                .ok_or(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
                ))?;
            let semantic_entry =
                semantic_function
                    .kernel_entry()
                    .ok_or(ProductionPipelineError::Geometry(
                        crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
                    ))?;
            if semantic_entry.kernel_binding_identity().as_bytes()
                != &typed_root.kernel_binding_bytes()
                || semantic_entry.export_symbol().as_bytes() != typed_root.entry_symbol().as_bytes()
                || kernel.id.as_str() != typed_root.entry_symbol()
                || kernel.entry.as_str() != typed_root.entry_symbol()
                || formal.obligations().kernel().as_str() != typed_root.entry_symbol()
            {
                return Err(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
                ));
            }
            let source_launch = typed_root.source_launch().ok_or(
                ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
                ),
            )?;
            crate::production_geometry_v1::derive_production_geometry_v1(
                admitted.semantic_kir().module(),
                typed_root.entry_symbol(),
                semantic_function,
                source_launch,
                target_backend.contract().canonical_target(),
            )
            .map_err(ProductionPipelineError::Geometry)?;
        }
        let neutral_canonical = admitted.semantic_kir().canonical_kernel_ir_v13().ok_or(
            ProductionPipelineError::FinalGraphVerificationV13(
                ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                    stage: "lowered canonical KIR V13 custody",
                },
            ),
        )?;
        let prepared = prepare_v13_final_graph_verification(
            admitted.semantic_kir().module(),
            neutral_canonical,
            target_backend,
        )?;
        let verified = prepared.require_final_graph_pliron_schedule()?;
        let dialect_llvm_ir = target_backend
            .lower_v13_module_v1(
                verified.target_verification.final_canonical(),
                verified.target_verification.final_epoch,
                verified.target_verification.target_closure(),
            )
            .map_err(ProductionPipelineError::TargetBackend)?;
        let VerifiedV13TargetLoweringInput {
            target_module,
            kernel_ids,
            target_optimization,
            target_verification,
        } = verified;
        if kernel_ids.len() != target_module.kernels.len()
            || kernel_ids
                .iter()
                .zip(&target_module.kernels)
                .any(|(kernel_id, kernel)| kernel_id != &kernel.id)
        {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        }
        target_verification
            .validate_target_module(&target_module)
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        let workgroups = exact_target_workgroup_roster_v1(&target_module)?;
        let llvm_ir = target_backend
            .bind_worker_layout_v1(&dialect_llvm_ir)
            .map_err(ProductionPipelineError::TargetBackend)?;
        Ok(TargetLoweredProductionCompilation {
            admitted,
            ranked_verification,
            target_module,
            target_optimization,
            target_verification,
            workgroups,
            llvm_ir,
            bindings,
        })
    }
}

impl TargetLoweredProductionCompilation {
    pub(crate) fn module(&self) -> &fe2o3_kernel_ir::Module {
        &self.target_module
    }

    pub(crate) fn target_name(&self) -> &'static str {
        self.bindings
            .rustc_target
            .backend()
            .contract()
            .canonical_target()
    }

    pub(crate) fn canonical_kernel_ir_version(&self) -> u16 {
        debug_assert!(
            self.admitted
                .semantic_kir()
                .canonical_kernel_ir_v13()
                .is_some()
        );
        13
    }

    pub(crate) fn guarded_store_count(&self) -> usize {
        self.target_module
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    fe2o3_kernel_ir::OperationKind::GuardedStore { .. }
                )
            })
            .count()
    }

    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm_ir
    }

    pub(crate) fn workgroup_sizes(&self) -> &[(String, fe2o3_kernel_ir::WorkgroupSize)] {
        &self.workgroups
    }

    pub(crate) fn semantic_function_count(&self) -> usize {
        self.admitted
            .semantic_kir()
            .semantic()
            .semantic()
            .functions()
            .len()
    }

    pub(crate) fn semantic_u32_induction_checked_addition_count(&self) -> usize {
        self.ranked_verification.checked_additions_examined()
    }

    pub(crate) fn semantic_u32_induction_certificate_count(&self) -> usize {
        self.ranked_verification.induction_certificate_count()
    }

    pub(crate) fn correspondence_block_count(&self) -> usize {
        self.admitted.semantic_kir().correspondence().blocks().len()
    }

    pub(crate) fn formal_witness_extent(&self) -> u64 {
        self.admitted.witness_extent()
    }

    pub(crate) fn formal_allocation_count(&self) -> usize {
        self.admitted
            .kernels()
            .iter()
            .map(|kernel| kernel.obligations().allocations().len())
            .sum()
    }

    pub(crate) fn formal_access_count(&self) -> usize {
        self.admitted
            .kernels()
            .iter()
            .map(|kernel| kernel.obligations().accesses().len())
            .sum()
    }

    pub(crate) fn ranked_dynamic_index_discharge_count(&self) -> usize {
        self.admitted
            .kernels()
            .iter()
            .map(|kernel| kernel.ranked_discharged_reasons().len())
            .sum()
    }

    pub(crate) fn runtime_bounds_requirement_count(&self) -> usize {
        self.admitted
            .kernels()
            .iter()
            .map(|kernel| kernel.obligations().bounds_requirements().len())
            .sum()
    }

    pub(crate) fn runtime_alias_requirement_count(&self) -> usize {
        self.admitted
            .kernels()
            .iter()
            .map(|kernel| kernel.obligations().runtime_alias_requirements().len())
            .sum()
    }

    pub(crate) fn inter_invocation_conflict_count(&self) -> usize {
        self.admitted
            .kernels()
            .iter()
            .map(|kernel| kernel.obligations().inter_invocation_conflicts().len())
            .sum()
    }

    pub(crate) fn retained_identity_and_transaction_binding_count(&self) -> usize {
        let _ = (
            &self.bindings.rustc_identity_inventory,
            &self.bindings.rustc_preflight_plan,
            &self.bindings.typed_descriptor_roots,
            &self.bindings.transaction.producer,
            &self.bindings.transaction.output_dir,
            &self.bindings.transaction.compiler_ffi_envelope,
        );
        6 + self
            .bindings
            .transaction
            .compiler_custody
            .retained_protected_binding_count()
            + self.target_verification.retained_record_count()
    }

    pub(crate) fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn target_optimization_pass_count(&self) -> usize {
        self.target_optimization.transformations().len()
    }

    pub(crate) fn target_optimization_mutating_pass_count(&self) -> usize {
        self.target_optimization
            .transformations()
            .iter()
            .filter(|record| record.changed())
            .count()
    }

    pub(crate) const fn target_optimization_initial_epoch(&self) -> u64 {
        self.target_optimization.initial_epoch()
    }

    pub(crate) const fn target_optimization_final_epoch(&self) -> u64 {
        self.target_optimization.final_epoch()
    }

    pub(crate) fn v13_target_capability_closure(
        &self,
    ) -> Result<
        &crate::production_backend_v1::ProductionBackendCapabilityClosureV1,
        ProductionFinalGraphVerificationErrorV13,
    > {
        Ok(self.target_verification.target_closure())
    }

    pub(crate) fn v13_final_graph_verification_report(
        &self,
    ) -> Result<
        &fe2o3_pliron::ProductionFinalGraphVerificationReportV1,
        ProductionFinalGraphVerificationErrorV13,
    > {
        Ok(self.target_verification.final_graph_report())
    }

    fn prepare_simulation_bundle_v8(
        &mut self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV8, ProductionPipelineError> {
        let Self {
            admitted,
            target_module,
            target_optimization,
            target_verification,
            bindings,
            ..
        } = self;
        let lowered = admitted.semantic_kir();
        lowered
            .verify_equivalence()
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        require_complete_simulation_debug_source_capture_v2(bindings.debug_capture_gap)?;
        if bindings
            .rustc_preflight_plan
            .rustc_identity_inventory_sha256()
            != bindings.rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        let production_identity = lowered.canonical_kernel_ir_identity();
        let (canonical_v13, final_epoch) = target_verification.prepare_simulation_bundle_v8_graph(
            target_module,
            production_identity,
            target_optimization,
        )?;
        let canonical_identity = *canonical_v13.identity();
        let production = fe2o3_kernel_ir::SimulationProductionKirIdentityV8::new(
            13,
            *canonical_identity.digest(),
            canonical_identity.canonical_length(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV8)?;
        let inventory_receipt =
            fe2o3_compiler_lineage::InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
                bindings.rustc_identity_inventory.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let preflight_receipt =
            fe2o3_compiler_lineage::InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
                bindings.rustc_preflight_plan.canonical_transcript(),
            )
            .map_err(ProductionPipelineError::SimulationSourceLineage)?;
        let inventory_identity = inventory_receipt.identity();
        let preflight_identity = preflight_receipt.identity();
        let lineage = fe2o3_kernel_ir::SimulationSourceLineageV1::new(
            *inventory_identity.sha256(),
            inventory_identity.byte_len(),
            *preflight_identity.sha256(),
            preflight_identity.byte_len(),
        )
        .map_err(ProductionPipelineError::SimulationBundle)?;
        let prepared = fe2o3_kernel_ir::PreparedSimulationBundleV8::new(
            lineage,
            production,
            final_epoch,
            bindings
                .rustc_target
                .backend()
                .contract()
                .canonical_target(),
            canonical_v13,
        )
        .map_err(ProductionPipelineError::SimulationBundleV8)?;
        let debug_map = compiler_debug_source_map_v2(
            lowered,
            &bindings.debug_source_files,
            &bindings.debug_source_scopes,
            &bindings.debug_source_variables,
            prepared.debug_source_map_binding(),
        )?;
        let semantic = lowered.semantic().semantic();
        let mut semantic_mir = Vec::new();
        semantic_mir
            .try_reserve_exact(semantic.canonical_encoding().len())
            .map_err(|_| {
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "semantic MIR V8 bundle allocation failed",
                )
            })?;
        semantic_mir.extend_from_slice(semantic.canonical_encoding());
        let subject = *prepared.subject_identity();
        let kir_digest = *prepared.canonical_kir_v13_digest();
        let kir_bytes = prepared.canonical_kir_v13_length();
        let legacy_storage = compiler_semantic_storage_map_v1(
            lowered,
            &bindings.debug_source_variables,
            SemanticStorageMapBindingInputV1 {
                container_identity: subject,
                subject_identity: subject,
                canonical_kir_digest: kir_digest,
                canonical_kir_bytes: kir_bytes,
            },
        )?;
        let storage = fe2o3_kernel_ir::SemanticStorageMapV8::new(
            subject,
            semantic.wire_version().as_u16(),
            *semantic.semantic_sha256().as_bytes(),
            semantic.canonical_encoding().len() as u64,
            *semantic.target_layout_identity().as_bytes(),
            kir_digest,
            kir_bytes,
            legacy_storage.kernels().to_vec(),
            legacy_storage.variables().to_vec(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV8)?;
        let legacy_aggregate = compiler_semantic_storage_map_v2(lowered, subject)?;
        let aggregate = fe2o3_kernel_ir::SemanticAggregateStorageMapV8::new(
            subject,
            kir_digest,
            kir_bytes,
            legacy_aggregate.kernels().to_vec(),
        )
        .map_err(ProductionPipelineError::SimulationBundleV8)?;
        prepared
            .finalize(debug_map, semantic_mir, storage, aggregate)
            .map_err(ProductionPipelineError::SimulationBundleV8)
    }

    fn into_simulation_bundle_v8(
        mut self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV8, ProductionPipelineError> {
        if !self
            .bindings
            .transaction
            .compiler_custody
            .is_extraction_only()
        {
            return Err(ProductionPipelineError::WorkerHandoffExtractionRequiresExtractionCustody);
        }
        self.prepare_simulation_bundle_v8()
    }

    pub(crate) fn into_inert_worker_handoff_for_extraction(
        self,
    ) -> Result<fe2o3_compiler_ffi::CompilerModuleHandoffV2, ProductionPipelineError> {
        let Self {
            admitted,
            ranked_verification: _,
            target_module,
            target_optimization: _,
            mut target_verification,
            workgroups: _,
            llvm_ir,
            bindings,
        } = self;
        let AuthenticatedProductionBindings {
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            kernel_contexts: _,
            reference_effect_bindings: _,
            debug_source_files: _,
            debug_source_scopes: _,
            debug_source_variables: _,
            debug_capture_gap: _,
            typed_descriptor_roots,
            transaction,
        } = bindings;
        if rustc_preflight_plan.rustc_identity_inventory_sha256()
            != rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        if !transaction.compiler_custody.is_extraction_only() {
            return Err(ProductionPipelineError::WorkerHandoffExtractionRequiresExtractionCustody);
        }
        target_verification
            .validate_target_module(&target_module)
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        let compiler_module = AuthenticatedProductionTargetModule {
            admitted,
            target: rustc_target.device_target(),
            target_module,
            llvm_ir,
            typed_descriptor_roots,
            compiler_ffi_envelope: transaction.compiler_ffi_envelope,
        };
        let prepared =
            crate::production_worker_handoff::prepare_production_worker_handoff_for_extraction(
                compiler_module,
            )
            .map_err(ProductionPipelineError::WorkerHandoff)?;
        let (handoff, _, _) = prepared
            .into_validated_parts()
            .map_err(ProductionPipelineError::WorkerHandoff)?;
        target_verification
            .validate_retained_records()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        Ok(handoff)
    }

    pub(crate) fn into_inert_semantic_worker_handoff_for_extraction(
        self,
        invocation: fe2o3_rustc_invocation::RustcInvocationDescriptorV3,
    ) -> Result<fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3, ProductionPipelineError>
    {
        let Self {
            admitted,
            ranked_verification,
            target_module,
            target_optimization,
            mut target_verification,
            workgroups: _,
            llvm_ir,
            bindings,
        } = self;
        let AuthenticatedProductionBindings {
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            kernel_contexts: _,
            reference_effect_bindings: _,
            debug_source_files,
            debug_source_scopes,
            debug_source_variables,
            debug_capture_gap,
            typed_descriptor_roots,
            transaction,
        } = bindings;
        if rustc_preflight_plan.rustc_identity_inventory_sha256()
            != rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        if !transaction.compiler_custody.is_extraction_only() {
            return Err(ProductionPipelineError::WorkerHandoffExtractionRequiresExtractionCustody);
        }
        let semantic_debug_inputs = prepare_production_semantic_debug_inputs_v1(
            admitted.semantic_kir(),
            &rustc_identity_inventory,
            &rustc_preflight_plan,
            &rustc_target,
            &debug_source_files,
            &debug_source_scopes,
            &debug_source_variables,
            debug_capture_gap,
        );
        let semantic_lineage =
            crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3::try_prepare(
                &rustc_identity_inventory,
                &rustc_preflight_plan,
                &rustc_target,
                &ranked_verification,
                &admitted,
                &target_module,
                &target_optimization,
                target_verification.final_v13_epoch(),
                &llvm_ir,
                semantic_debug_inputs,
            )
            .map_err(ProductionPipelineError::SemanticLineage)?;
        let target = rustc_target.device_target();
        target_verification
            .validate_target_module(&target_module)
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        let compiler_module = AuthenticatedProductionTargetModule {
            admitted,
            target,
            target_module,
            llvm_ir,
            typed_descriptor_roots,
            compiler_ffi_envelope: transaction.compiler_ffi_envelope,
        };
        let prepared =
            crate::production_worker_handoff::prepare_production_worker_handoff_for_extraction(
                compiler_module,
            )
            .map_err(ProductionPipelineError::WorkerHandoff)?;
        let (handoff, descriptor_source, _) = prepared
            .into_validated_parts()
            .map_err(ProductionPipelineError::WorkerHandoff)?;
        let handoff = semantic_lineage
            .finish_for_inert_extraction(invocation, target, &descriptor_source, handoff)
            .map_err(ProductionPipelineError::SemanticLineage)?;
        target_verification
            .validate_retained_records()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        Ok(handoff)
    }

    fn prepare_worker_handoff(
        mut self,
        final_v13_symbols: crate::single_codegen_ownership_v1::FinalV13ExpectedDeviceSymbolRosterV1,
    ) -> Result<PreparedProductionWorkerPublication, ProductionPipelineError> {
        let bundle_v8 = self.prepare_simulation_bundle_v8()?;
        let w4_diagnostic_source_map_v2 = bundle_v8.debug_map().to_vec().into_boxed_slice();
        let target_closure = self
            .v13_target_capability_closure()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        let final_graph_report = self
            .v13_final_graph_verification_report()
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        if final_graph_report.resources().closure_identity() != target_closure.identity() {
            return Err(ProductionPipelineError::FinalGraphVerificationV13(
                ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                    stage: "downstream W5 closure and W4 final-graph report",
                },
            ));
        }
        eprintln!(
            "[rustc-codegen-fe2o3] production compilation lowered {} admitted semantic function(s) into verified target-neutral Kernel IR module `{}` with {} exact block correspondence record(s), then admitted composed formal/ranked memory evidence for a {}-invocation structural witness with {} allocation(s), {} formal access(es), {} ranked dynamic-index discharge(s), {} runtime bounds requirement(s), {} runtime alias requirement(s), and {} inter-invocation conflict(s), applied {} structurally replayed target-KIR optimization pass(es), including {} mutating pass(es), across epoch {}..={}, and lowered exact target-bound KIR with ordered compiler-selected-or-retained workgroups {:?} to {} byte(s) of deterministic {} LLVM text while retaining {} identity/transaction binding(s); final-graph functional proof and W4 remain mandatory before publication; artifact/launch authority {}; preparing exact compiler-module handoff",
            self.semantic_function_count(),
            self.module().id,
            self.correspondence_block_count(),
            self.formal_witness_extent(),
            self.formal_allocation_count(),
            self.formal_access_count(),
            self.ranked_dynamic_index_discharge_count(),
            self.runtime_bounds_requirement_count(),
            self.runtime_alias_requirement_count(),
            self.inter_invocation_conflict_count(),
            self.target_optimization_pass_count(),
            self.target_optimization_mutating_pass_count(),
            self.target_optimization_initial_epoch(),
            self.target_optimization_final_epoch(),
            self.workgroup_sizes(),
            self.llvm_ir().len(),
            self.bindings
                .rustc_target
                .backend()
                .contract()
                .canonical_target(),
            self.retained_identity_and_transaction_binding_count(),
            self.grants_artifact_or_launch_authority(),
        );
        let Self {
            admitted,
            ranked_verification,
            target_module,
            target_optimization,
            target_verification,
            workgroups: _,
            llvm_ir,
            bindings,
        } = self;
        let AuthenticatedProductionBindings {
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            kernel_contexts: _,
            reference_effect_bindings,
            debug_source_files,
            debug_source_scopes,
            debug_source_variables,
            debug_capture_gap,
            typed_descriptor_roots,
            transaction,
        } = bindings;
        let ProductionTransactionBindings {
            producer,
            output_dir,
            compiler_ffi_envelope,
            compiler_custody,
        } = transaction;
        if rustc_preflight_plan.rustc_identity_inventory_sha256()
            != rustc_identity_inventory.sha256()
        {
            return Err(ProductionPipelineError::RustcLineageMismatch);
        }
        let semantic_debug_inputs = prepare_production_semantic_debug_inputs_v1(
            admitted.semantic_kir(),
            &rustc_identity_inventory,
            &rustc_preflight_plan,
            &rustc_target,
            &debug_source_files,
            &debug_source_scopes,
            &debug_source_variables,
            debug_capture_gap,
        );
        let ProtectedProductionPublicationCustody {
            attempt,
            invocation,
            compiler_execution,
        } = compiler_custody.into_publication_custody()?;
        let bundle_v8 =
            production_bundle_transaction_v8::PreparedProductionBundleTransactionV8::try_new(
                attempt, bundle_v8,
            )
            .map_err(ProductionPipelineError::ProductionBundleTransactionV8)?;
        let semantic_lineage = crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3::try_prepare(
            &rustc_identity_inventory,
            &rustc_preflight_plan,
            &rustc_target,
            &ranked_verification,
            &admitted,
            &target_module,
            &target_optimization,
            target_verification.final_v13_epoch(),
            &llvm_ir,
            semantic_debug_inputs,
        )
        .map_err(ProductionPipelineError::SemanticLineage)?;
        target_verification
            .validate_target_module(&target_module)
            .map_err(ProductionPipelineError::FinalGraphVerificationV13)?;
        let final_graph_functional = ProductionFinalGraphFunctionalCustodyV1::try_new(
            &admitted,
            target_verification,
            ranked_verification,
            reference_effect_bindings,
        )?;
        let compiler_module = AuthenticatedProductionTargetModule {
            admitted,
            target: rustc_target.device_target(),
            target_module,
            llvm_ir,
            typed_descriptor_roots,
            compiler_ffi_envelope,
        };
        let prepared = crate::production_worker_handoff::prepare_production_worker_handoff(
            compiler_module,
            final_v13_symbols,
        )
        .map_err(ProductionPipelineError::WorkerHandoff)?;
        Ok(PreparedProductionWorkerPublication {
            producer,
            output_dir,
            attempt,
            invocation,
            compiler_execution,
            semantic_lineage,
            rustc_target,
            final_graph_functional,
            w4_diagnostic_source_map_v2,
            prepared,
            bundle_v8,
        })
    }

    fn publish_worker_handoff(
        self,
        final_v13_symbols: crate::single_codegen_ownership_v1::FinalV13ExpectedDeviceSymbolRosterV1,
    ) -> Result<PublishedProductionWorkerTransaction, ProductionPipelineError> {
        let publication = self.prepare_worker_handoff(final_v13_symbols)?;
        let invocation = (*publication.invocation)
            .finish_for_publication()
            .map_err(ProductionPipelineError::ProtectedRustcInvocation)?;
        let (module_handoff, compiler_descriptor_source, generated_host_contract_identities) =
            publication
                .prepared
                .into_validated_parts()
                .map_err(ProductionPipelineError::WorkerHandoff)?;
        let compiler_policy = publication.compiler_execution.policy_identity();
        let mut functional = publication.final_graph_functional;
        functional.validate_live_owner()?;
        let finished = publication
            .semantic_lineage
            .finish_capability_v5(
                &invocation,
                publication.rustc_target.device_target(),
                &compiler_descriptor_source,
                module_handoff,
            )
            .map_err(ProductionPipelineError::SemanticLineage)?;
        let (legacy_handoff, proof_lineage, semantic_mir_identity, source_refinement) =
            finished.into_parts();
        let final_canonical = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_canonical_bytes(
            functional
                .target
                .final_canonical()
                .canonical_bytes()
                .to_vec(),
        )
        .map_err(ProductionPipelineError::TargetCapabilityAnalysis)?;
        let prepared_capability =
            crate::production_worker_handoff::prepare_production_w4_capability_carriage_v5(
                final_canonical,
                *functional.target.pre_optimization_canonical.identity(),
                functional.target.pre_optimization_epoch,
                functional.target.final_epoch,
                proof_lineage,
                semantic_mir_identity,
                compiler_policy,
                source_refinement,
                functional.target.target_closure(),
                functional.target.final_graph_report(),
                functional.target.verified_graph.module(),
                publication.w4_diagnostic_source_map_v2.into_vec(),
            )
            .map_err(ProductionPipelineError::WorkerHandoff)?;
        let (w4_subject, w4_target_context) = prepared_capability.w4_inputs();
        let w4 = functional.execute_w4_capability_witness(w4_subject, &w4_target_context)?;
        let capability_inputs = prepared_capability
            .finish(&w4)
            .map_err(ProductionPipelineError::WorkerHandoff)?;
        let w4_schedule = w4.into_analysis_schedule();
        let mut publication_owner =
            ProductionW1W6PublicationOwnerV1::try_bind(functional, w4_schedule)?;
        let capability_handoff =
            publication_owner.prepare_capability_handoff(legacy_handoff, capability_inputs)?;
        let bundle_v8 = publication
            .bundle_v8
            .bind_capability_handoff(&capability_handoff)
            .map_err(ProductionPipelineError::ProductionBundleTransactionV8)?;
        invocation
            .revalidate_for_publication()
            .map_err(ProductionPipelineError::ProtectedRustcInvocation)?;
        publication_owner.validate_for_publication()?;
        let simulation_bundle = bundle_v8
            .inert_simulation_bundle()
            .map_err(ProductionPipelineError::ProductionBundleTransactionV8)?;
        let receipt = fe2o3_artifact_transaction::publish_compiler_capability_handoff_v5(
            &publication.output_dir,
            &publication.producer,
            publication.attempt,
            &capability_handoff,
            &simulation_bundle,
        )
        .map_err(ProductionPipelineError::CapabilityV5Publication)?;
        persist_protected_compiler_completion_package_v5(
            &publication.output_dir,
            publication.attempt,
            &receipt,
            &capability_handoff,
            &simulation_bundle,
            generated_host_contract_identities.into_vec(),
        )?;
        let bundle_v8 = bundle_v8
            .bind_publication(&receipt)
            .map_err(ProductionPipelineError::ProductionBundleTransactionV8)?;
        let subject = fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1::from_capability_publication_v5(
            &receipt,
            &capability_handoff,
        )
        .map_err(ProductionPipelineError::CompilerExecutionSubject)?;
        let bundle_v8 = bundle_v8
            .bind_compiler_execution_subject(&subject)
            .map_err(ProductionPipelineError::ProductionBundleTransactionV8)?;
        let carriage = (*publication.compiler_execution)
            .acquire(subject.clone())
            .map_err(ProductionPipelineError::ProtectedCompilerExecution)?;
        let transport = fe2o3_artifact_transaction::publish_compiler_execution_receipt_transport_for_capability_v5(
            &publication.output_dir,
            &publication.producer,
            &receipt,
            &subject,
            carriage.canonical_bytes(),
        )
        .map_err(ProductionPipelineError::CompilerExecutionReceiptTransport)?;
        if transport.subject() != subject.identity()
            || transport.length() != carriage.canonical_bytes().len()
        {
            return Err(ProductionPipelineError::CompilerExecutionReceiptTransportBindingMismatch);
        }
        Ok(PublishedProductionWorkerTransaction { subject, bundle_v8 })
    }
}

fn persist_protected_compiler_completion_package_v5(
    output_dir: &Path,
    attempt: BuildAttempt,
    receipt: &fe2o3_artifact_transaction::CompilerCapabilityHandoffReceiptV5,
    handoff: &fe2o3_compiler_ffi::InertProductionCapabilityHandoffV5,
    simulation_bundle: &fe2o3_compiler_ffi::InertSimulationBundleV8,
    generated_host_contract_identities: Vec<[u8; 32]>,
) -> Result<(), ProductionPipelineError> {
    use fe2o3_artifact_transaction::{
        NoRetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1,
    };
    use fe2o3_worker_v3_verification_protocol::{
        ExactIdentityCoordinateV5, InertWorkerV3ProtectedCompilerInputV5,
        MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5, WorkerV3VerificationProductionAttemptV5,
        worker_v3_protected_compiler_input_name_v5,
        worker_v3_protected_compiler_input_redo_name_v5,
    };

    let transaction = fe2o3_compiler_ffi::InertProductionCapabilityTransactionV5::new(
        fe2o3_compiler_ffi::InertProductionCapabilityHandoffV5::decode(handoff.canonical_bytes())
            .map_err(protected_compiler_completion_package_error_v5)?,
        fe2o3_compiler_ffi::InertSimulationBundleV8::from_verified_canonical_bytes(
            simulation_bundle.identity().sha256(),
            simulation_bundle.canonical_bytes().to_vec(),
        )
        .map_err(protected_compiler_completion_package_error_v5)?,
    )
    .map_err(protected_compiler_completion_package_error_v5)?;
    if transaction.identity() != receipt.payload_identity()
        || transaction.canonical_bytes().len() != receipt.length()
        || handoff.identity() != receipt.handoff_identity()
        || simulation_bundle.identity() != receipt.simulation_bundle_identity()
    {
        return Err(ProductionPipelineError::ProtectedCompilerCompletionPackage(
            "published V5+V8 transaction coordinates changed before compiler completion".to_owned(),
        ));
    }
    let attempt = WorkerV3VerificationProductionAttemptV5::new(
        attempt.generation(),
        *attempt.session().as_bytes(),
        *attempt.invocation().as_bytes(),
    )
    .map_err(protected_compiler_completion_package_error_v5)?;
    let package = InertWorkerV3ProtectedCompilerInputV5::new(
        attempt,
        *receipt.transaction_identity().as_bytes(),
        ExactIdentityCoordinateV5::new(handoff.identity().sha256(), handoff.identity().byte_len())
            .map_err(protected_compiler_completion_package_error_v5)?,
        transaction.canonical_bytes(),
        generated_host_contract_identities,
    )
    .map_err(protected_compiler_completion_package_error_v5)?;
    let descriptor = File::open(output_dir).map(OwnedFd::from).map_err(|error| {
        ProductionPipelineError::ProtectedCompilerCompletionPackage(format!(
            "cannot retain output root: {error}",
        ))
    })?;
    let root = RetainedDurableDirectoryV1::admit_service_owned(descriptor)
        .map_err(protected_compiler_completion_package_error_v5)?;
    let canonical = worker_v3_protected_compiler_input_name_v5(attempt);
    let redo = worker_v3_protected_compiler_input_redo_name_v5(attempt);
    let bytes = package.canonical_bytes();
    let current = root
        .read_private(&canonical, MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5)
        .map_err(protected_compiler_completion_package_error_v5)?;
    let pending = root
        .read_private(&redo, MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5)
        .map_err(protected_compiler_completion_package_error_v5)?;
    if current.as_deref().is_some_and(|observed| observed != bytes)
        || pending.as_deref().is_some_and(|observed| observed != bytes)
    {
        return Err(ProductionPipelineError::ProtectedCompilerCompletionPackage(
            "attempt journal contains a different compiler-completion package".to_owned(),
        ));
    }
    let mut hooks = NoRetainedDurableDirectoryHooksV1;
    match (current.as_deref(), pending.as_deref()) {
        (Some(_), None) => Ok(()),
        (expected, Some(_)) => root
            .promote_validated_redo(
                &canonical,
                &redo,
                expected,
                bytes,
                MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5,
                &mut hooks,
            )
            .map_err(protected_compiler_completion_package_error_v5),
        (None, None) => root
            .commit_record(
                &canonical,
                &redo,
                bytes,
                MAX_WORKER_V3_PROTECTED_COMPILER_INPUT_BYTES_V5,
                &mut hooks,
            )
            .map_err(protected_compiler_completion_package_error_v5),
    }
}

fn protected_compiler_completion_package_error_v5(
    error: impl fmt::Display,
) -> ProductionPipelineError {
    ProductionPipelineError::ProtectedCompilerCompletionPackage(error.to_string())
}

fn require_complete_simulation_debug_source_capture_v2(
    gap: Option<fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1>,
) -> Result<(), ProductionPipelineError> {
    match gap {
        None => Ok(()),
        Some(gap) => Err(ProductionPipelineError::SimulationDebugSourceCaptureUnavailable(gap)),
    }
}

struct ExactDebugMapFunctionV1<'a> {
    function_ordinal: u64,
    body: &'a fe2o3_kernel_ir::FunctionBody,
    block_ordinals: BTreeMap<fe2o3_kernel_ir::BlockId, usize>,
}

fn exact_debug_map_functions_v1(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
) -> Result<
    BTreeMap<
        (
            fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
        ),
        ExactDebugMapFunctionV1<'_>,
    >,
    ProductionPipelineError,
> {
    let defined_count = lowered
        .module()
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .count();
    if defined_count == 0 || lowered.correspondence().lowered_functions().len() < defined_count {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "live correspondence does not cover every defined KIR function",
        ));
    }
    let mut layouts = BTreeMap::new();
    let mut ordinals = BTreeSet::new();
    let mut physical_functions = BTreeMap::new();
    for record in lowered.correspondence().lowered_functions() {
        let mut matches = lowered
            .module()
            .functions
            .iter()
            .enumerate()
            .filter(|(_, function)| &function.id == record.kernel_ir_function());
        let Some((ordinal, function)) = matches.next() else {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "live correspondence names an unknown KIR function",
            ));
        };
        if matches.next().is_some() || function.body.is_none() {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "live correspondence has an ambiguous KIR function owner",
            ));
        }
        let role_matches = matches!(
            (record.role(), function.role),
            (
                fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::KernelEntry,
                fe2o3_kernel_ir::FunctionRole::KernelEntry
            ) | (
                fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::InternalHelper,
                fe2o3_kernel_ir::FunctionRole::InternalHelper
            )
        );
        if !role_matches {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "live correspondence KIR function role differs",
            ));
        }
        if let Some((semantic_function, role)) =
            physical_functions.insert(ordinal, (record.semantic_function(), record.role()))
            && (semantic_function != record.semantic_function()
                || role != fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::InternalHelper
                || record.role()
                    != fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::InternalHelper)
        {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "only one exact semantic helper may share a physical KIR function",
            ));
        }
        ordinals.insert(ordinal);
        let body = function.body.as_ref().expect("body checked");
        let block_ordinals = body
            .blocks
            .iter()
            .enumerate()
            .map(|(block_ordinal, block)| (block.id, block_ordinal))
            .collect::<BTreeMap<_, _>>();
        if block_ordinals.len() != body.blocks.len() {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "KIR body has duplicate block identities",
            ));
        }
        let key = (record.correspondence_owner(), record.semantic_function());
        if layouts
            .insert(
                key,
                ExactDebugMapFunctionV1 {
                    function_ordinal: u64::try_from(ordinal).map_err(|_| {
                        ProductionPipelineError::SimulationDebugMapCorrespondence(
                            "KIR function ordinal does not fit the source-map wire",
                        )
                    })?,
                    body,
                    block_ordinals,
                },
            )
            .is_some()
        {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "live correspondence has a duplicate semantic function owner",
            ));
        }
    }
    if ordinals.len() != defined_count {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "live correspondence omits a defined KIR function",
        ));
    }
    Ok(layouts)
}

fn kernel_storage_map_body_v1(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    selected_root: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    selected_body: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
) -> Result<(usize, &fe2o3_kernel_ir::FunctionBody), ProductionPipelineError> {
    let layouts = exact_debug_map_functions_v1(lowered)?;
    let mut entries = lowered
        .correspondence()
        .lowered_functions()
        .iter()
        .filter(|record| {
            record.role() == fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::KernelEntry
                && record.correspondence_owner() == selected_root
                && record.semantic_function() == selected_body
        });
    let entry = entries
        .next()
        .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "selected semantic kernel has no exact KIR function correspondence",
        ))?;
    if entries.next().is_some() {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "selected semantic kernel has ambiguous KIR function correspondence",
        ));
    }
    let layout = layouts
        .get(&(entry.correspondence_owner(), entry.semantic_function()))
        .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "selected semantic kernel has no exact KIR function layout",
        ))?;
    let ordinal = usize::try_from(layout.function_ordinal).map_err(|_| {
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "selected KIR function ordinal does not fit this host",
        )
    })?;
    Ok((ordinal, layout.body))
}

fn prepare_production_semantic_debug_inputs_v1(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    rustc_identity_inventory: &crate::collector::AuthenticatedRustcIdentityInventoryV3,
    rustc_preflight_plan: &crate::collector::AuthenticatedRustcPreflightPlanV3,
    rustc_target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    captured_files: &[fe2o3_kernel_ir::DebugSourceMapFileV1],
    captured_scopes: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceScopeV2],
    captured_variables: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableV2],
    capture_gap: Option<fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1>,
) -> crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1 {
    if let Some(gap) = capture_gap {
        return crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::unavailable(
            gap,
        );
    }
    match compiler_production_semantic_debug_source_map_v1(
        lowered,
        rustc_identity_inventory,
        rustc_preflight_plan,
        rustc_target,
        captured_files,
        captured_scopes,
        captured_variables,
    ) {
        Ok((source_map, canonical_kir_v7)) => {
            crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::Available {
                source_map: Box::new(source_map),
                canonical_kir_v7,
            }
        }
        Err(
            ProductionPipelineError::SimulationProductionKirV9
            | ProductionPipelineError::SimulationProductionKirV11,
        ) => {
            crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::unavailable(
                fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable,
            )
        }
        Err(
            ProductionPipelineError::SimulationDebugMap(
                fe2o3_kernel_ir::DebugSourceMapErrorV1::InvalidLength
                | fe2o3_kernel_ir::DebugSourceMapErrorV1::ResourceLimit
                | fe2o3_kernel_ir::DebugSourceMapErrorV1::AllocationFailure
                | fe2o3_kernel_ir::DebugSourceMapErrorV1::Encoding,
            )
            | ProductionPipelineError::SimulationDebugMapV2(
                fe2o3_kernel_ir::DebugSourceMapErrorV2::InvalidLength
                | fe2o3_kernel_ir::DebugSourceMapErrorV2::ResourceLimit
                | fe2o3_kernel_ir::DebugSourceMapErrorV2::AllocationFailure
                | fe2o3_kernel_ir::DebugSourceMapErrorV2::Encoding,
            ),
        ) => crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::unavailable(
            fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::ResourceLimit,
        ),
        Err(_) => crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::unavailable(
            fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn compiler_production_semantic_debug_source_map_v1(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    rustc_identity_inventory: &crate::collector::AuthenticatedRustcIdentityInventoryV3,
    rustc_preflight_plan: &crate::collector::AuthenticatedRustcPreflightPlanV3,
    rustc_target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    captured_files: &[fe2o3_kernel_ir::DebugSourceMapFileV1],
    captured_scopes: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceScopeV2],
    captured_variables: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableV2],
) -> Result<
    (
        fe2o3_kernel_ir::DebugSourceMapDocumentV2,
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7,
    ),
    ProductionPipelineError,
> {
    let production_identity = lowered.canonical_kernel_ir_identity();
    let production_identity = match production_identity.version() {
        fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V8 => {
            fe2o3_kernel_ir::SimulationProductionKirIdentityV1::v8(
                *production_identity.digest(),
                production_identity.canonical_length(),
            )
            .map_err(ProductionPipelineError::SimulationBundle)?
        }
        fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V9 => {
            return Err(ProductionPipelineError::SimulationProductionKirV9);
        }
        fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V11 => {
            return Err(ProductionPipelineError::SimulationProductionKirV11);
        }
        fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V12 => {
            return Err(ProductionPipelineError::SimulationProductionKirV12);
        }
        fe2o3_lower_mir_kernel::ProductionCanonicalKernelIrVersionV1::V13 => {
            return Err(ProductionPipelineError::SimulationProductionKirV13);
        }
    };
    let canonical_kir =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7::from_module(lowered.module().clone())
            .map_err(ProductionPipelineError::SimulationKernelIrV7)?;
    let mut prepared_kir_bytes = Vec::new();
    prepared_kir_bytes
        .try_reserve_exact(canonical_kir.canonical_bytes().len())
        .map_err(|_| {
            ProductionPipelineError::SemanticDebugFragment(
                fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::AllocationFailure,
            )
        })?;
    prepared_kir_bytes.extend_from_slice(canonical_kir.canonical_bytes());
    let prepared_kir =
        fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7::from_canonical_bytes(prepared_kir_bytes)
            .map_err(ProductionPipelineError::SimulationKernelIrV7)?;
    let inventory_receipt =
        fe2o3_compiler_lineage::InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
            rustc_identity_inventory.canonical_transcript(),
        )
        .map_err(ProductionPipelineError::SimulationSourceLineage)?;
    let preflight_receipt =
        fe2o3_compiler_lineage::InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
            rustc_preflight_plan.canonical_transcript(),
        )
        .map_err(ProductionPipelineError::SimulationSourceLineage)?;
    let inventory_identity = inventory_receipt.identity();
    let preflight_identity = preflight_receipt.identity();
    let lineage = fe2o3_kernel_ir::SimulationSourceLineageV1::new(
        *inventory_identity.sha256(),
        inventory_identity.byte_len(),
        *preflight_identity.sha256(),
        preflight_identity.byte_len(),
    )
    .map_err(ProductionPipelineError::SimulationBundle)?;
    let prepared = fe2o3_kernel_ir::PreparedSimulationBundleV1::new(
        fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
        lineage,
        production_identity,
        rustc_target.backend().contract().canonical_target(),
        prepared_kir,
    )
    .map_err(ProductionPipelineError::SimulationBundle)?;
    let source_map = compiler_debug_source_map_v2(
        lowered,
        captured_files,
        captured_scopes,
        captured_variables,
        prepared.debug_source_map_binding(),
    )?;
    Ok((source_map, canonical_kir))
}

fn compiler_debug_source_map_v1(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    captured_files: &[fe2o3_kernel_ir::DebugSourceMapFileV1],
    binding: fe2o3_kernel_ir::DebugSourceMapBindingV1,
) -> Result<fe2o3_kernel_ir::DebugSourceMapDocumentV1, ProductionPipelineError> {
    let function_layouts = exact_debug_map_functions_v1(lowered)?;

    let mut mapped = BTreeMap::new();
    let mut eliminated = BTreeSet::new();
    for span in lowered.correspondence().statement_operation_spans() {
        let layout = function_layouts
            .get(&(span.correspondence_owner(), span.semantic_function()))
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "statement correspondence has no exact KIR function owner",
            ))?;
        let source = lowered
            .semantic()
            .resolve_statement(
                span.semantic_function(),
                span.semantic_block(),
                span.statement_ordinal(),
            )
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "statement correspondence does not resolve in retained semantic MIR",
            ))?
            .source();
        insert_debug_operation_range_v1(
            layout.function_ordinal,
            layout.body,
            &layout.block_ordinals,
            (
                1,
                span.semantic_function().index(),
                span.semantic_block().index(),
                span.statement_ordinal(),
            ),
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
            source,
            &mut mapped,
            &mut eliminated,
        )?;
    }
    for span in lowered.correspondence().terminator_operation_spans() {
        let layout = function_layouts
            .get(&(span.correspondence_owner(), span.semantic_function()))
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "terminator correspondence has no exact KIR function owner",
            ))?;
        let source = lowered
            .semantic()
            .resolve_terminator(span.semantic_function(), span.semantic_block())
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "terminator correspondence does not resolve in retained semantic MIR",
            ))?
            .source();
        insert_debug_operation_range_v1(
            layout.function_ordinal,
            layout.body,
            &layout.block_ordinals,
            (
                2,
                span.semantic_function().index(),
                span.semantic_block().index(),
                0,
            ),
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
            source,
            &mut mapped,
            &mut eliminated,
        )?;
    }

    let mut synthetic = BTreeMap::new();
    for span in lowered.correspondence().synthetic_operation_spans() {
        let layout = function_layouts
            .get(&(span.correspondence_owner(), span.semantic_function()))
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "synthetic correspondence has no exact KIR function owner",
            ))?;
        let block_ordinal = debug_block_ordinal_v1(
            layout.body,
            &layout.block_ordinals,
            span.kernel_ir_block(),
            span.first_operation_ordinal(),
            span.operation_count(),
        )?;
        for operation in span.first_operation_ordinal()
            ..span
                .first_operation_ordinal()
                .checked_add(span.operation_count())
                .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "synthetic KIR operation range overflows",
                ))?
        {
            let site = fe2o3_kernel_ir::DebugSourceMapKirSiteV1::operation(
                layout.function_ordinal,
                block_ordinal,
                u64::from(operation),
            );
            let rule = match span.rule() {
                fe2o3_lower_mir_kernel::SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage => {
                    3
                }
                fe2o3_lower_mir_kernel::SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage => {
                    1
                }
                fe2o3_lower_mir_kernel::SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap => {
                    2
                }
            };
            let owner = (span.semantic_function().index(), rule);
            if mapped.contains_key(&site)
                || synthetic
                    .insert(site, owner)
                    .is_some_and(|previous| previous != owner)
            {
                return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "synthetic and semantic operation ranges overlap",
                ));
            }
        }
    }
    for layout in function_layouts.values() {
        for (block_ordinal, block) in layout.body.blocks.iter().enumerate() {
            for operation_ordinal in 0..block.operations.len() {
                let site = fe2o3_kernel_ir::DebugSourceMapKirSiteV1::operation(
                    layout.function_ordinal,
                    u64::try_from(block_ordinal).map_err(|_| {
                        ProductionPipelineError::SimulationDebugMapCorrespondence(
                            "KIR block ordinal does not fit the source-map wire",
                        )
                    })?,
                    u64::try_from(operation_ordinal).map_err(|_| {
                        ProductionPipelineError::SimulationDebugMapCorrespondence(
                            "KIR operation ordinal does not fit the source-map wire",
                        )
                    })?,
                );
                if mapped.contains_key(&site) == synthetic.contains_key(&site) {
                    return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                        "KIR operation is not covered exactly once by semantic or synthetic correspondence",
                    ));
                }
            }
        }
    }

    let referenced_files = mapped
        .values()
        .map(|(_, span)| span)
        .chain(&eliminated)
        .map(|span| span.file_identity())
        .collect::<BTreeSet<_>>();
    let captured_files = captured_files
        .iter()
        .map(|file| (file.identity(), file))
        .collect::<BTreeMap<_, _>>();
    let files = referenced_files
        .into_iter()
        .map(|identity| {
            captured_files.get(&identity).cloned().cloned().ok_or(
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "semantic source span has no same-session rustc file observation",
                ),
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let sites = mapped
        .into_iter()
        .map(|(site, (_, span))| {
            fe2o3_kernel_ir::DebugSourceMapSiteV1::new(site, vec![span])
                .map_err(ProductionPipelineError::SimulationDebugMap)
        })
        .collect::<Result<Vec<_>, _>>()?;
    fe2o3_kernel_ir::DebugSourceMapDocumentV1::new(
        binding,
        files,
        sites,
        eliminated.into_iter().collect(),
    )
    .map_err(ProductionPipelineError::SimulationDebugMap)
}

fn compiler_debug_source_map_v2(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    captured_files: &[fe2o3_kernel_ir::DebugSourceMapFileV1],
    captured_scopes: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceScopeV2],
    captured_variables: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableV2],
    binding: fe2o3_kernel_ir::DebugSourceMapBindingV1,
) -> Result<fe2o3_kernel_ir::DebugSourceMapDocumentV2, ProductionPipelineError> {
    let base = compiler_debug_source_map_v1(lowered, captured_files, binding)?;
    let function_layouts = exact_debug_map_functions_v1(lowered)?;
    let mut function_by_semantic = BTreeMap::new();
    for ((_, semantic_function), layout) in &function_layouts {
        if let Some(previous) =
            function_by_semantic.insert(*semantic_function, layout.function_ordinal)
            && previous != layout.function_ordinal
        {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "one semantic function maps to different physical KIR functions",
            ));
        }
    }

    let mut parameter_by_local = BTreeMap::new();
    for binding in lowered.correspondence().parameter_bindings() {
        if !function_layouts
            .contains_key(&(binding.correspondence_owner(), binding.semantic_function()))
        {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "KIR parameter correspondence has no exact function instance",
            ));
        }
        let key = (binding.semantic_function(), binding.semantic_local());
        if let Some(previous) = parameter_by_local.insert(key, binding.kernel_ir_value())
            && previous != binding.kernel_ir_value()
        {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "shared helper parameter correspondence differs across roots",
            ));
        }
    }

    let selected_scope_count = captured_scopes
        .iter()
        .filter(|scope| function_by_semantic.contains_key(&scope.function))
        .count();
    if selected_scope_count > fe2o3_kernel_ir::MAX_DEBUG_SOURCE_SCOPES_V2 {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "compiler source scopes exceed the bounded V2 map domain",
        ));
    }
    let mut scopes = Vec::new();
    scopes
        .try_reserve_exact(selected_scope_count)
        .map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "compiler source-scope map allocation failed",
            )
        })?;
    for scope in captured_scopes
        .iter()
        .filter(|scope| function_by_semantic.contains_key(&scope.function))
    {
        let function_ordinal = *function_by_semantic.get(&scope.function).ok_or(
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "compiler source scope has no exact KIR function owner",
            ),
        )?;
        scopes.push(
            fe2o3_kernel_ir::DebugSourceScopeV2::new(
                scope.identity,
                function_ordinal,
                scope.parent_identity,
                scope.depth,
                debug_source_scope_span_v2(scope.source)?,
            )
            .map_err(ProductionPipelineError::SimulationDebugMapV2)?,
        );
    }
    let scope_identities = scopes
        .iter()
        .map(|scope| scope.identity())
        .collect::<BTreeSet<_>>();

    let selected_variable_count = captured_variables
        .iter()
        .filter(|variable| function_by_semantic.contains_key(&variable.function))
        .count();
    if selected_variable_count > fe2o3_kernel_ir::MAX_DEBUG_SOURCE_VARIABLES_V2 {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "compiler source variables exceed the bounded V2 map domain",
        ));
    }
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(selected_variable_count)
        .map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "compiler source-variable map allocation failed",
            )
        })?;
    for variable in captured_variables
        .iter()
        .filter(|variable| function_by_semantic.contains_key(&variable.function))
    {
        let function_ordinal = *function_by_semantic.get(&variable.function).ok_or(
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "compiler source variable has no exact KIR function owner",
            ),
        )?;
        if !scope_identities.contains(&variable.scope_identity) {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "compiler source variable references an unretained lexical scope",
            ));
        }
        let name = variable.name.clone().ok_or(
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "rustc source-variable name is empty, control-containing, or exceeds the V2 bound",
            ),
        )?;
        let (fallback, parameter) = match variable.class {
            crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableClassV2::Local(local) => {
                match parameter_by_local
                    .get(&(variable.function, local))
                    .copied()
                    .filter(|_| variable.entry_value_preserved)
                {
                    Some(value) => (
                        fe2o3_kernel_ir::DebugSourceVariableFallbackV2::NotInScope,
                        Some(value),
                    ),
                    None => (
                        fe2o3_kernel_ir::DebugSourceVariableFallbackV2::Unrepresented,
                        None,
                    ),
                }
            }
            crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableClassV2::Unrepresented => (
                fe2o3_kernel_ir::DebugSourceVariableFallbackV2::Unrepresented,
                None,
            ),
        };
        let mut emitted = fe2o3_kernel_ir::DebugSourceVariableV2::new(
            variable.identity,
            name,
            function_ordinal,
            variable.scope_identity,
            fallback,
            Vec::new(),
        )
        .map_err(ProductionPipelineError::SimulationDebugMapV2)?;
        if let Some(value) = parameter {
            emitted = emitted
                .with_function_binding(
                    fe2o3_kernel_ir::DebugSourceVariableFunctionBindingV2::new(
                        1,
                        u64::from(value.0),
                    )
                    .map_err(ProductionPipelineError::SimulationDebugMapV2)?,
                )
                .map_err(ProductionPipelineError::SimulationDebugMapV2)?;
        }
        variables.push(emitted);
    }

    let captured_files = captured_files
        .iter()
        .map(|file| (file.identity(), file.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut files = base
        .files()
        .iter()
        .map(|file| (file.identity(), file.clone()))
        .collect::<BTreeMap<_, _>>();
    for scope in &scopes {
        let identity = scope.span().file_identity();
        if let std::collections::btree_map::Entry::Vacant(entry) = files.entry(identity) {
            let file = captured_files.get(&identity).cloned().ok_or(
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "source-variable scope has no same-session rustc file observation",
                ),
            )?;
            entry.insert(file);
        }
    }
    let mut file_values = Vec::new();
    file_values.try_reserve_exact(files.len()).map_err(|_| {
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "compiler source-map V2 file allocation failed",
        )
    })?;
    file_values.extend(files.into_values());
    let mut sites = Vec::new();
    sites.try_reserve_exact(base.sites().len()).map_err(|_| {
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "compiler source-map V2 site allocation failed",
        )
    })?;
    sites.extend_from_slice(base.sites());
    let mut eliminated = Vec::new();
    eliminated
        .try_reserve_exact(base.eliminated().len())
        .map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "compiler source-map V2 eliminated-span allocation failed",
            )
        })?;
    eliminated.extend_from_slice(base.eliminated());
    fe2o3_kernel_ir::DebugSourceMapDocumentV2::new(
        base.binding(),
        file_values,
        sites,
        eliminated,
        scopes,
        variables,
    )
    .map_err(ProductionPipelineError::SimulationDebugMapV2)
}

#[derive(Clone, Copy)]
struct SemanticStorageMapBindingInputV1 {
    container_identity: [u8; 32],
    subject_identity: [u8; 32],
    canonical_kir_digest: [u8; 32],
    canonical_kir_bytes: u64,
}

fn compiler_semantic_storage_map_v1(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    captured_variables: &[crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableV2],
    binding: SemanticStorageMapBindingInputV1,
) -> Result<fe2o3_kernel_ir::SemanticStorageMapV1, ProductionPipelineError> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiPassModeV1, SemanticLocalRoleV1, SemanticSourceArgumentOwnershipV1,
    };

    let semantic = lowered.semantic().semantic();
    let selection = semantic.select_kernel_body_v1().ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage map requires one exact semantic kernel body",
        ),
    )?;
    let function = semantic
        .functions()
        .get(selection.body().index() as usize)
        .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage map semantic body is absent",
        ))?;
    let (kir_function_ordinal, kir_body) =
        kernel_storage_map_body_v1(lowered, selection.root(), selection.body())?;
    let kir_function = lowered.module().functions.get(kir_function_ordinal).ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage map KIR function is absent",
        ),
    )?;
    if kir_body.parameters.len() != kir_function.signature.parameters.len() {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage map KIR parameter identities and types differ in length",
        ));
    }

    let parameter_bindings = lowered
        .correspondence()
        .parameter_bindings()
        .iter()
        .copied()
        .filter(|binding| binding.semantic_function() == selection.body())
        .collect::<Vec<_>>();
    let source_types = function.abi().source_input_types();
    let ownership = function.abi().source_argument_ownership();
    if source_types.len() != ownership.len() {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage map source types and ownership differ in length",
        ));
    }
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(source_types.len())
        .map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "typed storage argument allocation failed",
            )
        })?;
    for (source_ordinal, (&semantic_type, &source_ownership)) in
        source_types.iter().zip(ownership).enumerate()
    {
        let source_ordinal = u32::try_from(source_ordinal).map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "typed storage source ordinal does not fit the wire",
            )
        })?;
        let semantic_local = function
            .locals()
            .iter()
            .enumerate()
            .find_map(|(index, local)| {
                (local.role() == SemanticLocalRoleV1::Argument(source_ordinal)).then_some(index)
            })
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "typed storage source argument has no exact semantic local",
            ))?;
        let semantic_local = u32::try_from(semantic_local).map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "typed storage semantic local does not fit the wire",
            )
        })?;
        let abi_ignored = function
            .abi()
            .adjusted_arguments()
            .get(source_ordinal as usize)
            .is_some_and(|argument| matches!(argument.mode(), SemanticAbiPassModeV1::Ignore));
        let storage = compiler_parameter_storage_v1(
            semantic_local,
            semantic_type.index(),
            source_ownership,
            abi_ignored,
            &parameter_bindings,
            kir_body,
            &kir_function.signature.parameters,
            semantic.types(),
        )?;
        arguments.push(fe2o3_kernel_ir::SemanticArgumentStorageV1::new(
            source_ordinal,
            semantic_local,
            semantic_type.index(),
            compiler_ownership_v1(source_ownership)?,
            storage,
        ));
    }

    let mut variables = Vec::new();
    let selected_variable_count = captured_variables
        .iter()
        .filter(|variable| variable.function == selection.body())
        .count();
    variables
        .try_reserve_exact(selected_variable_count)
        .map_err(|_| {
            ProductionPipelineError::SimulationDebugMapCorrespondence(
                "typed storage variable allocation failed",
            )
        })?;
    for variable in captured_variables
        .iter()
        .filter(|variable| variable.function == selection.body())
    {
        let (semantic_local, semantic_type, storage) = match variable.class {
            crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableClassV2::Local(local) => {
                let declaration = function.locals().get(local.index() as usize).ok_or(
                    ProductionPipelineError::SimulationDebugMapCorrespondence(
                        "typed source variable references an absent semantic local",
                    ),
                )?;
                let variable_ownership = match declaration.role() {
                    SemanticLocalRoleV1::Argument(source_ordinal) => ownership
                        .get(source_ordinal as usize)
                        .copied()
                        .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                            "typed source variable argument ownership is absent",
                        ))?,
                    SemanticLocalRoleV1::Return | SemanticLocalRoleV1::Temporary => {
                        SemanticSourceArgumentOwnershipV1::ByValue
                    }
                };
                let storage = if variable.entry_value_preserved {
                    compiler_parameter_storage_v1(
                        local.index(),
                        declaration.ty().index(),
                        variable_ownership,
                        false,
                        &parameter_bindings,
                        kir_body,
                        &kir_function.signature.parameters,
                        semantic.types(),
                    )?
                } else {
                    fe2o3_kernel_ir::SemanticStorageBindingV1::Unavailable {
                        reason: fe2o3_kernel_ir::SemanticStorageUnavailableReasonV1::OptimizedOut,
                    }
                };
                (Some(local.index()), Some(declaration.ty().index()), storage)
            }
            crate::rustc_semantic_plan_v1::RetainedDebugSourceVariableClassV2::Unrepresented => (
                None,
                None,
                fe2o3_kernel_ir::SemanticStorageBindingV1::Unavailable {
                    reason: fe2o3_kernel_ir::SemanticStorageUnavailableReasonV1::UnrepresentedSourceVariable,
                },
            ),
        };
        variables.push(fe2o3_kernel_ir::SemanticVariableStorageV1::new(
            variable.identity,
            variable.function.index(),
            semantic_local,
            semantic_type,
            storage,
        ));
    }

    fe2o3_kernel_ir::SemanticStorageMapV1::new(
        binding.container_identity,
        binding.subject_identity,
        semantic.wire_version().as_u16(),
        *semantic.semantic_sha256().as_bytes(),
        semantic.canonical_encoding().len() as u64,
        *semantic.target_layout_identity().as_bytes(),
        binding.canonical_kir_digest,
        binding.canonical_kir_bytes,
        vec![fe2o3_kernel_ir::SemanticKernelStorageV1::new(
            selection.root().index(),
            selection.body().index(),
            u32::try_from(kir_function_ordinal).map_err(|_| {
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "typed storage KIR function ordinal does not fit the wire",
                )
            })?,
            arguments,
        )],
        variables,
    )
    .map_err(ProductionPipelineError::SimulationBundleV3)
}

fn compiler_semantic_storage_map_v2(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    container_identity: [u8; 32],
) -> Result<fe2o3_kernel_ir::SemanticStorageMapV2, ProductionPipelineError> {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticAbiPassModeV1, SemanticLocalRoleV1};

    const MAP_ERROR: &str = "aggregate storage map is not exact";
    let semantic = lowered.semantic().semantic();
    let selection = semantic.select_kernel_body_v1().ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR),
    )?;
    let function = semantic
        .functions()
        .get(selection.body().index() as usize)
        .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
            MAP_ERROR,
        ))?;
    let (kir_function_ordinal, kir_body) =
        kernel_storage_map_body_v1(lowered, selection.root(), selection.body())?;
    let kir_function = lowered.module().functions.get(kir_function_ordinal).ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR),
    )?;
    if kir_body.parameters.len() != kir_function.signature.parameters.len() {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            MAP_ERROR,
        ));
    }

    let mut slots = Vec::new();
    slots
        .try_reserve_exact(kir_function.signature.parameters.len())
        .map_err(|_| ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR))?;
    let mut next = 0_u32;
    let mut kernarg_alignment = 1_u32;
    for ty in &kir_function.signature.parameters {
        let (width, alignment, metadata_relative) = match ty {
            fe2o3_kernel_ir::Type::Scalar(scalar) => {
                let width = match scalar {
                    fe2o3_kernel_ir::ScalarType::Bool
                    | fe2o3_kernel_ir::ScalarType::I8
                    | fe2o3_kernel_ir::ScalarType::U8 => 1,
                    fe2o3_kernel_ir::ScalarType::I16
                    | fe2o3_kernel_ir::ScalarType::U16
                    | fe2o3_kernel_ir::ScalarType::F16
                    | fe2o3_kernel_ir::ScalarType::Bf16 => 2,
                    fe2o3_kernel_ir::ScalarType::I32
                    | fe2o3_kernel_ir::ScalarType::U32
                    | fe2o3_kernel_ir::ScalarType::F32 => 4,
                    fe2o3_kernel_ir::ScalarType::I64
                    | fe2o3_kernel_ir::ScalarType::U64
                    | fe2o3_kernel_ir::ScalarType::F64
                    | fe2o3_kernel_ir::ScalarType::Index => 8,
                    fe2o3_kernel_ir::ScalarType::I128 | fe2o3_kernel_ir::ScalarType::U128 => 16,
                };
                (width, width, None)
            }
            fe2o3_kernel_ir::Type::Pointer(_) => (8, 8, None),
            fe2o3_kernel_ir::Type::Slice(_) => (8, 8, Some(8)),
            fe2o3_kernel_ir::Type::Unit => {
                return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "unit KIR parameters have no physical simulator slot",
                ));
            }
            fe2o3_kernel_ir::Type::KernelContext(_)
            | fe2o3_kernel_ir::Type::GlobalCapability(_)
            | fe2o3_kernel_ir::Type::ExecutionCapability(_) => {
                return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "logical capability KIR parameters have no legacy physical simulator slot",
                ));
            }
        };
        next = align_up_u32_v1(next, alignment).ok_or(
            ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR),
        )?;
        let value = fe2o3_kernel_ir::SemanticKernargSlotV2::new(next, width, alignment);
        let metadata = metadata_relative
            .map(|relative| {
                next.checked_add(relative)
                    .map(|offset| {
                        fe2o3_kernel_ir::SemanticKernargSlotV2::new(offset, width, alignment)
                    })
                    .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                        MAP_ERROR,
                    ))
            })
            .transpose()?;
        let total_width = metadata_relative.map_or(width, |relative| relative + width);
        next = next.checked_add(total_width).ok_or(
            ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR),
        )?;
        kernarg_alignment = kernarg_alignment.max(alignment);
        slots.push((value, metadata));
    }
    let explicit_kernarg_bytes = align_up_u32_v1(next, kernarg_alignment).ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR),
    )?;

    let source_types = function.abi().source_input_types();
    let ownership = function.abi().source_argument_ownership();
    if source_types.len() != ownership.len()
        || source_types.len() != function.abi().adjusted_arguments().len()
    {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            MAP_ERROR,
        ));
    }
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(source_types.len())
        .map_err(|_| ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR))?;
    for (source_ordinal, ((&semantic_type, &source_ownership), abi)) in source_types
        .iter()
        .zip(ownership)
        .zip(function.abi().adjusted_arguments())
        .enumerate()
    {
        let source_ordinal = u32::try_from(source_ordinal)
            .map_err(|_| ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR))?;
        let semantic_local = function
            .locals()
            .iter()
            .enumerate()
            .find_map(|(local, declaration)| {
                (declaration.role() == SemanticLocalRoleV1::Argument(source_ordinal))
                    .then_some(local)
            })
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                MAP_ERROR,
            ))?;
        let semantic_local_u32 = u32::try_from(semantic_local)
            .map_err(|_| ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR))?;
        let direct_matches = lowered
            .correspondence()
            .parameter_bindings()
            .iter()
            .filter(|binding| {
                binding.semantic_function() == selection.body()
                    && binding.semantic_local().index() == semantic_local_u32
            })
            .copied();
        let mut direct_probe = direct_matches.clone();
        let direct = direct_probe.next();
        let direct_is_unique = direct.is_some() && direct_probe.next().is_none();
        let component_matches = lowered
            .correspondence()
            .parameter_component_bindings()
            .iter()
            .filter(|binding| {
                binding.semantic_function() == selection.body()
                    && binding.semantic_local().index() == semantic_local_u32
            });
        let component_count = component_matches.clone().count();
        let ignored_matches = lowered
            .correspondence()
            .ignored_parameter_bindings()
            .iter()
            .filter(|binding| {
                binding.semantic_function() == selection.body()
                    && binding.semantic_local().index() == semantic_local_u32
            })
            .copied();
        let mut ignored_probe = ignored_matches.clone();
        let ignored = ignored_probe.next();
        let ignored_is_unique = ignored.is_some() && ignored_probe.next().is_none();
        let storage =
            if direct_is_unique {
                let binding = direct.expect("unique direct binding is present");
                if component_count != 0 || ignored.is_some() {
                    return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                        MAP_ERROR,
                    ));
                }
                let mut retained = Vec::new();
                retained.try_reserve_exact(1).map_err(|_| {
                    ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR)
                })?;
                retained.push(compiler_component_storage_v2(
                    Vec::new(),
                    binding.kernel_ir_value(),
                    kir_body,
                    &kir_function.signature.parameters,
                    &slots,
                )?);
                fe2o3_kernel_ir::SemanticComponentStorageBindingV2::exact(retained)
            } else if direct.is_none() && component_count != 0 && ignored.is_none() {
                let mut retained = Vec::new();
                retained.try_reserve_exact(component_count).map_err(|_| {
                    ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR)
                })?;
                for component in component_matches {
                    let mut path = Vec::new();
                    path.try_reserve_exact(component.projection().len())
                        .map_err(|_| {
                            ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR)
                        })?;
                    path.extend(
                        component.projection().iter().map(|projection| {
                            match projection {
                    fe2o3_lower_mir_kernel::SemanticKirParameterProjectionV1::Field(index) => {
                        fe2o3_kernel_ir::SemanticStorageProjectionV2::Field { index: *index }
                    }
                    fe2o3_lower_mir_kernel::SemanticKirParameterProjectionV1::ArrayIndex(index) => {
                        fe2o3_kernel_ir::SemanticStorageProjectionV2::ArrayElement {
                            index: u64::from(*index),
                        }
                    }
                }
                        }),
                    );
                    retained.push(compiler_component_storage_v2(
                        path,
                        component.kernel_ir_value(),
                        kir_body,
                        &kir_function.signature.parameters,
                        &slots,
                    )?);
                }
                fe2o3_kernel_ir::SemanticComponentStorageBindingV2::exact(retained)
            } else if direct.is_none()
                && component_count == 0
                && ignored_is_unique
                && matches!(abi.mode(), SemanticAbiPassModeV1::Ignore)
            {
                fe2o3_kernel_ir::SemanticComponentStorageBindingV2::exact(Vec::new())
            } else {
                return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                    MAP_ERROR,
                ));
            };
        arguments.push(fe2o3_kernel_ir::SemanticArgumentStorageV2::new(
            source_ordinal,
            semantic_local_u32,
            semantic_type.index(),
            compiler_ownership_v1(source_ownership)?,
            storage,
        ));
    }

    let mut kernels = Vec::new();
    kernels
        .try_reserve_exact(1)
        .map_err(|_| ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR))?;
    kernels.push(fe2o3_kernel_ir::SemanticKernelStorageV2::new(
        selection.root().index(),
        selection.body().index(),
        u32::try_from(kir_function_ordinal)
            .map_err(|_| ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR))?,
        explicit_kernarg_bytes,
        kernarg_alignment,
        arguments,
    ));
    fe2o3_kernel_ir::SemanticStorageMapV2::new(container_identity, kernels)
        .map_err(ProductionPipelineError::SimulationBundleV4)
}

fn compiler_component_storage_v2(
    path: Vec<fe2o3_kernel_ir::SemanticStorageProjectionV2>,
    value: fe2o3_kernel_ir::ValueId,
    body: &fe2o3_kernel_ir::FunctionBody,
    parameter_types: &[fe2o3_kernel_ir::Type],
    slots: &[(
        fe2o3_kernel_ir::SemanticKernargSlotV2,
        Option<fe2o3_kernel_ir::SemanticKernargSlotV2>,
    )],
) -> Result<fe2o3_kernel_ir::SemanticKirComponentStorageV2, ProductionPipelineError> {
    const MAP_ERROR: &str = "aggregate component has no exact KIR physical slot";
    let ordinal = body
        .parameters
        .iter()
        .position(|candidate| *candidate == value)
        .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
            MAP_ERROR,
        ))?;
    let (representation, expected_metadata) = match parameter_types.get(ordinal) {
        Some(fe2o3_kernel_ir::Type::Scalar(_)) => (
            fe2o3_kernel_ir::SemanticKirComponentRepresentationV2::ScalarValue,
            false,
        ),
        Some(fe2o3_kernel_ir::Type::Pointer(_)) => (
            fe2o3_kernel_ir::SemanticKirComponentRepresentationV2::RegionPointer,
            false,
        ),
        Some(fe2o3_kernel_ir::Type::Slice(_)) => (
            fe2o3_kernel_ir::SemanticKirComponentRepresentationV2::RegionSlice,
            true,
        ),
        Some(
            fe2o3_kernel_ir::Type::Unit
            | fe2o3_kernel_ir::Type::KernelContext(_)
            | fe2o3_kernel_ir::Type::GlobalCapability(_)
            | fe2o3_kernel_ir::Type::ExecutionCapability(_),
        )
        | None => {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                MAP_ERROR,
            ));
        }
    };
    let (value_slot, metadata_slot) = slots.get(ordinal).copied().ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR),
    )?;
    if metadata_slot.is_some() != expected_metadata {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            MAP_ERROR,
        ));
    }
    Ok(fe2o3_kernel_ir::SemanticKirComponentStorageV2::new(
        path,
        u32::try_from(ordinal)
            .map_err(|_| ProductionPipelineError::SimulationDebugMapCorrespondence(MAP_ERROR))?,
        value.0,
        representation,
        value_slot,
        metadata_slot,
    ))
}

const fn align_up_u32_v1(value: u32, alignment: u32) -> Option<u32> {
    let mask = alignment - 1;
    match value.checked_add(mask) {
        Some(value) => Some(value & !mask),
        None => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn compiler_parameter_storage_v1(
    semantic_local: u32,
    semantic_type: u32,
    ownership: fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1,
    abi_ignored: bool,
    bindings: &[fe2o3_lower_mir_kernel::SemanticKirParameterBindingV1],
    body: &fe2o3_kernel_ir::FunctionBody,
    parameter_types: &[fe2o3_kernel_ir::Type],
    semantic_types: &[fe2o3_mir_model::semantic_mir_v1::SemanticTypeDeclV1],
) -> Result<fe2o3_kernel_ir::SemanticStorageBindingV1, ProductionPipelineError> {
    let matching = bindings
        .iter()
        .filter(|binding| binding.semantic_local().index() == semantic_local)
        .collect::<Vec<_>>();
    let [binding] = matching.as_slice() else {
        return Ok(if matching.is_empty() {
            fe2o3_kernel_ir::SemanticStorageBindingV1::Unavailable {
                reason: if abi_ignored {
                    fe2o3_kernel_ir::SemanticStorageUnavailableReasonV1::AbiIgnored
                } else {
                    fe2o3_kernel_ir::SemanticStorageUnavailableReasonV1::NoRetainedKirStorage
                },
            }
        } else {
            fe2o3_kernel_ir::SemanticStorageBindingV1::Ambiguous
        });
    };
    let Some(parameter_ordinal) = body
        .parameters
        .iter()
        .position(|value| *value == binding.kernel_ir_value())
    else {
        return Ok(fe2o3_kernel_ir::SemanticStorageBindingV1::Unavailable {
            reason: fe2o3_kernel_ir::SemanticStorageUnavailableReasonV1::NoRetainedKirStorage,
        });
    };
    let kir_type = parameter_types.get(parameter_ordinal).ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage parameter type is absent",
        ),
    )?;
    let semantic = semantic_types.get(semantic_type as usize).ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage semantic type is absent",
        ),
    )?;
    let representation = match (semantic.shape(), kir_type, ownership) {
        (
            fe2o3_mir_model::semantic_mir_v1::SemanticTypeShapeV1::Scalar(_)
            | fe2o3_mir_model::semantic_mir_v1::SemanticTypeShapeV1::ValidityScalar(_),
            fe2o3_kernel_ir::Type::Scalar(_),
            _,
        ) => fe2o3_kernel_ir::SemanticKirStorageRepresentationV1::Scalar,
        (_, fe2o3_kernel_ir::Type::Slice(_), ownership)
            if ownership
                != fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1::ByValue =>
        {
            fe2o3_kernel_ir::SemanticKirStorageRepresentationV1::RegionSlice
        }
        (_, fe2o3_kernel_ir::Type::Pointer(_), ownership)
            if ownership
                != fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1::ByValue =>
        {
            fe2o3_kernel_ir::SemanticKirStorageRepresentationV1::RegionPointer
        }
        _ => fe2o3_kernel_ir::SemanticKirStorageRepresentationV1::OpaqueFlattened,
    };
    Ok(
        fe2o3_kernel_ir::SemanticStorageBindingV1::ExactKirParameter {
            kir_parameter_ordinal: u32::try_from(parameter_ordinal).map_err(|_| {
                ProductionPipelineError::SimulationDebugMapCorrespondence(
                    "typed storage KIR parameter ordinal does not fit the wire",
                )
            })?,
            kir_value_ordinal: binding.kernel_ir_value().0,
            representation,
        },
    )
}

fn compiler_ownership_v1(
    ownership: fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1,
) -> Result<fe2o3_kernel_ir::SemanticArgumentOwnershipV1, ProductionPipelineError> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1 as Source;
    match ownership {
        Source::ByValue => Ok(fe2o3_kernel_ir::SemanticArgumentOwnershipV1::ByValue),
        Source::SharedBorrow => Ok(fe2o3_kernel_ir::SemanticArgumentOwnershipV1::SharedBorrow),
        Source::UniqueBorrow => Ok(fe2o3_kernel_ir::SemanticArgumentOwnershipV1::UniqueBorrow),
        Source::ExclusiveOwner => Ok(fe2o3_kernel_ir::SemanticArgumentOwnershipV1::ExclusiveOwner),
        Source::RawPointer => Ok(fe2o3_kernel_ir::SemanticArgumentOwnershipV1::RawPointer),
        Source::Unspecified => Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "typed storage map rejects unspecified source ownership",
        )),
    }
}

fn debug_source_scope_span_v2(
    source: fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
) -> Result<fe2o3_kernel_ir::DebugSourceMapSpanV1, ProductionPipelineError> {
    let origin =
        source
            .call_site()
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "source-variable scope has no resolved source call site",
            ))?;
    let (byte_start, byte_end) = origin.byte_range();
    let (line, column) = origin.start_coordinate();
    fe2o3_kernel_ir::DebugSourceMapSpanV1::new_eliminated(
        *origin.file().as_bytes(),
        byte_start,
        byte_end,
        line,
        column,
    )
    .map_err(ProductionPipelineError::SimulationDebugMap)
}

type ExactDebugSemanticConstructV1 = (u8, u32, u32, u32);
type ExactDebugMappedOperationsV1 = BTreeMap<
    fe2o3_kernel_ir::DebugSourceMapKirSiteV1,
    (
        ExactDebugSemanticConstructV1,
        fe2o3_kernel_ir::DebugSourceMapSpanV1,
    ),
>;

#[allow(clippy::too_many_arguments)]
fn insert_debug_operation_range_v1(
    function_ordinal: u64,
    body: &fe2o3_kernel_ir::FunctionBody,
    block_ordinals: &BTreeMap<fe2o3_kernel_ir::BlockId, usize>,
    semantic_owner: ExactDebugSemanticConstructV1,
    block: fe2o3_kernel_ir::BlockId,
    first_operation: u32,
    operation_count: u32,
    source: fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
    mapped: &mut ExactDebugMappedOperationsV1,
    eliminated: &mut BTreeSet<fe2o3_kernel_ir::DebugSourceMapSpanV1>,
) -> Result<(), ProductionPipelineError> {
    // V1 intentionally resolves every macro-originated construct to rustc's
    // final source call site. Expansion-chain identity remains in semantic MIR
    // but is not serialized as a source-map span in this version.
    let origin =
        source
            .call_site()
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "semantic operation has no resolved source call site",
            ))?;
    let (byte_start, byte_end) = origin.byte_range();
    let (line, column) = origin.start_coordinate();
    if operation_count != 0 && byte_start >= byte_end {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "resolved source call-site span is empty",
        ));
    }
    let source_span = if operation_count == 0 {
        fe2o3_kernel_ir::DebugSourceMapSpanV1::new_eliminated(
            *origin.file().as_bytes(),
            byte_start,
            byte_end,
            line,
            column,
        )
    } else {
        fe2o3_kernel_ir::DebugSourceMapSpanV1::new(
            *origin.file().as_bytes(),
            byte_start,
            byte_end,
            line,
            column,
        )
    }
    .map_err(ProductionPipelineError::SimulationDebugMap)?;
    let block_ordinal = debug_block_ordinal_v1(
        body,
        block_ordinals,
        block,
        first_operation,
        operation_count,
    )?;
    if operation_count == 0 {
        eliminated.insert(source_span);
        return Ok(());
    }
    let end = first_operation.checked_add(operation_count).ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "semantic KIR operation range overflows",
        ),
    )?;
    for operation in first_operation..end {
        let site = fe2o3_kernel_ir::DebugSourceMapKirSiteV1::operation(
            function_ordinal,
            block_ordinal,
            u64::from(operation),
        );
        if mapped
            .insert(site, (semantic_owner, source_span))
            .is_some_and(|previous| previous != (semantic_owner, source_span))
        {
            return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "one KIR operation is attributed to multiple semantic constructs",
            ));
        }
    }
    Ok(())
}

fn debug_block_ordinal_v1(
    body: &fe2o3_kernel_ir::FunctionBody,
    block_ordinals: &BTreeMap<fe2o3_kernel_ir::BlockId, usize>,
    block: fe2o3_kernel_ir::BlockId,
    first_operation: u32,
    operation_count: u32,
) -> Result<u64, ProductionPipelineError> {
    let ordinal = *block_ordinals.get(&block).ok_or(
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "correspondence names an unknown KIR block",
        ),
    )?;
    let operation_end = usize::try_from(first_operation)
        .ok()
        .and_then(|first| {
            usize::try_from(operation_count)
                .ok()
                .and_then(|count| first.checked_add(count))
        })
        .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "KIR operation range does not fit this compiler host",
        ))?;
    if operation_end
        > body
            .blocks
            .get(ordinal)
            .ok_or(ProductionPipelineError::SimulationDebugMapCorrespondence(
                "correspondence KIR block ordinal is unavailable",
            ))?
            .operations
            .len()
    {
        return Err(ProductionPipelineError::SimulationDebugMapCorrespondence(
            "correspondence KIR operation range is outside its block",
        ));
    }
    u64::try_from(ordinal).map_err(|_| {
        ProductionPipelineError::SimulationDebugMapCorrespondence(
            "KIR block ordinal does not fit the source-map wire",
        )
    })
}

impl RankedVerifiedProductionCompilation {
    pub(crate) fn ranked_roots(
        &self,
    ) -> &[crate::production_ranked_projection_v1::ProductionRankedRootProgramV1] {
        self.ranked.roots()
    }

    pub(crate) fn ranked_root_count(&self) -> usize {
        self.ranked.root_count()
    }

    pub(crate) fn semantic_function_count(&self) -> usize {
        self.ranked.semantic_function_count()
    }

    pub(crate) fn semantic_callable_count(&self) -> usize {
        self.ranked.semantic_callable_count()
    }

    pub(crate) fn bounds_are_clean(&self) -> bool {
        self.ranked.bounds_are_clean()
    }

    pub(crate) fn all_kernel_checks_are_clean(&self) -> bool {
        self.ranked.all_kernel_checks_are_clean()
    }

    pub(crate) fn retained_identity_and_transaction_binding_count(&self) -> usize {
        let _ = (
            &self.bindings.rustc_identity_inventory,
            &self.bindings.rustc_preflight_plan,
            &self.bindings.typed_descriptor_roots,
            &self.bindings.transaction.producer,
            &self.bindings.transaction.output_dir,
            &self.bindings.transaction.compiler_ffi_envelope,
        );
        6 + self
            .bindings
            .transaction
            .compiler_custody
            .retained_protected_binding_count()
    }

    pub(crate) fn grants_artifact_or_launch_authority(&self) -> bool {
        self.ranked.grants_artifact_or_launch_authority()
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Retains the collector-sealed closure without granting semantic authority.
    /// The next transition must authenticate every imported MIR fact.
    pub(crate) fn from_collected_device_closure(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
        build_attempt: BuildAttempt,
        invocation: AdmittedProtectedRustcInvocationV1,
        compiler_execution: AdmittedProtectedCompilerExecutionV1,
    ) -> Result<Self, ProductionPipelineError> {
        Self::from_collected_device_closure_with_custody(
            tcx,
            closure,
            producer,
            output_dir,
            ProductionCompilerCustody::protected(invocation, compiler_execution, build_attempt),
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
        )
    }

    pub(crate) fn from_collected_device_closure_for_extraction(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
    ) -> Result<Self, ProductionPipelineError> {
        Self::from_collected_device_closure_with_custody(
            tcx,
            closure,
            producer,
            output_dir,
            ProductionCompilerCustody::extraction_only(),
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )
    }

    pub(crate) fn from_collected_device_closure_for_simulation_v2(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
    ) -> Result<Self, ProductionPipelineError> {
        Self::from_collected_device_closure_with_custody(
            tcx,
            closure,
            producer,
            output_dir,
            ProductionCompilerCustody::extraction_only(),
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::SourceVariables,
        )
    }

    fn from_collected_device_closure_with_custody(
        tcx: TyCtxt<'tcx>,
        closure: AuthenticatedCollectedKernelClosureV1<'tcx>,
        producer: ProducerIdentity,
        output_dir: PathBuf,
        compiler_custody: ProductionCompilerCustody,
        debug_source_capture: crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2,
    ) -> Result<Self, ProductionPipelineError> {
        if closure.function_count() == 0 {
            return Err(ProductionPipelineError::EmptyCollectedDeviceClosure);
        }
        let typed_descriptor_roots = closure
            .rederive_typed_descriptor_roots(tcx)
            .map_err(ProductionPipelineError::DescriptorEvidence)?;
        let compiler_ffi_envelope = closure.compiler_ffi_observation().cloned();
        Ok(Self {
            stage: CollectedRustStage {
                tcx,
                closure,
                typed_descriptor_roots,
                debug_source_capture,
                transaction: ProductionTransactionBindings {
                    producer,
                    output_dir,
                    compiler_ffi_envelope,
                    compiler_custody,
                },
            },
            invariant_session: PhantomData,
        })
    }

    fn import_semantic_mir(
        self,
    ) -> Result<ProductionCompilation<'tcx, AdmittedSemanticMirStage>, ProductionPipelineError>
    {
        let CollectedRustStage {
            tcx,
            closure,
            typed_descriptor_roots,
            debug_source_capture,
            transaction,
        } = self.stage;
        let crate::collector::ConstructedProductionSemanticMirV1 {
            semantic_mir,
            rustc_identity_inventory,
            rustc_preflight_plan,
            rustc_target,
            kernel_contexts,
            reference_effect_bindings,
            debug_source_files,
            debug_source_scopes,
            debug_source_variables,
            debug_capture_gap,
        } = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            closure,
            debug_source_capture,
        )
        .map_err(ProductionPipelineError::SemanticImport)?;
        let typed_descriptor_roots =
            crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
                typed_descriptor_roots,
                &semantic_mir,
            )
            .map_err(ProductionPipelineError::DescriptorEvidence)?;
        Ok(ProductionCompilation {
            stage: AdmittedSemanticMirStage {
                semantic_mir,
                bindings: AuthenticatedProductionBindings {
                    rustc_identity_inventory,
                    rustc_preflight_plan,
                    rustc_target,
                    kernel_contexts: Some(kernel_contexts),
                    reference_effect_bindings,
                    debug_source_files,
                    debug_source_scopes,
                    debug_source_variables,
                    debug_capture_gap,
                    typed_descriptor_roots,
                    transaction,
                },
            },
            invariant_session: PhantomData,
        })
    }

    /// Consumes the only production transaction through import and verification.
    pub(crate) fn verify_general_kernel_checks(
        self,
    ) -> Result<RankedVerifiedProductionCompilation, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()
    }

    /// Consumes the sole production transaction through exact semantic MIR,
    /// formal memory admission, and exact authenticated-target LLVM lowering.
    pub(crate) fn lower_production_target(
        self,
    ) -> Result<TargetLoweredProductionCompilation, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .admit_formal_memory()?
            .lower_production_target()
    }

    /// Consumes the sole production transaction through the same admitted
    /// source, ranked checks, and target-neutral lowering as production, then
    /// emits an inert exact-V7 simulation input. No target lowering or
    /// artifact transaction is entered.
    pub(crate) fn export_simulation_bundle_v1(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV1, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .into_simulation_bundle_v1(
                fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
            )
    }

    /// Emits the explicit V2 simulation envelope with compiler-produced,
    /// exact-KIR-bound source-variable metadata. This remains inert and grants
    /// no compiler, proof, artifact, hardware, load, or launch authority.
    pub(crate) fn export_simulation_bundle_v2(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV2, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .into_simulation_bundle_v2(
                fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
            )
    }

    /// Emits V3 with the exact admitted semantic MIR and its independently
    /// versioned semantic-local to KIR-storage projection.
    pub(crate) fn export_simulation_bundle_v3(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV3, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .into_simulation_bundle_v3(
                fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
            )
    }

    /// Emits V4 with compiler-rederived one-to-many aggregate component and
    /// physical simulator-kernarg correspondence. The KFD descriptor path is
    /// intentionally not entered.
    pub(crate) fn export_simulation_bundle_v4(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV4, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .into_simulation_bundle_v4(
                fe2o3_kernel_ir::SimulationCompilerExecutionBindingV1::UnavailableExtractionOnly,
            )
    }

    /// Emits self-contained V5 with the exact V10 re-encoding of the same
    /// producer-owned V8/V9 module and independently bound debug/storage data.
    /// This extraction-only path grants no compiler, artifact, load, launch,
    /// proof, or hardware authority.
    pub(crate) fn export_simulation_bundle_v5(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV5, ProductionPipelineError> {
        let admitted = self.import_semantic_mir()?;
        admitted
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .into_simulation_bundle_v5()
    }

    /// Exports an authority-free V6 simulation bundle whose exact same-module
    /// execution body is canonical KIR V11.
    pub(crate) fn export_simulation_bundle_v6(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV6, ProductionPipelineError> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .verify_general_kernel_checks()?
            .lower_target_neutral()?
            .into_simulation_bundle_v6()
    }

    /// Exports an authority-free V8 bundle by consuming the exact target-lowered
    /// V13 graph together with its retained target closure and W4 verification
    /// result. There is no projection to an older KIR or simulation schema.
    pub(crate) fn export_simulation_bundle_v8(
        self,
    ) -> Result<fe2o3_kernel_ir::VerifiedSimulationBundleV8, ProductionPipelineError> {
        self.lower_production_target()?.into_simulation_bundle_v8()
    }

    /// Publishes the exact production compiler module into the managed,
    /// preselected attempt-scoped protocol. This grants no link, artifact, load,
    /// or launch authority.
    pub(crate) fn publish_worker_handoff(
        self,
        final_v13_symbols: crate::single_codegen_ownership_v1::FinalV13ExpectedDeviceSymbolRosterV1,
    ) -> Result<PublishedProductionWorkerTransaction, ProductionPipelineError> {
        self.lower_production_target()?
            .publish_worker_handoff(final_v13_symbols)
    }

    /// Retains the original extraction milestone while consuming the same
    /// transaction and importer as the production backend.
    pub(crate) fn require_semantic_mir_import(self) -> ProductionPipelineError {
        match self.import_semantic_mir() {
            Ok(transaction) => match transaction.construct_semantic_middle_end() {
                Ok(transaction) => match transaction.construct_semantic_ssa() {
                    Ok(transaction) => transaction.require_target_neutral_lowering(),
                    Err(error) => error,
                },
                Err(error) => error,
            },
            Err(error) => error,
        }
    }
}

impl<'tcx> ProductionCompilation<'tcx, AdmittedSemanticMirStage> {
    fn construct_semantic_middle_end(
        self,
    ) -> Result<ProductionCompilation<'tcx, EquivalentSemanticMirStage>, ProductionPipelineError>
    {
        let AdmittedSemanticMirStage {
            semantic_mir,
            bindings,
        } = self.stage;
        let semantic_mir = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            semantic_mir,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .map_err(ProductionPipelineError::SemanticMiddleEnd)?;
        Ok(ProductionCompilation {
            stage: EquivalentSemanticMirStage {
                semantic_mir,
                bindings,
            },
            invariant_session: PhantomData,
        })
    }
}

impl<'tcx> ProductionCompilation<'tcx, EquivalentSemanticMirStage> {
    fn construct_semantic_ssa(
        self,
    ) -> Result<ProductionCompilation<'tcx, SsaSemanticMirStage>, ProductionPipelineError> {
        let EquivalentSemanticMirStage {
            semantic_mir,
            bindings,
        } = self.stage;
        let semantic_ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
            semantic_mir,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .map_err(ProductionPipelineError::SemanticSsa)?;
        Ok(ProductionCompilation {
            stage: SsaSemanticMirStage {
                semantic_ssa,
                bindings,
            },
            invariant_session: PhantomData,
        })
    }
}

impl<'tcx> ProductionCompilation<'tcx, SsaSemanticMirStage> {
    fn require_target_neutral_lowering(self) -> ProductionPipelineError {
        let SsaSemanticMirStage {
            semantic_ssa,
            bindings,
        } = self.stage;
        let error =
            crate::collector::ProductionSemanticImportErrorV1::TargetNeutralLoweringPending {
                functions: semantic_ssa.source_semantic().functions().len(),
                callables: semantic_ssa.source_semantic().callables().len(),
                rustc_identity_inventory_sha256: bindings.rustc_identity_inventory.sha256(),
                rustc_preflight_plan_sha256: bindings.rustc_preflight_plan.sha256(),
                semantic_sha256: *semantic_ssa.source_semantic().semantic_sha256().as_bytes(),
            };
        drop((semantic_ssa, bindings));
        ProductionPipelineError::SemanticImport(error)
    }

    fn verify_general_kernel_checks(
        self,
    ) -> Result<RankedVerifiedProductionCompilation, ProductionPipelineError> {
        let SsaSemanticMirStage {
            semantic_ssa,
            bindings,
        } = self.stage;
        crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
            &bindings.typed_descriptor_roots,
            semantic_ssa.source_semantic(),
        )
        .map_err(ProductionPipelineError::DescriptorEvidence)?;
        let ranked_roots = bindings
            .typed_descriptor_roots
            .iter()
            .map(|typed_root| {
                let source_launch = typed_root.source_launch().ok_or(
                    ProductionPipelineError::Geometry(
                        crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
                    ),
                )?;
                Ok(
                    crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                        typed_root.logical_name(),
                        typed_root.kernel_binding_bytes(),
                        source_launch,
                    ),
                )
            })
            .collect::<Result<Vec<_>, ProductionPipelineError>>()?;
        let ranked =
            crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_v1(
                semantic_ssa,
                &ranked_roots,
                &bindings.reference_effect_bindings,
            )
            .map_err(ProductionPipelineError::RankedProjection)?;
        Ok(RankedVerifiedProductionCompilation { ranked, bindings })
    }
}

impl RankedVerifiedProductionCompilation {
    fn lower_target_neutral(
        self,
    ) -> Result<TargetNeutralProductionCompilation, ProductionPipelineError> {
        let Self {
            ranked,
            mut bindings,
        } = self;
        let kernel_contexts = bindings
            .kernel_contexts
            .take()
            .ok_or(ProductionPipelineError::SemanticImport(
                crate::collector::ProductionSemanticImportErrorV1::KernelContextBinding(
                    "production bindings lost kernel-context custody",
                ),
            ))?
            .into_lowering_inputs(
                &bindings.rustc_identity_inventory,
                &bindings.rustc_target,
                ranked.roots(),
                &bindings.typed_descriptor_roots,
            )
            .map_err(ProductionPipelineError::SemanticImport)?;
        let roster_receipt = ranked
            .into_verified_roster_receipt()
            .map_err(ProductionPipelineError::RankedVerification)?;
        debug_assert!(!roster_receipt.grants_artifact_or_launch_authority());
        debug_assert!(roster_receipt.verify_equivalence().is_ok());
        debug_assert_ne!(
            roster_receipt.canonical_roster_identity().as_bytes(),
            &[0; 32],
        );
        let (receipt, ranked_verification) = roster_receipt
            .into_module_verified_receipt()
            .map_err(ProductionPipelineError::RankedVerification)?;
        debug_assert!(ranked_verification.every_functional_verification_is_coherent());
        let lowered = fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_kernel_contexts(
                receipt,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                kernel_contexts,
            )
            .map_err(ProductionPipelineError::TargetNeutralLowering)?;
        let exact_translation_roster = {
            let mut translations = lowered.mir_pliron_translation_validations();
            translations.len() == lowered.module().kernels.len()
                && translations
                    .by_ref()
                    .zip(&lowered.module().kernels)
                    .all(|((function_name, _), kernel)| function_name == kernel.id.as_str())
        };
        if !exact_translation_roster {
            return Err(ProductionPipelineError::MissingMirPlironTranslationValidation);
        }
        Ok(TargetNeutralProductionCompilation {
            lowered,
            ranked_verification,
            bindings,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_backend(target: &str) -> crate::production_backend_v1::ProductionBackendTargetV1 {
        crate::production_backend_v1::ProductionBackendTargetV1::from_configured_target(target)
            .expect("test production backend")
    }

    fn contextual_v13_module() -> fe2o3_kernel_ir::Module {
        let context = fe2o3_kernel_ir::KernelContextTypeV1::new(
            "contextual_entry",
            [1; 32],
            [2; 32],
            [3; 32],
        );
        let source =
            fe2o3_kernel_ir::KernelContextSourceIdentityV1::new([4; 32], [5; 32], [6; 32], [7; 32]);
        let mut block = fe2o3_kernel_ir::BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
        block
            .operations
            .push(fe2o3_kernel_ir::Operation::kernel_context_issue(
                fe2o3_kernel_ir::ValueId(0),
                context,
                source,
            ));
        block.terminator = Some(fe2o3_kernel_ir::Terminator::Return { values: vec![] });

        let mut module = fe2o3_kernel_ir::Module::new("contextual-v13-transaction");
        module
            .functions
            .push(fe2o3_kernel_ir::Function::kernel_entry(
                "contextual_entry",
                fe2o3_kernel_ir::Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "contextual_kernel",
            "contextual_entry",
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn contextual_v13_prepared() -> PreparedV13FinalGraphVerification {
        let module = contextual_v13_module();
        let canonical =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        prepare_v13_final_graph_verification(&module, &canonical, &test_backend("gfx942:xnack-"))
            .unwrap()
    }

    #[test]
    fn contextual_v13_transaction_releases_only_the_verified_final_graph() {
        let prepared = contextual_v13_prepared();
        assert_eq!(prepared.target_module.kernels.len(), 1);
        assert_eq!(prepared.optimizer.final_epoch(), prepared.final_epoch);
        assert_eq!(
            prepared
                .optimizer
                .transformations()
                .last()
                .expect("nonempty V6 preservation chain")
                .output_epoch(),
            prepared.final_epoch,
        );
        let mut verified = prepared.require_final_graph_pliron_schedule().unwrap();
        assert_eq!(verified.target_module.kernels.len(), 1);
        assert_eq!(
            &verified
                .target_verification
                .final_canonical()
                .canonical_bytes()[8..10],
            &fe2o3_kernel_ir::KERNEL_IR_VERSION_V13.to_le_bytes(),
        );
        verified.target_verification.validate_live_owner().unwrap();
        assert_eq!(
            verified.target_verification.final_canonical().identity(),
            verified
                .target_verification
                .final_graph_report()
                .final_graph(),
        );
    }

    #[test]
    fn v13_closure_and_final_report_survive_the_gate_and_reject_substitution() {
        let prepared = contextual_v13_prepared();
        let expected_final_graph = *prepared.final_canonical.identity();
        let expected_final_epoch = prepared.final_epoch;
        let expected_closure = prepared.target_closure.identity();
        let verified = prepared.require_final_graph_pliron_schedule().unwrap();
        assert_eq!(
            verified.target_verification.target_closure().identity(),
            expected_closure,
        );
        assert_eq!(
            verified
                .target_verification
                .final_graph_report()
                .final_graph(),
            &expected_final_graph,
        );
        assert_eq!(
            verified
                .target_verification
                .final_graph_report()
                .final_epoch(),
            expected_final_epoch,
        );
        verified
            .target_verification
            .validate_target_module(&verified.target_module)
            .unwrap();

        let mut substituted_module = verified.target_module.clone();
        substituted_module.id = fe2o3_kernel_ir::ModuleId::new("substituted-final-graph");
        assert!(matches!(
            verified
                .target_verification
                .validate_target_module(&substituted_module),
            Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "retained W5 closure and W4 final-graph report"
            })
        ));

        let mut other_module = contextual_v13_module();
        other_module.id = fe2o3_kernel_ir::ModuleId::new("substituted-target-closure");
        let other_canonical =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(other_module.clone())
                .unwrap();
        let other = prepare_v13_final_graph_verification(
            &other_module,
            &other_canonical,
            &test_backend("gfx942:xnack-"),
        )
        .unwrap()
        .require_final_graph_pliron_schedule()
        .unwrap();
        let ProductionV13TargetVerificationCustody {
            target_closure: substituted_closure,
            verified_graph: _,
            ..
        } = other.target_verification;
        let ProductionV13TargetVerificationCustody {
            target_closure: _,
            verified_graph,
            pre_optimization_canonical,
            pre_optimization_epoch,
            final_epoch,
        } = verified.target_verification;
        assert!(matches!(
            ProductionV13TargetVerificationCustody::try_new(
                verified_graph,
                pre_optimization_canonical,
                pre_optimization_epoch,
                final_epoch,
                substituted_closure,
            ),
            Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "retained W5 closure and W4 final-graph report"
            })
        ));
    }

    #[test]
    fn v13_final_graph_gate_rejects_missing_and_stale_capability_results() {
        let prepared = contextual_v13_prepared();
        let valid = prepared.capability_results();
        require_exact_v13_capability_results(
            &prepared.neutral_identity,
            &prepared.optimized_identity,
            prepared.final_canonical.identity(),
            prepared.final_epoch,
            valid,
        )
        .unwrap();

        let missing = ProductionV13CapabilityResults {
            optimizer: None,
            ..valid
        };
        assert!(matches!(
            require_exact_v13_capability_results(
                &prepared.neutral_identity,
                &prepared.optimized_identity,
                prepared.final_canonical.identity(),
                prepared.final_epoch,
                missing,
            ),
            Err(ProductionFinalGraphVerificationErrorV13::MissingResult {
                stage: "V6 optimizer preservation chain"
            })
        ));

        assert!(matches!(
            require_exact_v13_capability_results(
                &prepared.neutral_identity,
                &prepared.optimized_identity,
                prepared.final_canonical.identity(),
                prepared.final_epoch + 1,
                valid,
            ),
            Err(ProductionFinalGraphVerificationErrorV13::StaleResult {
                stage: "optimized final graph",
                ..
            })
        ));
    }

    #[test]
    fn v13_final_graph_gate_rejects_an_optimizer_from_another_graph() {
        let prepared = contextual_v13_prepared();
        let mut other_module = contextual_v13_module();
        other_module.id = fe2o3_kernel_ir::ModuleId::new("other-v13-optimizer-input");
        let other_canonical =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(other_module.clone())
                .unwrap();
        let other = prepare_v13_final_graph_verification(
            &other_module,
            &other_canonical,
            &test_backend("gfx942:xnack-"),
        )
        .unwrap();
        let invalid = ProductionV13CapabilityResults {
            optimizer: Some(&other.optimizer),
            ..prepared.capability_results()
        };
        assert!(matches!(
            require_exact_v13_capability_results(
                &prepared.neutral_identity,
                &prepared.optimized_identity,
                prepared.final_canonical.identity(),
                prepared.final_epoch,
                invalid,
            ),
            Err(ProductionFinalGraphVerificationErrorV13::SubjectMismatch {
                stage: "V6 optimizer input"
            })
        ));
    }

    #[test]
    fn v13_production_transaction_rejects_nonfinal_target_answers() {
        use dialect_amdgcn::ProductionTargetCapabilityDiagnosticCodeV1 as Code;
        use fe2o3_kernel_ir::{AddressSpace, SynchronizationScope, TargetCapability};

        let cases = [(
            TargetCapability::Atomic {
                width_bits: 32,
                address_space: AddressSpace::Global,
                max_scope: SynchronizationScope::Device,
            },
            "gfx942:xnack-",
            Code::OmittedAxis,
        )];

        for (capability, target, expected_code) in cases {
            let mut module = contextual_v13_module();
            module.required_capabilities.insert(capability);
            let canonical =
                fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
            let error = match prepare_v13_final_graph_verification(
                &module,
                &canonical,
                &test_backend(target),
            ) {
                Ok(prepared) => panic!(
                    "nonfinal target answer entered target-bound custody for {target}: {:?}",
                    prepared
                        .target_closure
                        .validated_resource_evidence_v1()
                        .unwrap()
                        .decisions(),
                ),
                Err(error) => error,
            };
            match error {
                ProductionPipelineError::TargetBackend(
                    crate::production_backend_v1::ProductionBackendErrorV1::CapabilityClosure(
                        error,
                    ),
                ) => {
                    assert_eq!(error.diagnostic_code(), expected_code);
                }
                error => panic!("unexpected production transaction error: {error}"),
            }
        }
    }

    #[test]
    fn explicit_v2_simulation_export_requires_complete_source_capture() {
        assert!(require_complete_simulation_debug_source_capture_v2(None).is_ok());

        let error = require_complete_simulation_debug_source_capture_v2(Some(
            fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable,
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionPipelineError::SimulationDebugSourceCaptureUnavailable(
                fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::
                    SourceObservationUnrepresentable
            )
        ));
        assert!(error.to_string().contains("source-variable name"));

        assert!(matches!(
            require_complete_simulation_debug_source_capture_v2(Some(
                fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::ResourceLimit,
            )),
            Err(
                ProductionPipelineError::SimulationDebugSourceCaptureUnavailable(
                    fe2o3_kernel_ir::ProductionSemanticDebugProducerGapV1::ResourceLimit
                )
            )
        ));
    }

    #[test]
    fn target_workgroup_roster_rejects_empty_and_missing_entries_without_omission() {
        let empty = fe2o3_kernel_ir::Module::new("empty_workgroups");
        assert!(exact_target_workgroup_roster_v1(&empty).is_err());

        let mut module = fe2o3_kernel_ir::Module::new("workgroups");
        let mut first = fe2o3_kernel_ir::Kernel::new(
            "first",
            "first",
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Static(64),
            },
        );
        first.workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1));
        let second = fe2o3_kernel_ir::Kernel::new(
            "second",
            "second",
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Static(64),
            },
        );
        module.kernels.extend([first, second]);
        assert!(exact_target_workgroup_roster_v1(&module).is_err());

        module.kernels[1].workgroup_size = Some(fe2o3_kernel_ir::WorkgroupSize::new(128, 1, 1));
        assert_eq!(
            exact_target_workgroup_roster_v1(&module).unwrap().as_ref(),
            &[
                (
                    "first".to_owned(),
                    fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1),
                ),
                (
                    "second".to_owned(),
                    fe2o3_kernel_ir::WorkgroupSize::new(128, 1, 1),
                ),
            ],
        );
    }

    #[test]
    fn host_only_and_device_dispositions_are_exact() {
        assert_eq!(disposition(0), ProductionDisposition::HostOnly);
        assert_eq!(disposition(1), ProductionDisposition::DeviceTransaction);
        assert_eq!(
            disposition(usize::MAX),
            ProductionDisposition::DeviceTransaction
        );
    }

    #[test]
    fn private_production_implementation_is_unversioned() {
        let backend = include_str!("lib.rs");
        let pipeline = include_str!("production_pipeline.rs");
        assert!(backend.contains("mod production_pipeline;"));
        for retired in [
            concat!("production_pipeline", "_v1"),
            concat!("ProductionPipelineError", "V1"),
            concat!("ProductionCompilation", "V1"),
            concat!("ProductionDisposition", "V1"),
            concat!("ProductionCompilerCustody", "V1"),
            concat!("RetainedProductionDeviceAdmission", "V1"),
        ] {
            assert!(!backend.contains(retired), "backend retains {retired}");
            assert!(!pipeline.contains(retired), "pipeline retains {retired}");
        }
    }

    #[test]
    fn custom_llvm_configuration_is_terminal_before_construction() {
        assert!(reject_custom_llvm_configuration(false).is_ok());
        assert!(matches!(
            reject_custom_llvm_configuration(true),
            Err(ProductionPipelineError::CustomLlvmConfiguration)
        ));
    }

    #[test]
    fn extraction_custody_cannot_enter_protected_publication() {
        assert!(matches!(
            ProductionCompilerCustody::extraction_only().into_publication_custody(),
            Err(ProductionPipelineError::ExtractionCannotPublish)
        ));
    }

    #[test]
    fn production_layout_binding_uses_the_measured_worker_spelling() {
        let backend = test_backend("gfx942:xnack-");
        let legacy = format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{}\"\n\ndefine void @body() {{ ret void }}\n",
            dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT
        );
        let bound = backend.bind_worker_layout_v1(&legacy).unwrap();
        assert!(bound.starts_with(&format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{}\"\n\n",
            crate::production_target_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1
        )));
        assert!(bound.contains("target datalayout = \"e-p:64:64-"));
        assert!(bound.ends_with("define void @body() { ret void }\n"));
        assert_eq!(bound.matches("target triple =").count(), 1);
        assert_eq!(bound.matches("target datalayout =").count(), 1);
    }

    #[test]
    fn production_layout_binding_rejects_noncanonical_headers() {
        let backend = test_backend("gfx942:xnack-");
        let canonical = format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{}\"\n\ndefine void @body() {{ ret void }}\n",
            dialect_amdgcn::GFX942_XNACK_MINUS_DATA_LAYOUT
        );
        for hostile in [
            canonical.replacen("target triple", "source_filename", 1),
            canonical.replacen("\n\n", "\n", 1),
            format!("{canonical}target datalayout = \"e-p:64:64\"\n"),
        ] {
            assert!(backend.bind_worker_layout_v1(&hostile).is_err());
        }
    }

    #[test]
    fn production_target_lowering_is_v13_only() {
        let source = include_str!("production_pipeline.rs");
        let transaction = source
            .split("impl FormalMemoryAdmittedProductionCompilation")
            .nth(1)
            .expect("target-lowering stage")
            .split("impl TargetLoweredProductionCompilation")
            .next()
            .expect("bounded target-lowering body");
        assert!(transaction.contains("prepare_v13_final_graph_verification("));
        assert!(transaction.contains(".lower_v13_module_v1("));
        assert!(transaction.contains(".bind_worker_layout_v1("));
        let implementation = source
            .split("#[cfg(test)]")
            .next()
            .expect("production implementation");
        for forbidden in [
            "dialect_amdgcn",
            "fe2o3_amd_target",
            "ProductionAmdTargetProfileV1",
            "gfx942",
            "gfx950",
            ".bind_legacy_target_v1(",
            ".lower_legacy_module_v1(",
            "ProductionTargetVerificationCustody",
        ] {
            assert!(
                !implementation.contains(forbidden),
                "production core bypasses its target-neutral backend boundary with {forbidden}",
            );
        }
        assert!(!transaction.contains("required_capabilities.insert"));
    }

    #[test]
    fn w4_resource_input_uses_only_replay_validated_closure_payload() {
        let source = include_str!("production_pipeline.rs");
        let adapter = source
            .split("fn execute_w4_capability_witness(")
            .nth(1)
            .expect("W4 resource adapter")
            .split("fn prepare_simulation_bundle_v8_graph(")
            .next()
            .expect("bounded W4 resource adapter");
        let validate = adapter
            .find(".validated_resource_evidence_v1()")
            .expect("replay-validated W5 evidence");
        let construct = adapter
            .find("retain_production_w4_target_resource_input_v5(")
            .expect("typed W4 target resource");
        let execute = adapter
            .find(".execute_w4_capability_witness(subject.clone(), target)")
            .expect("W4 execution");
        assert!(validate < construct && construct < execute);
        assert!(adapter.contains("result.require_exact_subject_v1(&subject)"));
        assert!(adapter.contains("evidence.canonical_closure().to_vec()"));
        assert!(adapter.contains("evidence.decisions().to_vec()"));
        assert!(!adapter.contains("target_context.canonical_closure()"));
        assert!(!adapter.contains("target_context.decisions()"));
    }

    #[test]
    fn contextual_v13_path_verifies_before_llvm_and_publication() {
        let source = include_str!("production_pipeline.rs");
        let prepare = source
            .split("fn prepare_v13_final_graph_verification(")
            .nth(1)
            .expect("V13 transaction preparation")
            .split("impl PreparedV13FinalGraphVerification")
            .next()
            .expect("bounded V13 preparation");
        let optimize = prepare
            .find("optimize_production_kernel_ir_module_v6")
            .expect("closed V6 optimizer");
        let structural = prepare
            .find("admit_production_kernel_ir_structural_replay_v6")
            .expect("independent V6 replay");
        let target = prepare
            .find("close_semantic_capabilities_v1")
            .expect("exact V13 target capability closure");
        assert!(optimize < structural && structural < target);

        let final_gate = source
            .split("fn require_final_graph_pliron_schedule(")
            .nth(1)
            .expect("terminal final-graph gate")
            .split("struct PreparedProductionWorkerPublication")
            .next()
            .expect("bounded final-graph gate");
        assert!(final_gate.contains("ProductionFinalGraphOwnerV1::try_new"));
        assert!(final_gate.contains(".verify()"));
        assert!(!final_gate.contains("FinalGraphPlironExecutorUnavailable"));
        for forbidden in [
            "lower_compiler_module_to_",
            "prepare_production_worker_handoff",
            "publish_compiler_module_handoff",
        ] {
            assert!(
                !final_gate.contains(forbidden),
                "unverified V13 final graph reached {forbidden}",
            );
        }

        let transaction = source
            .split("impl FormalMemoryAdmittedProductionCompilation")
            .nth(1)
            .expect("target-lowering stage")
            .split("impl TargetLoweredProductionCompilation")
            .next()
            .expect("bounded target-lowering body");
        let verify = transaction
            .find("require_final_graph_pliron_schedule()")
            .expect("owner-held final-graph verification");
        let lower = transaction
            .find(".lower_v13_module_v1(")
            .expect("exact verified V13 backend lowering");
        assert!(verify < lower);
    }

    #[test]
    fn worker_publication_cannot_bypass_general_pliron_checks() {
        let source = include_str!("production_pipeline.rs");
        let transaction = source
            .split("pub(crate) fn lower_production_target(")
            .nth(1)
            .expect("production target transaction")
            .split("pub(crate) fn publish_worker_handoff(")
            .next()
            .expect("bounded transaction body");
        let verify = transaction
            .find(".verify_general_kernel_checks()?")
            .expect("mandatory general PLIRON checks");
        let ssa = transaction
            .find(".construct_semantic_ssa()?")
            .expect("mandatory semantic SSA custody");
        let lower = transaction
            .find(".lower_target_neutral()?")
            .expect("target-neutral lowering");
        assert!(
            ssa < verify && verify < lower,
            "semantic SSA, ranked verification, and lowering typestates are out of order",
        );
        assert!(
            include_str!("production_ranked_projection_v1.rs")
                .contains("prepare_reference_effect_request_v2")
        );
    }

    #[test]
    fn referenced_kernels_complete_all_functional_gates_before_kir_lowering() {
        let projection = include_str!("production_ranked_projection_v1.rs");
        let semantic = projection
            .find("derive_and_reconcile_mir_pliron_semantic_contract_v1")
            .expect("compiler-owned semantic-contract derivation");
        let parallel = projection
            .find("derive_and_require_parallel_reference_contract_v1")
            .expect("compiler-owned parallel-contract derivation");
        let aggregate = projection
            .find("authenticate_mir_pliron_contract_per_compilation_v2")
            .expect("effect-IR-derived aggregate per-compilation Verus gate");
        assert!(semantic < parallel && parallel < aggregate);

        let pipeline = include_str!("production_pipeline.rs");
        let roster = pipeline
            .find(".into_verified_roster_receipt()")
            .expect("ranked roster verification transition");
        let module = pipeline[roster..]
            .find(".into_module_verified_receipt()")
            .map(|offset| roster + offset)
            .expect("complete ranked module receipt transition");
        let lowering = pipeline[module..]
            .find("ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks")
            .map(|offset| module + offset)
            .expect("KIR lowering transition");
        assert!(
            roster < module && module < lowering,
            "KIR lowering ran before functional verification"
        );
    }

    #[test]
    fn ranked_roster_receipt_reaches_complete_module_kir_authority() {
        let pipeline = include_str!("production_pipeline.rs");
        let roster = pipeline
            .find(".into_verified_roster_receipt()")
            .expect("ranked roster receipt transition");
        let module = pipeline[roster..]
            .find(".into_module_verified_receipt()")
            .map(|offset| roster + offset)
            .expect("complete module receipt authority");
        let kir = pipeline[module..]
            .find("ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks")
            .map(|offset| module + offset)
            .expect("KIR authority transition");
        assert!(roster < module && module < kir);
        assert!(!pipeline.contains(concat!("MultiRoot", "TargetNeutralLowering")));
    }

    #[test]
    fn production_publication_has_one_protected_custody_path() {
        let pipeline = include_str!("production_pipeline.rs");
        let worker = include_str!("production_worker_handoff.rs");
        let lineage = include_str!("production_semantic_lineage_v3.rs");
        for removed in [
            concat!("ProductionCompilerModule", "PublicationV1"),
            concat!("PreparedProductionCompiler", "PublicationV1"),
            concat!("ProtectedHandoff", "RequiresV2"),
            concat!("UnprotectedHandoff", "RequiresV1"),
            concat!("publish_worker_handoff", "_v3"),
            concat!("publish_prepared_production_v1", "_worker_handoff("),
            concat!("PreparedProductionV1", "WorkerHandoffV1"),
            concat!("PreparedProductionLineage", "WorkerHandoffV3"),
            concat!("prepare_production_v1", "_worker_handoff"),
        ] {
            assert!(
                !pipeline.contains(removed) && !worker.contains(removed),
                "obsolete production publication variant remains: {removed}",
            );
        }
        assert!(pipeline.contains("ProductionCompilerCustody::protected("));
        assert!(pipeline.contains("compiler_execution"));
        assert!(!pipeline.contains(concat!(
            "let invocation_",
            "descriptor = invocation.descriptor().clone()"
        )));
        let _: fn(
            crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3,
            &crate::protected_rustc_invocation::FinishedProtectedRustcInvocationV3,
            fe2o3_compiler_ffi::DeviceTargetV1,
            &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
            fe2o3_compiler_ffi::CompilerModuleHandoffV2,
        ) -> Result<
            fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
            crate::production_semantic_lineage_v3::ProductionSemanticLineageErrorV3,
        > = crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3::finish;
        let _: fn(
            crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3,
            fe2o3_rustc_invocation::RustcInvocationDescriptorV3,
            fe2o3_compiler_ffi::DeviceTargetV1,
            &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
            fe2o3_compiler_ffi::CompilerModuleHandoffV2,
        ) -> Result<
            fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
            crate::production_semantic_lineage_v3::ProductionSemanticLineageErrorV3,
        > = crate::production_semantic_lineage_v3::PreparedProductionSemanticLineageV3::finish_for_inert_extraction;
        assert!(lineage.contains("invocation_custody: &FinishedProtectedRustcInvocationV3"));
        let inert_lineage_boundary = lineage
            .find("pub(crate) fn finish_for_inert_extraction(")
            .expect("explicit inert extraction boundary");
        assert!(
            !lineage[..inert_lineage_boundary].contains("invocation: RustcInvocationDescriptorV3")
        );
        assert_eq!(
            lineage[inert_lineage_boundary..]
                .matches("invocation: RustcInvocationDescriptorV3")
                .count(),
            2,
            "raw descriptors are confined to the inert extraction boundary and its helper",
        );

        let protected_lineage = lineage
            .find("pub(crate) fn finish(")
            .expect("protected lineage finalizer remains explicit");
        let inert_lineage = lineage[protected_lineage..]
            .find("pub(crate) fn finish_for_inert_extraction(")
            .map(|offset| protected_lineage + offset)
            .expect("inert lineage finalizer remains explicit");
        let protected_lineage = &lineage[protected_lineage..inert_lineage];
        let lineage_revalidation = protected_lineage
            .find(".revalidate_for_publication()")
            .expect("protected lineage revalidates live invocation custody");
        let lineage_descriptor = protected_lineage
            .find(".descriptor().clone()")
            .expect("protected lineage derives its descriptor from live custody");
        assert!(lineage_revalidation < lineage_descriptor);
        assert!(!protected_lineage.contains("invocation: RustcInvocationDescriptorV3"));
        assert!(!protected_lineage.contains("finish_for_inert_extraction"));

        let inert_pipeline = pipeline
            .find("pub(crate) fn into_inert_semantic_worker_handoff_for_extraction(")
            .expect("authority-free extraction path remains explicit");
        let protected_prepare = pipeline[inert_pipeline..]
            .find("fn prepare_worker_handoff(")
            .map(|offset| inert_pipeline + offset)
            .expect("protected publication path follows inert extraction");
        let inert_pipeline = &pipeline[inert_pipeline..protected_prepare];
        assert!(inert_pipeline.contains("if !transaction.compiler_custody.is_extraction_only()"));
        assert!(inert_pipeline.contains(".finish_for_inert_extraction("));
        for protected_operation in [
            concat!("publish_compiler_module_handoff", "_v3"),
            "InertCompilerExecutionSubjectV1::from_publication",
            ".acquire(subject.clone())",
            concat!("publish_compiler_execution_receipt_transport", "_v1"),
        ] {
            assert!(
                !inert_pipeline.contains(protected_operation),
                "inert extraction reached protected operation: {protected_operation}",
            );
        }
        assert_eq!(
            pipeline
                .matches(concat!("publish_compiler_capability_handoff", "_v5"))
                .count(),
            1,
            "production retains one durable native V5 publication path",
        );
        assert!(!pipeline.contains(concat!("publish_compiler_module_handoff", "_v3")));

        let protected_pipeline = pipeline[protected_prepare..]
            .find("fn publish_worker_handoff(")
            .map(|offset| protected_prepare + offset)
            .expect("protected publication method remains explicit");
        let protected_pipeline_end = pipeline[protected_pipeline..]
            .find("\nfn require_complete_simulation_debug_source_capture_v2(")
            .map(|offset| protected_pipeline + offset)
            .expect("protected publication method remains bounded");
        let protected_pipeline = &pipeline[protected_pipeline..protected_pipeline_end];
        assert!(!protected_pipeline.contains("finish_for_inert_extraction"));
        assert!(!protected_pipeline.contains("RustcInvocationDescriptorV3"));
        let lineage_finish = protected_pipeline
            .find(".semantic_lineage\n            .finish_capability_v5(")
            .expect("semantic lineage consumes live protected invocation custody");
        let final_revalidation = protected_pipeline[lineage_finish..]
            .find("invocation\n            .revalidate_for_publication()")
            .map(|offset| lineage_finish + offset)
            .expect("protected invocation is revalidated after lineage construction");
        let durable_publication = protected_pipeline[lineage_finish..]
            .find(concat!("publish_compiler_capability_handoff", "_v5"))
            .map(|offset| lineage_finish + offset)
            .expect("native V5 handoff publication remains present");
        let execution_subject = protected_pipeline[lineage_finish..]
            .find("InertCompilerExecutionSubjectV1::from_capability_publication_v5")
            .map(|offset| lineage_finish + offset)
            .expect("native V5 publication derives one canonical compiler-execution subject");
        assert!(lineage_finish < final_revalidation && final_revalidation < durable_publication);
        assert!(durable_publication < execution_subject);
        let receipt_acquisition = protected_pipeline[execution_subject..]
            .find(".acquire(subject.clone())")
            .map(|offset| execution_subject + offset)
            .expect("exact execution subject is sent to the protected issuer");
        let receipt_transport = protected_pipeline[receipt_acquisition..]
            .find(concat!(
                "publish_compiler_execution_receipt_transport_for_capability",
                "_v5"
            ))
            .map(|offset| receipt_acquisition + offset)
            .expect("issuer receipt is published beside the exact V5 handoff");
        assert!(execution_subject < receipt_acquisition && receipt_acquisition < receipt_transport);
    }

    #[test]
    fn production_module_contains_no_profile_selection_vocabulary() {
        let sources = [
            include_str!("production_pipeline.rs"),
            include_str!("collector/production_importer_v1.rs"),
            include_str!("rustc_semantic_adapter_v1.rs"),
            include_str!("rustc_semantic_plan_v1.rs"),
            include_str!("production_semantic_fn_abi_v1.rs"),
            include_str!("production_semantic_types_v1.rs"),
            include_str!("production_semantic_terminal_v1.rs"),
            include_str!("reference_effect_v1.rs"),
        ];
        for forbidden in [
            concat!("General", "Gemm"),
            concat!("Flash", "Attention"),
            concat!("Row", "Softmax"),
            concat!("Moe", "Top2"),
            concat!("export", "_name"),
            concat!("source", " substring"),
            concat!("MIR", " transcript"),
            concat!("legacy", "-v1"),
            concat!("kernel-ir", "-v1"),
            concat!("Collection", "Result"),
            concat!("target: AmdGpu", "Target"),
        ] {
            assert!(
                !sources[0].contains(forbidden),
                "production transaction contains forbidden selector term {forbidden:?}"
            );
        }

        for forbidden_importer_term in [
            concat!("General", "Gemm"),
            concat!("Flash", "Attention"),
            concat!("Row", "Softmax"),
            concat!("Moe", "Top2"),
            concat!("source", " substring"),
            concat!("MIR", " transcript"),
            concat!("legacy", "-v1"),
            concat!("kernel-ir", "-v1"),
        ] {
            assert!(
                sources
                    .iter()
                    .skip(1)
                    .all(|source| !source.contains(forbidden_importer_term)),
                "production importer contains forbidden selector term {forbidden_importer_term:?}"
            );
        }

        for forbidden_dependency in [
            concat!("mir_import", "_v2"),
            concat!("same_session", "_rustc_v1"),
            concat!("frontend_record", "_bridge"),
            concat!("semantic_type", "_adapter_v2"),
            concat!("source_", "debug"),
            concat!("semantic_", "features"),
            concat!("crate::", "collected_"),
            concat!("collected_", "general_gemm_v1"),
        ] {
            assert!(
                sources
                    .iter()
                    .skip(1)
                    .all(|source| !source.contains(forbidden_dependency)),
                "production importer depends on qualification module {forbidden_dependency:?}"
            );
        }
    }

    #[test]
    fn production_backend_authenticates_target_before_monomorphization() {
        let backend = include_str!("lib.rs");
        let codegen = backend
            .split_once("fn codegen_crate")
            .expect("codegen entry")
            .1;
        let authentication = codegen
            .find("authenticate_before_collection")
            .expect("pre-collection target authentication");
        let monomorphization = codegen
            .find("collect_and_partition_mono_items")
            .expect("rustc monomorphization");
        assert!(authentication < monomorphization);
    }

    #[test]
    fn process_isolated_extraction_uses_the_production_transaction() {
        let driver = include_str!("production_rustc_driver_v1.rs");
        for required in [
            "reject_custom_llvm_configuration",
            "ProductionCompilation::from_collected_device_closure_for_extraction",
            "require_semantic_mir_import",
        ] {
            assert!(
                driver.contains(required),
                "production extraction driver bypassed required transaction step {required:?}",
            );
        }
        for forbidden in [
            "construct_production_semantic_mir_v1",
            "require_production_semantic_import_v1",
        ] {
            assert!(
                !driver.contains(forbidden),
                "production extraction driver directly called importer entry {forbidden:?}",
            );
        }
    }
}
