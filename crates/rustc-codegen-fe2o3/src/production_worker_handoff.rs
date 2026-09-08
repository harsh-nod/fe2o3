//! Workload-neutral preparation of the production compiler-module handoff.
//!
//! This module is the only feature-free bridge from the target-lowered production
//! transaction to the canonical compiler/worker protocol. Qualification workers
//! and exact workload adapters live outside this dependency path.

use crate::compiler_descriptor::{
    CompilerDescriptorError, TypedDescriptorRootV1,
    construct_production_v1_compiler_descriptor_source_v1,
};
use crate::compiler_module_contract::{
    CompilerModuleRoleError, ExactTargetBindingError, construct_symbol_manifest,
    validate_envelope_module_roles, validate_exact_target_binding,
};
use crate::kernel_ir_codegen::{
    CompilerModuleConstructionError, bind_compiler_descriptor_source_v1,
    retain_production_compiler_module_text_v1,
};
use crate::production_backend_v1::{
    ProductionBackendCapabilityClosureV1, ProductionBackendResourceEvidenceV1,
};
use dialect_kernel::{AtomicScopeAttr, MemorySpaceAttr};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerFfiEnvelopeError, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestErrorV1, InertProductionCapabilityHandoffErrorV5,
    InertProductionCapabilityHandoffInputsV5, InertProductionCapabilityHandoffV5,
    InertProductionFinalGraphReportV5, InertProductionTargetCapabilityClosureV5,
    PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1, ProductionGfx942CompilerFfiEnvelopeKindV1,
    ProductionGfx950CompilerFfiEnvelopeKindV1, construct_production_gfx942_ocml_exp_envelope_v1,
    construct_production_gfx950_ocml_exp_envelope_v1,
    inspect_production_gfx942_compiler_ffi_envelope_v1,
    inspect_production_gfx950_compiler_ffi_envelope_v1,
};
use fe2o3_compiler_lineage::{
    InertCapabilityRefinementReceiptV1, InertMultiRootProofLineageV3, MultiRootProofRosterKindV3,
};
use fe2o3_kernel_analysis::{
    MAX_PLIRON_HOST_ALLOCATIONS_V1, PlironAtomicTargetCapabilityV1, PlironAtomicTargetContextV1,
    PlironHostAllocationV1, PlironLaunchContractV1, PlironLaunchTargetLimitsV1,
    ProductionW4FinalGraphCapabilityWitnessV1, ProductionW4FinalGraphSubjectV1,
    ProductionW4KernelRootV1, ProductionW4TargetResourceInputV1,
    derive_target_decision_set_identity_v1,
};
use fe2o3_kernel_ir::{
    AddressSpace, F32MathFunction, F32MathImplementation, FloatOperation, FunctionRole, Module,
    OperationKind, Type, VerifiedCanonicalKernelIrIdentityV13, VerifiedCanonicalKernelIrV13,
};
use fe2o3_proof_contracts::{
    CapabilityObligationSpecV1, CapabilityPropertyIdV1, CapabilitySubjectV1, DigestV1,
    ExecutableKirIdentityV1, InertCapabilityObligationSetV1, KernelIdentityV1,
    KernelRootIdentityV1, LaunchContractIdentityV1, StatementIdentityV1, TargetModelIdentityV1,
};
use fe2o3_target_spec::{
    TargetAddressSpaceV1, TargetCapabilityDecisionOutcomeV1, TargetCapabilityRequirementV1,
    TargetMemoryScopeV1, TargetResourceRequirementV1,
};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

/// Move-only output of the sole production compiler pipeline.
///
/// It retains the exact LLVM identity and descriptor embedded in the canonical
/// module handoff, but grants no worker, artifact, load, or launch authority.
pub(crate) struct PreparedProductionWorkerHandoff {
    llvm_ir_sha256: [u8; 32],
    handoff: CompilerModuleHandoffV2,
    compiler_descriptor_source: CompilerDescriptorSourceV1,
    generated_host_contract_identities: Box<[[u8; 32]]>,
}

impl PreparedProductionWorkerHandoff {
    pub(crate) fn into_validated_parts(
        self,
    ) -> Result<
        (
            CompilerModuleHandoffV2,
            CompilerDescriptorSourceV1,
            Box<[[u8; 32]]>,
        ),
        ProductionWorkerHandoffError,
    > {
        let Self {
            llvm_ir_sha256,
            handoff,
            compiler_descriptor_source,
            generated_host_contract_identities,
        } = self;
        if Sha256::digest(handoff.module_bytes()).as_slice() != llvm_ir_sha256 {
            return Err(ProductionWorkerHandoffError::MissingProductionBindings);
        }
        Ok((
            handoff,
            compiler_descriptor_source,
            generated_host_contract_identities,
        ))
    }
}

const CAPABILITY_TARGET_MODEL_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-TARGET-MODEL/V5\0";
const CAPABILITY_ROOT_LAUNCH_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-ROOT-LAUNCH/V5\0";
const CAPABILITY_STATEMENT_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-STATEMENT/V5\0";

const STATIC_CAPABILITY_PROPERTIES_V5: [CapabilityPropertyIdV1; 15] = [
    CapabilityPropertyIdV1::TYPING,
    CapabilityPropertyIdV1::BOUNDS,
    CapabilityPropertyIdV1::INITIALIZATION,
    CapabilityPropertyIdV1::HIERARCHICAL_OWNERSHIP,
    CapabilityPropertyIdV1::DATA_RACE_FREEDOM,
    CapabilityPropertyIdV1::ATOMIC_LEGALITY,
    CapabilityPropertyIdV1::UNIFORMITY,
    CapabilityPropertyIdV1::BARRIER_CONVERGENCE,
    CapabilityPropertyIdV1::WORKGROUP_MEMORY_EPOCHS,
    CapabilityPropertyIdV1::TENSOR_LAYOUT,
    CapabilityPropertyIdV1::EFFECTS,
    CapabilityPropertyIdV1::RESOURCE_LEGALITY,
    CapabilityPropertyIdV1::TARGET_CAPABILITY_CLOSURE,
    CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT,
    CapabilityPropertyIdV1::MACHINE_REFINEMENT,
];

/// Exact producer-owned inputs needed to extend a frozen V3 handoff with native V13 custody.
pub(crate) struct ProductionCapabilityCarriageInputsV5 {
    final_canonical: VerifiedCanonicalKernelIrV13,
    proof_lineage: InertMultiRootProofLineageV3,
    semantic_mir_identity: [u8; 32],
    subjects: Vec<CapabilitySubjectV1>,
    compiler_policy: [u8; 32],
    source_refinement: InertCapabilityRefinementReceiptV1,
    analysis: ProductionCapabilityAnalysisCarriageV5,
}

impl ProductionCapabilityCarriageInputsV5 {
    /// Retains exact established-stage owners without constructing refinement evidence.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        final_canonical: VerifiedCanonicalKernelIrV13,
        proof_lineage: InertMultiRootProofLineageV3,
        semantic_mir_identity: [u8; 32],
        subjects: Vec<CapabilitySubjectV1>,
        compiler_policy: [u8; 32],
        source_refinement: InertCapabilityRefinementReceiptV1,
        analysis: ProductionCapabilityAnalysisCarriageV5,
    ) -> Self {
        Self {
            final_canonical,
            proof_lineage,
            semantic_mir_identity,
            subjects,
            compiler_policy,
            source_refinement,
            analysis,
        }
    }
}

/// Opaque authority-free output of the W4 analysis-witness boundary.
///
/// W6 never derives these records from `Debug`, `Display`, or boolean summaries. The W4 witness
/// adapter must supply the exact canonical obligation, target-closure, and final-graph records.
pub(crate) struct ProductionCapabilityAnalysisCarriageV5 {
    obligations: Vec<InertCapabilityObligationSetV1>,
    target_closure: InertProductionTargetCapabilityClosureV5,
    final_graph_report: InertProductionFinalGraphReportV5,
}

impl ProductionCapabilityAnalysisCarriageV5 {
    pub(crate) fn from_witness_records(
        obligations: Vec<InertCapabilityObligationSetV1>,
        target_closure: InertProductionTargetCapabilityClosureV5,
        final_graph_report: InertProductionFinalGraphReportV5,
    ) -> Self {
        Self {
            obligations,
            target_closure,
            final_graph_report,
        }
    }
}

/// Staged V13 owner retained while the live PLIRON graph executes W4.
pub(crate) struct PreparedProductionW4CapabilityCarriageV5 {
    final_canonical: VerifiedCanonicalKernelIrV13,
    proof_lineage: InertMultiRootProofLineageV3,
    semantic_mir_identity: [u8; 32],
    subjects: Vec<CapabilitySubjectV1>,
    compiler_policy: [u8; 32],
    source_refinement: InertCapabilityRefinementReceiptV1,
    target_closure: InertProductionTargetCapabilityClosureV5,
    w4_subject: ProductionW4FinalGraphSubjectV1,
    w4_target: ProductionW4TargetResourceInputV1,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn retain_production_w4_target_resource_input_v5(
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    target_identity: [u8; 32],
    launch_identity: [u8; 32],
    closure_identity: [u8; 32],
    canonical_closure: Vec<u8>,
    target_decisions: Vec<fe2o3_target_spec::TargetCapabilityDecisionV1>,
    atomic_target: PlironAtomicTargetContextV1,
    launch_contract: PlironLaunchContractV1,
) -> Result<ProductionW4TargetResourceInputV1, ProductionWorkerHandoffError> {
    ProductionW4TargetResourceInputV1::try_new(
        final_graph,
        final_epoch,
        target_identity,
        launch_identity,
        closure_identity,
        canonical_closure,
        target_decisions,
        atomic_target,
        launch_contract,
    )
    .map_err(ProductionWorkerHandoffError::CapabilityW4Input)
}

impl PreparedProductionW4CapabilityCarriageV5 {
    pub(crate) fn w4_inputs(
        &self,
    ) -> (
        ProductionW4FinalGraphSubjectV1,
        ProductionW4TargetResourceInputV1,
    ) {
        (self.w4_subject.clone(), self.w4_target.clone())
    }

    pub(crate) fn finish(
        self,
        witness: &ProductionW4FinalGraphCapabilityWitnessV1,
    ) -> Result<ProductionCapabilityCarriageInputsV5, ProductionWorkerHandoffError> {
        let retained_functional_refinement = functional_refinement_identity_v5(
            &self.proof_lineage,
            self.final_canonical.identity(),
            self.w4_subject.final_epoch(),
        )?;
        if witness.subject() != &self.w4_subject
            || witness.subject().functional_refinement_identity() != &retained_functional_refinement
            || witness.target_resource() != &self.w4_target
            || witness.canonical_kir() != self.final_canonical.canonical_bytes()
            || witness.stages().iter().any(|stage| {
                !matches!(
                    stage.outcome(),
                    fe2o3_kernel_analysis::ProductionW4CapabilityOutcomeV1::Clean
                )
            })
        {
            return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "live W4 witness",
            ));
        }
        fe2o3_kernel_analysis::ProductionW4CanonicalEncodingV1::decode(
            witness.canonical_encoding(),
        )
        .map_err(ProductionWorkerHandoffError::CapabilityW4Encoding)?;
        let obligations = self
            .subjects
            .iter()
            .copied()
            .map(|subject| {
                capability_obligations_v5(
                    subject,
                    witness,
                    retained_functional_refinement,
                    &self.source_refinement,
                    &self.target_closure,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let final_graph_report = InertProductionFinalGraphReportV5::new(
            *self.final_canonical.identity().digest(),
            self.final_canonical.identity().canonical_length(),
            self.w4_subject.final_epoch(),
            witness.canonical_encoding().to_vec(),
        )
        .map_err(ProductionWorkerHandoffError::CapabilityCarriage)?;
        Ok(ProductionCapabilityCarriageInputsV5::new(
            self.final_canonical,
            self.proof_lineage,
            self.semantic_mir_identity,
            self.subjects,
            self.compiler_policy,
            self.source_refinement,
            ProductionCapabilityAnalysisCarriageV5::from_witness_records(
                obligations,
                self.target_closure,
                final_graph_report,
            ),
        ))
    }
}

/// Derives the complete canonical V13 subject roster before executing W4.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_production_w4_capability_carriage_v5(
    final_canonical: VerifiedCanonicalKernelIrV13,
    pre_optimization_graph: VerifiedCanonicalKernelIrIdentityV13,
    pre_optimization_epoch: u64,
    final_epoch: u64,
    proof_lineage: InertMultiRootProofLineageV3,
    semantic_mir_identity: [u8; 32],
    compiler_policy: [u8; 32],
    source_refinement: InertCapabilityRefinementReceiptV1,
    live_closure: &ProductionBackendCapabilityClosureV1,
    live_report: &fe2o3_pliron::ProductionFinalGraphVerificationReportV1,
    module: &Module,
    diagnostic_source_map_v2: Vec<u8>,
) -> Result<PreparedProductionW4CapabilityCarriageV5, ProductionWorkerHandoffError> {
    let live_closure = live_closure
        .validated_resource_evidence_v1()
        .map_err(ProductionWorkerHandoffError::ProductionBackend)?;
    final_canonical
        .revalidate()
        .map_err(|_| ProductionWorkerHandoffError::CapabilitySubjectMismatch("final V13 graph"))?;
    validate_final_graph_report_subject_v5(
        &live_closure,
        live_report,
        final_canonical.identity(),
        final_epoch,
    )?;
    let target_model = capability_target_model_identity_v5(live_closure.model())?;
    let subjects = capability_subject_roster_v5(
        &proof_lineage,
        final_canonical.identity(),
        final_epoch,
        target_model,
        live_closure.launch_evidence_identity(),
    )?;
    let target_closure = InertProductionTargetCapabilityClosureV5::new_for_subject_roster(
        live_closure.closure_identity(),
        *final_canonical.identity().digest(),
        final_canonical.identity().canonical_length(),
        final_epoch,
        target_model,
        &subjects,
        live_closure.canonical_closure().to_vec(),
    )
    .map_err(ProductionWorkerHandoffError::CapabilityCarriage)?;
    validate_target_closure_subject_v5(&live_closure, &subjects, &target_closure)?;

    let w4_roots = module
        .kernels
        .iter()
        .map(|kernel| {
            let launch = proof_lineage
                .roster(MultiRootProofRosterKindV3::MiddleEnd)
                .canonical_kernel_order()
                .iter()
                .enumerate()
                .find_map(|(subject_index, root_index)| {
                    let root = proof_lineage
                        .roster(MultiRootProofRosterKindV3::MiddleEnd)
                        .root(*root_index as usize)?;
                    (root.kernel_id() == kernel.id.as_str()).then_some(
                        *subjects[subject_index]
                            .launch_contract()
                            .digest()
                            .as_bytes(),
                    )
                })
                .ok_or(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                    "W4 kernel root roster",
                ))?;
            ProductionW4KernelRootV1::try_new(kernel.id.clone(), kernel.entry.clone(), launch)
                .map_err(ProductionWorkerHandoffError::CapabilityW4Input)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let aggregate_launch = *target_closure.launch_contract().digest().as_bytes();
    let target_identity = *target_model.digest().as_bytes();
    let functional_refinement_identity =
        functional_refinement_identity_v5(&proof_lineage, final_canonical.identity(), final_epoch)?;
    let w4_subject = ProductionW4FinalGraphSubjectV1::try_new(
        pre_optimization_graph,
        pre_optimization_epoch,
        *final_canonical.identity(),
        final_epoch,
        compiler_policy,
        functional_refinement_identity,
        target_identity,
        aggregate_launch,
        live_closure.closure_identity(),
        w4_roots,
    )
    .and_then(|subject| subject.with_diagnostic_source_map_v1(diagnostic_source_map_v2))
    .map_err(ProductionWorkerHandoffError::CapabilityW4Input)?;
    let w4_target = retain_production_w4_target_resource_input_v5(
        *final_canonical.identity(),
        final_epoch,
        target_identity,
        aggregate_launch,
        live_closure.closure_identity(),
        live_closure.canonical_closure().to_vec(),
        live_closure.decisions().to_vec(),
        closure_conditioned_atomic_target_v5(&live_closure, module)?,
        closure_conditioned_launch_contract_v5(&live_closure, module)?,
    )?;
    Ok(PreparedProductionW4CapabilityCarriageV5 {
        final_canonical,
        proof_lineage,
        semantic_mir_identity,
        subjects,
        compiler_policy,
        source_refinement,
        target_closure,
        w4_subject,
        w4_target,
    })
}

fn functional_refinement_identity_v5(
    proof_lineage: &InertMultiRootProofLineageV3,
    final_graph: &VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
) -> Result<[u8; 32], ProductionWorkerHandoffError> {
    let roster = proof_lineage.roster(MultiRootProofRosterKindV3::VerusExecution);
    if roster.neutral_kir().digest() != *final_graph.digest()
        || roster.neutral_kir().canonical_length() != final_graph.canonical_length()
        || roster.neutral_kir().graph_epoch() != final_epoch
        || roster.root_count() == 0
        || roster.root_count() != roster.canonical_kernel_order().len()
    {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "functional-refinement proof roster",
        ));
    }
    for index in 0..roster.root_count() {
        let root =
            roster
                .root(index)
                .ok_or(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                    "functional-refinement proof root",
                ))?;
        let evidence =
            fe2o3_verifier::CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(
                root.payload(),
            )
            .map_err(|_| {
                ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                    "signed functional-refinement receipt",
                )
            })?;
        if !evidence.authenticates_signed_receipt_under_embedded_key() {
            return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "signed functional-refinement receipt",
            ));
        }
    }
    Ok(Sha256::digest(roster.canonical_bytes()).into())
}

/// Extends one exact frozen V3 worker handoff with the native V13 capability carrier.
///
/// This is the narrow integration point after the target-lowered stage. It consumes all
/// source-side evidence and returns the only value accepted by the V5 transaction.
pub(crate) fn prepare_production_capability_handoff_v5(
    legacy_handoff: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3,
    target_closure: &ProductionBackendCapabilityClosureV1,
    final_graph_report: &fe2o3_pliron::ProductionFinalGraphVerificationReportV1,
    inputs: ProductionCapabilityCarriageInputsV5,
) -> Result<InertProductionCapabilityHandoffV5, ProductionWorkerHandoffError> {
    let target_closure = target_closure
        .validated_resource_evidence_v1()
        .map_err(ProductionWorkerHandoffError::ProductionBackend)?;
    let ProductionCapabilityCarriageInputsV5 {
        final_canonical,
        proof_lineage,
        semantic_mir_identity,
        subjects,
        compiler_policy,
        source_refinement,
        analysis,
    } = inputs;
    let ProductionCapabilityAnalysisCarriageV5 {
        obligations,
        target_closure: target_closure_record,
        final_graph_report: final_report_record,
    } = analysis;
    final_canonical
        .revalidate()
        .map_err(|_| ProductionWorkerHandoffError::CapabilitySubjectMismatch("final V13 graph"))?;
    if subjects.is_empty()
        || subjects.iter().any(|subject| {
            final_canonical.identity().digest() != subject.executable_kir().digest().as_bytes()
                || subject.executable_kir_epoch() != final_graph_report.final_epoch()
        })
        || final_canonical.identity().canonical_length()
            != proof_lineage.neutral_kir().canonical_length()
        || final_graph_report.final_graph() != final_canonical.identity()
    {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "final graph identity or epoch",
        ));
    }
    validate_target_closure_subject_v5(&target_closure, &subjects, &target_closure_record)?;
    validate_final_graph_report_subject_v5(
        &target_closure,
        final_graph_report,
        final_canonical.identity(),
        final_graph_report.final_epoch(),
    )?;
    validate_analysis_carriage_v5(
        &target_closure,
        final_graph_report,
        final_canonical.identity(),
        &subjects,
        &obligations,
        &target_closure_record,
        &final_report_record,
    )?;
    let handoff_inputs = InertProductionCapabilityHandoffInputsV5::new(
        semantic_mir_identity,
        subjects[0],
        compiler_policy,
    )
    .map_err(ProductionWorkerHandoffError::CapabilityCarriage)?;
    let executable_kir =
        fe2o3_compiler_lineage::InertCanonicalKernelIrV13ReceiptV5::from_canonical_preimage(
            final_canonical.canonical_bytes().to_vec(),
        )
        .map_err(|error| {
            ProductionWorkerHandoffError::CapabilityCarriage(
                InertProductionCapabilityHandoffErrorV5::from(error),
            )
        })?;
    InertProductionCapabilityHandoffV5::new_multi_root(
        legacy_handoff,
        executable_kir,
        proof_lineage,
        handoff_inputs,
        source_refinement,
        obligations,
        target_closure_record,
        final_report_record,
    )
    .map_err(ProductionWorkerHandoffError::CapabilityCarriage)
}

#[allow(clippy::too_many_arguments)]
fn validate_analysis_carriage_v5(
    live_closure: &ProductionBackendResourceEvidenceV1<'_>,
    live_report: &fe2o3_pliron::ProductionFinalGraphVerificationReportV1,
    final_graph: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    subjects: &[CapabilitySubjectV1],
    obligations: &[InertCapabilityObligationSetV1],
    closure: &InertProductionTargetCapabilityClosureV5,
    report: &InertProductionFinalGraphReportV5,
) -> Result<(), ProductionWorkerHandoffError> {
    if subjects.is_empty()
        || subjects.len() != obligations.len()
        || subjects
            .iter()
            .zip(obligations)
            .any(|(subject, obligations)| obligations.subject() != *subject)
        || closure.closure_identity() != live_closure.closure_identity()
        || closure.neutral_graph() != *final_graph.digest()
        || closure.neutral_graph_bytes() != final_graph.canonical_length()
        || closure.neutral_epoch() != live_report.final_epoch()
        || closure.canonical_bytes().is_empty()
        || subjects
            .iter()
            .any(|subject| closure.target_model() != subject.target_model())
        || report.final_graph() != *final_graph.digest()
        || report.final_graph_bytes() != final_graph.canonical_length()
        || report.final_epoch() != live_report.final_epoch()
        || subjects
            .iter()
            .any(|subject| report.final_epoch() != subject.executable_kir_epoch())
        || fe2o3_kernel_analysis::ProductionW4CanonicalEncodingV1::decode(
            report.canonical_results(),
        )
        .is_err()
    {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "W4 analysis witness carriage",
        ));
    }
    Ok(())
}

fn validate_target_closure_subject_v5(
    closure: &ProductionBackendResourceEvidenceV1<'_>,
    subjects: &[CapabilitySubjectV1],
    record: &InertProductionTargetCapabilityClosureV5,
) -> Result<(), ProductionWorkerHandoffError> {
    closure
        .model()
        .validate()
        .map_err(|_| ProductionWorkerHandoffError::CapabilitySubjectMismatch("target model"))?;
    let live_subject = closure.subject();
    let target_model = capability_target_model_identity_v5(closure.model())?;
    if subjects.is_empty()
        || subjects.iter().any(|subject| {
            subject.executable_kir().digest().as_bytes() != &live_subject.digest()
                || subject.executable_kir_epoch() != live_subject.epoch()
                || subject.target_model() != target_model
        })
        || record.closure_identity() != closure.closure_identity()
        || record.neutral_graph() != live_subject.digest()
        || record.neutral_graph_bytes() != live_subject.canonical_length()
        || record.neutral_epoch() != live_subject.epoch()
        || record.target_model() != target_model
        || closure.decisions().is_empty()
        || closure
            .decisions()
            .iter()
            .any(|decision| decision.validate().is_err() || decision.model() != closure.model())
    {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "target capability closure",
        ));
    }
    Ok(())
}

fn validate_final_graph_report_subject_v5(
    closure: &ProductionBackendResourceEvidenceV1<'_>,
    report: &fe2o3_pliron::ProductionFinalGraphVerificationReportV1,
    final_graph: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
) -> Result<(), ProductionWorkerHandoffError> {
    let bridge = report.bridge();
    let resources = report.resources();
    let capability = report.capability();
    if report.final_graph() != final_graph
        || report.final_epoch() != final_epoch
        || resources.final_graph() != final_graph
        || resources.final_epoch() != final_epoch
        || resources.closure_identity() != closure.closure_identity()
        || capability.canonical_identity() != final_graph
        || capability.graph_epoch() != final_epoch
        || !bridge.is_exact()
        || bridge.input().digest() != *final_graph.digest()
        || bridge.output().digest() != *final_graph.digest()
        || bridge.input().canonical_bytes() != final_graph.canonical_length()
        || bridge.output().canonical_bytes() != final_graph.canonical_length()
        || report.functions().is_empty()
        || report
            .functions()
            .iter()
            .any(|function| !function.checks().is_clean())
    {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "final graph report",
        ));
    }
    Ok(())
}

fn capability_target_model_identity_v5(
    model: fe2o3_target_spec::TargetCapabilityModelIdentityV1,
) -> Result<TargetModelIdentityV1, ProductionWorkerHandoffError> {
    model
        .validate()
        .map_err(|_| ProductionWorkerHandoffError::CapabilitySubjectMismatch("target model"))?;
    let mut digest = Sha256::new();
    digest.update(CAPABILITY_TARGET_MODEL_DOMAIN_V5);
    for word in model.profile_fingerprint().words() {
        digest.update(word.to_le_bytes());
    }
    for word in model.revision_fingerprint().words() {
        digest.update(word.to_le_bytes());
    }
    Ok(TargetModelIdentityV1::from_untrusted_digest(
        DigestV1::from_untrusted_bytes(digest.finalize().into()),
    ))
}

fn capability_subject_roster_v5(
    proof_lineage: &InertMultiRootProofLineageV3,
    final_graph: &VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    target_model: TargetModelIdentityV1,
    launch_evidence: [u8; 32],
) -> Result<Vec<CapabilitySubjectV1>, ProductionWorkerHandoffError> {
    let roster = proof_lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
    if roster.neutral_kir().digest() != *final_graph.digest()
        || roster.neutral_kir().canonical_length() != final_graph.canonical_length()
        || roster.neutral_kir().graph_epoch() != final_epoch
        || roster.root_count() != roster.canonical_kernel_order().len()
    {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "proof-lineage graph roster",
        ));
    }
    roster
        .canonical_kernel_order()
        .iter()
        .enumerate()
        .map(|(subject_index, root_index)| {
            let root = roster.root(*root_index as usize).ok_or(
                ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                    "canonical kernel permutation",
                ),
            )?;
            let mut digest = Sha256::new();
            digest.update(CAPABILITY_ROOT_LAUNCH_DOMAIN_V5);
            digest.update(launch_evidence);
            digest.update((subject_index as u64).to_le_bytes());
            digest.update(root.semantic_root().to_le_bytes());
            digest.update(root.semantic_root_identity());
            digest.update(root.kernel_binding());
            digest.update([root.source_rank()]);
            for dimension in root.workgroup() {
                digest.update(dimension.to_le_bytes());
            }
            digest.update((root.kernel_id().len() as u64).to_le_bytes());
            digest.update(root.kernel_id().as_bytes());
            CapabilitySubjectV1::new(
                KernelIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    root.kernel_binding(),
                )),
                KernelRootIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    root.semantic_root_identity(),
                )),
                ExecutableKirIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    *final_graph.digest(),
                )),
                final_epoch,
                target_model,
                LaunchContractIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    digest.finalize().into(),
                )),
            )
            .map_err(ProductionWorkerHandoffError::CapabilityCodec)
        })
        .collect()
}

fn capability_obligations_v5(
    subject: CapabilitySubjectV1,
    witness: &ProductionW4FinalGraphCapabilityWitnessV1,
    functional_refinement_identity: [u8; 32],
    source_refinement: &InertCapabilityRefinementReceiptV1,
    target_closure: &InertProductionTargetCapabilityClosureV5,
) -> Result<InertCapabilityObligationSetV1, ProductionWorkerHandoffError> {
    let source = source_refinement.identity();
    let specs = STATIC_CAPABILITY_PROPERTIES_V5
        .into_iter()
        .map(|property| {
            let mut digest = Sha256::new();
            digest.update(CAPABILITY_STATEMENT_DOMAIN_V5);
            digest.update(subject.kernel().digest().as_bytes());
            digest.update(subject.root().digest().as_bytes());
            digest.update(subject.executable_kir().digest().as_bytes());
            digest.update(subject.executable_kir_epoch().to_le_bytes());
            digest.update(functional_refinement_identity);
            digest.update(subject.target_model().digest().as_bytes());
            digest.update(subject.launch_contract().digest().as_bytes());
            digest.update(property.namespace().as_bytes());
            digest.update(property.schema_version().to_le_bytes());
            digest.update(property.code().to_le_bytes());
            digest.update(witness.identity().digest());
            digest.update(witness.identity().canonical_length().to_le_bytes());
            digest.update(target_closure.closure_identity());
            digest.update(source.sha256());
            digest.update(source.byte_len().to_le_bytes());
            CapabilityObligationSpecV1::new(
                property,
                StatementIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    digest.finalize().into(),
                )),
            )
        })
        .collect();
    InertCapabilityObligationSetV1::from_specs(subject, specs)
        .map_err(ProductionWorkerHandoffError::CapabilityCodec)
}

fn closure_conditioned_atomic_target_v5(
    closure: &ProductionBackendResourceEvidenceV1<'_>,
    module: &Module,
) -> Result<PlironAtomicTargetContextV1, ProductionWorkerHandoffError> {
    let mut capabilities = BTreeSet::new();
    for decision in closure.decisions().iter().copied() {
        if !matches!(
            decision.outcome(),
            TargetCapabilityDecisionOutcomeV1::Supported
                | TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(_)
        ) {
            return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "nonfinal target atomic decision",
            ));
        }
        let TargetCapabilityRequirementV1::Atomic(atomic) = decision.requirement() else {
            continue;
        };
        let width = u32::from(atomic.value_type().bit_width());
        let memory_space = match atomic.address_space() {
            TargetAddressSpaceV1::Global => MemorySpaceAttr::Global,
            TargetAddressSpaceV1::Workgroup => MemorySpaceAttr::Workgroup,
            TargetAddressSpaceV1::Private
            | TargetAddressSpaceV1::Constant
            | TargetAddressSpaceV1::Generic => {
                return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                    "unrepresentable target atomic address space",
                ));
            }
        };
        let max_scope = match atomic.scope() {
            TargetMemoryScopeV1::Invocation => AtomicScopeAttr::SingleThread,
            TargetMemoryScopeV1::Workgroup => AtomicScopeAttr::Workgroup,
            TargetMemoryScopeV1::Device => AtomicScopeAttr::Device,
            TargetMemoryScopeV1::System => AtomicScopeAttr::System,
            TargetMemoryScopeV1::Subgroup => {
                return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                    "subgroup atomic scope has no exact ranked PLIRON representation",
                ));
            }
        };
        capabilities.insert(
            PlironAtomicTargetCapabilityV1::new(width, memory_space, max_scope)
                .map_err(ProductionWorkerHandoffError::CapabilityAtomicTarget)?,
        );
    }
    let target = PlironAtomicTargetContextV1::new(capabilities)
        .map_err(ProductionWorkerHandoffError::CapabilityAtomicTarget)?;
    if closure.system_atomic_memory_eligible() {
        let origins = exact_global_host_allocation_origins_v5(module)?;
        target
            .with_system_coherent_allocations(origins)
            .map_err(ProductionWorkerHandoffError::CapabilityAtomicTarget)
    } else {
        Ok(target)
    }
}

fn closure_conditioned_launch_contract_v5(
    closure: &ProductionBackendResourceEvidenceV1<'_>,
    module: &Module,
) -> Result<PlironLaunchContractV1, ProductionWorkerHandoffError> {
    let target_identity = capability_target_model_identity_v5(closure.model())?;
    launch_contract_from_exact_decisions_v5(
        closure.decisions(),
        target_identity.digest().as_bytes(),
        module,
        closure.target().pointer_width_bits(),
        closure.target().wave_width_bits(),
    )
}

fn launch_contract_from_exact_decisions_v5(
    decisions: &[fe2o3_target_spec::TargetCapabilityDecisionV1],
    target_identity: &[u8; 32],
    module: &Module,
    pointer_width_bits: u16,
    default_subgroup_width: u16,
) -> Result<PlironLaunchContractV1, ProductionWorkerHandoffError> {
    let origins = exact_global_host_allocation_origins_v5(module)?;
    let mut max_workgroup_extents = [0_u64; 3];
    let mut max_workgroup_invocations = 0_u64;
    let mut supported_subgroup_sizes = BTreeSet::new();
    let mut max_workgroup_memory_bytes = 0_u64;
    for decision in decisions.iter().copied() {
        if !matches!(
            decision.outcome(),
            TargetCapabilityDecisionOutcomeV1::Supported
                | TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(_)
        ) {
            return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "nonfinal target launch decision",
            ));
        }
        match decision.requirement() {
            TargetCapabilityRequirementV1::SubgroupSize(width) => {
                supported_subgroup_sizes.insert(u64::from(width));
            }
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(value),
            ) => {
                max_workgroup_invocations = max_workgroup_invocations.max(u64::from(value));
            }
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x, y, z },
            ) => {
                for (current, observed) in
                    max_workgroup_extents
                        .iter_mut()
                        .zip([u64::from(x), u64::from(y), u64::from(z)])
                {
                    *current = (*current).max(observed);
                }
            }
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(value)
                | TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(value),
            ) => {
                max_workgroup_memory_bytes = max_workgroup_memory_bytes.max(value);
            }
            _ => {}
        }
    }
    if max_workgroup_extents.contains(&0) || max_workgroup_invocations == 0 {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "target decisions omit exact workgroup limits",
        ));
    }
    if supported_subgroup_sizes.is_empty() {
        supported_subgroup_sizes.insert(u64::from(default_subgroup_width));
    }
    let required_global_alignment = exact_global_storage_alignment_v5(module, pointer_width_bits)?;
    let limits = PlironLaunchTargetLimitsV1::new(
        [u32::MAX as u64; 3],
        max_workgroup_extents,
        max_workgroup_invocations,
        supported_subgroup_sizes.into_iter().collect(),
        max_workgroup_memory_bytes,
        required_global_alignment,
        origins.len(),
    )
    .map_err(ProductionWorkerHandoffError::CapabilityLaunchContract)?;
    let allocations = origins
        .into_iter()
        .map(PlironHostAllocationV1::runtime_required)
        .collect::<Result<Vec<_>, _>>()
        .map_err(ProductionWorkerHandoffError::CapabilityLaunchContract)?;
    let (decision_set_identity, _) =
        derive_target_decision_set_identity_v1(decisions, target_identity)
            .map_err(ProductionWorkerHandoffError::CapabilityW4Input)?;
    PlironLaunchContractV1::new_bound_to_target_decisions(
        limits,
        allocations,
        decision_set_identity,
    )
    .map_err(ProductionWorkerHandoffError::CapabilityLaunchContract)
}

fn exact_global_host_allocation_origins_v5(
    module: &Module,
) -> Result<BTreeSet<u64>, ProductionWorkerHandoffError> {
    let mut origins = BTreeSet::new();
    for kernel in &module.kernels {
        let function = module.function(&kernel.entry).ok_or(
            ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "kernel entry for host allocation contract",
            ),
        )?;
        for (index, parameter) in function.signature.parameters.iter().enumerate() {
            if !is_physical_global_storage_v5(parameter) {
                continue;
            }
            let origin = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                    "global allocation origin overflow",
                ))?;
            origins.insert(origin);
        }
    }
    if origins.len() > MAX_PLIRON_HOST_ALLOCATIONS_V1 {
        return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "global allocation origin limit",
        ));
    }
    Ok(origins)
}

fn is_physical_global_storage_v5(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Pointer(pointer) if pointer.address_space == AddressSpace::Global
    ) || matches!(
        ty,
        Type::Slice(slice) if slice.address_space == AddressSpace::Global
    )
}

fn exact_global_storage_alignment_v5(
    module: &Module,
    pointer_width_bits: u16,
) -> Result<u64, ProductionWorkerHandoffError> {
    let mut alignment = 1_u64;
    for kernel in &module.kernels {
        let function = module.function(&kernel.entry).ok_or(
            ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "kernel entry for host alignment contract",
            ),
        )?;
        for parameter in &function.signature.parameters {
            let element = match parameter {
                Type::Pointer(pointer) if pointer.address_space == AddressSpace::Global => {
                    Some(pointer.pointee.as_ref())
                }
                Type::Slice(slice) if slice.address_space == AddressSpace::Global => {
                    Some(slice.element.as_ref())
                }
                _ => None,
            };
            if let Some(element) = element {
                alignment = alignment.max(type_storage_alignment_v5(element, pointer_width_bits)?);
            }
        }
    }
    Ok(alignment)
}

fn type_storage_alignment_v5(
    ty: &Type,
    pointer_width_bits: u16,
) -> Result<u64, ProductionWorkerHandoffError> {
    let bits = match ty {
        Type::Scalar(scalar) => scalar.bit_width().unwrap_or(pointer_width_bits),
        Type::Pointer(_) | Type::Slice(_) => pointer_width_bits,
        Type::Unit => 8,
        Type::KernelContext(_) | Type::GlobalCapability(_) | Type::ExecutionCapability(_) => {
            return Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "logical capability used as physical global element",
            ));
        }
    };
    let bytes = u64::from(bits).div_ceil(8).max(1);
    bytes.checked_next_power_of_two().ok_or(
        ProductionWorkerHandoffError::CapabilitySubjectMismatch(
            "global storage alignment overflow",
        ),
    )
}

/// Prepares the exact target-lowered production module for the managed worker.
///
/// This transition derives the closed symbol manifest, binds the target and
/// code-object envelope, and constructs canonical coordination bytes. It does
/// not invoke LLVM, link, publish an artifact, load, or launch.
pub(crate) fn prepare_production_worker_handoff(
    authenticated: crate::production_pipeline::AuthenticatedProductionTargetModule,
    final_v13_symbols: crate::single_codegen_ownership_v1::FinalV13ExpectedDeviceSymbolRosterV1,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    prepare_production_worker_handoff_inner(authenticated, Some(final_v13_symbols))
}

pub(crate) fn prepare_production_worker_handoff_for_extraction(
    authenticated: crate::production_pipeline::AuthenticatedProductionTargetModule,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    prepare_production_worker_handoff_inner(authenticated, None)
}

fn prepare_production_worker_handoff_inner(
    authenticated: crate::production_pipeline::AuthenticatedProductionTargetModule,
    final_v13_symbols: Option<
        crate::single_codegen_ownership_v1::FinalV13ExpectedDeviceSymbolRosterV1,
    >,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    let (formal, target, module, llvm_ir, typed_roots, compiler_ffi_envelope) =
        authenticated.into_parts();
    validate_exact_target_binding(target, &module)?;
    let canonical_kernel_ir_identity = *formal
        .semantic_kir()
        .canonical_kernel_ir_identity()
        .digest();
    let compiler_module = retain_production_compiler_module_text_v1(&module, llvm_ir)
        .map_err(ProductionWorkerHandoffError::CompilerModule)?;
    let envelope = derive_production_compiler_ffi_envelope(
        target,
        &module,
        &compiler_module,
        compiler_ffi_envelope,
        canonical_kernel_ir_identity,
    )?;
    validate_exact_target_binding(envelope.target(), &module)?;
    validate_envelope_module_roles(&envelope, &compiler_module)?;
    let generated_host_contract_identities = typed_roots
        .iter()
        .map(TypedDescriptorRootV1::generated_host_contract_identity)
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let descriptor_source = construct_production_v1_compiler_descriptor_source_v1(
        &envelope,
        &module,
        &compiler_module,
        &typed_roots,
        &formal,
    )
    .map_err(ProductionWorkerHandoffError::CompilerDescriptor)?;
    let compiler_module = bind_compiler_descriptor_source_v1(compiler_module, &descriptor_source)
        .map_err(ProductionWorkerHandoffError::CompilerModule)?;
    let symbol_manifest = construct_symbol_manifest(&compiler_module)
        .map_err(ProductionWorkerHandoffError::SymbolManifest)?;
    if let Some(final_v13_symbols) = final_v13_symbols {
        final_v13_symbols
            .verify_compiler_module_manifest_v1(&symbol_manifest)
            .map_err(ProductionWorkerHandoffError::SingleCodegenOwnership)?;
    }
    let llvm_ir_sha256 = Sha256::digest(compiler_module.llvm_ir().as_bytes()).into();
    let handoff = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        envelope,
        symbol_manifest,
        compiler_module.llvm_ir().as_bytes(),
    )
    .map_err(ProductionWorkerHandoffError::Handoff)?;
    Ok(PreparedProductionWorkerHandoff {
        llvm_ir_sha256,
        handoff,
        compiler_descriptor_source: descriptor_source,
        generated_host_contract_identities,
    })
}

fn derive_production_compiler_ffi_envelope(
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    module: &Module,
    compiler_module: &crate::kernel_ir_codegen::InertCompilerModuleTextV1,
    observed_source_envelope: Option<CompilerFfiEnvelopeV1>,
    canonical_kernel_ir_identity: [u8; 32],
) -> Result<CompilerFfiEnvelopeV1, ProductionWorkerHandoffError> {
    let profile = match target.to_string().as_str() {
        "gfx942:xnack-" => ProductionOcmlTargetV1::Gfx942,
        "gfx950:xnack-" => ProductionOcmlTargetV1::Gfx950,
        _ => {
            return match observed_source_envelope {
                Some(envelope) => Ok(envelope),
                None => CompilerFfiEnvelopeV1::for_module_without_device_ffi(
                    target,
                    CodeObjectVersion::V6,
                )
                .map_err(ProductionWorkerHandoffError::CompilerEnvelope),
            };
        }
    };

    if observed_source_envelope
        .as_ref()
        .is_some_and(|envelope| envelope.directional_symbols().total_count() != 0)
    {
        return Err(profile.source_ffi_not_admitted());
    }

    let ocml_imports = typed_ocml_imports(module);
    match ocml_imports.as_slice() {
        [] => {
            if !compiler_module.external_declarations().is_empty()
                || compiler_module.llvm_ir().contains("@__ocml_")
            {
                return Err(profile.import_policy_error());
            }
            let envelope =
                CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                    .map_err(ProductionWorkerHandoffError::CompilerEnvelope)?;
            debug_assert!(profile.inspects_no_device_ffi(&envelope));
            Ok(envelope)
        }
        [symbol] if *symbol == PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1 => {
            if !reachable_ocml_exp_f32_call(module) {
                return Err(profile.exp_not_executable_error());
            }
            if compiler_module.external_declarations() != [PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1]
                || !has_exact_ocml_exp_llvm_shape(compiler_module.llvm_ir())
            {
                return Err(profile.llvm_mismatch_error());
            }
            profile.construct_exp_envelope(canonical_kernel_ir_identity)
        }
        _ => Err(profile.import_policy_error()),
    }
}

#[derive(Clone, Copy)]
enum ProductionOcmlTargetV1 {
    Gfx942,
    Gfx950,
}

impl ProductionOcmlTargetV1 {
    fn construct_exp_envelope(
        self,
        canonical_kernel_ir_identity: [u8; 32],
    ) -> Result<CompilerFfiEnvelopeV1, ProductionWorkerHandoffError> {
        match self {
            Self::Gfx942 => {
                construct_production_gfx942_ocml_exp_envelope_v1(canonical_kernel_ir_identity)
            }
            Self::Gfx950 => {
                construct_production_gfx950_ocml_exp_envelope_v1(canonical_kernel_ir_identity)
            }
        }
        .map_err(ProductionWorkerHandoffError::CompilerEnvelope)
    }

    fn inspects_no_device_ffi(self, envelope: &CompilerFfiEnvelopeV1) -> bool {
        match self {
            Self::Gfx942 => matches!(
                inspect_production_gfx942_compiler_ffi_envelope_v1(envelope),
                Some(ProductionGfx942CompilerFfiEnvelopeKindV1::NoDeviceFfi)
            ),
            Self::Gfx950 => matches!(
                inspect_production_gfx950_compiler_ffi_envelope_v1(envelope),
                Some(ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi)
            ),
        }
    }

    const fn source_ffi_not_admitted(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942SourceFfiNotAdmitted,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950SourceFfiNotAdmitted,
        }
    }

    const fn import_policy_error(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942OcmlImportPolicy,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950OcmlImportPolicy,
        }
    }

    const fn exp_not_executable_error(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942OcmlExpNotExecutable,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950OcmlExpNotExecutable,
        }
    }

    const fn llvm_mismatch_error(self) -> ProductionWorkerHandoffError {
        match self {
            Self::Gfx942 => ProductionWorkerHandoffError::Gfx942OcmlLlvmMismatch,
            Self::Gfx950 => ProductionWorkerHandoffError::Gfx950OcmlLlvmMismatch,
        }
    }
}

fn typed_ocml_imports(module: &Module) -> Vec<&'static str> {
    let mut imports = module
        .functions
        .iter()
        .filter_map(|function| {
            let FloatOperation::F32Math {
                function,
                implementation: F32MathImplementation::OcmlAbiV1,
                ..
            } = FloatOperation::from_intrinsic_id(&function.id)?
            else {
                return None;
            };
            Some(match function {
                F32MathFunction::Exp => PRODUCTION_GFX942_OCML_EXP_F32_SYMBOL_V1,
                F32MathFunction::Sin => "__ocml_sin_f32",
                F32MathFunction::Cos => "__ocml_cos_f32",
                F32MathFunction::Exp2 => "__ocml_exp2_f32",
                F32MathFunction::Ln => "__ocml_log_f32",
                F32MathFunction::Log2 => "__ocml_log2_f32",
                F32MathFunction::Log10 => "__ocml_log10_f32",
                F32MathFunction::Sqrt
                | F32MathFunction::FusedMultiplyAdd
                | F32MathFunction::Floor
                | F32MathFunction::Ceil
                | F32MathFunction::Truncate
                | F32MathFunction::RoundTiesEven
                | F32MathFunction::Abs => return None,
            })
        })
        .collect::<Vec<_>>();
    imports.sort_unstable();
    imports.dedup();
    imports
}

fn reachable_ocml_exp_f32_call(module: &Module) -> bool {
    let mut pending = module
        .kernels
        .iter()
        .map(|kernel| kernel.entry.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    while let Some(function_id) = pending.pop() {
        if !visited.insert(function_id.clone()) {
            continue;
        }
        let Some(function) = module
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
        else {
            continue;
        };
        let Some(body) = &function.body else {
            continue;
        };
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            let OperationKind::Call { callee, .. } = &operation.kind else {
                continue;
            };
            if FloatOperation::from_intrinsic_id(callee).is_some_and(|operation| {
                matches!(
                    operation,
                    FloatOperation::F32Math {
                        function: F32MathFunction::Exp,
                        implementation: F32MathImplementation::OcmlAbiV1,
                        ..
                    }
                )
            }) {
                return module.function(callee).is_some_and(|declaration| {
                    declaration.role == FunctionRole::ExternalImport && declaration.body.is_none()
                });
            }
            pending.push(callee.as_str().to_owned());
        }
    }
    false
}

fn has_exact_ocml_exp_llvm_shape(llvm_ir: &str) -> bool {
    llvm_ir
        .matches("declare float @__ocml_exp_f32(float)")
        .count()
        == 1
        && llvm_ir.matches("call float @__ocml_exp_f32(float ").count() >= 1
        && llvm_ir
            .split("@__ocml_")
            .skip(1)
            .all(|suffix| suffix.starts_with("exp_f32"))
}

#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum ProductionWorkerHandoffError {
    MissingProductionBindings,
    ProductionBackend(crate::production_backend_v1::ProductionBackendErrorV1),
    CapabilitySubjectMismatch(&'static str),
    CapabilityCarriage(InertProductionCapabilityHandoffErrorV5),
    CapabilityCodec(fe2o3_proof_contracts::CapabilityCodecErrorV1),
    CapabilityW4Input(fe2o3_kernel_analysis::ProductionW4InputErrorV1),
    CapabilityW4Encoding(fe2o3_kernel_analysis::ProductionW4EncodingErrorV1),
    CapabilityAtomicTarget(fe2o3_kernel_analysis::PlironAtomicTargetContextErrorV1),
    CapabilityLaunchContract(fe2o3_kernel_analysis::PlironLaunchContractInputErrorV1),
    MissingExternalDeclaration(String),
    MissingCompilerDefinition(String),
    TargetBindingMismatch {
        module: Vec<String>,
        envelope: String,
    },
    Gfx942SourceFfiNotAdmitted,
    Gfx942OcmlImportPolicy,
    Gfx942OcmlExpNotExecutable,
    Gfx942OcmlLlvmMismatch,
    Gfx950SourceFfiNotAdmitted,
    Gfx950OcmlImportPolicy,
    Gfx950OcmlExpNotExecutable,
    Gfx950OcmlLlvmMismatch,
    CompilerModule(CompilerModuleConstructionError),
    CompilerEnvelope(CompilerFfiEnvelopeError),
    CompilerDescriptor(CompilerDescriptorError),
    SymbolManifest(CompilerModuleSymbolManifestErrorV1),
    SingleCodegenOwnership(String),
    Handoff(CompilerModuleHandoffErrorV2),
}

impl fmt::Display for ProductionWorkerHandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingProductionBindings => formatter
                .write_str("production compiler handoff lost its exact LLVM identity binding"),
            Self::ProductionBackend(error) => {
                write!(formatter, "production backend evidence failed: {error}")
            }
            Self::CapabilitySubjectMismatch(field) => {
                write!(formatter, "production V5 capability subject mismatch: {field}")
            }
            Self::CapabilityCarriage(error) => {
                write!(formatter, "production V5 capability carriage failed: {error}")
            }
            Self::CapabilityCodec(error) => {
                write!(formatter, "production V5 capability contract failed: {error:?}")
            }
            Self::CapabilityW4Input(error) => {
                write!(formatter, "production V5 W4 input failed: {error}")
            }
            Self::CapabilityW4Encoding(error) => {
                write!(formatter, "production V5 W4 encoding failed: {error}")
            }
            Self::CapabilityAtomicTarget(error) => {
                write!(formatter, "production V5 atomic target failed: {error}")
            }
            Self::CapabilityLaunchContract(error) => {
                write!(formatter, "production V5 launch contract failed: {error}")
            }
            Self::MissingExternalDeclaration(symbol) => write!(
                formatter,
                "compiler FFI import {symbol:?} is absent from the whole Kernel IR module"
            ),
            Self::MissingCompilerDefinition(symbol) => write!(
                formatter,
                "compiler FFI export {symbol:?} is absent from the whole Kernel IR module"
            ),
            Self::TargetBindingMismatch { module, envelope } => write!(
                formatter,
                "compiler-module exact target bindings {module:?} do not match production envelope target {envelope:?}"
            ),
            Self::Gfx942SourceFfiNotAdmitted => formatter.write_str(
                "production gfx942 admits no caller/source-authored device FFI envelope",
            ),
            Self::Gfx942OcmlImportPolicy => formatter.write_str(
                "production gfx942 admits either no device FFI or only compiler-derived __ocml_exp_f32",
            ),
            Self::Gfx942OcmlExpNotExecutable => formatter.write_str(
                "production gfx942 OCML exp declaration is not called from the executable Kernel IR closure",
            ),
            Self::Gfx942OcmlLlvmMismatch => formatter.write_str(
                "production gfx942 LLVM does not retain the exact Kernel-IR-derived OCML exp declaration/call closure",
            ),
            Self::Gfx950SourceFfiNotAdmitted => formatter.write_str(
                "production gfx950 admits no caller/source-authored device FFI envelope",
            ),
            Self::Gfx950OcmlImportPolicy => formatter.write_str(
                "production gfx950 admits either no device FFI or only compiler-derived __ocml_exp_f32",
            ),
            Self::Gfx950OcmlExpNotExecutable => formatter.write_str(
                "production gfx950 OCML exp declaration is not called from the executable Kernel IR closure",
            ),
            Self::Gfx950OcmlLlvmMismatch => formatter.write_str(
                "production gfx950 LLVM does not retain the exact Kernel-IR-derived OCML exp declaration/call closure",
            ),
            Self::CompilerModule(error) => {
                write!(
                    formatter,
                    "whole compiler-module construction failed: {error}"
                )
            }
            Self::CompilerEnvelope(error) => {
                write!(formatter, "exact compiler FFI envelope failed: {error}")
            }
            Self::CompilerDescriptor(error) => {
                write!(
                    formatter,
                    "compiler descriptor construction failed: {error}"
                )
            }
            Self::SymbolManifest(error) => {
                write!(
                    formatter,
                    "compiler symbol manifest construction failed: {error}"
                )
            }
            Self::SingleCodegenOwnership(error) => {
                write!(formatter, "single-code-generation ownership failed: {error}")
            }
            Self::Handoff(error) => {
                write!(
                    formatter,
                    "compiler-module handoff construction failed: {error}"
                )
            }
        }
    }
}

impl Error for ProductionWorkerHandoffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProductionBackend(error) => Some(error),
            Self::CapabilityCarriage(error) => Some(error),
            Self::CapabilityW4Input(error) => Some(error),
            Self::CapabilityW4Encoding(error) => Some(error),
            Self::CapabilityAtomicTarget(error) => Some(error),
            Self::CapabilityLaunchContract(error) => Some(error),
            Self::CompilerModule(error) => Some(error),
            Self::CompilerEnvelope(error) => Some(error),
            Self::CompilerDescriptor(error) => Some(error),
            Self::SymbolManifest(error) => Some(error),
            Self::Handoff(error) => Some(error),
            Self::MissingProductionBindings
            | Self::CapabilitySubjectMismatch(_)
            | Self::CapabilityCodec(_)
            | Self::MissingExternalDeclaration(_)
            | Self::MissingCompilerDefinition(_)
            | Self::Gfx942SourceFfiNotAdmitted
            | Self::Gfx942OcmlImportPolicy
            | Self::Gfx942OcmlExpNotExecutable
            | Self::Gfx942OcmlLlvmMismatch
            | Self::Gfx950SourceFfiNotAdmitted
            | Self::Gfx950OcmlImportPolicy
            | Self::Gfx950OcmlExpNotExecutable
            | Self::Gfx950OcmlLlvmMismatch
            | Self::SingleCodegenOwnership(_)
            | Self::TargetBindingMismatch { .. } => None,
        }
    }
}

impl From<ExactTargetBindingError> for ProductionWorkerHandoffError {
    fn from(error: ExactTargetBindingError) -> Self {
        Self::TargetBindingMismatch {
            module: error.module,
            envelope: error.envelope,
        }
    }
}

impl From<CompilerModuleRoleError> for ProductionWorkerHandoffError {
    fn from(error: CompilerModuleRoleError) -> Self {
        match error {
            CompilerModuleRoleError::MissingExternalDeclaration(symbol) => {
                Self::MissingExternalDeclaration(symbol)
            }
            CompilerModuleRoleError::MissingCompilerDefinition(symbol) => {
                Self::MissingCompilerDefinition(symbol)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel_ir_codegen::construct_inert_compiler_module_text_for_target_v1;
    use fe2o3_compiler_ffi::DeviceTargetV1;
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, AssemblyConstraint, AssemblyOperand, AssemblyOption,
        AssemblySourceIdentity, Atomic, AtomicKind, Barrier, BarrierSemantics, BasicBlock, BlockId,
        Convergence, DebugSourceMapBindingV1, DebugSourceMapDocumentV2, DebugSourceMapFileV1,
        ExecutionCapabilityRequirementV1, Fence, Function, Gfx950LdsTransposeFormatV1,
        Gfx950LdsTransposeOperationKindV1, Gfx950LdsTransposeOperationV1, InlineAssembly,
        InlineAssemblyTarget, IntegerSwitchCase, Kernel, LaunchDomain, LaunchExtent,
        MatrixOperation, MemoryAccess, MemoryElementType, MemoryIntrinsicOperation, MemoryLayout,
        MemoryOrdering, Operation, OperationKind, ResourceCapabilityRequirementV1, ScalarType,
        Signature, SwitchCase, SynchronizationScope, TargetCapability, TensorLayoutContractV1,
        Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13, VolatileAccessContract,
        WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier, WorkgroupMemory,
        WorkgroupMemoryExtent, WorkgroupSize,
    };
    use fe2o3_target_spec::{
        TargetAddressSpaceV1, TargetArchitectureFamilyV1, TargetArtifactFormatV1,
        TargetAtomicOperationV1, TargetAtomicRequirementV1, TargetCapabilityModelIdentityV1,
        TargetCapabilityQueryV1, TargetExecutionModelV1, TargetMemoryOrderingV1,
        TargetMemoryScopeV1, TargetProfileSpecV1, TargetScalarKindV1, TargetScalarTypeV1,
        TargetVendorV1, query_target_capability_v1,
    };

    const W4_TEST_TARGET_PROFILE: TargetProfileSpecV1 = TargetProfileSpecV1::from_static_parts(
        TargetVendorV1::Other,
        TargetArchitectureFamilyV1::Other,
        "w4-worker-test",
        None,
        None,
        TargetArtifactFormatV1::NativeObject,
        TargetExecutionModelV1::GpuGrid,
        None,
        &[],
    );
    const W4_TEST_TARGET_MODEL: TargetCapabilityModelIdentityV1 =
        TargetCapabilityModelIdentityV1::new_unchecked(W4_TEST_TARGET_PROFILE, "w4-worker-test-v1");

    struct W4TestTarget;

    impl TargetCapabilityQueryV1 for W4TestTarget {
        fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
            W4_TEST_TARGET_MODEL
        }

        fn query_outcome(
            &self,
            _requirement: TargetCapabilityRequirementV1,
        ) -> TargetCapabilityDecisionOutcomeV1 {
            TargetCapabilityDecisionOutcomeV1::Supported
        }
    }

    fn math_module(functions: &[F32MathFunction], called: &[F32MathFunction]) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        for (index, function) in called.iter().copied().enumerate() {
            block.operations.push(
                FloatOperation::F32Math {
                    function,
                    implementation: F32MathImplementation::OcmlAbiV1,
                    arguments: vec![ValueId(0)],
                }
                .operation(ValueId(u32::try_from(index + 1).unwrap())),
            );
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "kernel",
            Signature::new(vec![Type::F32], vec![]),
            vec![ValueId(0)],
            vec![block],
        );
        let mut kernel = Kernel::new(
            "kernel",
            "kernel",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(256, 1, 1));
        let mut module = Module::new("tests::production_gfx942_ocml");
        module.functions.push(entry);
        for function in functions {
            module.functions.push(
                FloatOperation::F32Math {
                    function: *function,
                    implementation: F32MathImplementation::OcmlAbiV1,
                    arguments: vec![ValueId(0)],
                }
                .declaration(),
            );
        }
        module.kernels.push(kernel);
        module
    }

    fn gfx942_compiler_module(
        module: &Module,
    ) -> crate::kernel_ir_codegen::InertCompilerModuleTextV1 {
        construct_inert_compiler_module_text_for_target_v1(
            module,
            Some(DeviceTargetV1::parse("gfx942:xnack-").unwrap()),
        )
        .unwrap()
    }

    fn w4_global_module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let entry = Function::kernel_entry(
            "kernel",
            Signature::new(
                vec![
                    Type::F32,
                    Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
                    Type::pointer(
                        Type::Scalar(ScalarType::U128),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    ),
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        );
        let mut kernel = Kernel::new(
            "kernel",
            "kernel",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new("tests::w4_runtime_contract");
        module.functions.push(entry);
        module.kernels.push(kernel);
        module
    }

    fn w4_all_safety_families_module() -> Module {
        let u32_ty = Type::Scalar(ScalarType::U32);
        let global_u32 = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let mut intrinsic_block = BasicBlock::new(BlockId(0));
        intrinsic_block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::INDEX),
            OperationKind::Intrinsic(fe2o3_kernel_ir::IntrinsicOperation::global_id_1d()),
        ));
        intrinsic_block.terminator = Some(Terminator::Return { values: vec![] });
        let intrinsic = Function::kernel_entry(
            "all_families_entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![intrinsic_block],
        );

        let mut memory_intrinsic_block = BasicBlock::new(BlockId(0));
        memory_intrinsic_block.operations.push(Operation::new(
            vec![],
            OperationKind::MemoryIntrinsic(MemoryIntrinsicOperation::VolatileStore {
                pointer: ValueId(0),
                value: ValueId(1),
                element: MemoryElementType::Scalar(ScalarType::U32),
                address_space: AddressSpace::Global,
                layout: MemoryLayout::new(4, 4),
                contract: VolatileAccessContract::rust_allocation_store(),
            }),
        ));
        memory_intrinsic_block.terminator = Some(Terminator::Return { values: vec![] });
        let memory_intrinsic = Function::internal_helper(
            "memory_intrinsic",
            Signature::new(vec![global_u32.clone(), u32_ty.clone()], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![memory_intrinsic_block],
        );

        let private_u32 =
            Type::pointer(u32_ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
        let workgroup_u32 = Type::pointer(
            u32_ty.clone(),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        let mut memory_block = BasicBlock::new(BlockId(0));
        memory_block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(4), private_u32),
                OperationKind::Alloca {
                    element: u32_ty.clone(),
                    count: Some(ValueId(3)),
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(5), u32_ty.clone()),
                OperationKind::GuardedLoad {
                    pointer: ValueId(0),
                    predicate: ValueId(1),
                    fallback: ValueId(2),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::GuardedStore {
                    pointer: ValueId(0),
                    predicate: ValueId(1),
                    value: ValueId(5),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                }),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(6), workgroup_u32),
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element: u32_ty.clone(),
                    extent: WorkgroupMemoryExtent::Static(64),
                    alignment: 16,
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::Fence(Fence {
                    memory_scope: SynchronizationScope::Device,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::Release,
                        [AddressSpace::Global],
                    ),
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                    convergence: Convergence::uniform(SynchronizationScope::Workgroup),
                }),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(7), u32_ty.clone()),
                OperationKind::Atomic(Atomic {
                    kind: AtomicKind::Add,
                    pointer: ValueId(0),
                    value: Some(ValueId(5)),
                    compare: None,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                    scope: SynchronizationScope::Device,
                    ordering: MemoryOrdering::AcquireRelease,
                    failure_ordering: None,
                }),
            ),
        ];
        memory_block.terminator = Some(Terminator::Return { values: vec![] });
        let mut memory = Function::internal_helper(
            "memory_and_synchronization",
            Signature::new(
                vec![global_u32, Type::BOOL, u32_ty.clone(), Type::INDEX],
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
            vec![memory_block],
        );
        memory
            .required_capabilities
            .insert(TargetCapability::Execution(
                ExecutionCapabilityRequirementV1::Resource(
                    ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(256),
                ),
            ));

        let mut matrix_parameters = vec![Type::Scalar(ScalarType::Bf16); 8];
        matrix_parameters.extend(vec![Type::F32; 4]);
        let mut matrix_block = BasicBlock::new(BlockId(0));
        matrix_block.operations.push(Operation::new(
            (12..16)
                .map(|id| ValueDef::new(ValueId(id), Type::F32))
                .collect(),
            OperationKind::Matrix(
                MatrixOperation::multiply_accumulate(
                    [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                    [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                    [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
                )
                .with_declared_tensor_layout(
                    TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                ),
            ),
        ));
        matrix_block.terminator = Some(Terminator::Return { values: vec![] });
        let matrix = Function::internal_helper(
            "matrix",
            Signature::new(matrix_parameters.clone(), vec![]),
            (0..matrix_parameters.len() as u32).map(ValueId).collect(),
            vec![matrix_block],
        );

        let workgroup_u8 = Type::pointer(
            Type::Scalar(ScalarType::U8),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        );
        let mut target_block = BasicBlock::new(BlockId(0));
        target_block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(0), workgroup_u8),
                OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1::full(
                    Gfx950LdsTransposeOperationKindV1::Current {
                        format: Gfx950LdsTransposeFormatV1::Fp8E4M3,
                    },
                )),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(1), u32_ty.clone()),
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::LaneId,
                    WaveWidth::Wave64,
                )),
            ),
        ];
        target_block.terminator = Some(Terminator::Return { values: vec![] });
        let target_specific = Function::kernel_entry(
            "target_specific",
            Signature::new(vec![], vec![]),
            vec![],
            vec![target_block],
        );

        let mut assembly_block = BasicBlock::new(BlockId(0));
        assembly_block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(2), u32_ty.clone()),
            OperationKind::InlineAssembly(InlineAssembly {
                target: InlineAssemblyTarget::AmdGpuGfx942,
                source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                mnemonic: "v_add_u32".to_owned(),
                operands: vec![
                    AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                    AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
                    AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
                ],
                options: [
                    AssemblyOption::NoMemory,
                    AssemblyOption::Pure,
                    AssemblyOption::NoStack,
                ]
                .into_iter()
                .collect(),
                declared_effects: Default::default(),
            }),
        ));
        assembly_block.terminator = Some(Terminator::Return { values: vec![] });
        let assembly = Function::internal_helper(
            "inline_assembly",
            Signature::new(vec![u32_ty.clone(), u32_ty], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![assembly_block],
        );

        let mut switch_entry = BasicBlock::new(BlockId(0));
        switch_entry.terminator = Some(Terminator::Switch {
            selector: ValueId(0),
            cases: vec![SwitchCase {
                value: 7,
                target: BlockId(1),
                arguments: vec![],
            }],
            default_target: BlockId(2),
            default_arguments: vec![],
        });
        let mut switch_case = BasicBlock::new(BlockId(1));
        switch_case.terminator = Some(Terminator::Return { values: vec![] });
        let mut switch_default = BasicBlock::new(BlockId(2));
        switch_default.terminator = Some(Terminator::Return { values: vec![] });
        let switch = Function::internal_helper(
            "switch",
            Signature::new(vec![Type::INDEX], vec![]),
            vec![ValueId(0)],
            vec![switch_entry, switch_case, switch_default],
        );

        let mut integer_entry = BasicBlock::new(BlockId(0));
        integer_entry.terminator = Some(Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: vec![IntegerSwitchCase {
                value: fe2o3_kernel_ir::Constant::I32(-7),
                target: BlockId(1),
                arguments: vec![],
            }],
            default_target: BlockId(2),
            default_arguments: vec![],
        });
        let mut integer_case = BasicBlock::new(BlockId(1));
        integer_case.terminator = Some(Terminator::Return { values: vec![] });
        let mut integer_default = BasicBlock::new(BlockId(2));
        integer_default.terminator = Some(Terminator::Return { values: vec![] });
        let integer_switch = Function::internal_helper(
            "integer_switch",
            Signature::new(vec![Type::Scalar(ScalarType::I32)], vec![]),
            vec![ValueId(0)],
            vec![integer_entry, integer_case, integer_default],
        );

        let mut unreachable_block = BasicBlock::new(BlockId(0));
        unreachable_block.terminator = Some(Terminator::Unreachable);
        let unreachable = Function::internal_helper(
            "unreachable",
            Signature::new(vec![], vec![]),
            vec![],
            vec![unreachable_block],
        );

        let mut module = Module::new("tests::w4_all_safety_families");
        module.functions = vec![
            intrinsic,
            memory_intrinsic,
            memory,
            matrix,
            target_specific,
            assembly,
            switch,
            integer_switch,
            unreachable,
        ];
        for function in &mut module.functions {
            function
                .required_capabilities
                .extend(function.derived_capabilities());
        }
        let mut kernel = Kernel::new(
            "all_families_kernel",
            "all_families_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        let mut target_kernel = Kernel::new(
            "target_specific_kernel",
            "target_specific",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        target_kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(target_kernel);
        module
    }

    fn w4_target_decisions() -> Vec<fe2o3_target_spec::TargetCapabilityDecisionV1> {
        let requirements = BTreeSet::from([
            TargetCapabilityRequirementV1::SubgroupSize(64),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(64),
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x: 64, y: 1, z: 1 },
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(32_768),
            ),
        ]);
        requirements
            .into_iter()
            .map(|requirement| query_target_capability_v1(&W4TestTarget, requirement).unwrap())
            .collect()
    }

    fn w4_all_safety_family_target_decisions() -> Vec<fe2o3_target_spec::TargetCapabilityDecisionV1>
    {
        BTreeSet::from([
            TargetCapabilityRequirementV1::Atomic(TargetAtomicRequirementV1::new(
                TargetScalarTypeV1::new(TargetScalarKindV1::UnsignedInteger, 32),
                TargetAtomicOperationV1::Add,
                TargetMemoryOrderingV1::AcquireRelease,
                TargetMemoryScopeV1::Device,
                TargetAddressSpaceV1::Global,
            )),
            TargetCapabilityRequirementV1::SubgroupSize(64),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(64),
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x: 64, y: 1, z: 1 },
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(256),
            ),
        ])
        .into_iter()
        .map(|requirement| query_target_capability_v1(&W4TestTarget, requirement).unwrap())
        .collect()
    }

    fn w4_diagnostic_source_map(canonical: &VerifiedCanonicalKernelIrV13) -> Vec<u8> {
        DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                [0x31; 32],
                *canonical.identity().digest(),
                canonical.identity().canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new([0x32; 32], 1, "all_families.rs".to_owned()).unwrap()],
            vec![],
            vec![],
            vec![],
            vec![],
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap()
    }

    #[test]
    fn w4_worker_handoff_carries_all_live_safety_families_exactly() {
        use dialect_gpu::CanonicalKirSafetyFamilyV1;
        use fe2o3_kernel_analysis::{
            PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1, ProductionCapabilityAnalysisKindV1,
            ProductionW4FinalGraphExecutionV1, ProductionW4FinalGraphSubjectV1,
            ProductionW4KernelRootV1, derive_live_target_closure_identity_v1,
            production_w4_required_stage_mask_v1,
        };

        const FINAL_EPOCH: u64 = 41;
        let module = w4_all_safety_families_module();
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let target_identity = *capability_target_model_identity_v5(W4_TEST_TARGET_MODEL)
            .unwrap()
            .digest()
            .as_bytes();
        let closure = b"w4-all-live-safety-families-v1".to_vec();
        let closure_identity = derive_live_target_closure_identity_v1(&closure);
        let decisions = w4_all_safety_family_target_decisions();
        let target_contract = fe2o3_pliron::ProductionFinalGraphTargetContractV1::try_new(
            &canonical,
            FINAL_EPOCH,
            *canonical.identity(),
            FINAL_EPOCH - 1,
            closure_identity,
            W4_TEST_TARGET_MODEL,
            decisions.iter().copied(),
        )
        .unwrap();
        let roots = [
            ProductionW4KernelRootV1::try_new(
                "all_families_kernel".into(),
                "all_families_entry".into(),
                [0x34; 32],
            )
            .unwrap(),
            ProductionW4KernelRootV1::try_new(
                "target_specific_kernel".into(),
                "target_specific".into(),
                [0x37; 32],
            )
            .unwrap(),
        ];
        let subject = ProductionW4FinalGraphSubjectV1::try_new(
            *canonical.identity(),
            FINAL_EPOCH - 1,
            *canonical.identity(),
            FINAL_EPOCH,
            [0x35; 32],
            [0x38; 32],
            target_identity,
            [0x36; 32],
            closure_identity,
            roots,
        )
        .unwrap()
        .with_diagnostic_source_map_v1(w4_diagnostic_source_map(&canonical))
        .unwrap();
        let atomic_target =
            PlironAtomicTargetContextV1::new([PlironAtomicTargetCapabilityV1::new(
                32,
                MemorySpaceAttr::Global,
                AtomicScopeAttr::Device,
            )
            .unwrap()])
            .unwrap();
        let launch_contract =
            launch_contract_from_exact_decisions_v5(&decisions, &target_identity, &module, 64, 64)
                .unwrap();
        let target = retain_production_w4_target_resource_input_v5(
            *canonical.identity(),
            FINAL_EPOCH,
            target_identity,
            [0x36; 32],
            closure_identity,
            closure,
            decisions,
            atomic_target,
            launch_contract,
        )
        .unwrap();
        assert!(!target.grants_target_or_launch_authority());
        assert_eq!(target.final_graph(), canonical.identity());
        assert_eq!(target.final_epoch(), FINAL_EPOCH);
        assert_eq!(subject.final_graph(), canonical.identity());
        assert_eq!(subject.final_epoch(), FINAL_EPOCH);
        assert!(subject.diagnostic_source_map().is_some());
        let expected_masks = [
            0xc007, 0xdda7, 0xe927, 0xd9a7, 0xd9a7, 0xd28f, 0xd0e7, 0xd087, 0xfa8f, 0xfda7, 0xffbf,
            0xffbf, 0xc00f, 0xffff, 0xfe0b, 0xfe0b, 0xfe0b,
        ];
        assert_eq!(
            CanonicalKirSafetyFamilyV1::ALL.map(production_w4_required_stage_mask_v1),
            expected_masks
        );
        assert_eq!(
            PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
                .iter()
                .map(|stage| stage.kind())
                .collect::<Vec<_>>(),
            [
                ProductionCapabilityAnalysisKindV1::CanonicalTyping,
                ProductionCapabilityAnalysisKindV1::CapabilityProvenance,
                ProductionCapabilityAnalysisKindV1::ResourceLegality,
                ProductionCapabilityAnalysisKindV1::Uniformity,
                ProductionCapabilityAnalysisKindV1::TensorLayout,
                ProductionCapabilityAnalysisKindV1::MemoryBounds,
                ProductionCapabilityAnalysisKindV1::AtomicLegality,
                ProductionCapabilityAnalysisKindV1::RaceFreedom,
                ProductionCapabilityAnalysisKindV1::HierarchicalOwnership,
                ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                ProductionCapabilityAnalysisKindV1::PipelineProtocol,
                ProductionCapabilityAnalysisKindV1::Initialization,
                ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                ProductionCapabilityAnalysisKindV1::EffectRefinement,
                ProductionCapabilityAnalysisKindV1::SemanticRefinement,
            ]
        );

        let mut owner = fe2o3_pliron::ProductionFinalGraphOwnerV1::try_new(
            VerifiedCanonicalKernelIrV13::from_canonical_bytes(
                canonical.canonical_bytes().to_vec(),
            )
            .unwrap(),
            FINAL_EPOCH,
            target_contract,
        )
        .unwrap()
        .verify()
        .expect("all-family final graph reaches the verified owner");
        let execution = owner
            .execute_w4_capability_witness(subject.clone(), target)
            .expect("all-family verified owner executes W4");
        let witness = match execution {
            ProductionW4FinalGraphExecutionV1::Complete(witness) => witness,
            ProductionW4FinalGraphExecutionV1::NonClean(result) => panic!(
                "all-family ordinary production owner returned {:?}: {result}",
                result.outcome()
            ),
        };
        assert_eq!(witness.subject(), &subject);
        assert_eq!(witness.subject().final_epoch(), FINAL_EPOCH);
        assert!(witness.subject().diagnostic_source_map().is_some());
        assert!(!witness.grants_compiler_refinement_authority());
        assert!(!witness.grants_lowering_artifact_or_launch_authority());
        let inventory = witness.typed_safety_inventory();
        assert_eq!(inventory.operation_count(), 17);
        assert_eq!(inventory.required_stage_mask(), u16::MAX);
        assert!(!inventory.grants_any_authority());
        for family in CanonicalKirSafetyFamilyV1::ALL {
            assert_eq!(inventory.family_count(family), 1, "{family:?}");
        }
        assert_eq!(
            witness
                .stages()
                .iter()
                .map(|stage| stage.kind())
                .collect::<Vec<_>>(),
            PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
                .iter()
                .map(|stage| stage.kind())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn w4_launch_contract_uses_exact_decisions_and_runtime_host_preconditions() {
        let module = w4_global_module();
        let decisions = w4_target_decisions();
        let target_identity = capability_target_model_identity_v5(W4_TEST_TARGET_MODEL).unwrap();
        let contract = launch_contract_from_exact_decisions_v5(
            &decisions,
            target_identity.digest().as_bytes(),
            &module,
            64,
            64,
        )
        .unwrap();
        let expected_identity =
            derive_target_decision_set_identity_v1(&decisions, target_identity.digest().as_bytes())
                .unwrap()
                .0;

        assert_eq!(
            contract.target_decision_set_identity(),
            Some(&expected_identity)
        );
        assert_eq!(contract.limits().max_workgroup_extents(), [64, 1, 1]);
        assert_eq!(contract.limits().max_workgroup_invocations(), 64);
        assert_eq!(contract.limits().max_workgroup_memory_bytes(), 32_768);
        assert_eq!(contract.limits().required_global_alignment(), 16);
        assert_eq!(contract.limits().max_global_allocations(), 2);
        assert_eq!(
            contract
                .host_allocations()
                .map(PlironHostAllocationV1::origin)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        assert!(
            contract
                .host_allocations()
                .all(PlironHostAllocationV1::requires_runtime_validation)
        );
    }

    #[test]
    fn w4_launch_contract_rejects_decisions_without_exact_workgroup_limits() {
        let module = w4_global_module();
        let decisions = [query_target_capability_v1(
            &W4TestTarget,
            TargetCapabilityRequirementV1::SubgroupSize(64),
        )
        .unwrap()];
        let target_identity = capability_target_model_identity_v5(W4_TEST_TARGET_MODEL).unwrap();
        assert!(matches!(
            launch_contract_from_exact_decisions_v5(
                &decisions,
                target_identity.digest().as_bytes(),
                &module,
                64,
                64,
            ),
            Err(ProductionWorkerHandoffError::CapabilitySubjectMismatch(
                "target decisions omit exact workgroup limits"
            ))
        ));
    }

    #[test]
    fn reachable_gfx942_exp_derives_compiler_owned_envelope() {
        let module = math_module(&[F32MathFunction::Exp], &[F32MathFunction::Exp]);
        let compiler_module = gfx942_compiler_module(&module);
        let identity = [0x37; 32];
        let envelope = derive_production_compiler_ffi_envelope(
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            &module,
            &compiler_module,
            None,
            identity,
        )
        .unwrap();
        assert_eq!(
            inspect_production_gfx942_compiler_ffi_envelope_v1(&envelope),
            Some(ProductionGfx942CompilerFfiEnvelopeKindV1::OcmlExpF32 {
                canonical_kernel_ir_identity: identity,
            })
        );
    }

    #[test]
    fn gfx942_exp_rejects_source_ffi_unreachable_and_second_import() {
        let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
        let reachable = math_module(&[F32MathFunction::Exp], &[F32MathFunction::Exp]);
        let reachable_compiler = gfx942_compiler_module(&reachable);
        let source = construct_production_gfx942_ocml_exp_envelope_v1([0x38; 32]).unwrap();
        assert!(matches!(
            derive_production_compiler_ffi_envelope(
                target,
                &reachable,
                &reachable_compiler,
                Some(source),
                [0x37; 32],
            ),
            Err(ProductionWorkerHandoffError::Gfx942SourceFfiNotAdmitted)
        ));

        let unreachable = math_module(&[F32MathFunction::Exp], &[]);
        let unreachable_compiler = gfx942_compiler_module(&unreachable);
        assert!(matches!(
            derive_production_compiler_ffi_envelope(
                target,
                &unreachable,
                &unreachable_compiler,
                None,
                [0x37; 32],
            ),
            Err(ProductionWorkerHandoffError::Gfx942OcmlExpNotExecutable)
        ));

        let second = math_module(
            &[F32MathFunction::Exp, F32MathFunction::Sin],
            &[F32MathFunction::Exp, F32MathFunction::Sin],
        );
        let second_compiler = gfx942_compiler_module(&second);
        assert!(matches!(
            derive_production_compiler_ffi_envelope(
                target,
                &second,
                &second_compiler,
                None,
                [0x37; 32],
            ),
            Err(ProductionWorkerHandoffError::Gfx942OcmlImportPolicy)
        ));
    }

    #[test]
    fn production_source_has_no_qualification_or_worker_v2_dependency() {
        for source in [
            include_str!("production_worker_handoff.rs"),
            include_str!("compiler_module_contract.rs"),
        ] {
            for forbidden in [
                concat!("worker_v2", "_producer"),
                concat!("collected_scalar", "_gemm"),
                concat!("collected_tiled", "_gemm"),
                concat!("collected_flash", "_attention"),
                concat!("collected_", "moe"),
                concat!("collected_row", "_softmax"),
            ] {
                assert!(
                    !source.contains(forbidden),
                    "production handoff depends on obsolete variant {forbidden:?}"
                );
            }
        }
    }
}
