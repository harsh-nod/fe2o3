//! Exact, authority-free W4 outcome for one optimized production graph.
//!
//! Its caller must lend the exact canonical KIR owner and the corresponding
//! live functions from the existing production PLIRON owner. Execution
//! capabilities are checked against both owners without constructing a new
//! graph. W6 must keep those owners alive and call the exact handoff validator
//! before consuming this inert result.

use std::{collections::BTreeMap, error::Error, fmt, mem::size_of, sync::Arc};

use dialect_gpu::{
    CanonicalKirSafetyContractV1, CanonicalKirSafetyFamilyV1,
    ExecutionCapabilityOp as PlironExecutionCapabilityOp, ExecutionLayoutOp,
    canonical_kir_safety_contract_v1,
};
use dialect_kernel::{
    AtomicScopeAttr, MemorySpaceAttr, OwnershipCoverageAttr, SourceCoordinateAttr,
};
use fe2o3_kernel_ir::{
    DebugSourceMapDocumentV2, DebugSourceMapErrorV2, FunctionId, KernelId, LaunchExtent, Module,
    OperationKind, Terminator, VerifiedCanonicalKernelIrErrorV13,
    VerifiedCanonicalKernelIrIdentityV13, VerifiedCanonicalKernelIrV13, decode_module_v13,
    encode_execution_capability_contract_v1,
};
use fe2o3_pliron_owner_core::{ContextIdentity, require_context_identity};
use fe2o3_target_spec::{
    TargetCapabilityDecisionOutcomeV1, TargetCapabilityDecisionV1, TargetCapabilityModelIdentityV1,
    TargetCapabilityRequirementV1,
};
use pliron::{builtin::ops::FuncOp, context::Context, operation::Operation};
use sha2::{Digest, Sha256};

use crate::pliron_function_inventory::BoundedPlironFunctionInventoryV1;
use crate::{
    ExecutionCapabilityFinalGraphErrorV1, ExecutionCapabilityFinalGraphReportV1,
    HierarchicalOwnershipReportV1, HierarchicalRegionIdentityV1,
    KernelCapabilityPreservationAnalysisV1, KernelCheckPassKindV1, KernelCheckStatusV1,
    MAX_EXECUTION_SEMANTIC_OPERATIONS_V1, PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
    PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2, PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1,
    PlironAtomicTargetContextV1, PlironBarrierReportV1, PlironCanonicalExecutionLayoutV1,
    PlironEffectRefinementReportV1, PlironIrIdentityErrorV1, PlironIrStructuralIdentityV1,
    PlironLaunchContractV1, PlironPassPreservationReportV1, PlironPipelineProtocolReportV1,
    PlironProgressReportV1, PlironSemanticRefinementReportV1, PlironTensorLayoutReportV1,
    PlironWorkgroupMemoryFindingV1, PlironWorkgroupMemoryReportV1,
    ProductionAnalysisConfigurationV1, ProductionAnalysisWitnessCheckerV1,
    ProductionAnalysisWitnessCoverageV1, ProductionAnalysisWitnessGapV1,
    ProductionCapabilityAnalysisExecutorV1, ProductionCapabilityAnalysisKindV1,
    ProductionCapabilityAnalysisStageV1, ProductionPlironPreloweringErrorV2,
    ProductionPlironPreloweringReportV2, ProductionW4AnalysisObligationKindV1,
    ProductionW4AnalysisObligationStageV1, ProductionW4AnalysisScheduleErrorV1,
    RankedBoundsReportV1, RankedRaceReportV1,
    analyze_execution_capability_final_graph_with_composed_conflicts_v1,
    analyze_kernel_capability_preservation_v1, derive_pliron_ir_structural_identity_v1,
    require_production_pliron_checks_with_atomic_and_target_before_lowering_v2,
    require_production_pliron_checks_with_atomic_target_and_canonical_layout_before_lowering_v2,
    require_production_pliron_checks_with_atomic_target_before_lowering_v2,
    validate_production_w4_analysis_obligation_schedule_v1,
};

pub const PRODUCTION_W4_FINAL_GRAPH_CAPABILITY_WITNESS_VERSION_V1: u16 = 1;
pub const MAX_PRODUCTION_W4_KERNEL_ROOTS_V1: usize = 1_024;
pub const MAX_PRODUCTION_W4_FUNCTIONS_V1: usize = 1_024;
pub const MAX_PRODUCTION_W4_IDENTITY_NAME_BYTES_V1: usize = 1_024;
pub const MAX_PRODUCTION_W4_TARGET_CLOSURE_BYTES_V1: usize = 1024 * 1024;
pub const MAX_PRODUCTION_W4_TARGET_DECISIONS_V1: usize = 16_384;
pub const MAX_PRODUCTION_W4_TARGET_DECISION_ENCODING_BYTES_V1: usize = 4_096;
pub const PRODUCTION_W4_TYPED_SAFETY_FAMILY_COUNT_V1: usize = 17;
pub const MAX_PRODUCTION_W4_DIAGNOSTIC_BYTES_V1: usize = 4_096;
pub const MAX_PRODUCTION_W4_REPORT_ENCODING_BYTES_V1: usize = 4 * 1024 * 1024;
pub const MAX_PRODUCTION_W4_WITNESS_ENCODING_BYTES_V1: usize = 32 * 1024 * 1024;
pub const PRODUCTION_W4_REJECTED_DIAGNOSTIC_V1: &str = "FE2O3-W4-REJECTED-001";
pub const PRODUCTION_W4_INCOMPLETE_DIAGNOSTIC_V1: &str = "FE2O3-W4-INCOMPLETE-001";
pub const PRODUCTION_W4_UNSUPPORTED_DIAGNOSTIC_V1: &str = "FE2O3-W4-UNSUPPORTED-001";

const WITNESS_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-W4-FINAL-GRAPH-CAPABILITY-WITNESS/V1\0";
const CHECKER_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-W4-CHECKER-SET/V1\0";
const WITNESS_CHECKSUM_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-W4-WITNESS-CHECKSUM/V1\0";
const KIR_PLIRON_BRIDGE_V13_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/CANONICAL-KIR-V13/V1\0";
const KIR_PLIRON_EXECUTION_OPERATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/KIR-PLIRON-BRIDGE/EXECUTION-CAPABILITY-OPERATION/V1\0";
const LIVE_TARGET_CLOSURE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-TARGET-CAPABILITY-CLOSURE/V1\0";
const CAPABILITY_TARGET_MODEL_DOMAIN_V5: &[u8] = b"FE2O3/PRODUCTION-CAPABILITY-TARGET-MODEL/V5\0";
const TARGET_DECISION_SET_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-W4-TARGET-DECISION-SET/V1\0";
const DIAGNOSTIC_SOURCE_MAP_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-W4-DIAGNOSTIC-SOURCE-MAP/V1\0";
const ANALYSIS_SCHEDULE_WITNESS_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-W4/ANALYSIS-SCHEDULE-WITNESS/V1\0";
const ANALYSIS_OBLIGATION_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-W4/ANALYSIS-OBLIGATION-EVIDENCE/V1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionW4DiagnosticSourceMapInnerV1 {
    identity: [u8; 32],
    canonical_length: u64,
    document: DebugSourceMapDocumentV2,
}

/// Canonical compiler source-map custody bound to the exact final V13 graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4DiagnosticSourceMapV1 {
    inner: Arc<ProductionW4DiagnosticSourceMapInnerV1>,
}

impl ProductionW4DiagnosticSourceMapV1 {
    pub fn from_canonical_v2_bytes(
        final_graph: VerifiedCanonicalKernelIrIdentityV13,
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, ProductionW4InputErrorV1> {
        let canonical_length = u64::try_from(canonical_bytes.len())
            .map_err(|_| ProductionW4InputErrorV1::DiagnosticSourceMapTooLarge)?;
        let document = DebugSourceMapDocumentV2::from_canonical_json_bytes(&canonical_bytes)
            .map_err(ProductionW4InputErrorV1::DiagnosticSourceMap)?;
        let binding = document.binding().canonical_kir();
        if binding.digest() != *final_graph.digest()
            || binding.canonical_bytes() != final_graph.canonical_length()
        {
            return Err(ProductionW4InputErrorV1::DiagnosticSourceMapGraphMismatch);
        }
        let mut digest = Sha256::new();
        digest.update(DIAGNOSTIC_SOURCE_MAP_IDENTITY_DOMAIN_V1);
        digest.update(canonical_length.to_le_bytes());
        digest.update(canonical_bytes);
        Ok(Self {
            inner: Arc::new(ProductionW4DiagnosticSourceMapInnerV1 {
                identity: digest.finalize().into(),
                canonical_length,
                document,
            }),
        })
    }

    pub fn identity(&self) -> &[u8; 32] {
        &self.inner.identity
    }

    pub fn canonical_length(&self) -> u64 {
        self.inner.canonical_length
    }

    pub fn document(&self) -> &DebugSourceMapDocumentV2 {
        &self.inner.document
    }
}

/// Exact kernel entry and launch subject expected in canonical KIR order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4KernelRootV1 {
    kernel: KernelId,
    entry: FunctionId,
    launch_identity: [u8; 32],
}

impl ProductionW4KernelRootV1 {
    pub fn try_new(
        kernel: KernelId,
        entry: FunctionId,
        launch_identity: [u8; 32],
    ) -> Result<Self, ProductionW4InputErrorV1> {
        require_name("kernel", kernel.as_str(), kernel.retained_capacity_bytes())?;
        require_name("entry", entry.as_str(), entry.retained_capacity_bytes())?;
        require_nonzero_identity("kernel launch", &launch_identity)?;
        Ok(Self {
            kernel,
            entry,
            launch_identity,
        })
    }

    pub const fn kernel(&self) -> &KernelId {
        &self.kernel
    }

    pub const fn entry(&self) -> &FunctionId {
        &self.entry
    }

    pub const fn launch_identity(&self) -> &[u8; 32] {
        &self.launch_identity
    }
}

/// Complete immutable subject binding supplied by the production owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4FinalGraphSubjectV1 {
    pre_optimization_graph: VerifiedCanonicalKernelIrIdentityV13,
    pre_optimization_epoch: u64,
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    policy_identity: [u8; 32],
    functional_refinement_identity: [u8; 32],
    checker_identity: [u8; 32],
    target_identity: [u8; 32],
    launch_identity: [u8; 32],
    target_closure_identity: [u8; 32],
    kernel_roots: Box<[ProductionW4KernelRootV1]>,
    diagnostic_source_map: Option<ProductionW4DiagnosticSourceMapV1>,
}

impl ProductionW4FinalGraphSubjectV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        pre_optimization_graph: VerifiedCanonicalKernelIrIdentityV13,
        pre_optimization_epoch: u64,
        final_graph: VerifiedCanonicalKernelIrIdentityV13,
        final_epoch: u64,
        policy_identity: [u8; 32],
        functional_refinement_identity: [u8; 32],
        target_identity: [u8; 32],
        launch_identity: [u8; 32],
        target_closure_identity: [u8; 32],
        kernel_roots: impl IntoIterator<Item = ProductionW4KernelRootV1>,
    ) -> Result<Self, ProductionW4InputErrorV1> {
        if final_epoch < pre_optimization_epoch {
            return Err(ProductionW4InputErrorV1::EpochRegression {
                pre_optimization_epoch,
                final_epoch,
            });
        }
        if final_graph != pre_optimization_graph && final_epoch == pre_optimization_epoch {
            return Err(ProductionW4InputErrorV1::ChangedGraphWithoutEpochAdvance {
                epoch: final_epoch,
            });
        }
        require_nonzero_identity("policy", &policy_identity)?;
        require_nonzero_identity("functional refinement", &functional_refinement_identity)?;
        require_nonzero_identity("target", &target_identity)?;
        require_nonzero_identity("launch", &launch_identity)?;
        require_nonzero_identity("target closure", &target_closure_identity)?;
        let roots = collect_bounded(
            kernel_roots,
            MAX_PRODUCTION_W4_KERNEL_ROOTS_V1,
            "kernel roots",
        )?;
        if roots.is_empty() {
            return Err(ProductionW4InputErrorV1::EmptyKernelRoots);
        }
        Ok(Self {
            pre_optimization_graph,
            pre_optimization_epoch,
            final_graph,
            final_epoch,
            policy_identity,
            functional_refinement_identity,
            checker_identity: production_w4_checker_identity_v1(),
            target_identity,
            launch_identity,
            target_closure_identity,
            kernel_roots: roots.into_boxed_slice(),
            diagnostic_source_map: None,
        })
    }

    pub fn with_diagnostic_source_map_v1(
        mut self,
        canonical_v2_bytes: Vec<u8>,
    ) -> Result<Self, ProductionW4InputErrorV1> {
        self.diagnostic_source_map =
            Some(ProductionW4DiagnosticSourceMapV1::from_canonical_v2_bytes(
                self.final_graph,
                canonical_v2_bytes,
            )?);
        Ok(self)
    }

    pub const fn pre_optimization_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.pre_optimization_graph
    }
    pub const fn pre_optimization_epoch(&self) -> u64 {
        self.pre_optimization_epoch
    }
    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }
    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }
    pub const fn policy_identity(&self) -> &[u8; 32] {
        &self.policy_identity
    }
    pub const fn functional_refinement_identity(&self) -> &[u8; 32] {
        &self.functional_refinement_identity
    }
    pub const fn checker_identity(&self) -> &[u8; 32] {
        &self.checker_identity
    }
    pub const fn target_identity(&self) -> &[u8; 32] {
        &self.target_identity
    }
    pub const fn launch_identity(&self) -> &[u8; 32] {
        &self.launch_identity
    }
    pub const fn target_closure_identity(&self) -> &[u8; 32] {
        &self.target_closure_identity
    }
    pub fn kernel_roots(&self) -> &[ProductionW4KernelRootV1] {
        &self.kernel_roots
    }
    pub fn diagnostic_source_map(&self) -> Option<&ProductionW4DiagnosticSourceMapV1> {
        self.diagnostic_source_map.as_ref()
    }
}

/// Exact W5 input used by the target-sensitive analyses.
///
/// The opaque canonical closure bytes must be compared with the sealed W5
/// owner by W6. Their presence here is custody, not validation authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4TargetResourceInputV1 {
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    target_identity: [u8; 32],
    launch_identity: [u8; 32],
    closure_identity: [u8; 32],
    canonical_closure: Box<[u8]>,
    target_model_identity: TargetCapabilityModelIdentityV1,
    decision_set_identity: [u8; 32],
    target_decisions: Box<[TargetCapabilityDecisionV1]>,
    atomic_target: PlironAtomicTargetContextV1,
    launch_contract: PlironLaunchContractV1,
}

impl ProductionW4TargetResourceInputV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        final_graph: VerifiedCanonicalKernelIrIdentityV13,
        final_epoch: u64,
        target_identity: [u8; 32],
        launch_identity: [u8; 32],
        closure_identity: [u8; 32],
        canonical_closure: Vec<u8>,
        target_decisions: Vec<TargetCapabilityDecisionV1>,
        atomic_target: PlironAtomicTargetContextV1,
        launch_contract: PlironLaunchContractV1,
    ) -> Result<Self, ProductionW4InputErrorV1> {
        require_nonzero_identity("target", &target_identity)?;
        require_nonzero_identity("launch", &launch_identity)?;
        require_nonzero_identity("target closure", &closure_identity)?;
        if canonical_closure.is_empty() {
            return Err(ProductionW4InputErrorV1::EmptyTargetClosure);
        }
        if canonical_closure.len() > MAX_PRODUCTION_W4_TARGET_CLOSURE_BYTES_V1 {
            return Err(ProductionW4InputErrorV1::ResourceLimitExceeded {
                resource: "target closure bytes",
                limit: MAX_PRODUCTION_W4_TARGET_CLOSURE_BYTES_V1,
            });
        }
        if derive_live_target_closure_identity_v1(&canonical_closure) != closure_identity {
            return Err(ProductionW4InputErrorV1::TargetClosureIdentityMismatch);
        }
        let (decision_set_identity, _) =
            derive_target_decision_set_identity_v1(&target_decisions, &target_identity)?;
        let target_model_identity = target_decisions[0].model();
        match launch_contract.target_decision_set_identity() {
            Some(bound) if bound == &decision_set_identity => {}
            Some(_) => {
                return Err(ProductionW4InputErrorV1::LaunchContractTargetDecisionsMismatch);
            }
            None => return Err(ProductionW4InputErrorV1::LaunchContractTargetDecisionsUnbound),
        }
        Ok(Self {
            final_graph,
            final_epoch,
            target_identity,
            launch_identity,
            closure_identity,
            canonical_closure: canonical_closure.into_boxed_slice(),
            target_model_identity,
            decision_set_identity,
            target_decisions: target_decisions.into_boxed_slice(),
            atomic_target,
            launch_contract,
        })
    }

    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }
    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }
    pub const fn target_identity(&self) -> &[u8; 32] {
        &self.target_identity
    }
    pub const fn launch_identity(&self) -> &[u8; 32] {
        &self.launch_identity
    }
    pub const fn closure_identity(&self) -> &[u8; 32] {
        &self.closure_identity
    }
    pub fn canonical_closure(&self) -> &[u8] {
        &self.canonical_closure
    }
    pub const fn target_model_identity(&self) -> TargetCapabilityModelIdentityV1 {
        self.target_model_identity
    }
    pub const fn decision_set_identity(&self) -> &[u8; 32] {
        &self.decision_set_identity
    }
    pub fn target_decisions(&self) -> &[TargetCapabilityDecisionV1] {
        &self.target_decisions
    }
    pub const fn atomic_target(&self) -> &PlironAtomicTargetContextV1 {
        &self.atomic_target
    }
    pub const fn launch_contract(&self) -> &PlironLaunchContractV1 {
        &self.launch_contract
    }
    pub const fn grants_target_or_launch_authority(&self) -> bool {
        false
    }
}

/// Derives the authority-free identity of exact canonical W5 closure bytes.
pub fn derive_live_target_closure_identity_v1(canonical_bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((LIVE_TARGET_CLOSURE_IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(LIVE_TARGET_CLOSURE_IDENTITY_DOMAIN_V1);
    digest.update((canonical_bytes.len() as u64).to_le_bytes());
    digest.update(canonical_bytes);
    digest.finalize().into()
}

fn target_model_identity_v5(model: TargetCapabilityModelIdentityV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(CAPABILITY_TARGET_MODEL_DOMAIN_V5);
    for word in model.profile_fingerprint().words() {
        digest.update(word.to_le_bytes());
    }
    for word in model.revision_fingerprint().words() {
        digest.update(word.to_le_bytes());
    }
    digest.finalize().into()
}

pub fn derive_target_decision_set_identity_v1(
    decisions: &[TargetCapabilityDecisionV1],
    target_identity: &[u8; 32],
) -> Result<([u8; 32], usize), ProductionW4InputErrorV1> {
    let Some(first) = decisions.first().copied() else {
        return Err(ProductionW4InputErrorV1::EmptyTargetDecisions);
    };
    if decisions.len() > MAX_PRODUCTION_W4_TARGET_DECISIONS_V1 {
        return Err(ProductionW4InputErrorV1::ResourceLimitExceeded {
            resource: "target capability decisions",
            limit: MAX_PRODUCTION_W4_TARGET_DECISIONS_V1,
        });
    }
    first
        .validate()
        .map_err(|_| ProductionW4InputErrorV1::InvalidTargetDecision)?;
    let model = first.model();
    if target_model_identity_v5(model) != *target_identity {
        return Err(ProductionW4InputErrorV1::TargetDecisionModelMismatch);
    }
    let mut digest = Sha256::new();
    digest.update(TARGET_DECISION_SET_IDENTITY_DOMAIN_V1);
    digest.update((decisions.len() as u64).to_le_bytes());
    let mut previous = None;
    for decision in decisions.iter().copied() {
        decision
            .validate()
            .map_err(|_| ProductionW4InputErrorV1::InvalidTargetDecision)?;
        if decision.model() != model {
            return Err(ProductionW4InputErrorV1::TargetDecisionModelMismatch);
        }
        let requirement = decision.requirement();
        if previous.is_some_and(|previous| previous >= requirement) {
            return Err(ProductionW4InputErrorV1::TargetDecisionOrder);
        }
        if !matches!(
            decision.outcome(),
            TargetCapabilityDecisionOutcomeV1::Supported
                | TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(_)
        ) {
            return Err(ProductionW4InputErrorV1::TargetDecisionNotAdmitted);
        }
        let encoded = canonical_target_decision_v1(decision)?;
        digest.update((encoded.len() as u64).to_le_bytes());
        digest.update(encoded.as_bytes());
        previous = Some(requirement);
    }
    Ok((digest.finalize().into(), decisions.len()))
}

fn canonical_target_decision_v1(
    decision: TargetCapabilityDecisionV1,
) -> Result<String, ProductionW4InputErrorV1> {
    let mut writer = BoundedTextWriter::new(MAX_PRODUCTION_W4_TARGET_DECISION_ENCODING_BYTES_V1);
    decision.encode_canonical(&mut writer).map_err(|_| {
        ProductionW4InputErrorV1::ResourceLimitExceeded {
            resource: "target decision canonical bytes",
            limit: MAX_PRODUCTION_W4_TARGET_DECISION_ENCODING_BYTES_V1,
        }
    })?;
    Ok(writer.finish())
}

/// One exact positional KIR-to-PLIRON function binding lent by the graph owner.
#[derive(Clone, Copy)]
pub struct ProductionW4LiveFunctionV1<'a> {
    function: &'a FunctionId,
    pliron: &'a FuncOp,
}

impl<'a> ProductionW4LiveFunctionV1<'a> {
    pub const fn new(function: &'a FunctionId, pliron: &'a FuncOp) -> Self {
        Self { function, pliron }
    }
    pub const fn function(&self) -> &'a FunctionId {
        self.function
    }
    pub const fn pliron(&self) -> &'a FuncOp {
        self.pliron
    }
}

/// Exact typed safety-carrier inventory rederived from the live V13 graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4TypedSafetyInventoryV1 {
    family_counts: [u32; PRODUCTION_W4_TYPED_SAFETY_FAMILY_COUNT_V1],
    required_stage_mask: u16,
    operation_count: u32,
    identity: [u8; 32],
}

impl ProductionW4TypedSafetyInventoryV1 {
    pub const fn family_count(&self, family: CanonicalKirSafetyFamilyV1) -> u32 {
        self.family_counts[family.ordinal()]
    }
    pub const fn operation_count(&self) -> u32 {
        self.operation_count
    }
    pub const fn required_stage_mask(&self) -> u16 {
        self.required_stage_mask
    }
    pub const fn requires_stage(&self, stage: ProductionCapabilityAnalysisKindV1) -> bool {
        self.required_stage_mask & stage_bit(stage) != 0
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn grants_any_authority(&self) -> bool {
        false
    }
}

/// Bounded diagnostic retained for a rejected or unavailable property.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionW4CounterexampleLocationV1 {
    block: usize,
    operation: usize,
}

impl ProductionW4CounterexampleLocationV1 {
    pub const fn block(self) -> usize {
        self.block
    }
    pub const fn operation(self) -> usize {
        self.operation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionW4CounterexampleClassV1 {
    CanonicalExecution,
    ResourceLegality,
    TensorLayout,
    MemoryBounds,
    AtomicLegality,
    RaceConflict,
    OwnershipConflict,
    Uniformity,
    BarrierOrder,
    CollectiveParticipation,
    PipelineEpochProtocol,
    ReadBeforeInitialization,
    MemoryVisibility,
    EffectRefinement,
    SemanticRefinement,
}

/// A rejected property has an executable counterexample class and bounded
/// payload. Incomplete and unsupported results never receive this type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4BoundedCounterexampleV1 {
    obligation: ProductionW4AnalysisObligationKindV1,
    class: ProductionW4CounterexampleClassV1,
    primary: Option<ProductionW4CounterexampleLocationV1>,
    related: Option<ProductionW4CounterexampleLocationV1>,
    detail: String,
    truncated: bool,
}

impl ProductionW4BoundedCounterexampleV1 {
    pub const fn obligation(&self) -> ProductionW4AnalysisObligationKindV1 {
        self.obligation
    }
    pub const fn class(&self) -> ProductionW4CounterexampleClassV1 {
        self.class
    }
    pub const fn primary(&self) -> Option<ProductionW4CounterexampleLocationV1> {
        self.primary
    }
    pub const fn related(&self) -> Option<ProductionW4CounterexampleLocationV1> {
        self.related
    }
    pub fn detail(&self) -> &str {
        &self.detail
    }
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4BoundedDiagnosticV1 {
    stage: ProductionCapabilityAnalysisKindV1,
    function: Option<FunctionId>,
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    source_map_identity: Option<[u8; 32]>,
    counterexample: Option<ProductionW4BoundedCounterexampleV1>,
    detail: String,
    truncated: bool,
}

impl ProductionW4BoundedDiagnosticV1 {
    pub const fn stage(&self) -> ProductionCapabilityAnalysisKindV1 {
        self.stage
    }
    pub const fn function(&self) -> Option<&FunctionId> {
        self.function.as_ref()
    }
    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }
    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }
    pub const fn source_map_identity(&self) -> Option<&[u8; 32]> {
        self.source_map_identity.as_ref()
    }
    pub const fn counterexample(&self) -> Option<&ProductionW4BoundedCounterexampleV1> {
        self.counterexample.as_ref()
    }
    pub fn detail(&self) -> &str {
        &self.detail
    }
    pub const fn truncated(&self) -> bool {
        self.truncated
    }
}

/// Conservative outcome lattice for each W4 concern.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionW4CapabilityOutcomeV1 {
    Clean,
    Rejected(ProductionW4BoundedDiagnosticV1),
    Incomplete(ProductionW4BoundedDiagnosticV1),
    Unsupported(ProductionW4BoundedDiagnosticV1),
}

impl ProductionW4CapabilityOutcomeV1 {
    pub const fn status(&self) -> KernelCheckStatusV1 {
        match self {
            Self::Clean => KernelCheckStatusV1::Clean,
            Self::Rejected(_) => KernelCheckStatusV1::Rejected,
            Self::Incomplete(_) | Self::Unsupported(_) => KernelCheckStatusV1::Incomplete,
        }
    }

    pub const fn diagnostic_code(&self) -> Option<&'static str> {
        match self {
            Self::Clean => None,
            Self::Rejected(_) => Some(PRODUCTION_W4_REJECTED_DIAGNOSTIC_V1),
            Self::Incomplete(_) => Some(PRODUCTION_W4_INCOMPLETE_DIAGNOSTIC_V1),
            Self::Unsupported(_) => Some(PRODUCTION_W4_UNSUPPORTED_DIAGNOSTIC_V1),
        }
    }
}

/// Exact source from which one clean logical stage was derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionW4OutcomeSourceV1 {
    CanonicalKirV13,
    CapabilityProvenanceReplayV1,
    TargetResourceClosureV1,
    PlironPass(KernelCheckPassKindV1),
    EmbeddedPlironPass(KernelCheckPassKindV1),
    CanonicalExecutionCapabilityAndPlironPass(KernelCheckPassKindV1),
    CanonicalExecutionCapabilityAndEmbeddedPlironPass(KernelCheckPassKindV1),
}

/// Independent implementation used to discharge one production obligation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionW4IndependentObligationCheckerV1 {
    CanonicalTypingV13,
    CapabilityProvenanceV1,
    TargetResourceClosureV1,
    UniformityFreshLiveIrV1,
    TensorLayoutFreshLiveIrV1,
    MemoryBoundsFreshLiveIrV1,
    AtomicLegalityFreshLiveIrV1,
    HappensBeforeFreshLiveIrV1,
    RaceFreedomFreshLiveIrV1,
    HierarchicalOwnershipFreshLiveIrV1,
    BarrierConvergenceFreshLiveIrV1,
    BarrierOrderFreshLiveIrV1,
    PipelineProtocolFreshLiveIrV1,
    InitializationFreshLiveIrV1,
    MemoryVisibilityFreshLiveIrV1,
    WorkgroupMemoryEpochsFreshLiveIrV1,
    CollectiveParticipationFreshLiveIrV1,
    EffectRefinementFreshLiveIrV1,
    SemanticRefinementFreshLiveIrV1,
}

/// One independently replayed production-pass checkpoint used by a logical
/// obligation. The structural identity and mutation epoch bind it to the live
/// PLIRON function retained by the full W4 witness.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionW4IndependentPassEvidenceV1 {
    function: FunctionId,
    structural_sha256: [u8; 32],
    structural_bytes: usize,
    pass: KernelCheckPassKindV1,
    checker: ProductionAnalysisWitnessCheckerV1,
    checkpoint_position: usize,
    checkpoint_epoch: u64,
    checked_units: usize,
}

impl ProductionW4IndependentPassEvidenceV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }
    pub const fn structural_sha256(&self) -> &[u8; 32] {
        &self.structural_sha256
    }
    pub const fn structural_bytes(&self) -> usize {
        self.structural_bytes
    }
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        self.pass
    }
    pub const fn checker(&self) -> ProductionAnalysisWitnessCheckerV1 {
        self.checker
    }
    pub const fn checkpoint_position(&self) -> usize {
        self.checkpoint_position
    }
    pub const fn checkpoint_epoch(&self) -> u64 {
        self.checkpoint_epoch
    }
    pub const fn checked_units(&self) -> usize {
        self.checked_units
    }
}

/// Bounded coverage retained from the exact final KIR and independent PLIRON
/// replays. Zero is meaningful for a vacuous obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionW4IndependentObligationCoverageV1 {
    checked_kir_functions: usize,
    checked_kir_operations: usize,
    checked_barrier_sites: usize,
    checked_collective_sites: usize,
    checked_epoch_transitions: usize,
    checked_memory_effects: usize,
    checked_pliron_units: usize,
    declared_effect_contracts: usize,
    proved_effect_contracts: usize,
}

impl ProductionW4IndependentObligationCoverageV1 {
    pub const fn checked_kir_functions(self) -> usize {
        self.checked_kir_functions
    }
    pub const fn checked_kir_operations(self) -> usize {
        self.checked_kir_operations
    }
    pub const fn checked_barrier_sites(self) -> usize {
        self.checked_barrier_sites
    }
    pub const fn checked_collective_sites(self) -> usize {
        self.checked_collective_sites
    }
    pub const fn checked_epoch_transitions(self) -> usize {
        self.checked_epoch_transitions
    }
    pub const fn checked_memory_effects(self) -> usize {
        self.checked_memory_effects
    }
    pub const fn checked_pliron_units(self) -> usize {
        self.checked_pliron_units
    }
    pub const fn declared_effect_contracts(self) -> usize {
        self.declared_effect_contracts
    }
    pub const fn proved_effect_contracts(self) -> usize {
        self.proved_effect_contracts
    }
}

/// Typed, subject-bound evidence produced only after the obligation-specific
/// independent checks succeed. This record grants no authority.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionW4IndependentObligationEvidenceV1 {
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    pliron_epoch: u64,
    checker: ProductionW4IndependentObligationCheckerV1,
    pass_evidence: Box<[ProductionW4IndependentPassEvidenceV1]>,
    coverage: ProductionW4IndependentObligationCoverageV1,
    identity: [u8; 32],
}

impl ProductionW4IndependentObligationEvidenceV1 {
    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }
    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }
    pub const fn pliron_epoch(&self) -> u64 {
        self.pliron_epoch
    }
    pub const fn checker(&self) -> ProductionW4IndependentObligationCheckerV1 {
        self.checker
    }
    pub fn pass_evidence(&self) -> &[ProductionW4IndependentPassEvidenceV1] {
        &self.pass_evidence
    }
    pub const fn coverage(&self) -> ProductionW4IndependentObligationCoverageV1 {
        self.coverage
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn grants_any_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4StageResultV1 {
    kind: ProductionCapabilityAnalysisKindV1,
    source: ProductionW4OutcomeSourceV1,
    outcome: ProductionW4CapabilityOutcomeV1,
}

/// One exact, final-graph-bound discharge from the immutable W4 obligation
/// schedule. Its identity commits to the owner-stage evidence and all prior
/// dependencies; it carries no proof or lowering authority.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionW4AnalysisObligationResultV1 {
    kind: ProductionW4AnalysisObligationKindV1,
    owner: ProductionCapabilityAnalysisKindV1,
    evidence: ProductionW4IndependentObligationEvidenceV1,
}

impl ProductionW4AnalysisObligationResultV1 {
    pub const fn kind(&self) -> ProductionW4AnalysisObligationKindV1 {
        self.kind
    }

    pub const fn owner(&self) -> ProductionCapabilityAnalysisKindV1 {
        self.owner
    }

    pub const fn evidence_identity(&self) -> &[u8; 32] {
        self.evidence.identity()
    }

    pub const fn independent_evidence(&self) -> &ProductionW4IndependentObligationEvidenceV1 {
        &self.evidence
    }
}

/// Concise move-only W4 schedule witness intended for production handoff. The
/// type can be constructed only by executing all analyses against one exact
/// final V13 graph and live PLIRON epoch.
#[must_use = "W4 schedule evidence must be consumed by the production verifier"]
pub struct ProductionW4AnalysisScheduleWitnessV1 {
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    pliron_epoch: u64,
    checker_identity: [u8; 32],
    target_closure_identity: [u8; 32],
    obligations: Box<[ProductionW4AnalysisObligationResultV1]>,
    identity: [u8; 32],
}

impl ProductionW4AnalysisScheduleWitnessV1 {
    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub const fn pliron_epoch(&self) -> u64 {
        self.pliron_epoch
    }

    pub const fn checker_identity(&self) -> &[u8; 32] {
        &self.checker_identity
    }

    pub const fn target_closure_identity(&self) -> &[u8; 32] {
        &self.target_closure_identity
    }

    pub fn obligations(&self) -> &[ProductionW4AnalysisObligationResultV1] {
        &self.obligations
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_lowering_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl ProductionW4StageResultV1 {
    pub const fn kind(&self) -> ProductionCapabilityAnalysisKindV1 {
        self.kind
    }
    pub const fn source(&self) -> ProductionW4OutcomeSourceV1 {
        self.source
    }
    pub const fn outcome(&self) -> &ProductionW4CapabilityOutcomeV1 {
        &self.outcome
    }
}

/// Exact typed outcomes for one live PLIRON function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4FunctionOutcomeV1 {
    function: FunctionId,
    structural_identity: PlironIrStructuralIdentityV1,
    checks: ProductionPlironPreloweringReportV2,
}

impl ProductionW4FunctionOutcomeV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }
    pub const fn structural_identity(&self) -> &PlironIrStructuralIdentityV1 {
        &self.structural_identity
    }
    pub const fn checks(&self) -> &ProductionPlironPreloweringReportV2 {
        &self.checks
    }
    pub const fn tensor_layout(&self) -> &PlironTensorLayoutReportV1 {
        self.checks.tensor_layout()
    }
    pub const fn bounds(&self) -> &RankedBoundsReportV1 {
        self.checks.bounds()
    }
    pub const fn atomic_legality(&self) -> &crate::PlironAtomicLegalityReportV1 {
        self.checks.atomics()
    }
    pub const fn race_freedom(&self) -> &RankedRaceReportV1 {
        self.checks.race()
    }
    pub const fn hierarchical_ownership(&self) -> &crate::HierarchicalOwnershipReportV1 {
        self.checks.ownership()
    }
    pub const fn uniformity_and_barriers(&self) -> &PlironBarrierReportV1 {
        self.checks.barriers()
    }
    pub const fn pipeline_protocol(&self) -> &PlironPipelineProtocolReportV1 {
        self.checks.pipeline_protocol()
    }
    pub const fn initialization_visibility_and_epochs(&self) -> &PlironWorkgroupMemoryReportV1 {
        self.checks.workgroup()
    }
    pub const fn effect_refinement(&self) -> &PlironEffectRefinementReportV1 {
        self.checks.semantics().effect_refinement()
    }
    pub const fn semantic_refinement(&self) -> &PlironSemanticRefinementReportV1 {
        self.checks.semantics()
    }
    pub const fn exact_pass_preservation(&self) -> &PlironPassPreservationReportV1 {
        self.checks.preservation()
    }
}

/// Stable identity over the deterministic witness encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionW4WitnessIdentityV1 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl ProductionW4WitnessIdentityV1 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Bounded canonical witness envelope accepted only after its domain,
/// version, and payload checksum have been verified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionW4CanonicalEncodingV1 {
    bytes: Box<[u8]>,
}

impl ProductionW4CanonicalEncodingV1 {
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionW4EncodingErrorV1> {
        let header_len = WITNESS_DOMAIN_V1
            .len()
            .checked_add(size_of::<u16>())
            .ok_or(ProductionW4EncodingErrorV1::LengthOverflow)?;
        let minimum = header_len
            .checked_add(32)
            .ok_or(ProductionW4EncodingErrorV1::LengthOverflow)?;
        if bytes.len() < minimum || bytes.len() > MAX_PRODUCTION_W4_WITNESS_ENCODING_BYTES_V1 {
            return Err(ProductionW4EncodingErrorV1::MalformedEnvelope);
        }
        if !bytes.starts_with(WITNESS_DOMAIN_V1) {
            return Err(ProductionW4EncodingErrorV1::WrongDomain);
        }
        let version = u16::from_le_bytes(
            bytes[WITNESS_DOMAIN_V1.len()..header_len]
                .try_into()
                .map_err(|_| ProductionW4EncodingErrorV1::MalformedEnvelope)?,
        );
        if version != PRODUCTION_W4_FINAL_GRAPH_CAPABILITY_WITNESS_VERSION_V1 {
            return Err(ProductionW4EncodingErrorV1::UnsupportedVersion { observed: version });
        }
        let payload_len = bytes.len() - 32;
        let mut checksum = Sha256::new();
        checksum.update(WITNESS_CHECKSUM_DOMAIN_V1);
        checksum.update(&bytes[..payload_len]);
        let expected: [u8; 32] = checksum.finalize().into();
        if bytes[payload_len..] != expected {
            return Err(ProductionW4EncodingErrorV1::ChecksumMismatch);
        }
        Ok(Self {
            bytes: bytes.into(),
        })
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn reencode(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
}

/// Complete move-only W4 result. It contains no seal or authority token.
#[must_use = "W4 analysis custody must be consumed by the sealed W6 verifier"]
pub struct ProductionW4FinalGraphCapabilityWitnessV1 {
    subject: ProductionW4FinalGraphSubjectV1,
    canonical_kir: Box<[u8]>,
    capability_provenance: KernelCapabilityPreservationAnalysisV1,
    execution_capability_semantics: ExecutionCapabilityFinalGraphReportV1,
    typed_safety_inventory: ProductionW4TypedSafetyInventoryV1,
    target_resource: ProductionW4TargetResourceInputV1,
    context_identity: ContextIdentity,
    pliron_epoch: u64,
    stages: Box<[ProductionW4StageResultV1]>,
    analysis_schedule: ProductionW4AnalysisScheduleWitnessV1,
    functions: Box<[ProductionW4FunctionOutcomeV1]>,
    canonical_encoding: Box<[u8]>,
    identity: ProductionW4WitnessIdentityV1,
}

impl ProductionW4FinalGraphCapabilityWitnessV1 {
    pub const fn subject(&self) -> &ProductionW4FinalGraphSubjectV1 {
        &self.subject
    }
    pub fn canonical_kir(&self) -> &[u8] {
        &self.canonical_kir
    }
    pub const fn capability_provenance(&self) -> &KernelCapabilityPreservationAnalysisV1 {
        &self.capability_provenance
    }
    pub const fn execution_capability_semantics(&self) -> &ExecutionCapabilityFinalGraphReportV1 {
        &self.execution_capability_semantics
    }
    pub const fn typed_safety_inventory(&self) -> &ProductionW4TypedSafetyInventoryV1 {
        &self.typed_safety_inventory
    }
    pub const fn target_resource(&self) -> &ProductionW4TargetResourceInputV1 {
        &self.target_resource
    }
    pub fn stages(&self) -> &[ProductionW4StageResultV1] {
        &self.stages
    }
    pub const fn analysis_schedule(&self) -> &ProductionW4AnalysisScheduleWitnessV1 {
        &self.analysis_schedule
    }
    pub fn into_analysis_schedule(self) -> ProductionW4AnalysisScheduleWitnessV1 {
        self.analysis_schedule
    }
    pub fn functions(&self) -> &[ProductionW4FunctionOutcomeV1] {
        &self.functions
    }
    pub fn canonical_encoding(&self) -> &[u8] {
        &self.canonical_encoding
    }
    pub const fn identity(&self) -> ProductionW4WitnessIdentityV1 {
        self.identity
    }
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
    pub const fn grants_lowering_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Narrow W6 handoff check against the still-live production owners.
    #[allow(clippy::too_many_arguments)]
    pub fn require_exact_w6_handoff_v1(
        &self,
        canonical: &VerifiedCanonicalKernelIrV13,
        module: &Module,
        subject: &ProductionW4FinalGraphSubjectV1,
        target_resource: &ProductionW4TargetResourceInputV1,
        schedule: &[ProductionCapabilityAnalysisStageV1],
        context: &Context,
        functions: &[ProductionW4LiveFunctionV1<'_>],
    ) -> Result<(), ProductionW4HandoffErrorV1> {
        canonical
            .revalidate()
            .map_err(ProductionW4HandoffErrorV1::Canonical)?;
        if subject != &self.subject
            || target_resource != &self.target_resource
            || canonical.identity() != self.subject.final_graph()
            || canonical.canonical_bytes() != &*self.canonical_kir
            || decode_module_v13(canonical.canonical_bytes()).ok().as_ref() != Some(module)
        {
            return Err(ProductionW4HandoffErrorV1::SubjectSubstituted);
        }
        require_exact_schedule(schedule).map_err(ProductionW4HandoffErrorV1::Execution)?;
        if !analysis_schedule_witness_is_exact(
            &self.analysis_schedule,
            &self.subject,
            &self.target_resource,
            self.pliron_epoch,
            &self.stages,
            &self.execution_capability_semantics,
            &self.functions,
        ) {
            return Err(ProductionW4HandoffErrorV1::ScheduleWitnessSubstituted);
        }
        let context_identity = require_context_identity(context)
            .map_err(|_| ProductionW4HandoffErrorV1::ContextSubstituted)?;
        let epoch = current_pliron_epoch(context)
            .map_err(|_| ProductionW4HandoffErrorV1::ContextSubstituted)?;
        if context_identity != self.context_identity || epoch != self.pliron_epoch {
            return Err(ProductionW4HandoffErrorV1::ContextSubstituted);
        }
        if functions.len() != self.functions.len() {
            return Err(ProductionW4HandoffErrorV1::FunctionSubstituted);
        }
        for (live, retained) in functions.iter().zip(&self.functions) {
            if live.function != retained.function() {
                return Err(ProductionW4HandoffErrorV1::FunctionSubstituted);
            }
            let observed = derive_pliron_ir_structural_identity_v1(context, live.pliron)
                .map_err(ProductionW4HandoffErrorV1::PlironIdentity)?;
            if !retained.structural_identity.exactly_matches(&observed) {
                return Err(ProductionW4HandoffErrorV1::FunctionSubstituted);
            }
        }
        let after = current_pliron_epoch(context)
            .map_err(|_| ProductionW4HandoffErrorV1::ContextSubstituted)?;
        if after != epoch {
            return Err(ProductionW4HandoffErrorV1::ContextSubstituted);
        }
        Ok(())
    }
}

/// A failed analysis retains its typed pipeline error and bounded diagnostic.
#[derive(Debug)]
pub struct ProductionW4NonCleanResultV1 {
    subject: ProductionW4FinalGraphSubjectV1,
    outcome: ProductionW4CapabilityOutcomeV1,
    pipeline_error: Option<Box<ProductionPlironPreloweringErrorV2>>,
    canonical_encoding: Box<[u8]>,
    identity: ProductionW4WitnessIdentityV1,
}

impl ProductionW4NonCleanResultV1 {
    pub const fn subject(&self) -> &ProductionW4FinalGraphSubjectV1 {
        &self.subject
    }
    pub const fn outcome(&self) -> &ProductionW4CapabilityOutcomeV1 {
        &self.outcome
    }
    pub fn pipeline_error(&self) -> Option<&ProductionPlironPreloweringErrorV2> {
        self.pipeline_error.as_deref()
    }
    pub fn canonical_encoding(&self) -> &[u8] {
        &self.canonical_encoding
    }
    pub const fn identity(&self) -> ProductionW4WitnessIdentityV1 {
        self.identity
    }
    pub fn require_exact_subject_v1(
        &self,
        subject: &ProductionW4FinalGraphSubjectV1,
    ) -> Result<(), ProductionW4HandoffErrorV1> {
        if subject != &self.subject {
            return Err(ProductionW4HandoffErrorV1::DiagnosticSubjectSubstituted);
        }
        ProductionW4CanonicalEncodingV1::decode(&self.canonical_encoding)
            .map_err(ProductionW4HandoffErrorV1::DiagnosticEncoding)?;
        let expected = encode_non_clean_witness(&self.subject, &self.outcome)
            .map_err(ProductionW4HandoffErrorV1::Execution)?;
        if expected.as_slice() != self.canonical_encoding.as_ref()
            || witness_identity(&expected) != self.identity
        {
            return Err(ProductionW4HandoffErrorV1::DiagnosticSubstituted);
        }
        Ok(())
    }
    pub const fn grants_any_authority(&self) -> bool {
        false
    }
}

impl fmt::Display for ProductionW4NonCleanResultV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.outcome {
            ProductionW4CapabilityOutcomeV1::Rejected(diagnostic)
            | ProductionW4CapabilityOutcomeV1::Incomplete(diagnostic)
            | ProductionW4CapabilityOutcomeV1::Unsupported(diagnostic) => write!(
                formatter,
                "error[{}]: W4 {:?} for {:?}: {}",
                self.outcome
                    .diagnostic_code()
                    .expect("non-clean outcome has a diagnostic code"),
                diagnostic.stage,
                diagnostic.function,
                diagnostic.detail
            ),
            ProductionW4CapabilityOutcomeV1::Clean => {
                formatter.write_str("invalid clean W4 non-clean result")
            }
        }
    }
}

// Keep the complete witness inline so this move-only result has one owner and no hidden allocation.
#[allow(clippy::large_enum_variant)]
pub enum ProductionW4FinalGraphExecutionV1 {
    Complete(ProductionW4FinalGraphCapabilityWitnessV1),
    NonClean(ProductionW4NonCleanResultV1),
}

impl ProductionW4FinalGraphExecutionV1 {
    pub fn complete(self) -> Option<ProductionW4FinalGraphCapabilityWitnessV1> {
        match self {
            Self::Complete(witness) => Some(witness),
            Self::NonClean(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum ProductionW4ExecutionErrorV1 {
    Canonical(VerifiedCanonicalKernelIrErrorV13),
    CanonicalDecode,
    CanonicalModuleSubstituted,
    FinalGraphSubstituted,
    TargetResourceSubstituted,
    KernelRootsSubstituted,
    FunctionOrderSubstituted,
    EmptyFunctions,
    ScheduleLength {
        expected: usize,
        observed: usize,
    },
    ScheduleStage {
        position: usize,
    },
    ScheduleDependency {
        position: usize,
    },
    MandatoryPassOrder,
    ObligationSchedule(ProductionW4AnalysisScheduleErrorV1),
    ContextIdentityUnavailable,
    MutationEpochUnavailable,
    MutationDuringAnalysis {
        before: u64,
        after: u64,
    },
    CapabilityProvenance(VerifiedCanonicalKernelIrErrorV13),
    ExecutionCapabilitySemantics(ExecutionCapabilityFinalGraphErrorV1),
    MissingStageEvidence {
        stage: ProductionCapabilityAnalysisKindV1,
    },
    InvalidDiagnosticClassification,
    Encoding(ProductionW4EncodingErrorV1),
}

impl fmt::Display for ProductionW4ExecutionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(error) => write!(f, "canonical final KIR V13 is invalid: {error}"),
            Self::CanonicalDecode => f.write_str("canonical final KIR V13 could not be decoded"),
            Self::CanonicalModuleSubstituted => {
                f.write_str("decoded canonical KIR module was substituted")
            }
            Self::FinalGraphSubstituted => {
                f.write_str("final graph identity or epoch was substituted")
            }
            Self::TargetResourceSubstituted => {
                f.write_str("target, launch, or W5 closure input was substituted")
            }
            Self::KernelRootsSubstituted => {
                f.write_str("kernel roots or per-kernel launch subjects were substituted")
            }
            Self::FunctionOrderSubstituted => {
                f.write_str("defined KIR to PLIRON function order was substituted")
            }
            Self::EmptyFunctions => f.write_str("final graph has no defined functions"),
            Self::ScheduleLength { expected, observed } => {
                write!(f, "W4 schedule length is {observed}, expected {expected}")
            }
            Self::ScheduleStage { position } => write!(
                f,
                "W4 schedule stage {position} was omitted, reordered, or substituted"
            ),
            Self::ScheduleDependency { position } => write!(
                f,
                "W4 schedule stage {position} has a missing or forward dependency"
            ),
            Self::MandatoryPassOrder => {
                f.write_str("W4 mandatory PLIRON pass order differs from the production pipeline")
            }
            Self::ObligationSchedule(error) => {
                write!(f, "W4 obligation schedule is invalid: {error:?}")
            }
            Self::ContextIdentityUnavailable => {
                f.write_str("production PLIRON context identity is unavailable")
            }
            Self::MutationEpochUnavailable => {
                f.write_str("production PLIRON mutation epoch is unavailable")
            }
            Self::MutationDuringAnalysis { before, after } => write!(
                f,
                "production PLIRON changed during W4 analysis: epoch {before} became {after}"
            ),
            Self::CapabilityProvenance(error) => {
                write!(f, "capability provenance analysis failed: {error}")
            }
            Self::ExecutionCapabilitySemantics(error) => {
                write!(
                    f,
                    "execution capability final-graph analysis failed: {error}"
                )
            }
            Self::MissingStageEvidence { stage } => {
                write!(f, "W4 stage {stage:?} lacks exact clean evidence")
            }
            Self::InvalidDiagnosticClassification => f.write_str(
                "W4 rejected diagnostics require a bounded counterexample and non-rejected diagnostics forbid one",
            ),
            Self::Encoding(error) => error.fmt(f),
        }
    }
}

impl Error for ProductionW4ExecutionErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionW4InputErrorV1 {
    EmptyIdentity {
        component: &'static str,
    },
    EmptyName {
        component: &'static str,
    },
    NameTooLarge {
        component: &'static str,
        limit: usize,
    },
    EpochRegression {
        pre_optimization_epoch: u64,
        final_epoch: u64,
    },
    ChangedGraphWithoutEpochAdvance {
        epoch: u64,
    },
    EmptyKernelRoots,
    EmptyTargetClosure,
    TargetClosureIdentityMismatch,
    EmptyTargetDecisions,
    InvalidTargetDecision,
    TargetDecisionModelMismatch,
    TargetDecisionOrder,
    TargetDecisionNotAdmitted,
    LaunchContractTargetDecisionsUnbound,
    LaunchContractTargetDecisionsMismatch,
    DiagnosticSourceMap(DebugSourceMapErrorV2),
    DiagnosticSourceMapTooLarge,
    DiagnosticSourceMapGraphMismatch,
    ResourceLimitExceeded {
        resource: &'static str,
        limit: usize,
    },
}

impl fmt::Display for ProductionW4InputErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentity { component } => write!(f, "{component} identity must be nonzero"),
            Self::EmptyName { component } => {
                write!(f, "{component} identity name must not be empty")
            }
            Self::NameTooLarge { component, limit } => write!(
                f,
                "{component} identity name exceeds {limit} retained bytes"
            ),
            Self::EpochRegression {
                pre_optimization_epoch,
                final_epoch,
            } => write!(
                f,
                "final epoch {final_epoch} precedes pre-optimization epoch {pre_optimization_epoch}"
            ),
            Self::ChangedGraphWithoutEpochAdvance { epoch } => write!(
                f,
                "final graph changed without advancing optimization epoch {epoch}"
            ),
            Self::EmptyKernelRoots => f.write_str("W4 subject must bind at least one kernel root"),
            Self::EmptyTargetClosure => f.write_str("W4 target closure encoding must not be empty"),
            Self::TargetClosureIdentityMismatch => {
                f.write_str("W4 target closure bytes do not match the closure identity")
            }
            Self::EmptyTargetDecisions => f.write_str("W4 target decision set must not be empty"),
            Self::InvalidTargetDecision => f.write_str("W4 target decision is not canonical"),
            Self::TargetDecisionModelMismatch => {
                f.write_str("W4 target decisions do not match one exact target model")
            }
            Self::TargetDecisionOrder => {
                f.write_str("W4 target decisions are not strictly ordered and unique")
            }
            Self::TargetDecisionNotAdmitted => {
                f.write_str("W4 target decision is unsupported, incomplete, or unreviewed")
            }
            Self::LaunchContractTargetDecisionsUnbound => f.write_str(
                "W4 launch contract is not bound to the exact target capability decision set",
            ),
            Self::LaunchContractTargetDecisionsMismatch => {
                f.write_str("W4 launch contract names a different target capability decision set")
            }
            Self::DiagnosticSourceMap(error) => {
                write!(f, "W4 diagnostic source map is invalid: {error}")
            }
            Self::DiagnosticSourceMapTooLarge => {
                f.write_str("W4 diagnostic source-map length cannot be represented")
            }
            Self::DiagnosticSourceMapGraphMismatch => {
                f.write_str("W4 diagnostic source map names a different canonical final graph")
            }
            Self::ResourceLimitExceeded { resource, limit } => {
                write!(f, "{resource} exceeds W4 limit {limit}")
            }
        }
    }
}

impl Error for ProductionW4InputErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionW4EncodingErrorV1 {
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    LengthOverflow,
    MalformedEnvelope,
    WrongDomain,
    UnsupportedVersion {
        observed: u16,
    },
    ChecksumMismatch,
}

impl fmt::Display for ProductionW4EncodingErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit { resource, limit } => {
                write!(f, "{resource} exceeds W4 encoding limit {limit}")
            }
            Self::LengthOverflow => f.write_str("W4 encoding length cannot be represented"),
            Self::MalformedEnvelope => f.write_str("W4 canonical envelope is malformed"),
            Self::WrongDomain => f.write_str("W4 canonical envelope has the wrong domain"),
            Self::UnsupportedVersion { observed } => {
                write!(f, "W4 canonical envelope version {observed} is unsupported")
            }
            Self::ChecksumMismatch => f.write_str("W4 canonical envelope checksum differs"),
        }
    }
}

impl Error for ProductionW4EncodingErrorV1 {}

#[derive(Debug)]
pub enum ProductionW4HandoffErrorV1 {
    Canonical(VerifiedCanonicalKernelIrErrorV13),
    Execution(ProductionW4ExecutionErrorV1),
    PlironIdentity(PlironIrIdentityErrorV1),
    SubjectSubstituted,
    ContextSubstituted,
    FunctionSubstituted,
    ScheduleWitnessSubstituted,
    DiagnosticSubjectSubstituted,
    DiagnosticEncoding(ProductionW4EncodingErrorV1),
    DiagnosticSubstituted,
}

impl fmt::Display for ProductionW4HandoffErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(error) => write!(f, "canonical W6 handoff graph is invalid: {error}"),
            Self::Execution(error) => error.fmt(f),
            Self::PlironIdentity(error) => error.fmt(f),
            Self::SubjectSubstituted => f.write_str(
                "W4 subject, target resource, or canonical bytes were substituted at W6 handoff",
            ),
            Self::ContextSubstituted => {
                f.write_str("W4 PLIRON context or mutation epoch was substituted at W6 handoff")
            }
            Self::FunctionSubstituted => {
                f.write_str("W4 live function identity was substituted at W6 handoff")
            }
            Self::ScheduleWitnessSubstituted => {
                f.write_str("W4 analysis schedule evidence was omitted, reordered, or substituted")
            }
            Self::DiagnosticSubjectSubstituted => {
                f.write_str("W4 non-clean diagnostic was substituted across final-graph subjects")
            }
            Self::DiagnosticEncoding(error) => error.fmt(f),
            Self::DiagnosticSubstituted => {
                f.write_str("W4 non-clean diagnostic fields or identity were substituted")
            }
        }
    }
}

impl Error for ProductionW4HandoffErrorV1 {}

/// Runs W4 over the exact live final graph without constructing another graph.
#[allow(clippy::result_large_err)]
pub fn execute_production_w4_final_graph_capability_witness_v1(
    canonical: &VerifiedCanonicalKernelIrV13,
    module: &Module,
    subject: ProductionW4FinalGraphSubjectV1,
    target_resource: ProductionW4TargetResourceInputV1,
    schedule: &[ProductionCapabilityAnalysisStageV1],
    context: &Context,
    functions: &[ProductionW4LiveFunctionV1<'_>],
) -> Result<ProductionW4FinalGraphExecutionV1, ProductionW4ExecutionErrorV1> {
    canonical
        .revalidate()
        .map_err(ProductionW4ExecutionErrorV1::Canonical)?;
    if canonical.identity() != subject.final_graph() {
        return Err(ProductionW4ExecutionErrorV1::FinalGraphSubstituted);
    }
    let decoded = decode_module_v13(canonical.canonical_bytes())
        .map_err(|_| ProductionW4ExecutionErrorV1::CanonicalDecode)?;
    if &decoded != module {
        return Err(ProductionW4ExecutionErrorV1::CanonicalModuleSubstituted);
    }
    require_exact_schedule(schedule)?;
    require_exact_target_resource(&subject, &target_resource)?;
    require_exact_kernel_roots(module, &subject)?;
    let defined = module
        .functions
        .iter()
        .filter(|function| function.body.is_some())
        .map(|function| &function.id)
        .collect::<Vec<_>>();
    if defined.is_empty() {
        return Err(ProductionW4ExecutionErrorV1::EmptyFunctions);
    }
    if defined.len() > MAX_PRODUCTION_W4_FUNCTIONS_V1
        || functions.len() != defined.len()
        || functions
            .iter()
            .zip(&defined)
            .any(|(live, expected)| live.function != *expected)
    {
        return Err(ProductionW4ExecutionErrorV1::FunctionOrderSubstituted);
    }
    let context_identity = require_context_identity(context)
        .map_err(|_| ProductionW4ExecutionErrorV1::ContextIdentityUnavailable)?;
    let before_epoch = current_pliron_epoch(context)?;
    let capability_provenance =
        analyze_kernel_capability_preservation_v1(module, subject.final_epoch)
            .map_err(ProductionW4ExecutionErrorV1::CapabilityProvenance)?;
    if capability_provenance.canonical_identity() != subject.final_graph()
        || capability_provenance.graph_epoch() != subject.final_epoch()
    {
        return Err(ProductionW4ExecutionErrorV1::FinalGraphSubstituted);
    }
    let typed_safety_inventory = match require_exact_capability_live_graph(
        canonical.canonical_bytes(),
        module,
        schedule,
        context,
        functions,
    ) {
        Ok(inventory) => inventory,
        Err(detail) => {
            return non_clean_without_pipeline(
                subject,
                ProductionCapabilityAnalysisKindV1::CapabilityProvenance,
                None,
                KernelCheckStatusV1::Incomplete,
                detail,
            );
        }
    };
    let execution_capability_semantics =
        analyze_execution_capability_final_graph_with_composed_conflicts_v1(
            canonical,
            module,
            subject.final_epoch(),
        )
        .map_err(ProductionW4ExecutionErrorV1::ExecutionCapabilitySemantics)?;
    if let Some(finding) = schedule.iter().find_map(|stage| {
        let status = execution_capability_semantics.stage_status(stage.kind());
        execution_capability_semantics
            .findings()
            .iter()
            .find(|finding| finding.stage() == stage.kind() && finding.status() == status)
    }) {
        let function = Some(finding.location().function().clone());
        let detail = finding.to_string();
        if execution_finding_is_unsupported(finding.reason()) {
            return non_clean_unsupported_without_pipeline(
                subject,
                finding.stage(),
                function,
                detail,
            );
        }
        if finding.status() == KernelCheckStatusV1::Rejected {
            let (obligation, class) = execution_counterexample_classification(finding.reason());
            let mut diagnostic = bounded_diagnostic(&subject, finding.stage(), function, detail);
            attach_counterexample(
                &mut diagnostic,
                obligation,
                class,
                Some(ProductionW4CounterexampleLocationV1 {
                    block: finding.location().block().0 as usize,
                    operation: finding.location().operation(),
                }),
                finding
                    .related()
                    .map(|location| ProductionW4CounterexampleLocationV1 {
                        block: location.block().0 as usize,
                        operation: location.operation(),
                    }),
            );
            return build_non_clean_result(
                subject,
                ProductionW4CapabilityOutcomeV1::Rejected(diagnostic),
                None,
            );
        }
        return non_clean_without_pipeline(
            subject,
            finding.stage(),
            function,
            finding.status(),
            detail,
        );
    }

    let root_entries = subject
        .kernel_roots()
        .iter()
        .map(ProductionW4KernelRootV1::entry)
        .collect::<Vec<_>>();
    let mut outcomes = Vec::with_capacity(functions.len());
    for live in functions {
        let identity = match derive_pliron_ir_structural_identity_v1(context, live.pliron) {
            Ok(identity) => identity,
            Err(error) => {
                return non_clean_without_pipeline(
                    subject,
                    ProductionCapabilityAnalysisKindV1::CanonicalTyping,
                    Some(live.function.clone()),
                    KernelCheckStatusV1::Incomplete,
                    error.to_string(),
                );
            }
        };
        let checked = if root_entries.contains(&live.function) {
            let live_layouts = match live_execution_layout_count(context, live.pliron) {
                Ok(count) => count,
                Err(detail) => {
                    return non_clean_without_pipeline(
                        subject,
                        ProductionCapabilityAnalysisKindV1::ResourceLegality,
                        Some(live.function.clone()),
                        KernelCheckStatusV1::Incomplete,
                        detail,
                    );
                }
            };
            if live_layouts == 0 {
                let canonical_layout = match canonical_execution_layout_for_root(
                    module,
                    live.function,
                    &target_resource,
                ) {
                    Ok(layout) => layout,
                    Err(detail) => {
                        return non_clean_without_pipeline(
                            subject,
                            ProductionCapabilityAnalysisKindV1::ResourceLegality,
                            Some(live.function.clone()),
                            KernelCheckStatusV1::Incomplete,
                            detail,
                        );
                    }
                };
                require_production_pliron_checks_with_atomic_target_and_canonical_layout_before_lowering_v2(
                    context,
                    live.pliron,
                    target_resource.atomic_target(),
                    target_resource.launch_contract(),
                    canonical_layout,
                )
            } else {
                require_production_pliron_checks_with_atomic_and_target_before_lowering_v2(
                    context,
                    live.pliron,
                    target_resource.atomic_target(),
                    target_resource.launch_contract(),
                )
            }
        } else {
            require_production_pliron_checks_with_atomic_target_before_lowering_v2(
                context,
                live.pliron,
                target_resource.atomic_target(),
            )
        };
        let checks = match checked {
            Ok(report) => report,
            Err(error) => {
                let (stage, status) = classify_pipeline_error(&error);
                let diagnostic = bounded_diagnostic(
                    &subject,
                    stage,
                    Some(live.function.clone()),
                    error.to_string(),
                );
                let outcome = if pipeline_error_is_unsupported(&error) {
                    ProductionW4CapabilityOutcomeV1::Unsupported(diagnostic)
                } else {
                    match status {
                        KernelCheckStatusV1::Rejected => {
                            let (obligation, class, primary, related) =
                                pipeline_counterexample_classification(&error, stage);
                            let mut diagnostic = diagnostic;
                            attach_counterexample(
                                &mut diagnostic,
                                obligation,
                                class,
                                primary,
                                related,
                            );
                            ProductionW4CapabilityOutcomeV1::Rejected(diagnostic)
                        }
                        KernelCheckStatusV1::Clean | KernelCheckStatusV1::Incomplete => {
                            ProductionW4CapabilityOutcomeV1::Incomplete(diagnostic)
                        }
                    }
                };
                return build_non_clean_result(subject, outcome, Some(Box::new(error)));
            }
        };
        require_exact_function_report(&checks)?;
        let after_identity = derive_pliron_ir_structural_identity_v1(context, live.pliron)
            .map_err(|error| {
                ProductionW4ExecutionErrorV1::Encoding(ProductionW4EncodingErrorV1::ResourceLimit {
                    resource: error.code(),
                    limit: MAX_PRODUCTION_W4_WITNESS_ENCODING_BYTES_V1,
                })
            })?;
        if !identity.exactly_matches(&after_identity) {
            return Err(ProductionW4ExecutionErrorV1::MutationDuringAnalysis {
                before: before_epoch,
                after: current_pliron_epoch(context)?,
            });
        }
        outcomes.push(ProductionW4FunctionOutcomeV1 {
            function: live.function.clone(),
            structural_identity: identity,
            checks,
        });
    }
    let after_epoch = current_pliron_epoch(context)?;
    if after_epoch != before_epoch {
        return Err(ProductionW4ExecutionErrorV1::MutationDuringAnalysis {
            before: before_epoch,
            after: after_epoch,
        });
    }
    let (stages, analysis_schedule) = execute_clean_schedule(
        canonical,
        module,
        &subject,
        schedule,
        &capability_provenance,
        &execution_capability_semantics,
        &target_resource,
        &outcomes,
        before_epoch,
    )?;
    let canonical_encoding = encode_witness(
        canonical.canonical_bytes(),
        &subject,
        &target_resource,
        WitnessEncodingAnalysisV1 {
            capability: &capability_provenance,
            execution: &execution_capability_semantics,
            typed_safety: &typed_safety_inventory,
            pliron_epoch: before_epoch,
            stages: &stages,
            analysis_schedule: &analysis_schedule,
            functions: &outcomes,
        },
    )?;
    let identity = witness_identity(&canonical_encoding);
    Ok(ProductionW4FinalGraphExecutionV1::Complete(
        ProductionW4FinalGraphCapabilityWitnessV1 {
            subject,
            canonical_kir: canonical.canonical_bytes().into(),
            capability_provenance,
            execution_capability_semantics,
            typed_safety_inventory,
            target_resource,
            context_identity,
            pliron_epoch: before_epoch,
            stages: stages.into_boxed_slice(),
            analysis_schedule,
            functions: outcomes.into_boxed_slice(),
            canonical_encoding: canonical_encoding.into_boxed_slice(),
            identity,
        },
    ))
}

/// Stable checker identity covering order, dependencies, executors, and named
/// production analysis implementations.
pub fn production_w4_checker_identity_v1() -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(CHECKER_DOMAIN_V1);
    hash.update(PRODUCTION_W4_FINAL_GRAPH_CAPABILITY_WITNESS_VERSION_V1.to_le_bytes());
    hash.update(crate::EXECUTION_CAPABILITY_FINAL_GRAPH_SEMANTICS_VERSION_V1.to_le_bytes());
    for limit in [
        crate::MAX_EXECUTION_SEMANTIC_ROOTS_V1,
        crate::MAX_EXECUTION_SEMANTIC_FUNCTIONS_V1,
        crate::MAX_EXECUTION_SEMANTIC_BLOCKS_V1,
        MAX_EXECUTION_SEMANTIC_OPERATIONS_V1,
        crate::MAX_EXECUTION_SEMANTIC_CALL_EDGES_V1,
        crate::MAX_EXECUTION_SEMANTIC_FLOW_STATES_V1,
        crate::MAX_EXECUTION_SEMANTIC_SEQUENCE_STATES_V1,
        crate::MAX_EXECUTION_SEMANTIC_FINDINGS_V1,
    ] {
        hash.update((limit as u64).to_le_bytes());
    }
    hash.update(KIR_PLIRON_BRIDGE_V13_IDENTITY_DOMAIN_V1);
    hash.update(KIR_PLIRON_EXECUTION_OPERATION_IDENTITY_DOMAIN_V1);
    for family in CanonicalKirSafetyFamilyV1::ALL {
        hash.update([family.ordinal() as u8]);
        hash.update(family_required_stage_mask(family).to_le_bytes());
    }
    for stage in PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1 {
        hash.update([kind_tag(stage.kind()), executor_tag(stage.executor())]);
        hash.update((stage.dependencies().len() as u64).to_le_bytes());
        for dependency in stage.dependencies() {
            hash.update([kind_tag(*dependency)]);
        }
    }
    for stage in PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1 {
        for obligation in stage.obligations() {
            hash.update([
                analysis_obligation_tag(obligation.kind()),
                kind_tag(obligation.owner()),
                independent_obligation_checker_tag(independent_obligation_checker(
                    obligation.kind(),
                )),
            ]);
            hash.update((obligation.dependencies().len() as u64).to_le_bytes());
            for dependency in obligation.dependencies() {
                hash.update([analysis_obligation_tag(*dependency)]);
            }
        }
    }
    for pass in PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2 {
        hash.update([pass_tag(pass)]);
        hash.update(pass.name().as_bytes());
        hash.update([0]);
    }
    hash.finalize().into()
}

fn require_exact_schedule(
    schedule: &[ProductionCapabilityAnalysisStageV1],
) -> Result<(), ProductionW4ExecutionErrorV1> {
    if schedule.len() != PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len() {
        return Err(ProductionW4ExecutionErrorV1::ScheduleLength {
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
            return Err(ProductionW4ExecutionErrorV1::ScheduleStage { position });
        }
        if observed.dependencies().iter().any(|dependency| {
            !schedule[..position]
                .iter()
                .any(|stage| stage.kind() == *dependency)
        }) {
            return Err(ProductionW4ExecutionErrorV1::ScheduleDependency { position });
        }
    }
    let mandatory = schedule
        .iter()
        .filter_map(|stage| match stage.executor() {
            ProductionCapabilityAnalysisExecutorV1::MandatoryPlironPass(pass) => Some(pass),
            _ => None,
        })
        .collect::<Vec<_>>();
    if mandatory != PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2 {
        return Err(ProductionW4ExecutionErrorV1::MandatoryPassOrder);
    }
    Ok(())
}

fn require_exact_target_resource(
    subject: &ProductionW4FinalGraphSubjectV1,
    target: &ProductionW4TargetResourceInputV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    if target.final_graph() != subject.final_graph()
        || target.final_epoch() != subject.final_epoch()
        || target.target_identity() != subject.target_identity()
        || target.launch_identity() != subject.launch_identity()
        || target.closure_identity() != subject.target_closure_identity()
    {
        return Err(ProductionW4ExecutionErrorV1::TargetResourceSubstituted);
    }
    Ok(())
}

fn require_exact_kernel_roots(
    module: &Module,
    subject: &ProductionW4FinalGraphSubjectV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    if module.kernels.len() != subject.kernel_roots.len()
        || module
            .kernels
            .iter()
            .zip(&subject.kernel_roots)
            .any(|(kernel, root)| kernel.id != root.kernel || kernel.entry != root.entry)
    {
        return Err(ProductionW4ExecutionErrorV1::KernelRootsSubstituted);
    }
    Ok(())
}

fn live_execution_layout_count(context: &Context, function: &FuncOp) -> Result<usize, String> {
    let inventory =
        BoundedPlironFunctionInventoryV1::collect(context, function).map_err(|error| {
            format!(
                "live PLIRON {} count {} exceeds bound {} while selecting canonical launch custody",
                error.resource(),
                error.actual(),
                error.limit(),
            )
        })?;
    Ok(inventory
        .operations()
        .iter()
        .filter(|site| {
            Operation::get_op_dyn(site.pointer(), context)
                .downcast_ref::<ExecutionLayoutOp>()
                .is_some()
        })
        .count())
}

fn canonical_execution_layout_for_root(
    module: &Module,
    entry: &FunctionId,
    target: &ProductionW4TargetResourceInputV1,
) -> Result<PlironCanonicalExecutionLayoutV1, String> {
    let mut matching = module
        .kernels
        .iter()
        .filter(|kernel| &kernel.entry == entry);
    let kernel = matching
        .next()
        .ok_or_else(|| format!("kernel entry {entry} has no canonical launch root"))?;
    if matching.next().is_some() {
        return Err(format!(
            "kernel entry {entry} has multiple canonical launch roots"
        ));
    }
    let workgroup = kernel
        .workgroup_size
        .ok_or_else(|| format!("kernel entry {entry} has no exact canonical workgroup size"))?;
    let mut global_extents = [1_u64; 3];
    for (axis, extent) in kernel.domain.extents().enumerate() {
        global_extents[axis] = match extent {
            LaunchExtent::Dynamic => 0,
            LaunchExtent::Static(value) => u64::from(value),
        };
    }
    let subgroup_sizes = target
        .target_decisions()
        .iter()
        .filter_map(|decision| match decision.requirement() {
            TargetCapabilityRequirementV1::SubgroupSize(width) => Some(u64::from(width)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [subgroup_size] = subgroup_sizes.as_slice() else {
        return Err(format!(
            "kernel entry {entry} does not have one exact target-decided subgroup size"
        ));
    };
    PlironCanonicalExecutionLayoutV1::try_new(
        global_extents,
        [
            u64::from(workgroup.x),
            u64::from(workgroup.y),
            u64::from(workgroup.z),
        ],
        *subgroup_size,
    )
    .map_err(|error| format!("kernel entry {entry} has invalid canonical launch geometry: {error}"))
}

fn require_exact_capability_live_graph(
    canonical_kir: &[u8],
    module: &Module,
    schedule: &[ProductionCapabilityAnalysisStageV1],
    context: &Context,
    functions: &[ProductionW4LiveFunctionV1<'_>],
) -> Result<ProductionW4TypedSafetyInventoryV1, String> {
    let inventory =
        require_exact_typed_safety_live_graph(canonical_kir, module, schedule, context, functions)?;
    require_exact_execution_capability_live_graph(canonical_kir, module, context, functions)?;
    Ok(inventory)
}

fn require_exact_typed_safety_live_graph(
    canonical_kir: &[u8],
    module: &Module,
    schedule: &[ProductionCapabilityAnalysisStageV1],
    context: &Context,
    functions: &[ProductionW4LiveFunctionV1<'_>],
) -> Result<ProductionW4TypedSafetyInventoryV1, String> {
    let graph_epoch = bridge_v13_graph_epoch(canonical_kir)?;
    let mut counts = [0_u32; PRODUCTION_W4_TYPED_SAFETY_FAMILY_COUNT_V1];
    let mut operation_count = 0_u32;
    let mut required_stage_mask = 0_u16;

    for family in CanonicalKirSafetyFamilyV1::ALL {
        let mask = family_required_stage_mask(family);
        if mask == 0 || mask & !all_production_stage_mask() != 0 {
            return Err(format!(
                "typed safety family {family:?} has no exact bounded W4 stage coverage"
            ));
        }
    }

    for ((function_ordinal, function), live) in module
        .functions
        .iter()
        .enumerate()
        .filter(|(_, function)| function.body.is_some())
        .zip(functions)
    {
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| format!("defined helper {} lost its body", function.id))?;
        let function_ordinal = u32::try_from(function_ordinal)
            .map_err(|_| format!("helper {} has an unrepresentable ordinal", function.id))?;
        let mut expected = BTreeMap::new();
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            let block_ordinal_u32 = u32::try_from(block_ordinal).map_err(|_| {
                format!(
                    "helper {} has an unrepresentable block ordinal",
                    function.id
                )
            })?;
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                let Some(family) = safety_family_for_operation(&operation.kind) else {
                    continue;
                };
                let operation_ordinal_u32 = u32::try_from(operation_ordinal).map_err(|_| {
                    format!(
                        "helper {} KIR block {} has an unrepresentable operation ordinal",
                        function.id, block.id.0
                    )
                })?;
                let coordinate = SourceCoordinateAttr::new(
                    function_ordinal,
                    block_ordinal_u32,
                    operation_ordinal_u32,
                );
                expected.insert(
                    coordinate.components(),
                    CanonicalKirSafetyContractV1::Operation {
                        family,
                        contract: operation.clone(),
                        coordinate,
                        graph_epoch,
                    },
                );
            }
            if let Some((family, terminator)) = block.terminator.as_ref().and_then(|terminator| {
                safety_family_for_terminator(terminator).map(|family| (family, terminator))
            }) {
                let coordinate =
                    SourceCoordinateAttr::new(function_ordinal, block_ordinal_u32, u32::MAX);
                expected.insert(
                    coordinate.components(),
                    CanonicalKirSafetyContractV1::Terminator {
                        family,
                        contract: terminator.clone(),
                        coordinate,
                        graph_epoch,
                    },
                );
            }
        }

        let live_inventory = BoundedPlironFunctionInventoryV1::collect(context, live.pliron)
            .map_err(|failure| {
                format!(
                    "helper {} typed safety inventory exhausted {}: observed {}, limit {}",
                    function.id,
                    failure.resource(),
                    failure.actual(),
                    failure.limit()
                )
            })?;
        for site in live_inventory.operations() {
            let operation = Operation::get_op_dyn(site.pointer(), context);
            let observed = canonical_kir_safety_contract_v1(&*operation, context)
                .map_err(|error| error.to_string())?;
            let Some(observed) = observed else {
                continue;
            };
            let coordinate = observed.coordinate().components();
            let expected_contract = expected.remove(&coordinate).ok_or_else(|| {
                format!(
                    "helper {} has an extra or duplicate typed safety carrier at {coordinate:?}",
                    function.id
                )
            })?;
            if observed != expected_contract || observed.graph_epoch() != graph_epoch {
                return Err(format!(
                    "helper {} typed safety carrier at {coordinate:?} differs from canonical V13",
                    function.id
                ));
            }
            let family = observed.family();
            counts[family.ordinal()] = counts[family.ordinal()]
                .checked_add(1)
                .ok_or_else(|| "typed safety family count overflow".to_owned())?;
            operation_count = operation_count
                .checked_add(1)
                .ok_or_else(|| "typed safety operation count overflow".to_owned())?;
            required_stage_mask |= family_required_stage_mask(family);
        }
        if let Some((coordinate, missing)) = expected.into_iter().next() {
            return Err(format!(
                "helper {} canonical {:?} operation at {coordinate:?} is missing its typed safety carrier",
                function.id,
                missing.family()
            ));
        }
    }

    for stage in schedule {
        if required_stage_mask & stage_bit(stage.kind()) != 0
            && !PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
                .iter()
                .any(|expected| expected == stage)
        {
            return Err(format!(
                "typed safety inventory requires substituted W4 stage {:?}",
                stage.kind()
            ));
        }
    }
    for expected in PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1 {
        if required_stage_mask & stage_bit(expected.kind()) != 0
            && !schedule.iter().any(|stage| stage == &expected)
        {
            return Err(format!(
                "typed safety inventory requires missing W4 stage {:?}",
                expected.kind()
            ));
        }
    }

    let mut digest = Sha256::new();
    digest.update(b"FE2O3/PRODUCTION-W4/TYPED-SAFETY-INVENTORY/V1\0");
    digest.update(graph_epoch);
    for count in counts {
        digest.update(count.to_le_bytes());
    }
    digest.update(required_stage_mask.to_le_bytes());
    digest.update(operation_count.to_le_bytes());
    Ok(ProductionW4TypedSafetyInventoryV1 {
        family_counts: counts,
        required_stage_mask,
        operation_count,
        identity: digest.finalize().into(),
    })
}

fn safety_family_for_operation(kind: &OperationKind) -> Option<CanonicalKirSafetyFamilyV1> {
    use CanonicalKirSafetyFamilyV1 as Family;
    Some(match kind {
        OperationKind::Intrinsic(_) => Family::Intrinsic,
        OperationKind::MemoryIntrinsic(_) => Family::MemoryIntrinsic,
        OperationKind::Alloca { .. } => Family::Alloca,
        OperationKind::GuardedLoad { .. } => Family::GuardedLoad,
        OperationKind::GuardedStore { .. } => Family::GuardedStore,
        OperationKind::Barrier(_) => Family::Barrier,
        OperationKind::Atomic(_) => Family::Atomic,
        OperationKind::Fence(_) => Family::Fence,
        OperationKind::WorkgroupBarrier(_) => Family::WorkgroupBarrier,
        OperationKind::WorkgroupMemory(_) => Family::WorkgroupMemory,
        OperationKind::Matrix(_) => Family::Matrix,
        OperationKind::Gfx950LdsTranspose(_) => Family::TargetSpecificLdsTranspose,
        OperationKind::Wave(_) => Family::Wave,
        OperationKind::InlineAssembly(_) => Family::InlineAssembly,
        _ => return None,
    })
}

fn safety_family_for_terminator(terminator: &Terminator) -> Option<CanonicalKirSafetyFamilyV1> {
    use CanonicalKirSafetyFamilyV1 as Family;
    match terminator {
        Terminator::Switch { .. } => Some(Family::Switch),
        Terminator::IntegerSwitch { .. } => Some(Family::IntegerSwitch),
        Terminator::Unreachable => Some(Family::Unreachable),
        _ => None,
    }
}

const fn stage_bit(stage: ProductionCapabilityAnalysisKindV1) -> u16 {
    1_u16 << kind_tag(stage)
}

const fn all_production_stage_mask() -> u16 {
    u16::MAX
}

const fn family_required_stage_mask(family: CanonicalKirSafetyFamilyV1) -> u16 {
    use CanonicalKirSafetyFamilyV1 as Family;
    use ProductionCapabilityAnalysisKindV1 as Stage;

    let base = stage_bit(Stage::CanonicalTyping)
        | stage_bit(Stage::CapabilityProvenance)
        | stage_bit(Stage::EffectRefinement)
        | stage_bit(Stage::SemanticRefinement);
    let target = stage_bit(Stage::ResourceLegality);
    let memory = stage_bit(Stage::MemoryBounds)
        | stage_bit(Stage::RaceFreedom)
        | stage_bit(Stage::HierarchicalOwnership)
        | stage_bit(Stage::Initialization)
        | stage_bit(Stage::MemoryVisibility);
    let synchronization = stage_bit(Stage::Uniformity)
        | stage_bit(Stage::BarrierConvergence)
        | stage_bit(Stage::MemoryVisibility);
    let workgroup_lifetime = stage_bit(Stage::PipelineProtocol)
        | stage_bit(Stage::Initialization)
        | stage_bit(Stage::MemoryVisibility)
        | stage_bit(Stage::WorkgroupMemoryEpochs);
    let control = stage_bit(Stage::Uniformity)
        | stage_bit(Stage::BarrierConvergence)
        | stage_bit(Stage::PipelineProtocol)
        | stage_bit(Stage::Initialization)
        | stage_bit(Stage::MemoryVisibility)
        | stage_bit(Stage::WorkgroupMemoryEpochs);

    match family {
        Family::Intrinsic => base | target,
        Family::MemoryIntrinsic => base | target | memory | stage_bit(Stage::PipelineProtocol),
        Family::Alloca => {
            base | target
                | stage_bit(Stage::MemoryBounds)
                | stage_bit(Stage::HierarchicalOwnership)
                | stage_bit(Stage::Initialization)
                | stage_bit(Stage::WorkgroupMemoryEpochs)
        }
        Family::GuardedLoad | Family::GuardedStore => base | target | memory,
        Family::Barrier => base | target | synchronization | stage_bit(Stage::RaceFreedom),
        Family::Atomic => {
            base | target
                | stage_bit(Stage::MemoryBounds)
                | stage_bit(Stage::AtomicLegality)
                | stage_bit(Stage::RaceFreedom)
                | stage_bit(Stage::MemoryVisibility)
        }
        Family::Fence => {
            base | target | stage_bit(Stage::RaceFreedom) | stage_bit(Stage::MemoryVisibility)
        }
        Family::WorkgroupBarrier => {
            base | target
                | synchronization
                | stage_bit(Stage::RaceFreedom)
                | stage_bit(Stage::Initialization)
                | stage_bit(Stage::WorkgroupMemoryEpochs)
        }
        Family::WorkgroupMemory => {
            base | target
                | memory
                | stage_bit(Stage::PipelineProtocol)
                | stage_bit(Stage::WorkgroupMemoryEpochs)
        }
        Family::Matrix | Family::TargetSpecificLdsTranspose => {
            base | target
                | memory
                | synchronization
                | workgroup_lifetime
                | stage_bit(Stage::TensorLayout)
        }
        Family::Wave => base | target | stage_bit(Stage::Uniformity),
        Family::InlineAssembly => all_production_stage_mask(),
        Family::Switch | Family::IntegerSwitch | Family::Unreachable => base | control,
    }
}

/// Frozen W4 stage coverage required for one canonical V13 safety family.
pub const fn production_w4_required_stage_mask_v1(family: CanonicalKirSafetyFamilyV1) -> u16 {
    family_required_stage_mask(family)
}

fn require_exact_execution_capability_live_graph(
    canonical_kir: &[u8],
    module: &Module,
    context: &Context,
    functions: &[ProductionW4LiveFunctionV1<'_>],
) -> Result<(), String> {
    let graph_epoch = bridge_v13_graph_epoch(canonical_kir)?;
    let defined = module
        .functions
        .iter()
        .enumerate()
        .filter(|(_, function)| function.body.is_some());
    let mut checked = 0usize;
    for ((function_ordinal, function), live) in defined.zip(functions) {
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| format!("defined helper {} lost its body", function.id))?;
        let function_ordinal = u32::try_from(function_ordinal)
            .map_err(|_| format!("helper {} has an unrepresentable ordinal", function.id))?;
        let mut expected = BTreeMap::new();
        for (block_ordinal, block) in body.blocks.iter().enumerate() {
            let block_ordinal = u32::try_from(block_ordinal).map_err(|_| {
                format!(
                    "helper {} has an unrepresentable block ordinal",
                    function.id
                )
            })?;
            for (operation_ordinal, operation) in block.operations.iter().enumerate() {
                let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                    continue;
                };
                checked = checked.saturating_add(1);
                if checked > MAX_EXECUTION_SEMANTIC_OPERATIONS_V1 {
                    return Err(format!(
                        "execution-capability correspondence exceeds {} operations",
                        MAX_EXECUTION_SEMANTIC_OPERATIONS_V1
                    ));
                }
                let operation_ordinal = u32::try_from(operation_ordinal).map_err(|_| {
                    format!(
                        "helper {} KIR block {} has an unrepresentable operation ordinal",
                        function.id, block.id.0
                    )
                })?;
                if expected
                    .insert(
                        (function_ordinal, block_ordinal, operation_ordinal),
                        contract,
                    )
                    .is_some()
                {
                    return Err(format!(
                        "helper {} has duplicate canonical execution-capability coordinates",
                        function.id
                    ));
                }
            }
        }

        let inventory =
            BoundedPlironFunctionInventoryV1::collect(context, live.pliron).map_err(|failure| {
                format!(
                    "helper {} live PLIRON {} inventory exhausted: observed {}, limit {}",
                    function.id,
                    failure.resource(),
                    failure.actual(),
                    failure.limit()
                )
            })?;
        for site in inventory.operations() {
            let Some(operation) =
                Operation::get_op::<PlironExecutionCapabilityOp>(site.pointer(), context)
            else {
                continue;
            };
            let coordinate = operation.coordinate(context).ok_or_else(|| {
                format!(
                    "helper {} live PLIRON block {} op {} lacks a source coordinate",
                    function.id,
                    site.block(),
                    site.operation()
                )
            })?;
            let coordinate = coordinate.components();
            let contract = expected.remove(&coordinate).ok_or_else(|| {
                format!(
                    "helper {} has an extra or duplicate live execution capability at canonical coordinate ({}, {}, {})",
                    function.id, coordinate.0, coordinate.1, coordinate.2
                )
            })?;
            let observed = operation.contract(context).ok_or_else(|| {
                format!(
                    "helper {} live execution capability at ({}, {}, {}) has no canonical contract",
                    function.id, coordinate.0, coordinate.1, coordinate.2
                )
            })?;
            let operation_identity = bridge_execution_operation_identity(contract)?;
            if &observed != contract
                || operation.graph_epoch(context) != Some(graph_epoch)
                || operation.operation_identity(context) != Some(operation_identity)
                || operation.source_operation_identity(context) != Some(contract.source.operation)
            {
                return Err(format!(
                    "helper {} live execution capability at ({}, {}, {}) differs from canonical V13",
                    function.id, coordinate.0, coordinate.1, coordinate.2
                ));
            }
        }
        if let Some((coordinate, _)) = expected.into_iter().next() {
            return Err(format!(
                "helper {} canonical execution capability at ({}, {}, {}) is missing from the retained live PLIRON graph",
                function.id, coordinate.0, coordinate.1, coordinate.2
            ));
        }
    }
    Ok(())
}

fn bridge_v13_graph_epoch(canonical_kir: &[u8]) -> Result<[u8; 32], String> {
    let domain_length = u32::try_from(KIR_PLIRON_BRIDGE_V13_IDENTITY_DOMAIN_V1.len())
        .map_err(|_| "bridge V13 identity domain length overflow".to_owned())?;
    let canonical_length = u64::try_from(canonical_kir.len())
        .map_err(|_| "canonical KIR V13 length overflow".to_owned())?;
    let mut digest = Sha256::new();
    digest.update(domain_length.to_le_bytes());
    digest.update(KIR_PLIRON_BRIDGE_V13_IDENTITY_DOMAIN_V1);
    digest.update(canonical_length.to_le_bytes());
    digest.update(canonical_kir);
    Ok(digest.finalize().into())
}

fn bridge_execution_operation_identity(
    contract: &fe2o3_kernel_ir::ExecutionCapabilityOpV1,
) -> Result<[u8; 32], String> {
    let bytes = encode_execution_capability_contract_v1(contract)
        .ok_or_else(|| "execution-capability contract is not canonically encodable".to_owned())?;
    let length = u64::try_from(bytes.len())
        .map_err(|_| "execution-capability contract length overflow".to_owned())?;
    let mut digest = Sha256::new();
    digest.update(KIR_PLIRON_EXECUTION_OPERATION_IDENTITY_DOMAIN_V1);
    digest.update(contract.source.function);
    digest.update(contract.source.block.to_le_bytes());
    digest.update(contract.source.operation);
    digest.update(length.to_le_bytes());
    digest.update(bytes);
    Ok(digest.finalize().into())
}

fn require_exact_function_report(
    report: &ProductionPlironPreloweringReportV2,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    let validation = report.report_validation();
    let valid = report.is_clean()
        && report
            .target_contract()
            .is_none_or(|target| target.is_clean())
        && report.pass_order() == &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
        && report.preservation().is_exact_identity()
        && report.preservation().certificates().len()
            == PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2.len()
        && validation.stages().len() == PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2.len()
        && validation.all_reports_independently_validated()
        && validation
            .stages()
            .iter()
            .zip(PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2)
            .enumerate()
            .all(|(position, (stage, pass))| {
                stage.checkpoint().position() == position
                    && stage.checkpoint().pass() == pass
                    && stage.implementation().pass() == pass
                    && stage.analysis_status() == KernelCheckStatusV1::Clean
            });
    if valid {
        Ok(())
    } else {
        Err(ProductionW4ExecutionErrorV1::MandatoryPassOrder)
    }
}

fn classify_pipeline_error(
    error: &ProductionPlironPreloweringErrorV2,
) -> (ProductionCapabilityAnalysisKindV1, KernelCheckStatusV1) {
    use ProductionCapabilityAnalysisKindV1 as Kind;
    match error {
        ProductionPlironPreloweringErrorV2::TargetContract(error) => {
            (Kind::ResourceLegality, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::TensorLayout(error) => {
            (Kind::TensorLayout, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::Bounds(error) => {
            (Kind::MemoryBounds, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::Atomic(error) => {
            (Kind::AtomicLegality, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::Race(error) => {
            let stage = if error.report().findings().iter().any(|finding| {
                matches!(
                    finding,
                    crate::RankedRaceFindingV1::HappensBeforeIncomplete { .. }
                )
            }) {
                Kind::MemoryVisibility
            } else {
                Kind::RaceFreedom
            };
            (stage, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::Ownership(error) => {
            (Kind::HierarchicalOwnership, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::Barrier(error) => {
            (Kind::BarrierConvergence, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::PipelineProtocol(error) => {
            (Kind::PipelineProtocol, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::Workgroup(error) => {
            let stage = if error.report().findings().iter().any(|finding| {
                matches!(
                    finding,
                    PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization { .. }
                )
            }) {
                Kind::Initialization
            } else if error.report().findings().iter().any(|finding| {
                matches!(
                    finding,
                    PlironWorkgroupMemoryFindingV1::ConflictingEffects { .. }
                )
            }) {
                Kind::MemoryVisibility
            } else {
                Kind::WorkgroupMemoryEpochs
            };
            (stage, error.report().status())
        }
        ProductionPlironPreloweringErrorV2::Semantic(error) => {
            let report = error.report();
            let stage = if report.effect_refinement().status() != KernelCheckStatusV1::Clean {
                Kind::EffectRefinement
            } else {
                Kind::SemanticRefinement
            };
            (stage, report.status())
        }
        ProductionPlironPreloweringErrorV2::Preservation(_)
        | ProductionPlironPreloweringErrorV2::ReportValidation(_) => {
            (Kind::SemanticRefinement, KernelCheckStatusV1::Incomplete)
        }
    }
}

#[allow(clippy::type_complexity)]
fn pipeline_counterexample_classification(
    error: &ProductionPlironPreloweringErrorV2,
    stage: ProductionCapabilityAnalysisKindV1,
) -> (
    ProductionW4AnalysisObligationKindV1,
    ProductionW4CounterexampleClassV1,
    Option<ProductionW4CounterexampleLocationV1>,
    Option<ProductionW4CounterexampleLocationV1>,
) {
    use ProductionW4AnalysisObligationKindV1 as Obligation;
    use ProductionW4CounterexampleClassV1 as Class;

    let location = |block, operation| ProductionW4CounterexampleLocationV1 { block, operation };
    match error {
        ProductionPlironPreloweringErrorV2::TargetContract(_) => (
            Obligation::ResourceLegality,
            Class::ResourceLegality,
            None,
            None,
        ),
        ProductionPlironPreloweringErrorV2::TensorLayout(_) => {
            (Obligation::TensorLayout, Class::TensorLayout, None, None)
        }
        ProductionPlironPreloweringErrorV2::Bounds(_) => {
            (Obligation::MemoryBounds, Class::MemoryBounds, None, None)
        }
        ProductionPlironPreloweringErrorV2::Atomic(_) => (
            Obligation::AtomicLegality,
            Class::AtomicLegality,
            None,
            None,
        ),
        ProductionPlironPreloweringErrorV2::Race(error) => {
            let pair = error
                .report()
                .findings()
                .iter()
                .find_map(|finding| match finding {
                    crate::RankedRaceFindingV1::ConflictingEffects { first, second, .. }
                    | crate::RankedRaceFindingV1::InsufficientAtomicScope {
                        first, second, ..
                    } => Some((
                        location(first.location().block(), first.location().operation()),
                        location(second.location().block(), second.location().operation()),
                    )),
                    _ => None,
                });
            (
                Obligation::RaceFreedom,
                Class::RaceConflict,
                pair.map(|pair| pair.0),
                pair.map(|pair| pair.1),
            )
        }
        ProductionPlironPreloweringErrorV2::Ownership(_) => (
            Obligation::HierarchicalOwnership,
            Class::OwnershipConflict,
            None,
            None,
        ),
        ProductionPlironPreloweringErrorV2::Barrier(error) => {
            let finding = error
                .report()
                .findings()
                .iter()
                .find(|finding| finding.status() == KernelCheckStatusV1::Rejected);
            match finding {
                Some(crate::PlironBarrierFindingV1::DivergentBarrierTrace {
                    first_trace,
                    second_trace,
                    ..
                })
                | Some(crate::PlironBarrierFindingV1::DivergentBarrierPaths {
                    first_trace,
                    second_trace,
                }) => (
                    Obligation::BarrierOrder,
                    Class::BarrierOrder,
                    first_trace
                        .first()
                        .map(|&(block, operation)| location(block, operation)),
                    second_trace
                        .first()
                        .map(|&(block, operation)| location(block, operation)),
                ),
                Some(crate::PlironBarrierFindingV1::SimtProtocolViolation { issue }) => match issue
                    .as_ref()
                {
                    crate::PlironSimtProtocolIssueV1::PhaseMismatch { first, second, .. } => (
                        Obligation::BarrierOrder,
                        Class::BarrierOrder,
                        first.first().map(|event| {
                            location(event.location().block(), event.location().operation())
                        }),
                        second.first().map(|event| {
                            location(event.location().block(), event.location().operation())
                        }),
                    ),
                    crate::PlironSimtProtocolIssueV1::PartialTensorParticipation {
                        location: site,
                        ..
                    }
                    | crate::PlironSimtProtocolIssueV1::ClaimedActiveMaskMismatch {
                        location: site,
                        ..
                    } => (
                        Obligation::CollectiveParticipation,
                        Class::CollectiveParticipation,
                        Some(location(site.block(), site.operation())),
                        None,
                    ),
                    crate::PlironSimtProtocolIssueV1::ResourceLimitExceeded => (
                        Obligation::BarrierConvergence,
                        Class::Uniformity,
                        None,
                        None,
                    ),
                },
                _ => (
                    Obligation::BarrierConvergence,
                    Class::Uniformity,
                    None,
                    None,
                ),
            }
        }
        ProductionPlironPreloweringErrorV2::PipelineProtocol(error) => {
            let (primary, related) = error
                .report()
                .findings()
                .iter()
                .find_map(|finding| match finding {
                    crate::PlironPipelineProtocolFindingV1::OrphanEvent { block, operation } => {
                        Some((Some(location(*block, *operation)), None))
                    }
                    crate::PlironPipelineProtocolFindingV1::AliasedStorage {
                        first_block,
                        first_operation,
                        second_block,
                        second_operation,
                    } => Some((
                        Some(location(*first_block, *first_operation)),
                        Some(location(*second_block, *second_operation)),
                    )),
                    crate::PlironPipelineProtocolFindingV1::InvalidSchedule {
                        pipeline_block,
                        pipeline_operation,
                        event_block,
                        event_operation,
                        ..
                    } => Some((
                        Some(location(*pipeline_block, *pipeline_operation)),
                        event_block
                            .zip(*event_operation)
                            .map(|(block, operation)| location(block, operation)),
                    )),
                    _ => None,
                })
                .unwrap_or((None, None));
            (
                Obligation::WorkgroupMemoryEpochs,
                Class::PipelineEpochProtocol,
                primary,
                related,
            )
        }
        ProductionPlironPreloweringErrorV2::Workgroup(error) => {
            let finding = error
                .report()
                .findings()
                .iter()
                .find(|finding| finding.status() == KernelCheckStatusV1::Rejected);
            match finding {
                Some(PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization {
                    block,
                    operation,
                    ..
                }) => (
                    Obligation::Initialization,
                    Class::ReadBeforeInitialization,
                    Some(location(*block, *operation)),
                    None,
                ),
                Some(PlironWorkgroupMemoryFindingV1::ConflictingEffects {
                    first_block,
                    first_operation,
                    second_block,
                    second_operation,
                    ..
                }) => (
                    Obligation::MemoryVisibility,
                    Class::MemoryVisibility,
                    Some(location(*first_block, *first_operation)),
                    Some(location(*second_block, *second_operation)),
                ),
                _ => (
                    Obligation::WorkgroupMemoryEpochs,
                    Class::PipelineEpochProtocol,
                    None,
                    None,
                ),
            }
        }
        ProductionPlironPreloweringErrorV2::Semantic(_) => {
            if stage == ProductionCapabilityAnalysisKindV1::EffectRefinement {
                (
                    Obligation::EffectRefinement,
                    Class::EffectRefinement,
                    None,
                    None,
                )
            } else {
                (
                    Obligation::SemanticRefinement,
                    Class::SemanticRefinement,
                    None,
                    None,
                )
            }
        }
        ProductionPlironPreloweringErrorV2::Preservation(_)
        | ProductionPlironPreloweringErrorV2::ReportValidation(_) => (
            Obligation::SemanticRefinement,
            Class::SemanticRefinement,
            None,
            None,
        ),
    }
}

fn execution_finding_is_unsupported(reason: &crate::ExecutionCapabilitySemanticReasonV1) -> bool {
    matches!(
        reason,
        crate::ExecutionCapabilitySemanticReasonV1::UnsupportedExternalCall { .. }
            | crate::ExecutionCapabilitySemanticReasonV1::DynamicBarrierSequenceUnsupported { .. }
            | crate::ExecutionCapabilitySemanticReasonV1::UnsupportedMemoryEffect
    )
}

fn execution_counterexample_classification(
    reason: &crate::ExecutionCapabilitySemanticReasonV1,
) -> (
    ProductionW4AnalysisObligationKindV1,
    ProductionW4CounterexampleClassV1,
) {
    use crate::ExecutionCapabilitySemanticReasonV1 as Reason;
    use ProductionW4AnalysisObligationKindV1 as Obligation;
    use ProductionW4CounterexampleClassV1 as Class;
    match reason {
        Reason::NonUniformArrival { .. } | Reason::UniformityIncomplete => {
            (Obligation::Uniformity, Class::Uniformity)
        }
        Reason::BarrierSequenceMismatch { .. } | Reason::BarrierCountMismatch { .. } => {
            (Obligation::BarrierOrder, Class::BarrierOrder)
        }
        Reason::IncompleteCollectiveParticipation { .. } => (
            Obligation::CollectiveParticipation,
            Class::CollectiveParticipation,
        ),
        Reason::LdsReuseBeforeEpochTransition | Reason::StaleEpoch { .. } => (
            Obligation::WorkgroupMemoryEpochs,
            Class::PipelineEpochProtocol,
        ),
        Reason::LdsStateMismatch { .. } => {
            (Obligation::Initialization, Class::ReadBeforeInitialization)
        }
        Reason::InvalidAtomicOperation | Reason::InsufficientAtomicScope { .. } => {
            (Obligation::AtomicLegality, Class::AtomicLegality)
        }
        Reason::InsufficientAtomicOrdering { .. } => {
            (Obligation::MemoryVisibility, Class::MemoryVisibility)
        }
        Reason::ConflictingEffects | Reason::ConflictingAccessCorrespondenceUnavailable => {
            (Obligation::RaceFreedom, Class::RaceConflict)
        }
        Reason::ResourceLimit { .. }
        | Reason::UnsupportedExternalCall { .. }
        | Reason::RecursiveCapabilityCall { .. }
        | Reason::DynamicBarrierSequenceUnsupported { .. }
        | Reason::MissingEpochProvenance
        | Reason::UnsupportedMemoryEffect => {
            (Obligation::CanonicalTyping, Class::CanonicalExecution)
        }
    }
}

fn pipeline_error_is_unsupported(error: &ProductionPlironPreloweringErrorV2) -> bool {
    match error {
        ProductionPlironPreloweringErrorV2::Bounds(error) => {
            error.report().findings().iter().any(|finding| {
                matches!(
                    finding,
                    crate::RankedBoundsFindingV1::UnsupportedTerminator { .. }
                        | crate::RankedBoundsFindingV1::UnsupportedOperation { .. }
                )
            })
        }
        ProductionPlironPreloweringErrorV2::TensorLayout(error) => {
            error.report().findings().iter().any(|finding| {
                matches!(
                    finding,
                    crate::PlironTensorLayoutFindingV1::Contract {
                        finding: fe2o3_kernel_ir::TensorLayoutFindingV1::UnsupportedProfile
                            | fe2o3_kernel_ir::TensorLayoutFindingV1::UnsupportedSymbolicMap { .. },
                        ..
                    }
                )
            })
        }
        ProductionPlironPreloweringErrorV2::TargetContract(_)
        | ProductionPlironPreloweringErrorV2::Atomic(_)
        | ProductionPlironPreloweringErrorV2::Race(_)
        | ProductionPlironPreloweringErrorV2::Ownership(_)
        | ProductionPlironPreloweringErrorV2::Barrier(_)
        | ProductionPlironPreloweringErrorV2::PipelineProtocol(_)
        | ProductionPlironPreloweringErrorV2::Workgroup(_)
        | ProductionPlironPreloweringErrorV2::Semantic(_)
        | ProductionPlironPreloweringErrorV2::Preservation(_)
        | ProductionPlironPreloweringErrorV2::ReportValidation(_) => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_clean_schedule(
    canonical: &VerifiedCanonicalKernelIrV13,
    module: &Module,
    subject: &ProductionW4FinalGraphSubjectV1,
    schedule: &[ProductionCapabilityAnalysisStageV1],
    capability: &KernelCapabilityPreservationAnalysisV1,
    execution: &ExecutionCapabilityFinalGraphReportV1,
    target: &ProductionW4TargetResourceInputV1,
    functions: &[ProductionW4FunctionOutcomeV1],
    pliron_epoch: u64,
) -> Result<
    (
        Vec<ProductionW4StageResultV1>,
        ProductionW4AnalysisScheduleWitnessV1,
    ),
    ProductionW4ExecutionErrorV1,
> {
    require_exact_schedule(schedule)?;
    let canonical_is_exact = canonical.revalidate().is_ok()
        && canonical.identity() == subject.final_graph()
        && decode_module_v13(canonical.canonical_bytes()).ok().as_ref() == Some(module)
        && execution.canonical_identity() == subject.final_graph()
        && execution.final_epoch() == subject.final_epoch();
    let target_is_exact = target.final_graph() == subject.final_graph()
        && target.final_epoch() == subject.final_epoch()
        && target.target_identity() == subject.target_identity()
        && target.launch_identity() == subject.launch_identity()
        && target.closure_identity() == subject.target_closure_identity()
        && derive_live_target_closure_identity_v1(target.canonical_closure())
            == *target.closure_identity()
        && derive_target_decision_set_identity_v1(
            target.target_decisions(),
            target.target_identity(),
        )
        .is_ok_and(|(identity, count)| {
            identity == *target.decision_set_identity() && count == target.target_decisions().len()
        });

    let mut results = Vec::with_capacity(schedule.len());
    for stage in schedule {
        let dependencies_are_clean = stage.dependencies().iter().all(|dependency| {
            results.iter().any(|result: &ProductionW4StageResultV1| {
                result.kind == *dependency
                    && matches!(result.outcome, ProductionW4CapabilityOutcomeV1::Clean)
            })
        });
        let exact_evidence_is_clean = match stage.executor() {
            ProductionCapabilityAnalysisExecutorV1::CanonicalKirVerifierV13 => canonical_is_exact,
            ProductionCapabilityAnalysisExecutorV1::KernelCapabilityAffectedAnalysisV1 => {
                capability.graph_epoch() == target.final_epoch()
                    && capability.canonical_identity() == target.final_graph()
            }
            ProductionCapabilityAnalysisExecutorV1::MandatoryTargetCapabilityClosureV1 => {
                target_is_exact
            }
            ProductionCapabilityAnalysisExecutorV1::MandatoryPlironPass(pass) => functions
                .iter()
                .all(|function| exact_pass_evidence_is_clean(&function.checks, pass)),
            ProductionCapabilityAnalysisExecutorV1::EmbeddedInPlironPass(pass) => functions
                .iter()
                .all(|function| exact_pass_evidence_is_clean(&function.checks, pass)),
        };
        let execution_evidence_required = execution_semantics_owns_stage(stage.kind());
        if !dependencies_are_clean
            || !exact_evidence_is_clean
            || execution_evidence_required
                && execution.stage_status(stage.kind()) != KernelCheckStatusV1::Clean
        {
            return Err(ProductionW4ExecutionErrorV1::MissingStageEvidence {
                stage: stage.kind(),
            });
        }
        results.push(ProductionW4StageResultV1 {
            kind: stage.kind(),
            source: match stage.executor() {
                ProductionCapabilityAnalysisExecutorV1::CanonicalKirVerifierV13 => {
                    ProductionW4OutcomeSourceV1::CanonicalKirV13
                }
                ProductionCapabilityAnalysisExecutorV1::KernelCapabilityAffectedAnalysisV1 => {
                    ProductionW4OutcomeSourceV1::CapabilityProvenanceReplayV1
                }
                ProductionCapabilityAnalysisExecutorV1::MandatoryTargetCapabilityClosureV1 => {
                    ProductionW4OutcomeSourceV1::TargetResourceClosureV1
                }
                ProductionCapabilityAnalysisExecutorV1::MandatoryPlironPass(pass) => {
                    if execution_evidence_required {
                        ProductionW4OutcomeSourceV1::CanonicalExecutionCapabilityAndPlironPass(pass)
                    } else {
                        ProductionW4OutcomeSourceV1::PlironPass(pass)
                    }
                }
                ProductionCapabilityAnalysisExecutorV1::EmbeddedInPlironPass(pass) => {
                    if execution_evidence_required {
                        ProductionW4OutcomeSourceV1::CanonicalExecutionCapabilityAndEmbeddedPlironPass(
                            pass,
                        )
                    } else {
                        ProductionW4OutcomeSourceV1::EmbeddedPlironPass(pass)
                    }
                }
            },
            outcome: ProductionW4CapabilityOutcomeV1::Clean,
        });
    }
    let obligations = execute_analysis_obligation_schedule(
        subject,
        target,
        execution,
        functions,
        pliron_epoch,
        &results,
    )?;
    Ok((results, obligations))
}

fn execute_analysis_obligation_schedule(
    subject: &ProductionW4FinalGraphSubjectV1,
    target: &ProductionW4TargetResourceInputV1,
    execution: &ExecutionCapabilityFinalGraphReportV1,
    functions: &[ProductionW4FunctionOutcomeV1],
    pliron_epoch: u64,
    owners: &[ProductionW4StageResultV1],
) -> Result<ProductionW4AnalysisScheduleWitnessV1, ProductionW4ExecutionErrorV1> {
    validate_production_w4_analysis_obligation_schedule_v1(
        &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
    )
    .map_err(ProductionW4ExecutionErrorV1::ObligationSchedule)?;
    let mut obligations = Vec::with_capacity(PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1);
    for obligation in PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
        .iter()
        .flat_map(|stage| stage.obligations().iter().copied())
    {
        let owner = owners.iter().find(|result| {
            result.kind == obligation.owner()
                && matches!(result.outcome, ProductionW4CapabilityOutcomeV1::Clean)
        });
        let dependencies_are_exact = obligation.dependencies().iter().all(|dependency| {
            obligations
                .iter()
                .any(|result: &ProductionW4AnalysisObligationResultV1| result.kind == *dependency)
        });
        if owner.is_none() || !dependencies_are_exact {
            return Err(ProductionW4ExecutionErrorV1::MissingStageEvidence {
                stage: obligation.owner(),
            });
        }
        let evidence = derive_independent_obligation_evidence(
            subject,
            target,
            pliron_epoch,
            obligation,
            owner.expect("owner checked above"),
            &obligations,
            execution,
            functions,
        );
        let evidence = evidence.ok_or(ProductionW4ExecutionErrorV1::MissingStageEvidence {
            stage: obligation.owner(),
        })?;
        obligations.push(ProductionW4AnalysisObligationResultV1 {
            kind: obligation.kind(),
            owner: obligation.owner(),
            evidence,
        });
    }
    let obligations = obligations.into_boxed_slice();
    let identity = analysis_schedule_witness_identity(subject, target, pliron_epoch, &obligations);
    Ok(ProductionW4AnalysisScheduleWitnessV1 {
        final_graph: *subject.final_graph(),
        final_epoch: subject.final_epoch(),
        pliron_epoch,
        checker_identity: *subject.checker_identity(),
        target_closure_identity: *target.closure_identity(),
        obligations,
        identity,
    })
}

#[allow(clippy::too_many_arguments)]
fn derive_independent_obligation_evidence(
    subject: &ProductionW4FinalGraphSubjectV1,
    target: &ProductionW4TargetResourceInputV1,
    pliron_epoch: u64,
    obligation: ProductionW4AnalysisObligationStageV1,
    owner: &ProductionW4StageResultV1,
    dependencies: &[ProductionW4AnalysisObligationResultV1],
    execution: &ExecutionCapabilityFinalGraphReportV1,
    functions: &[ProductionW4FunctionOutcomeV1],
) -> Option<ProductionW4IndependentObligationEvidenceV1> {
    if !matches!(owner.outcome, ProductionW4CapabilityOutcomeV1::Clean)
        || owner.kind != obligation.owner()
        || execution.canonical_identity() != subject.final_graph()
        || execution.final_epoch() != subject.final_epoch()
        || execution.status() != KernelCheckStatusV1::Clean
    {
        return None;
    }

    let checker = independent_obligation_checker(obligation.kind());
    let required_passes = independent_obligation_passes(obligation.kind());
    let mut pass_evidence = Vec::with_capacity(functions.len() * required_passes.len());
    for function in functions {
        if function.checks.preservation().input_mutation_epoch() != pliron_epoch
            || function.checks.preservation().output_mutation_epoch() != pliron_epoch
        {
            return None;
        }
        for pass in required_passes {
            let validation = exact_pass_validation(&function.checks, *pass, pliron_epoch)?;
            pass_evidence.push(ProductionW4IndependentPassEvidenceV1 {
                function: function.function.clone(),
                structural_sha256: *function.structural_identity.sha256(),
                structural_bytes: function.structural_identity.canonical_bytes_len(),
                pass: *pass,
                checker: validation.witness().checker(),
                checkpoint_position: validation.checkpoint().position(),
                checkpoint_epoch: validation.checkpoint().mutation_epoch(),
                checked_units: validation.witness().coverage().obligation_count(),
            });
        }
    }
    if !independent_obligation_reports_are_clean(obligation.kind(), execution, functions) {
        return None;
    }

    let declared_effect_contracts = functions
        .iter()
        .map(|function| function.effect_refinement().contract_count())
        .sum();
    let proved_effect_contracts = functions
        .iter()
        .map(|function| function.effect_refinement().proved_contract_count())
        .sum();
    let coverage = ProductionW4IndependentObligationCoverageV1 {
        checked_kir_functions: execution.checked_functions(),
        checked_kir_operations: execution.checked_operations(),
        checked_barrier_sites: execution.checked_barrier_sites(),
        checked_collective_sites: execution.checked_collective_sites(),
        checked_epoch_transitions: execution.checked_epoch_transitions(),
        checked_memory_effects: execution.checked_memory_effects(),
        checked_pliron_units: pass_evidence
            .iter()
            .map(|evidence| evidence.checked_units)
            .sum(),
        declared_effect_contracts,
        proved_effect_contracts,
    };
    let identity = analysis_obligation_evidence_identity(
        subject,
        target,
        pliron_epoch,
        obligation,
        owner,
        dependencies,
        checker,
        &pass_evidence,
        coverage,
    );
    Some(ProductionW4IndependentObligationEvidenceV1 {
        final_graph: *subject.final_graph(),
        final_epoch: subject.final_epoch(),
        pliron_epoch,
        checker,
        pass_evidence: pass_evidence.into_boxed_slice(),
        coverage,
        identity,
    })
}

fn exact_pass_validation<'a>(
    reports: &'a ProductionPlironPreloweringReportV2,
    pass: KernelCheckPassKindV1,
    pliron_epoch: u64,
) -> Option<&'a crate::ProductionAnalysisStageValidationV1> {
    reports
        .report_validation()
        .stages()
        .iter()
        .find(|evidence| {
            evidence.checkpoint().pass() == pass
                && evidence.implementation().pass() == pass
                && evidence.checkpoint().mutation_epoch() == pliron_epoch
                && evidence.analysis_status() == KernelCheckStatusV1::Clean
                && evidence.independent_validation_status() == KernelCheckStatusV1::Clean
                && evidence.remaining_witness_gap().is_none()
                && evidence.witness().coverage().is_complete()
        })
}

fn independent_obligation_reports_are_clean(
    obligation: ProductionW4AnalysisObligationKindV1,
    execution: &ExecutionCapabilityFinalGraphReportV1,
    functions: &[ProductionW4FunctionOutcomeV1],
) -> bool {
    use ProductionW4AnalysisObligationKindV1 as Obligation;
    match obligation {
        Obligation::CanonicalTyping
        | Obligation::CapabilityProvenance
        | Obligation::ResourceLegality => true,
        Obligation::Uniformity => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::Uniformity)
                == KernelCheckStatusV1::Clean
                && functions
                    .iter()
                    .all(|function| function.uniformity_and_barriers().is_clean())
        }
        Obligation::TensorLayout => functions
            .iter()
            .all(|function| function.tensor_layout().is_clean()),
        Obligation::MemoryBounds => functions
            .iter()
            .all(|function| function.bounds().is_clean()),
        Obligation::AtomicLegality => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::AtomicLegality)
                == KernelCheckStatusV1::Clean
                && functions
                    .iter()
                    .all(|function| function.atomic_legality().is_clean())
        }
        Obligation::HappensBefore => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::MemoryVisibility)
                == KernelCheckStatusV1::Clean
                && functions.iter().all(|function| {
                    function.race_freedom().is_clean()
                        && !function.race_freedom().findings().iter().any(|finding| {
                            matches!(
                                finding,
                                crate::RankedRaceFindingV1::HappensBeforeIncomplete { .. }
                            )
                        })
                })
        }
        Obligation::RaceFreedom => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::RaceFreedom)
                == KernelCheckStatusV1::Clean
                && functions
                    .iter()
                    .all(|function| function.race_freedom().is_clean())
        }
        Obligation::HierarchicalOwnership => functions
            .iter()
            .all(|function| function.hierarchical_ownership().is_clean()),
        Obligation::BarrierConvergence | Obligation::BarrierOrder => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::BarrierConvergence)
                == KernelCheckStatusV1::Clean
                && functions
                    .iter()
                    .all(|function| function.uniformity_and_barriers().is_clean())
        }
        Obligation::PipelineProtocol => functions
            .iter()
            .all(|function| function.pipeline_protocol().is_clean()),
        Obligation::Initialization => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::Initialization)
                == KernelCheckStatusV1::Clean
                && functions.iter().all(|function| {
                    function.initialization_visibility_and_epochs().is_clean()
                        && !function
                            .initialization_visibility_and_epochs()
                            .findings()
                            .iter()
                            .any(|finding| {
                                matches!(
                                    finding,
                                    PlironWorkgroupMemoryFindingV1::ReadBeforeInitialization { .. }
                                )
                            })
                })
        }
        Obligation::MemoryVisibility => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::MemoryVisibility)
                == KernelCheckStatusV1::Clean
                && functions.iter().all(|function| {
                    function.race_freedom().is_clean()
                        && function.initialization_visibility_and_epochs().is_clean()
                })
        }
        Obligation::WorkgroupMemoryEpochs => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs)
                == KernelCheckStatusV1::Clean
                && functions.iter().all(|function| {
                    function.pipeline_protocol().is_clean()
                        && function.initialization_visibility_and_epochs().is_clean()
                })
        }
        Obligation::CollectiveParticipation => {
            execution.stage_status(ProductionCapabilityAnalysisKindV1::BarrierConvergence)
                == KernelCheckStatusV1::Clean
                && functions
                    .iter()
                    .all(|function| function.uniformity_and_barriers().is_clean())
        }
        Obligation::EffectRefinement => functions.iter().all(|function| {
            let effect = function.effect_refinement();
            effect.is_clean() && effect.contract_count() == effect.proved_contract_count()
        }),
        Obligation::SemanticRefinement => functions
            .iter()
            .all(|function| function.semantic_refinement().is_clean()),
    }
}

fn independent_obligation_checker(
    obligation: ProductionW4AnalysisObligationKindV1,
) -> ProductionW4IndependentObligationCheckerV1 {
    use ProductionW4AnalysisObligationKindV1 as Obligation;
    use ProductionW4IndependentObligationCheckerV1 as Checker;
    match obligation {
        Obligation::CanonicalTyping => Checker::CanonicalTypingV13,
        Obligation::CapabilityProvenance => Checker::CapabilityProvenanceV1,
        Obligation::ResourceLegality => Checker::TargetResourceClosureV1,
        Obligation::Uniformity => Checker::UniformityFreshLiveIrV1,
        Obligation::TensorLayout => Checker::TensorLayoutFreshLiveIrV1,
        Obligation::MemoryBounds => Checker::MemoryBoundsFreshLiveIrV1,
        Obligation::AtomicLegality => Checker::AtomicLegalityFreshLiveIrV1,
        Obligation::HappensBefore => Checker::HappensBeforeFreshLiveIrV1,
        Obligation::RaceFreedom => Checker::RaceFreedomFreshLiveIrV1,
        Obligation::HierarchicalOwnership => Checker::HierarchicalOwnershipFreshLiveIrV1,
        Obligation::BarrierConvergence => Checker::BarrierConvergenceFreshLiveIrV1,
        Obligation::BarrierOrder => Checker::BarrierOrderFreshLiveIrV1,
        Obligation::PipelineProtocol => Checker::PipelineProtocolFreshLiveIrV1,
        Obligation::Initialization => Checker::InitializationFreshLiveIrV1,
        Obligation::MemoryVisibility => Checker::MemoryVisibilityFreshLiveIrV1,
        Obligation::WorkgroupMemoryEpochs => Checker::WorkgroupMemoryEpochsFreshLiveIrV1,
        Obligation::CollectiveParticipation => Checker::CollectiveParticipationFreshLiveIrV1,
        Obligation::EffectRefinement => Checker::EffectRefinementFreshLiveIrV1,
        Obligation::SemanticRefinement => Checker::SemanticRefinementFreshLiveIrV1,
    }
}

fn independent_obligation_passes(
    obligation: ProductionW4AnalysisObligationKindV1,
) -> &'static [KernelCheckPassKindV1] {
    use KernelCheckPassKindV1 as Pass;
    use ProductionW4AnalysisObligationKindV1 as Obligation;
    match obligation {
        Obligation::CanonicalTyping
        | Obligation::CapabilityProvenance
        | Obligation::ResourceLegality => &[],
        Obligation::Uniformity
        | Obligation::BarrierConvergence
        | Obligation::BarrierOrder
        | Obligation::CollectiveParticipation => &[Pass::BarrierConvergence],
        Obligation::TensorLayout => &[Pass::TensorLayout],
        Obligation::MemoryBounds => &[Pass::MemoryBounds],
        Obligation::AtomicLegality => &[Pass::AtomicLegality],
        Obligation::HappensBefore | Obligation::RaceFreedom => &[Pass::RaceFreedom],
        Obligation::HierarchicalOwnership => &[Pass::HierarchicalOwnership],
        Obligation::PipelineProtocol => &[Pass::PipelineProtocol],
        Obligation::Initialization => &[Pass::WorkgroupMemory],
        Obligation::MemoryVisibility => &[
            Pass::AtomicLegality,
            Pass::RaceFreedom,
            Pass::BarrierConvergence,
            Pass::WorkgroupMemory,
        ],
        Obligation::WorkgroupMemoryEpochs => &[Pass::PipelineProtocol, Pass::WorkgroupMemory],
        Obligation::EffectRefinement => &[Pass::HierarchicalOwnership, Pass::SemanticRefinement],
        Obligation::SemanticRefinement => &[Pass::SemanticRefinement],
    }
}

#[allow(clippy::too_many_arguments)]
fn analysis_obligation_evidence_identity(
    subject: &ProductionW4FinalGraphSubjectV1,
    target: &ProductionW4TargetResourceInputV1,
    pliron_epoch: u64,
    obligation: ProductionW4AnalysisObligationStageV1,
    owner: &ProductionW4StageResultV1,
    dependencies: &[ProductionW4AnalysisObligationResultV1],
    checker: ProductionW4IndependentObligationCheckerV1,
    pass_evidence: &[ProductionW4IndependentPassEvidenceV1],
    coverage: ProductionW4IndependentObligationCoverageV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ANALYSIS_OBLIGATION_EVIDENCE_DOMAIN_V1);
    digest.update(subject.final_graph().digest());
    digest.update(subject.final_graph().canonical_length().to_le_bytes());
    digest.update(subject.final_epoch().to_le_bytes());
    digest.update(pliron_epoch.to_le_bytes());
    digest.update(subject.checker_identity());
    digest.update(target.closure_identity());
    digest.update([analysis_obligation_tag(obligation.kind())]);
    digest.update([kind_tag(obligation.owner()), source_tag(owner.source)]);
    digest.update([independent_obligation_checker_tag(checker)]);
    for dependency in obligation.dependencies() {
        let evidence = dependencies
            .iter()
            .find(|result| result.kind == *dependency)
            .expect("topological dependency checked before identity construction");
        digest.update(evidence.evidence_identity());
    }
    digest.update((pass_evidence.len() as u64).to_le_bytes());
    for evidence in pass_evidence {
        digest.update(evidence.function.as_str().as_bytes());
        digest.update([0]);
        digest.update(evidence.structural_sha256);
        digest.update(evidence.structural_bytes.to_le_bytes());
        digest.update([
            pass_tag(evidence.pass),
            witness_checker_tag(evidence.checker),
        ]);
        digest.update(evidence.checkpoint_position.to_le_bytes());
        digest.update(evidence.checkpoint_epoch.to_le_bytes());
        digest.update(evidence.checked_units.to_le_bytes());
    }
    for value in independent_coverage_values(coverage) {
        digest.update(value.to_le_bytes());
    }
    digest.finalize().into()
}

const fn independent_coverage_values(
    coverage: ProductionW4IndependentObligationCoverageV1,
) -> [usize; 9] {
    [
        coverage.checked_kir_functions,
        coverage.checked_kir_operations,
        coverage.checked_barrier_sites,
        coverage.checked_collective_sites,
        coverage.checked_epoch_transitions,
        coverage.checked_memory_effects,
        coverage.checked_pliron_units,
        coverage.declared_effect_contracts,
        coverage.proved_effect_contracts,
    ]
}

fn analysis_schedule_witness_identity(
    subject: &ProductionW4FinalGraphSubjectV1,
    target: &ProductionW4TargetResourceInputV1,
    pliron_epoch: u64,
    obligations: &[ProductionW4AnalysisObligationResultV1],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(ANALYSIS_SCHEDULE_WITNESS_DOMAIN_V1);
    digest.update(subject.final_graph().digest());
    digest.update(subject.final_graph().canonical_length().to_le_bytes());
    digest.update(subject.final_epoch().to_le_bytes());
    digest.update(pliron_epoch.to_le_bytes());
    digest.update(subject.checker_identity());
    digest.update(target.closure_identity());
    digest.update((obligations.len() as u64).to_le_bytes());
    for obligation in obligations {
        digest.update([analysis_obligation_tag(obligation.kind)]);
        digest.update([kind_tag(obligation.owner)]);
        digest.update(obligation.evidence_identity());
    }
    digest.finalize().into()
}

fn analysis_schedule_witness_is_exact(
    witness: &ProductionW4AnalysisScheduleWitnessV1,
    subject: &ProductionW4FinalGraphSubjectV1,
    target: &ProductionW4TargetResourceInputV1,
    pliron_epoch: u64,
    owners: &[ProductionW4StageResultV1],
    execution: &ExecutionCapabilityFinalGraphReportV1,
    functions: &[ProductionW4FunctionOutcomeV1],
) -> bool {
    if witness.final_graph() != subject.final_graph()
        || witness.final_epoch() != subject.final_epoch()
        || witness.pliron_epoch() != pliron_epoch
        || witness.checker_identity() != subject.checker_identity()
        || witness.target_closure_identity() != target.closure_identity()
        || witness.obligations().len() != PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1
    {
        return false;
    }
    let mut prior = Vec::with_capacity(witness.obligations().len());
    for (expected, observed) in PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
        .iter()
        .flat_map(|stage| stage.obligations().iter().copied())
        .zip(witness.obligations())
    {
        let Some(owner) = owners.iter().find(|result| {
            result.kind == expected.owner()
                && matches!(result.outcome, ProductionW4CapabilityOutcomeV1::Clean)
        }) else {
            return false;
        };
        let Some(evidence) = derive_independent_obligation_evidence(
            subject,
            target,
            pliron_epoch,
            expected,
            owner,
            &prior,
            execution,
            functions,
        ) else {
            return false;
        };
        if observed.kind() != expected.kind()
            || observed.owner() != expected.owner()
            || observed.independent_evidence() != &evidence
        {
            return false;
        }
        prior.push(ProductionW4AnalysisObligationResultV1 {
            kind: observed.kind(),
            owner: observed.owner(),
            evidence,
        });
    }
    witness.identity()
        == &analysis_schedule_witness_identity(subject, target, pliron_epoch, witness.obligations())
}

const fn execution_semantics_owns_stage(kind: ProductionCapabilityAnalysisKindV1) -> bool {
    matches!(
        kind,
        ProductionCapabilityAnalysisKindV1::Uniformity
            | ProductionCapabilityAnalysisKindV1::AtomicLegality
            | ProductionCapabilityAnalysisKindV1::RaceFreedom
            | ProductionCapabilityAnalysisKindV1::BarrierConvergence
            | ProductionCapabilityAnalysisKindV1::Initialization
            | ProductionCapabilityAnalysisKindV1::MemoryVisibility
            | ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs
    )
}

fn exact_pass_evidence_is_clean(
    reports: &ProductionPlironPreloweringReportV2,
    pass: KernelCheckPassKindV1,
) -> bool {
    let report_clean = match pass {
        KernelCheckPassKindV1::TensorLayout => reports.tensor_layout().is_clean(),
        KernelCheckPassKindV1::MemoryBounds => reports.bounds().is_clean(),
        KernelCheckPassKindV1::AtomicLegality => reports.atomics().is_clean(),
        KernelCheckPassKindV1::RaceFreedom => reports.race().is_clean(),
        KernelCheckPassKindV1::HierarchicalOwnership => reports.ownership().is_clean(),
        KernelCheckPassKindV1::BarrierConvergence => reports.barriers().is_clean(),
        KernelCheckPassKindV1::PipelineProtocol => reports.pipeline_protocol().is_clean(),
        KernelCheckPassKindV1::WorkgroupMemory => reports.workgroup().is_clean(),
        KernelCheckPassKindV1::SemanticRefinement => reports.semantics().is_clean(),
        KernelCheckPassKindV1::Structural | KernelCheckPassKindV1::ControlFlow => false,
    };
    report_clean
        && reports.report_validation().stages().iter().any(|evidence| {
            evidence.checkpoint().pass() == pass
                && evidence.implementation().pass() == pass
                && evidence.analysis_status() == KernelCheckStatusV1::Clean
                && evidence.independent_validation_status() == KernelCheckStatusV1::Clean
                && evidence.remaining_witness_gap().is_none()
                && evidence.witness().coverage().is_complete()
        })
}

fn non_clean_without_pipeline(
    subject: ProductionW4FinalGraphSubjectV1,
    stage: ProductionCapabilityAnalysisKindV1,
    function: Option<FunctionId>,
    status: KernelCheckStatusV1,
    detail: String,
) -> Result<ProductionW4FinalGraphExecutionV1, ProductionW4ExecutionErrorV1> {
    let mut diagnostic = bounded_diagnostic(&subject, stage, function, detail);
    let outcome = match status {
        KernelCheckStatusV1::Rejected => {
            let (obligation, class) = stage_counterexample_classification(stage);
            attach_counterexample(&mut diagnostic, obligation, class, None, None);
            ProductionW4CapabilityOutcomeV1::Rejected(diagnostic)
        }
        KernelCheckStatusV1::Clean | KernelCheckStatusV1::Incomplete => {
            ProductionW4CapabilityOutcomeV1::Incomplete(diagnostic)
        }
    };
    build_non_clean_result(subject, outcome, None)
}

fn stage_counterexample_classification(
    stage: ProductionCapabilityAnalysisKindV1,
) -> (
    ProductionW4AnalysisObligationKindV1,
    ProductionW4CounterexampleClassV1,
) {
    use ProductionCapabilityAnalysisKindV1 as Stage;
    use ProductionW4AnalysisObligationKindV1 as Obligation;
    use ProductionW4CounterexampleClassV1 as Class;
    match stage {
        Stage::CanonicalTyping | Stage::CapabilityProvenance => {
            (Obligation::CanonicalTyping, Class::CanonicalExecution)
        }
        Stage::ResourceLegality => (Obligation::ResourceLegality, Class::ResourceLegality),
        Stage::Uniformity => (Obligation::Uniformity, Class::Uniformity),
        Stage::TensorLayout => (Obligation::TensorLayout, Class::TensorLayout),
        Stage::MemoryBounds => (Obligation::MemoryBounds, Class::MemoryBounds),
        Stage::AtomicLegality => (Obligation::AtomicLegality, Class::AtomicLegality),
        Stage::RaceFreedom => (Obligation::RaceFreedom, Class::RaceConflict),
        Stage::HierarchicalOwnership => {
            (Obligation::HierarchicalOwnership, Class::OwnershipConflict)
        }
        Stage::BarrierConvergence => (Obligation::BarrierOrder, Class::BarrierOrder),
        Stage::PipelineProtocol | Stage::WorkgroupMemoryEpochs => (
            Obligation::WorkgroupMemoryEpochs,
            Class::PipelineEpochProtocol,
        ),
        Stage::Initialization => (Obligation::Initialization, Class::ReadBeforeInitialization),
        Stage::MemoryVisibility => (Obligation::MemoryVisibility, Class::MemoryVisibility),
        Stage::EffectRefinement => (Obligation::EffectRefinement, Class::EffectRefinement),
        Stage::SemanticRefinement => (Obligation::SemanticRefinement, Class::SemanticRefinement),
    }
}

fn non_clean_unsupported_without_pipeline(
    subject: ProductionW4FinalGraphSubjectV1,
    stage: ProductionCapabilityAnalysisKindV1,
    function: Option<FunctionId>,
    detail: String,
) -> Result<ProductionW4FinalGraphExecutionV1, ProductionW4ExecutionErrorV1> {
    let diagnostic = bounded_diagnostic(&subject, stage, function, detail);
    build_non_clean_result(
        subject,
        ProductionW4CapabilityOutcomeV1::Unsupported(diagnostic),
        None,
    )
}

fn build_non_clean_result(
    subject: ProductionW4FinalGraphSubjectV1,
    outcome: ProductionW4CapabilityOutcomeV1,
    pipeline_error: Option<Box<ProductionPlironPreloweringErrorV2>>,
) -> Result<ProductionW4FinalGraphExecutionV1, ProductionW4ExecutionErrorV1> {
    let counterexample_is_valid = match &outcome {
        ProductionW4CapabilityOutcomeV1::Rejected(diagnostic) => {
            diagnostic.counterexample().is_some()
        }
        ProductionW4CapabilityOutcomeV1::Clean
        | ProductionW4CapabilityOutcomeV1::Incomplete(_)
        | ProductionW4CapabilityOutcomeV1::Unsupported(_) => {
            outcome_counterexample(&outcome).is_none()
        }
    };
    if !counterexample_is_valid {
        return Err(ProductionW4ExecutionErrorV1::InvalidDiagnosticClassification);
    }
    let canonical_encoding = encode_non_clean_witness(&subject, &outcome)?;
    let identity = witness_identity(&canonical_encoding);
    Ok(ProductionW4FinalGraphExecutionV1::NonClean(
        ProductionW4NonCleanResultV1 {
            subject,
            outcome,
            pipeline_error,
            canonical_encoding: canonical_encoding.into_boxed_slice(),
            identity,
        },
    ))
}

fn outcome_counterexample(
    outcome: &ProductionW4CapabilityOutcomeV1,
) -> Option<&ProductionW4BoundedCounterexampleV1> {
    match outcome {
        ProductionW4CapabilityOutcomeV1::Rejected(diagnostic)
        | ProductionW4CapabilityOutcomeV1::Incomplete(diagnostic)
        | ProductionW4CapabilityOutcomeV1::Unsupported(diagnostic) => diagnostic.counterexample(),
        ProductionW4CapabilityOutcomeV1::Clean => None,
    }
}

fn bounded_diagnostic(
    subject: &ProductionW4FinalGraphSubjectV1,
    stage: ProductionCapabilityAnalysisKindV1,
    function: Option<FunctionId>,
    detail: String,
) -> ProductionW4BoundedDiagnosticV1 {
    let (detail, truncated) = truncate_utf8(detail, MAX_PRODUCTION_W4_DIAGNOSTIC_BYTES_V1);
    ProductionW4BoundedDiagnosticV1 {
        stage,
        function,
        final_graph: *subject.final_graph(),
        final_epoch: subject.final_epoch(),
        source_map_identity: subject
            .diagnostic_source_map()
            .map(|source_map| *source_map.identity()),
        counterexample: None,
        detail,
        truncated,
    }
}

fn attach_counterexample(
    diagnostic: &mut ProductionW4BoundedDiagnosticV1,
    obligation: ProductionW4AnalysisObligationKindV1,
    class: ProductionW4CounterexampleClassV1,
    primary: Option<ProductionW4CounterexampleLocationV1>,
    related: Option<ProductionW4CounterexampleLocationV1>,
) {
    let (detail, truncated) = truncate_utf8(
        diagnostic.detail.clone(),
        MAX_PRODUCTION_W4_DIAGNOSTIC_BYTES_V1,
    );
    diagnostic.counterexample = Some(ProductionW4BoundedCounterexampleV1 {
        obligation,
        class,
        primary,
        related,
        detail,
        truncated,
    });
}

fn truncate_utf8(mut value: String, limit: usize) -> (String, bool) {
    if value.len() <= limit {
        return (value, false);
    }
    let mut boundary = limit;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    (value, true)
}

fn current_pliron_epoch(context: &Context) -> Result<u64, ProductionW4ExecutionErrorV1> {
    context
        .ir_mutation_attempt_epoch()
        .map(|epoch| epoch.value())
        .map_err(|_| ProductionW4ExecutionErrorV1::MutationEpochUnavailable)
}

struct WitnessEncodingAnalysisV1<'a> {
    capability: &'a KernelCapabilityPreservationAnalysisV1,
    execution: &'a ExecutionCapabilityFinalGraphReportV1,
    typed_safety: &'a ProductionW4TypedSafetyInventoryV1,
    pliron_epoch: u64,
    stages: &'a [ProductionW4StageResultV1],
    analysis_schedule: &'a ProductionW4AnalysisScheduleWitnessV1,
    functions: &'a [ProductionW4FunctionOutcomeV1],
}

fn encode_witness(
    canonical_kir: &[u8],
    subject: &ProductionW4FinalGraphSubjectV1,
    target: &ProductionW4TargetResourceInputV1,
    analysis: WitnessEncodingAnalysisV1<'_>,
) -> Result<Vec<u8>, ProductionW4ExecutionErrorV1> {
    let mut writer = BoundedWriter::new(MAX_PRODUCTION_W4_WITNESS_ENCODING_BYTES_V1);
    writer.bytes(WITNESS_DOMAIN_V1)?;
    writer.u16(PRODUCTION_W4_FINAL_GRAPH_CAPABILITY_WITNESS_VERSION_V1)?;
    writer.u8(0)?;
    encode_subject(&mut writer, subject)?;
    writer.sized_bytes(canonical_kir)?;
    writer.sized_bytes(target.canonical_closure())?;
    encode_target_decisions(&mut writer, target)?;
    encode_atomic_target(&mut writer, target.atomic_target())?;
    encode_launch_contract(&mut writer, target.launch_contract())?;
    writer.u64(analysis.capability.graph_epoch())?;
    writer.identity(analysis.execution.canonical_identity())?;
    writer.u64(analysis.execution.final_epoch())?;
    writer.usize(analysis.execution.checked_roots())?;
    writer.usize(analysis.execution.checked_functions())?;
    writer.usize(analysis.execution.checked_operations())?;
    writer.usize(analysis.execution.checked_barrier_sites())?;
    writer.usize(analysis.execution.checked_collective_sites())?;
    writer.usize(analysis.execution.checked_epoch_transitions())?;
    writer.usize(analysis.execution.checked_atomic_operations())?;
    writer.usize(analysis.execution.checked_memory_effects())?;
    writer.usize(analysis.execution.checked_sequence_states())?;
    writer.u8(u8::from(
        analysis
            .execution
            .conflicting_effects_composed_with_retained_pliron(),
    ))?;
    writer.usize(analysis.execution.findings().len())?;
    writer.u8(status_tag(analysis.execution.status()))?;
    writer.bytes(analysis.typed_safety.identity())?;
    writer.u32(analysis.typed_safety.operation_count())?;
    writer.u16(analysis.typed_safety.required_stage_mask())?;
    for family in CanonicalKirSafetyFamilyV1::ALL {
        writer.u32(analysis.typed_safety.family_count(family))?;
    }
    writer.u64(analysis.pliron_epoch)?;
    writer.usize(analysis.stages.len())?;
    for stage in analysis.stages {
        writer.u8(kind_tag(stage.kind))?;
        writer.u8(source_tag(stage.source))?;
        writer.u8(status_tag(stage.outcome.status()))?;
    }
    writer.identity(analysis.analysis_schedule.final_graph())?;
    writer.u64(analysis.analysis_schedule.final_epoch())?;
    writer.u64(analysis.analysis_schedule.pliron_epoch())?;
    writer.bytes(analysis.analysis_schedule.checker_identity())?;
    writer.bytes(analysis.analysis_schedule.target_closure_identity())?;
    writer.bytes(analysis.analysis_schedule.identity())?;
    writer.usize(analysis.analysis_schedule.obligations().len())?;
    for obligation in analysis.analysis_schedule.obligations() {
        writer.u8(analysis_obligation_tag(obligation.kind()))?;
        writer.u8(kind_tag(obligation.owner()))?;
        encode_independent_obligation_evidence(&mut writer, obligation.independent_evidence())?;
    }
    writer.usize(analysis.functions.len())?;
    for function in analysis.functions {
        writer.string(function.function.as_str())?;
        writer.bytes(function.structural_identity.sha256())?;
        writer.usize(function.structural_identity.canonical_bytes_len())?;
        writer.usize(function.structural_identity.block_count())?;
        writer.usize(function.structural_identity.operation_count())?;
        writer.usize(function.structural_identity.value_count())?;
        encode_function_reports(&mut writer, &function.checks)?;
        let preservation = function.checks.preservation();
        writer.bytes(preservation.input_identity().sha256())?;
        writer.usize(preservation.input_identity().canonical_len())?;
        writer.bytes(preservation.output_identity().sha256())?;
        writer.usize(preservation.output_identity().canonical_len())?;
        writer.u64(preservation.input_mutation_epoch())?;
        writer.u64(preservation.output_mutation_epoch())?;
        writer.usize(preservation.certificates().len())?;
        for certificate in preservation.certificates() {
            writer.u8(pass_tag(certificate.pass()))?;
            writer.bytes(certificate.identity().sha256())?;
            writer.usize(certificate.identity().canonical_len())?;
            writer.u64(certificate.mutation_epoch())?;
        }
        let validations = function.checks.report_validation().stages();
        writer.usize(validations.len())?;
        for validation in validations {
            writer.usize(validation.checkpoint().position())?;
            writer.u8(pass_tag(validation.checkpoint().pass()))?;
            writer.u8(pass_tag(validation.implementation().pass()))?;
            writer.bytes(validation.checkpoint().identity_label().sha256())?;
            writer.usize(validation.checkpoint().identity_label().canonical_len())?;
            writer.u64(validation.checkpoint().mutation_epoch())?;
            writer.u8(status_tag(validation.analysis_status()))?;
            encode_analysis_configuration(&mut writer, validation.configuration())?;
            writer.u8(witness_checker_tag(validation.witness().checker()))?;
            encode_witness_coverage(&mut writer, validation.witness().coverage())?;
        }
    }
    finish_canonical_encoding(writer)
}

fn encode_independent_obligation_evidence(
    writer: &mut BoundedWriter,
    evidence: &ProductionW4IndependentObligationEvidenceV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    writer.identity(evidence.final_graph())?;
    writer.u64(evidence.final_epoch())?;
    writer.u64(evidence.pliron_epoch())?;
    writer.u8(independent_obligation_checker_tag(evidence.checker()))?;
    writer.usize(evidence.pass_evidence().len())?;
    for pass in evidence.pass_evidence() {
        writer.string(pass.function().as_str())?;
        writer.bytes(pass.structural_sha256())?;
        writer.usize(pass.structural_bytes())?;
        writer.u8(pass_tag(pass.pass()))?;
        writer.u8(witness_checker_tag(pass.checker()))?;
        writer.usize(pass.checkpoint_position())?;
        writer.u64(pass.checkpoint_epoch())?;
        writer.usize(pass.checked_units())?;
    }
    for value in independent_coverage_values(evidence.coverage()) {
        writer.usize(value)?;
    }
    writer.bytes(evidence.identity())
}

fn encode_non_clean_witness(
    subject: &ProductionW4FinalGraphSubjectV1,
    outcome: &ProductionW4CapabilityOutcomeV1,
) -> Result<Vec<u8>, ProductionW4ExecutionErrorV1> {
    let mut writer = BoundedWriter::new(MAX_PRODUCTION_W4_WITNESS_ENCODING_BYTES_V1);
    writer.bytes(WITNESS_DOMAIN_V1)?;
    writer.u16(PRODUCTION_W4_FINAL_GRAPH_CAPABILITY_WITNESS_VERSION_V1)?;
    writer.u8(1)?;
    encode_subject(&mut writer, subject)?;
    match outcome {
        ProductionW4CapabilityOutcomeV1::Clean => writer.u8(0)?,
        ProductionW4CapabilityOutcomeV1::Incomplete(diagnostic) => {
            writer.u8(1)?;
            encode_diagnostic(&mut writer, diagnostic)?;
        }
        ProductionW4CapabilityOutcomeV1::Rejected(diagnostic) => {
            writer.u8(2)?;
            encode_diagnostic(&mut writer, diagnostic)?;
        }
        ProductionW4CapabilityOutcomeV1::Unsupported(diagnostic) => {
            writer.u8(3)?;
            encode_diagnostic(&mut writer, diagnostic)?;
        }
    }
    finish_canonical_encoding(writer)
}

fn encode_subject(
    writer: &mut BoundedWriter,
    subject: &ProductionW4FinalGraphSubjectV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    writer.identity(subject.pre_optimization_graph())?;
    writer.u64(subject.pre_optimization_epoch())?;
    writer.identity(subject.final_graph())?;
    writer.u64(subject.final_epoch())?;
    writer.bytes(subject.policy_identity())?;
    writer.bytes(subject.functional_refinement_identity())?;
    writer.bytes(subject.checker_identity())?;
    writer.bytes(subject.target_identity())?;
    writer.bytes(subject.launch_identity())?;
    writer.bytes(subject.target_closure_identity())?;
    match subject.diagnostic_source_map() {
        Some(source_map) => {
            writer.u8(1)?;
            writer.bytes(source_map.identity())?;
            writer.u64(source_map.canonical_length())?;
        }
        None => writer.u8(0)?,
    }
    writer.usize(subject.kernel_roots.len())?;
    for root in &subject.kernel_roots {
        writer.string(root.kernel.as_str())?;
        writer.string(root.entry.as_str())?;
        writer.bytes(&root.launch_identity)?;
    }
    Ok(())
}

fn encode_diagnostic(
    writer: &mut BoundedWriter,
    diagnostic: &ProductionW4BoundedDiagnosticV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    writer.u8(kind_tag(diagnostic.stage))?;
    match &diagnostic.function {
        Some(function) => {
            writer.u8(1)?;
            writer.string(function.as_str())?;
        }
        None => writer.u8(0)?,
    }
    writer.identity(&diagnostic.final_graph)?;
    writer.u64(diagnostic.final_epoch)?;
    match diagnostic.source_map_identity {
        Some(identity) => {
            writer.u8(1)?;
            writer.bytes(&identity)?;
        }
        None => writer.u8(0)?,
    }
    match diagnostic.counterexample() {
        Some(counterexample) => {
            writer.u8(1)?;
            writer.u8(analysis_obligation_tag(counterexample.obligation()))?;
            writer.u8(counterexample_class_tag(counterexample.class()))?;
            encode_counterexample_location(writer, counterexample.primary())?;
            encode_counterexample_location(writer, counterexample.related())?;
            writer.string(counterexample.detail())?;
            writer.bool(counterexample.truncated())?;
        }
        None => writer.u8(0)?,
    }
    writer.string(&diagnostic.detail)?;
    writer.bool(diagnostic.truncated)
}

fn encode_counterexample_location(
    writer: &mut BoundedWriter,
    location: Option<ProductionW4CounterexampleLocationV1>,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    match location {
        Some(location) => {
            writer.u8(1)?;
            writer.usize(location.block())?;
            writer.usize(location.operation())
        }
        None => writer.u8(0),
    }
}

fn encode_target_decisions(
    writer: &mut BoundedWriter,
    target: &ProductionW4TargetResourceInputV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    writer.bytes(target.decision_set_identity())?;
    for word in target.target_model_identity().profile_fingerprint().words() {
        writer.u64(word)?;
    }
    for word in target
        .target_model_identity()
        .revision_fingerprint()
        .words()
    {
        writer.u64(word)?;
    }
    writer.usize(target.target_decisions().len())?;
    for decision in target.target_decisions().iter().copied() {
        let encoded = canonical_target_decision_v1(decision).map_err(|_| {
            ProductionW4ExecutionErrorV1::Encoding(ProductionW4EncodingErrorV1::ResourceLimit {
                resource: "target decision canonical bytes",
                limit: MAX_PRODUCTION_W4_TARGET_DECISION_ENCODING_BYTES_V1,
            })
        })?;
        writer.string(&encoded)?;
    }
    Ok(())
}

fn finish_canonical_encoding(
    writer: BoundedWriter,
) -> Result<Vec<u8>, ProductionW4ExecutionErrorV1> {
    let mut encoded = writer.finish();
    if encoded
        .len()
        .checked_add(32)
        .is_none_or(|length| length > MAX_PRODUCTION_W4_WITNESS_ENCODING_BYTES_V1)
    {
        return Err(ProductionW4ExecutionErrorV1::Encoding(
            ProductionW4EncodingErrorV1::ResourceLimit {
                resource: "canonical witness bytes",
                limit: MAX_PRODUCTION_W4_WITNESS_ENCODING_BYTES_V1,
            },
        ));
    }
    let mut checksum = Sha256::new();
    checksum.update(WITNESS_CHECKSUM_DOMAIN_V1);
    checksum.update(&encoded);
    encoded.extend_from_slice(&<[u8; 32]>::from(checksum.finalize()));
    Ok(encoded)
}

fn encode_atomic_target(
    writer: &mut BoundedWriter,
    target: &PlironAtomicTargetContextV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    writer.usize(target.capabilities().len())?;
    for capability in target.capabilities() {
        writer.u32(capability.element_width())?;
        writer.u8(memory_space_tag(capability.memory_space()))?;
        writer.u8(atomic_scope_tag(capability.max_scope()))?;
    }
    writer.usize(target.system_coherent_allocations().len())?;
    for allocation in target.system_coherent_allocations() {
        writer.u64(*allocation)?;
    }
    Ok(())
}

fn encode_launch_contract(
    writer: &mut BoundedWriter,
    contract: &PlironLaunchContractV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    let limits = contract.limits();
    for extent in limits.max_grid_extents() {
        writer.u64(extent)?;
    }
    for extent in limits.max_workgroup_extents() {
        writer.u64(extent)?;
    }
    writer.u64(limits.max_workgroup_invocations())?;
    writer.usize(limits.supported_subgroup_sizes().len())?;
    for width in limits.supported_subgroup_sizes() {
        writer.u64(*width)?;
    }
    writer.u64(limits.max_workgroup_memory_bytes())?;
    writer.u64(limits.required_global_alignment())?;
    writer.usize(limits.max_global_allocations())?;
    match contract.target_decision_set_identity() {
        Some(identity) => {
            writer.u8(1)?;
            writer.bytes(identity)?;
        }
        None => writer.u8(0)?,
    }
    writer.usize(contract.host_allocations().len())?;
    for allocation in contract.host_allocations() {
        writer.u64(allocation.origin())?;
        encode_optional_u64(writer, allocation.byte_length())?;
        encode_optional_u64(writer, allocation.guaranteed_alignment())?;
        encode_optional_u64(writer, allocation.noalias_class())?;
    }
    Ok(())
}

fn encode_function_reports(
    writer: &mut BoundedWriter,
    reports: &ProductionPlironPreloweringReportV2,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    match reports.target_contract() {
        Some(report) => {
            writer.u8(1)?;
            encode_clean_findings(writer, report.status(), report.findings().len())?;
            encode_optional_u64(writer, report.workgroup_memory_bytes())?;
            writer.usize(report.checked_global_allocation_count())?;
        }
        None => writer.u8(0)?,
    }
    encode_clean_findings(
        writer,
        reports.tensor_layout().status(),
        reports.tensor_layout().findings().len(),
    )?;
    encode_clean_findings(
        writer,
        reports.bounds().status(),
        reports.bounds().findings().len(),
    )?;
    encode_clean_findings(
        writer,
        reports.atomics().status(),
        reports.atomics().findings().len(),
    )?;
    encode_clean_findings(
        writer,
        reports.race().status(),
        reports.race().findings().len(),
    )?;
    encode_ownership_report(writer, reports.ownership())?;
    encode_clean_findings(
        writer,
        reports.barriers().status(),
        reports.barriers().findings().len(),
    )?;
    encode_pipeline_report(writer, reports.pipeline_protocol())?;
    encode_clean_findings(
        writer,
        reports.workgroup().status(),
        reports.workgroup().findings().len(),
    )?;
    encode_semantic_report(writer, reports.semantics())
}

fn encode_clean_findings(
    writer: &mut BoundedWriter,
    status: KernelCheckStatusV1,
    findings: usize,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    writer.u8(status_tag(status))?;
    writer.usize(findings)
}

fn encode_ownership_report(
    writer: &mut BoundedWriter,
    report: &HierarchicalOwnershipReportV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    encode_clean_findings(writer, report.status(), report.findings().len())?;
    writer.usize(report.regions().len())?;
    for region in report.regions() {
        writer.string(region.view())?;
        writer.u8(ownership_coverage_tag(region.coverage()))?;
        match region.identity() {
            HierarchicalRegionIdentityV1::Invocation(invocation) => {
                writer.u8(0)?;
                writer.usize(invocation.len())?;
                for coordinate in invocation {
                    writer.u64(*coordinate)?;
                }
            }
            HierarchicalRegionIdentityV1::Subgroup {
                workgroup,
                subgroup,
            } => {
                writer.u8(1)?;
                writer.u64(*workgroup)?;
                writer.u64(*subgroup)?;
            }
            HierarchicalRegionIdentityV1::Workgroup(workgroup) => {
                writer.u8(2)?;
                writer.u64(*workgroup)?;
            }
            HierarchicalRegionIdentityV1::Grid(grid) => {
                writer.u8(3)?;
                writer.u64(*grid)?;
            }
        }
        writer.usize(region.element_count())?;
        writer.usize(region.bounds().len())?;
        for bound in region.bounds() {
            writer.u64(bound.minimum())?;
            writer.u64(bound.maximum())?;
        }
        writer.bool(region.is_dense_rectangle())?;
    }
    let coverage = report.coverage_summary();
    writer.usize(coverage.total_view_declared())?;
    writer.usize(coverage.total_view_proved())?;
    writer.usize(coverage.collective_contributions_declared())?;
    writer.usize(coverage.collective_contributions_proved())
}

fn encode_pipeline_report(
    writer: &mut BoundedWriter,
    report: &PlironPipelineProtocolReportV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    encode_clean_findings(writer, report.status(), report.findings().len())?;
    writer.usize(report.certificates().len())?;
    for certificate in report.certificates() {
        writer.usize(certificate.pipeline_block())?;
        writer.usize(certificate.pipeline_operation())?;
        writer.u32(certificate.buffers())?;
        writer.u32(certificate.prefetch_distance())?;
        match certificate.dynamic_loop() {
            Some(summary) => {
                writer.u8(1)?;
                writer.usize(summary.prologue())?;
                encode_usize_slice(writer, summary.prologue_blocks())?;
                writer.usize(summary.header())?;
                encode_usize_slice(writer, summary.body())?;
                writer.usize(summary.exit())?;
                encode_usize_slice(writer, summary.drain_blocks())?;
                writer.string(summary.induction())?;
                writer.string(summary.bound())?;
                writer.u64(summary.step())?;
                writer.u32(summary.prefetched_epochs())?;
                writer.u32(summary.live_epoch_window())?;
                writer.u32(summary.drained_epochs())?;
            }
            None => writer.u8(0)?,
        }
        writer.usize(certificate.concrete_epochs())?;
        writer.usize(certificate.staged_writes())?;
        writer.usize(certificate.consuming_reads())?;
        writer.bool(certificate.access_refinement_proven())?;
    }
    Ok(())
}

fn encode_semantic_report(
    writer: &mut BoundedWriter,
    report: &PlironSemanticRefinementReportV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    encode_clean_findings(writer, report.status(), report.findings().len())?;
    writer.usize(report.reference_obligation_count())?;
    writer.usize(report.policy_checked_reference_obligation_count())?;
    writer.usize(report.numerical_obligation_count())?;
    writer.usize(report.policy_checked_numerical_obligation_count())?;
    writer.usize(report.collective_contract_count())?;
    writer.usize(report.policy_checked_collective_contract_count())?;
    writer.usize(report.typed_root_commitments().len())?;
    for commitment in report.typed_root_commitments() {
        for word in commitment {
            writer.u64(*word)?;
        }
    }
    writer.usize(report.numerical_certificates().len())?;
    for certificate in report.numerical_certificates() {
        writer.usize(certificate.block())?;
        writer.usize(certificate.operation())?;
        writer.u64(certificate.derived_absolute_error_f64_bits())?;
        writer.u64(certificate.derived_relative_error_f64_bits())?;
        writer.u64(certificate.requested_absolute_error_f64_bits())?;
        writer.u64(certificate.requested_relative_error_f64_bits())?;
    }
    encode_progress_report(writer, report.progress())?;
    encode_effect_report(writer, report.effect_refinement())
}

fn encode_progress_report(
    writer: &mut BoundedWriter,
    report: &PlironProgressReportV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    encode_clean_findings(writer, report.status(), report.findings().len())?;
    writer.usize(report.certificates().len())?;
    for certificate in report.certificates() {
        writer.usize(certificate.header())?;
        writer.usize(certificate.body())?;
        writer.usize(certificate.exit())?;
        writer.string(certificate.induction())?;
        writer.string(certificate.bound())?;
        writer.u64(certificate.step())?;
    }
    Ok(())
}

fn encode_effect_report(
    writer: &mut BoundedWriter,
    report: &PlironEffectRefinementReportV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    encode_clean_findings(writer, report.status(), report.findings().len())?;
    writer.usize(report.contract_count())?;
    writer.usize(report.proved_contract_count())
}

fn encode_analysis_configuration(
    writer: &mut BoundedWriter,
    configuration: &ProductionAnalysisConfigurationV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    match configuration {
        ProductionAnalysisConfigurationV1::FixedByImplementation => writer.u8(0),
        ProductionAnalysisConfigurationV1::AtomicTargetAgnostic => writer.u8(1),
        ProductionAnalysisConfigurationV1::AtomicTarget {
            capabilities,
            system_coherent_allocations,
        } => {
            writer.u8(2)?;
            writer.usize(capabilities.len())?;
            for capability in capabilities {
                writer.u32(capability.element_width())?;
                writer.u8(memory_space_tag(capability.memory_space()))?;
                writer.u8(atomic_scope_tag(capability.max_scope()))?;
            }
            writer.usize(system_coherent_allocations.len())?;
            for origin in system_coherent_allocations {
                writer.u64(*origin)?;
            }
            Ok(())
        }
    }
}

fn encode_witness_coverage(
    writer: &mut BoundedWriter,
    coverage: &ProductionAnalysisWitnessCoverageV1,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    match coverage {
        ProductionAnalysisWitnessCoverageV1::Complete { obligation_count } => {
            writer.u8(0)?;
            writer.usize(*obligation_count)
        }
        ProductionAnalysisWitnessCoverageV1::Rejected { reason } => {
            writer.u8(1)?;
            writer.string(reason)
        }
        ProductionAnalysisWitnessCoverageV1::Incomplete { gap, reason } => {
            writer.u8(2)?;
            writer.u8(witness_gap_tag(*gap))?;
            writer.string(reason)
        }
    }
}

fn encode_usize_slice(
    writer: &mut BoundedWriter,
    values: &[usize],
) -> Result<(), ProductionW4ExecutionErrorV1> {
    writer.usize(values.len())?;
    for value in values {
        writer.usize(*value)?;
    }
    Ok(())
}

fn encode_optional_u64(
    writer: &mut BoundedWriter,
    value: Option<u64>,
) -> Result<(), ProductionW4ExecutionErrorV1> {
    match value {
        Some(value) => {
            writer.u8(1)?;
            writer.u64(value)
        }
        None => writer.u8(0),
    }
}

fn witness_identity(bytes: &[u8]) -> ProductionW4WitnessIdentityV1 {
    let mut hash = Sha256::new();
    hash.update(WITNESS_DOMAIN_V1);
    hash.update(bytes);
    ProductionW4WitnessIdentityV1 {
        digest: hash.finalize().into(),
        canonical_length: bytes.len() as u64,
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
}

struct BoundedTextWriter {
    value: String,
    limit: usize,
}

impl BoundedTextWriter {
    fn new(limit: usize) -> Self {
        Self {
            value: String::new(),
            limit,
        }
    }

    fn finish(self) -> String {
        self.value
    }
}

impl fmt::Write for BoundedTextWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self
            .value
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > self.limit)
        {
            return Err(fmt::Error);
        }
        self.value.push_str(value);
        Ok(())
    }
}

impl BoundedWriter {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }
    fn reserve(&self, additional: usize) -> Result<(), ProductionW4ExecutionErrorV1> {
        if self
            .bytes
            .len()
            .checked_add(additional)
            .is_none_or(|length| length > self.limit)
        {
            Err(ProductionW4ExecutionErrorV1::Encoding(
                ProductionW4EncodingErrorV1::ResourceLimit {
                    resource: "canonical witness bytes",
                    limit: self.limit,
                },
            ))
        } else {
            Ok(())
        }
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.reserve(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.bytes(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.bytes(&value.to_le_bytes())
    }
    fn bool(&mut self, value: bool) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.u8(u8::from(value))
    }
    fn usize(&mut self, value: usize) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.u64(u64::try_from(value).map_err(|_| {
            ProductionW4ExecutionErrorV1::Encoding(ProductionW4EncodingErrorV1::LengthOverflow)
        })?)
    }
    fn sized_bytes(&mut self, value: &[u8]) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.usize(value.len())?;
        self.bytes(value)
    }
    fn string(&mut self, value: &str) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.sized_bytes(value.as_bytes())
    }
    fn identity(
        &mut self,
        value: &VerifiedCanonicalKernelIrIdentityV13,
    ) -> Result<(), ProductionW4ExecutionErrorV1> {
        self.bytes(value.digest())?;
        self.u64(value.canonical_length())
    }
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn collect_bounded<T>(
    values: impl IntoIterator<Item = T>,
    limit: usize,
    resource: &'static str,
) -> Result<Vec<T>, ProductionW4InputErrorV1> {
    let mut retained = Vec::new();
    for value in values {
        if retained.len() == limit {
            return Err(ProductionW4InputErrorV1::ResourceLimitExceeded { resource, limit });
        }
        retained.push(value);
    }
    Ok(retained)
}

fn require_nonzero_identity(
    component: &'static str,
    identity: &[u8; 32],
) -> Result<(), ProductionW4InputErrorV1> {
    if identity.iter().all(|byte| *byte == 0) {
        Err(ProductionW4InputErrorV1::EmptyIdentity { component })
    } else {
        Ok(())
    }
}

fn require_name(
    component: &'static str,
    value: &str,
    retained: usize,
) -> Result<(), ProductionW4InputErrorV1> {
    if value.is_empty() {
        Err(ProductionW4InputErrorV1::EmptyName { component })
    } else if retained > MAX_PRODUCTION_W4_IDENTITY_NAME_BYTES_V1 {
        Err(ProductionW4InputErrorV1::NameTooLarge {
            component,
            limit: MAX_PRODUCTION_W4_IDENTITY_NAME_BYTES_V1,
        })
    } else {
        Ok(())
    }
}

const fn kind_tag(kind: ProductionCapabilityAnalysisKindV1) -> u8 {
    match kind {
        ProductionCapabilityAnalysisKindV1::CanonicalTyping => 0,
        ProductionCapabilityAnalysisKindV1::CapabilityProvenance => 1,
        ProductionCapabilityAnalysisKindV1::ResourceLegality => 2,
        ProductionCapabilityAnalysisKindV1::Uniformity => 3,
        ProductionCapabilityAnalysisKindV1::TensorLayout => 4,
        ProductionCapabilityAnalysisKindV1::MemoryBounds => 5,
        ProductionCapabilityAnalysisKindV1::AtomicLegality => 6,
        ProductionCapabilityAnalysisKindV1::RaceFreedom => 7,
        ProductionCapabilityAnalysisKindV1::HierarchicalOwnership => 8,
        ProductionCapabilityAnalysisKindV1::BarrierConvergence => 9,
        ProductionCapabilityAnalysisKindV1::PipelineProtocol => 10,
        ProductionCapabilityAnalysisKindV1::Initialization => 11,
        ProductionCapabilityAnalysisKindV1::MemoryVisibility => 12,
        ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs => 13,
        ProductionCapabilityAnalysisKindV1::EffectRefinement => 14,
        ProductionCapabilityAnalysisKindV1::SemanticRefinement => 15,
    }
}

const fn analysis_obligation_tag(kind: ProductionW4AnalysisObligationKindV1) -> u8 {
    match kind {
        ProductionW4AnalysisObligationKindV1::CanonicalTyping => 0,
        ProductionW4AnalysisObligationKindV1::CapabilityProvenance => 1,
        ProductionW4AnalysisObligationKindV1::ResourceLegality => 2,
        ProductionW4AnalysisObligationKindV1::Uniformity => 3,
        ProductionW4AnalysisObligationKindV1::TensorLayout => 4,
        ProductionW4AnalysisObligationKindV1::MemoryBounds => 5,
        ProductionW4AnalysisObligationKindV1::AtomicLegality => 6,
        ProductionW4AnalysisObligationKindV1::HappensBefore => 7,
        ProductionW4AnalysisObligationKindV1::RaceFreedom => 8,
        ProductionW4AnalysisObligationKindV1::HierarchicalOwnership => 9,
        ProductionW4AnalysisObligationKindV1::BarrierConvergence => 10,
        ProductionW4AnalysisObligationKindV1::BarrierOrder => 11,
        ProductionW4AnalysisObligationKindV1::PipelineProtocol => 12,
        ProductionW4AnalysisObligationKindV1::Initialization => 13,
        ProductionW4AnalysisObligationKindV1::MemoryVisibility => 14,
        ProductionW4AnalysisObligationKindV1::WorkgroupMemoryEpochs => 15,
        ProductionW4AnalysisObligationKindV1::CollectiveParticipation => 16,
        ProductionW4AnalysisObligationKindV1::EffectRefinement => 17,
        ProductionW4AnalysisObligationKindV1::SemanticRefinement => 18,
    }
}

const fn independent_obligation_checker_tag(
    checker: ProductionW4IndependentObligationCheckerV1,
) -> u8 {
    use ProductionW4IndependentObligationCheckerV1 as Checker;
    match checker {
        Checker::CanonicalTypingV13 => 0,
        Checker::CapabilityProvenanceV1 => 1,
        Checker::TargetResourceClosureV1 => 2,
        Checker::UniformityFreshLiveIrV1 => 3,
        Checker::TensorLayoutFreshLiveIrV1 => 4,
        Checker::MemoryBoundsFreshLiveIrV1 => 5,
        Checker::AtomicLegalityFreshLiveIrV1 => 6,
        Checker::HappensBeforeFreshLiveIrV1 => 7,
        Checker::RaceFreedomFreshLiveIrV1 => 8,
        Checker::HierarchicalOwnershipFreshLiveIrV1 => 9,
        Checker::BarrierConvergenceFreshLiveIrV1 => 10,
        Checker::BarrierOrderFreshLiveIrV1 => 11,
        Checker::PipelineProtocolFreshLiveIrV1 => 12,
        Checker::InitializationFreshLiveIrV1 => 13,
        Checker::MemoryVisibilityFreshLiveIrV1 => 14,
        Checker::WorkgroupMemoryEpochsFreshLiveIrV1 => 15,
        Checker::CollectiveParticipationFreshLiveIrV1 => 16,
        Checker::EffectRefinementFreshLiveIrV1 => 17,
        Checker::SemanticRefinementFreshLiveIrV1 => 18,
    }
}

const fn counterexample_class_tag(class: ProductionW4CounterexampleClassV1) -> u8 {
    use ProductionW4CounterexampleClassV1 as Class;
    match class {
        Class::CanonicalExecution => 0,
        Class::ResourceLegality => 1,
        Class::TensorLayout => 2,
        Class::MemoryBounds => 3,
        Class::AtomicLegality => 4,
        Class::RaceConflict => 5,
        Class::OwnershipConflict => 6,
        Class::Uniformity => 7,
        Class::BarrierOrder => 8,
        Class::CollectiveParticipation => 9,
        Class::PipelineEpochProtocol => 10,
        Class::ReadBeforeInitialization => 11,
        Class::MemoryVisibility => 12,
        Class::EffectRefinement => 13,
        Class::SemanticRefinement => 14,
    }
}

const fn pass_tag(pass: KernelCheckPassKindV1) -> u8 {
    match pass {
        KernelCheckPassKindV1::Structural => 0,
        KernelCheckPassKindV1::ControlFlow => 1,
        KernelCheckPassKindV1::MemoryBounds => 2,
        KernelCheckPassKindV1::TensorLayout => 3,
        KernelCheckPassKindV1::AtomicLegality => 4,
        KernelCheckPassKindV1::RaceFreedom => 5,
        KernelCheckPassKindV1::HierarchicalOwnership => 6,
        KernelCheckPassKindV1::BarrierConvergence => 7,
        KernelCheckPassKindV1::PipelineProtocol => 8,
        KernelCheckPassKindV1::WorkgroupMemory => 9,
        KernelCheckPassKindV1::SemanticRefinement => 10,
    }
}

const fn status_tag(status: KernelCheckStatusV1) -> u8 {
    match status {
        KernelCheckStatusV1::Clean => 0,
        KernelCheckStatusV1::Incomplete => 1,
        KernelCheckStatusV1::Rejected => 2,
    }
}

const fn memory_space_tag(space: MemorySpaceAttr) -> u8 {
    match space {
        MemorySpaceAttr::Private => 0,
        MemorySpaceAttr::Workgroup => 1,
        MemorySpaceAttr::Global => 2,
    }
}

const fn atomic_scope_tag(scope: AtomicScopeAttr) -> u8 {
    match scope {
        AtomicScopeAttr::SingleThread => 0,
        AtomicScopeAttr::Workgroup => 1,
        AtomicScopeAttr::Agent => 2,
        AtomicScopeAttr::Device => 3,
        AtomicScopeAttr::System => 4,
    }
}

const fn ownership_coverage_tag(coverage: OwnershipCoverageAttr) -> u8 {
    match coverage {
        OwnershipCoverageAttr::ExactView => 0,
        OwnershipCoverageAttr::ExactEffectDomain => 1,
        OwnershipCoverageAttr::TotalView => 2,
        OwnershipCoverageAttr::CollectiveContributions => 3,
    }
}

const fn witness_checker_tag(checker: ProductionAnalysisWitnessCheckerV1) -> u8 {
    match checker {
        ProductionAnalysisWitnessCheckerV1::TensorLayoutFreshLiveIrReplayV2 => 0,
        ProductionAnalysisWitnessCheckerV1::BoundsExhaustiveRawIrReplayV1 => 1,
        ProductionAnalysisWitnessCheckerV1::AtomicFreshLiveIrReplayV2 => 2,
        ProductionAnalysisWitnessCheckerV1::RaceFreshLiveIrReplayV2 => 3,
        ProductionAnalysisWitnessCheckerV1::OwnershipFreshLiveIrReplayV2 => 4,
        ProductionAnalysisWitnessCheckerV1::BarrierFreshLiveIrReplayV2 => 5,
        ProductionAnalysisWitnessCheckerV1::PipelineFreshLiveIrReplayV2 => 6,
        ProductionAnalysisWitnessCheckerV1::WorkgroupFreshLiveIrReplayV2 => 7,
        ProductionAnalysisWitnessCheckerV1::SemanticFreshLiveIrReplayV2 => 8,
    }
}

const fn witness_gap_tag(gap: ProductionAnalysisWitnessGapV1) -> u8 {
    match gap {
        ProductionAnalysisWitnessGapV1::TensorLayoutExhaustiveDataflow => 0,
        ProductionAnalysisWitnessGapV1::BoundsPresburgerProofTranscript => 1,
        ProductionAnalysisWitnessGapV1::AtomicEnumerationCapabilityAndProvenance => 2,
        ProductionAnalysisWitnessGapV1::RaceEffectsAliasAndHappensBefore => 3,
        ProductionAnalysisWitnessGapV1::OwnershipDomainDisjointnessAndCoverage => 4,
        ProductionAnalysisWitnessGapV1::BarrierReachabilityUniformityAndPostdominance => 5,
        ProductionAnalysisWitnessGapV1::PipelineEpochLifecycleAndSlotReuse => 6,
        ProductionAnalysisWitnessGapV1::WorkgroupAllocationLifetimeAndConflict => 7,
        ProductionAnalysisWitnessGapV1::SemanticRootsControlEffectsAndNumerics => 8,
    }
}

const fn executor_tag(executor: ProductionCapabilityAnalysisExecutorV1) -> u8 {
    match executor {
        ProductionCapabilityAnalysisExecutorV1::CanonicalKirVerifierV13 => 0,
        ProductionCapabilityAnalysisExecutorV1::KernelCapabilityAffectedAnalysisV1 => 1,
        ProductionCapabilityAnalysisExecutorV1::MandatoryPlironPass(_) => 2,
        ProductionCapabilityAnalysisExecutorV1::EmbeddedInPlironPass(_) => 3,
        ProductionCapabilityAnalysisExecutorV1::MandatoryTargetCapabilityClosureV1 => 4,
    }
}

const fn source_tag(source: ProductionW4OutcomeSourceV1) -> u8 {
    match source {
        ProductionW4OutcomeSourceV1::CanonicalKirV13 => 0,
        ProductionW4OutcomeSourceV1::CapabilityProvenanceReplayV1 => 1,
        ProductionW4OutcomeSourceV1::TargetResourceClosureV1 => 2,
        ProductionW4OutcomeSourceV1::PlironPass(_) => 3,
        ProductionW4OutcomeSourceV1::EmbeddedPlironPass(_) => 4,
        ProductionW4OutcomeSourceV1::CanonicalExecutionCapabilityAndPlironPass(_) => 5,
        ProductionW4OutcomeSourceV1::CanonicalExecutionCapabilityAndEmbeddedPlironPass(_) => 6,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use dialect_gpu::{ExecutionDomainAttr, ExecutionLayoutOp};
    use dialect_kernel::{
        AtomicScopeAttr, DIALECT_NAME, MemorySpaceAttr, ReturnOp, TensorConvergenceAttr,
        TensorLayoutOp, register_dialect,
    };
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, DebugSourceMapBindingV1, DebugSourceMapDocumentV2,
        DebugSourceMapFileV1, Function, Kernel, LaunchDomain, LaunchExtent, Signature, Terminator,
        WorkgroupSize,
    };
    use fe2o3_pliron_owner_core::ensure_context_identity;
    use fe2o3_target_spec::{
        TargetArchitectureFamilyV1, TargetArtifactFormatV1, TargetCapabilityDecisionOutcomeV1,
        TargetCapabilityModelIdentityV1, TargetCapabilityQueryV1, TargetCapabilityRequirementV1,
        TargetExecutionModelV1, TargetProfileSpecV1, TargetResourceRequirementV1, TargetVendorV1,
        query_target_capability_v1,
    };
    use pliron::{
        builtin::{ops::FuncOp, types::FunctionType},
        context::Context,
        dialect::DialectName,
        op::Op,
        operation::{Operation, verify_operation},
        parsable::parse_from_str,
    };

    use crate::{
        PlironAtomicTargetCapabilityV1, PlironHostAllocationV1, PlironLaunchTargetLimitsV1,
    };

    use super::*;

    const TEST_TARGET_PROFILE: TargetProfileSpecV1 = TargetProfileSpecV1::from_static_parts(
        TargetVendorV1::Other,
        TargetArchitectureFamilyV1::Other,
        "w4-test",
        None,
        None,
        TargetArtifactFormatV1::NativeObject,
        TargetExecutionModelV1::GpuGrid,
        None,
        &[],
    );
    const TEST_TARGET_MODEL: TargetCapabilityModelIdentityV1 =
        TargetCapabilityModelIdentityV1::new_unchecked(TEST_TARGET_PROFILE, "w4-test-v1");

    struct TestTarget;

    impl TargetCapabilityQueryV1 for TestTarget {
        fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
            TEST_TARGET_MODEL
        }

        fn query_outcome(
            &self,
            _requirement: TargetCapabilityRequirementV1,
        ) -> TargetCapabilityDecisionOutcomeV1 {
            TargetCapabilityDecisionOutcomeV1::Supported
        }
    }

    fn target_identity() -> [u8; 32] {
        target_model_identity_v5(TEST_TARGET_MODEL)
    }

    fn target_decisions() -> Vec<TargetCapabilityDecisionV1> {
        BTreeSet::from([
            TargetCapabilityRequirementV1::SubgroupSize(64),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(64),
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x: 64, y: 1, z: 1 },
            ),
        ])
        .into_iter()
        .map(|requirement| query_target_capability_v1(&TestTarget, requirement).unwrap())
        .collect()
    }

    fn target_closure() -> (Vec<u8>, [u8; 32]) {
        let closure = b"validated-w5-closure".to_vec();
        let identity = derive_live_target_closure_identity_v1(&closure);
        (closure, identity)
    }

    fn diagnostic_source_map(canonical: &VerifiedCanonicalKernelIrV13) -> Vec<u8> {
        DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new(
                [9; 32],
                *canonical.identity().digest(),
                canonical.identity().canonical_length(),
            )
            .unwrap(),
            vec![DebugSourceMapFileV1::new([7; 32], 1, "kernel.rs".to_owned()).unwrap()],
            vec![],
            vec![],
            vec![],
            vec![],
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap()
    }

    fn context() -> Context {
        let mut context = Context::new();
        ensure_context_identity(&mut context).unwrap();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        dialect_proof::register_dialect(&mut context).unwrap();
        context
    }

    fn kir_module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("w4-witness");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn clean_function(context: &mut Context) -> (FuncOp, ReturnOp) {
        let function = FuncOp::new(
            context,
            "entry".try_into().unwrap(),
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
            &fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
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

    fn atomic_target() -> PlironAtomicTargetContextV1 {
        PlironAtomicTargetContextV1::new([
            PlironAtomicTargetCapabilityV1::new(
                32,
                MemorySpaceAttr::Global,
                AtomicScopeAttr::System,
            )
            .unwrap(),
            PlironAtomicTargetCapabilityV1::new(
                32,
                MemorySpaceAttr::Workgroup,
                AtomicScopeAttr::Workgroup,
            )
            .unwrap(),
        ])
        .unwrap()
    }

    fn launch_contract() -> PlironLaunchContractV1 {
        let limits = PlironLaunchTargetLimitsV1::new(
            [u32::MAX as u64; 3],
            [1_024, 1_024, 1_024],
            1_024,
            vec![1, 2, 4, 8, 16, 32, 64],
            1 << 20,
            16,
            64,
        )
        .unwrap();
        let allocations = (1..=32)
            .map(|origin| PlironHostAllocationV1::new(origin, 1 << 20, 64).unwrap())
            .collect();
        let decisions = target_decisions();
        let (identity, _) =
            derive_target_decision_set_identity_v1(&decisions, &target_identity()).unwrap();
        PlironLaunchContractV1::new_bound_to_target_decisions(limits, allocations, identity)
            .unwrap()
    }

    fn subject_and_target(
        canonical: &VerifiedCanonicalKernelIrV13,
    ) -> (
        ProductionW4FinalGraphSubjectV1,
        ProductionW4TargetResourceInputV1,
    ) {
        let root = ProductionW4KernelRootV1::try_new(
            KernelId::new("kernel"),
            FunctionId::new("entry"),
            [6; 32],
        )
        .unwrap();
        let (closure, closure_identity) = target_closure();
        let subject = ProductionW4FinalGraphSubjectV1::try_new(
            *canonical.identity(),
            4,
            *canonical.identity(),
            5,
            [1; 32],
            [2; 32],
            target_identity(),
            [3; 32],
            closure_identity,
            [root],
        )
        .unwrap();
        let target = ProductionW4TargetResourceInputV1::try_new(
            *canonical.identity(),
            5,
            target_identity(),
            [3; 32],
            closure_identity,
            closure,
            target_decisions(),
            atomic_target(),
            launch_contract(),
        )
        .unwrap();
        (subject, target)
    }

    fn complete_witness<'a>(
        canonical: &VerifiedCanonicalKernelIrV13,
        module: &Module,
        subject: ProductionW4FinalGraphSubjectV1,
        target: ProductionW4TargetResourceInputV1,
        context: &Context,
        function_id: &'a FunctionId,
        function: &'a FuncOp,
    ) -> ProductionW4FinalGraphCapabilityWitnessV1 {
        let live = [ProductionW4LiveFunctionV1::new(function_id, function)];
        match execute_production_w4_final_graph_capability_witness_v1(
            canonical,
            module,
            subject,
            target,
            &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
            context,
            &live,
        )
        .unwrap()
        {
            ProductionW4FinalGraphExecutionV1::Complete(witness) => witness,
            ProductionW4FinalGraphExecutionV1::NonClean(result) => {
                panic!("unexpected non-clean W4 result: {:?}", result.outcome())
            }
        }
    }

    #[test]
    fn complete_witness_retains_all_typed_outcomes_and_is_deterministic() {
        let module = kir_module();
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let function_id = &module.functions[0].id;

        let mut first_context = context();
        let (first_function, _) = clean_function(&mut first_context);
        let (subject, target) = subject_and_target(&canonical);
        let first = complete_witness(
            &canonical,
            &module,
            subject,
            target,
            &first_context,
            function_id,
            &first_function,
        );

        let mut second_context = context();
        let (second_function, _) = clean_function(&mut second_context);
        let (subject, target) = subject_and_target(&canonical);
        let second = complete_witness(
            &canonical,
            &module,
            subject,
            target,
            &second_context,
            function_id,
            &second_function,
        );

        assert_eq!(first.stages().len(), 16);
        assert!(
            first
                .stages()
                .iter()
                .all(|stage| { stage.outcome().status() == KernelCheckStatusV1::Clean })
        );
        assert_eq!(first.functions().len(), 1);
        let schedule = first.analysis_schedule();
        assert_eq!(schedule.final_graph(), canonical.identity());
        assert_eq!(schedule.final_epoch(), 5);
        assert_eq!(
            schedule.obligations().len(),
            PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1
        );
        assert_eq!(
            schedule
                .obligations()
                .iter()
                .map(ProductionW4AnalysisObligationResultV1::kind)
                .collect::<Vec<_>>(),
            PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
                .iter()
                .flat_map(|stage| stage.obligations())
                .map(|obligation| obligation.kind())
                .collect::<Vec<_>>()
        );
        assert!(!schedule.grants_compiler_refinement_authority());
        assert!(!schedule.grants_lowering_artifact_or_launch_authority());
        for (kind, checker) in [
            (
                ProductionW4AnalysisObligationKindV1::Uniformity,
                ProductionW4IndependentObligationCheckerV1::UniformityFreshLiveIrV1,
            ),
            (
                ProductionW4AnalysisObligationKindV1::HappensBefore,
                ProductionW4IndependentObligationCheckerV1::HappensBeforeFreshLiveIrV1,
            ),
            (
                ProductionW4AnalysisObligationKindV1::BarrierOrder,
                ProductionW4IndependentObligationCheckerV1::BarrierOrderFreshLiveIrV1,
            ),
            (
                ProductionW4AnalysisObligationKindV1::CollectiveParticipation,
                ProductionW4IndependentObligationCheckerV1::CollectiveParticipationFreshLiveIrV1,
            ),
            (
                ProductionW4AnalysisObligationKindV1::Initialization,
                ProductionW4IndependentObligationCheckerV1::InitializationFreshLiveIrV1,
            ),
            (
                ProductionW4AnalysisObligationKindV1::MemoryVisibility,
                ProductionW4IndependentObligationCheckerV1::MemoryVisibilityFreshLiveIrV1,
            ),
            (
                ProductionW4AnalysisObligationKindV1::WorkgroupMemoryEpochs,
                ProductionW4IndependentObligationCheckerV1::WorkgroupMemoryEpochsFreshLiveIrV1,
            ),
            (
                ProductionW4AnalysisObligationKindV1::EffectRefinement,
                ProductionW4IndependentObligationCheckerV1::EffectRefinementFreshLiveIrV1,
            ),
        ] {
            let evidence = schedule
                .obligations()
                .iter()
                .find(|obligation| obligation.kind() == kind)
                .unwrap()
                .independent_evidence();
            assert_eq!(evidence.final_graph(), canonical.identity());
            assert_eq!(evidence.final_epoch(), 5);
            assert_eq!(evidence.pliron_epoch(), schedule.pliron_epoch());
            assert_eq!(evidence.checker(), checker);
            assert!(!evidence.pass_evidence().is_empty());
            assert!(evidence.pass_evidence().iter().all(|pass| {
                pass.checkpoint_epoch() == schedule.pliron_epoch()
                    && pass.checked_units() != 0
                    && pass.structural_sha256()
                        == first.functions()[0].structural_identity().sha256()
            }));
            assert!(!evidence.grants_any_authority());
        }
        let function = &first.functions()[0];
        assert!(function.tensor_layout().is_clean());
        assert!(function.bounds().is_clean());
        assert!(function.atomic_legality().is_clean());
        assert!(function.race_freedom().is_clean());
        assert!(function.hierarchical_ownership().is_clean());
        assert!(function.uniformity_and_barriers().is_clean());
        assert!(function.pipeline_protocol().is_clean());
        assert!(function.initialization_visibility_and_epochs().is_clean());
        assert!(function.effect_refinement().is_clean());
        assert!(function.semantic_refinement().is_clean());
        assert_eq!(first.canonical_encoding(), second.canonical_encoding());
        assert_eq!(first.identity(), second.identity());
        let decoded = ProductionW4CanonicalEncodingV1::decode(first.canonical_encoding()).unwrap();
        assert_eq!(decoded.reencode(), first.canonical_encoding());
        let mut stale = first.canonical_encoding().to_vec();
        stale[WITNESS_DOMAIN_V1.len() + size_of::<u16>()] ^= 1;
        assert!(matches!(
            ProductionW4CanonicalEncodingV1::decode(&stale),
            Err(ProductionW4EncodingErrorV1::ChecksumMismatch)
        ));

        let mut third_context = context();
        let (third_function, _) = clean_function(&mut third_context);
        let (subject, _) = subject_and_target(&canonical);
        let (closure, closure_identity) = target_closure();
        let target = ProductionW4TargetResourceInputV1::try_new(
            *canonical.identity(),
            5,
            target_identity(),
            [3; 32],
            closure_identity,
            closure,
            target_decisions(),
            atomic_target()
                .with_system_coherent_allocations([9])
                .unwrap(),
            launch_contract(),
        )
        .unwrap();
        let mutated_target = complete_witness(
            &canonical,
            &module,
            subject,
            target,
            &third_context,
            function_id,
            &third_function,
        );
        assert_ne!(
            first.canonical_encoding(),
            mutated_target.canonical_encoding()
        );
        assert_ne!(first.identity(), mutated_target.identity());
        assert!(!first.grants_compiler_refinement_authority());
        assert!(!first.grants_lowering_artifact_or_launch_authority());
        let concise = mutated_target.into_analysis_schedule();
        assert_eq!(concise.final_graph(), canonical.identity());
        assert_eq!(
            concise.obligations().len(),
            PRODUCTION_W4_ANALYSIS_OBLIGATION_COUNT_V1
        );
    }

    #[test]
    fn exact_target_decisions_and_model_are_retained_and_fail_closed() {
        let module = kir_module();
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let (_, target) = subject_and_target(&canonical);
        let decisions = target_decisions();
        let (identity, count) =
            derive_target_decision_set_identity_v1(&decisions, &target_identity()).unwrap();
        assert_eq!(target.target_model_identity(), TEST_TARGET_MODEL);
        assert_eq!(target.target_decisions(), decisions);
        assert_eq!(target.decision_set_identity(), &identity);
        assert_eq!(count, decisions.len());
        assert_eq!(
            target.launch_contract().target_decision_set_identity(),
            Some(&identity)
        );

        let (closure, closure_identity) = target_closure();
        let unbound =
            PlironLaunchContractV1::new(launch_contract().limits().clone(), Vec::new()).unwrap();
        assert!(matches!(
            ProductionW4TargetResourceInputV1::try_new(
                *canonical.identity(),
                5,
                target_identity(),
                [3; 32],
                closure_identity,
                closure,
                decisions.clone(),
                atomic_target(),
                unbound,
            ),
            Err(ProductionW4InputErrorV1::LaunchContractTargetDecisionsUnbound)
        ));

        let mut reordered = decisions;
        reordered.reverse();
        let (closure, closure_identity) = target_closure();
        assert!(matches!(
            ProductionW4TargetResourceInputV1::try_new(
                *canonical.identity(),
                5,
                target_identity(),
                [3; 32],
                closure_identity,
                closure,
                reordered,
                atomic_target(),
                launch_contract(),
            ),
            Err(ProductionW4InputErrorV1::TargetDecisionOrder)
        ));

        let (closure, mut closure_identity) = target_closure();
        closure_identity[0] ^= 1;
        assert!(matches!(
            ProductionW4TargetResourceInputV1::try_new(
                *canonical.identity(),
                5,
                target_identity(),
                [3; 32],
                closure_identity,
                closure,
                target_decisions(),
                atomic_target(),
                launch_contract(),
            ),
            Err(ProductionW4InputErrorV1::TargetClosureIdentityMismatch)
        ));
    }

    #[test]
    fn every_typed_safety_family_has_frozen_exact_stage_coverage() {
        let observed = CanonicalKirSafetyFamilyV1::ALL.map(family_required_stage_mask);
        assert_eq!(
            observed,
            [
                0xc007, 0xdda7, 0xe927, 0xd9a7, 0xd9a7, 0xd28f, 0xd0e7, 0xd087, 0xfa8f, 0xfda7,
                0xffbf, 0xffbf, 0xc00f, 0xffff, 0xfe0b, 0xfe0b, 0xfe0b,
            ]
        );
        assert!(observed.into_iter().all(|mask| {
            mask != 0
                && mask & !all_production_stage_mask() == 0
                && mask & stage_bit(ProductionCapabilityAnalysisKindV1::CanonicalTyping) != 0
                && mask & stage_bit(ProductionCapabilityAnalysisKindV1::CapabilityProvenance) != 0
                && mask & stage_bit(ProductionCapabilityAnalysisKindV1::EffectRefinement) != 0
                && mask & stage_bit(ProductionCapabilityAnalysisKindV1::SemanticRefinement) != 0
        }));
    }

    #[test]
    fn diagnostic_source_map_and_non_clean_subject_are_canonical() {
        let module = kir_module();
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let (subject, _) = subject_and_target(&canonical);
        let subject = subject
            .with_diagnostic_source_map_v1(diagnostic_source_map(&canonical))
            .unwrap();
        let source_map = subject.diagnostic_source_map().unwrap();
        assert_eq!(
            source_map.document().binding().canonical_kir().digest(),
            *canonical.identity().digest()
        );

        let execution = non_clean_without_pipeline(
            subject.clone(),
            ProductionCapabilityAnalysisKindV1::RaceFreedom,
            Some(FunctionId::new("entry")),
            KernelCheckStatusV1::Rejected,
            "conflicting global writes".to_owned(),
        )
        .unwrap();
        let ProductionW4FinalGraphExecutionV1::NonClean(mut result) = execution else {
            panic!("expected non-clean result");
        };
        assert_eq!(result.outcome().status(), KernelCheckStatusV1::Rejected);
        assert_eq!(
            result.outcome().diagnostic_code(),
            Some(PRODUCTION_W4_REJECTED_DIAGNOSTIC_V1)
        );
        let ProductionW4CapabilityOutcomeV1::Rejected(diagnostic) = result.outcome() else {
            panic!("expected rejected diagnostic");
        };
        assert_eq!(diagnostic.final_graph(), canonical.identity());
        assert_eq!(diagnostic.final_epoch(), 5);
        let counterexample = diagnostic.counterexample().unwrap();
        assert_eq!(
            counterexample.obligation(),
            ProductionW4AnalysisObligationKindV1::RaceFreedom
        );
        assert_eq!(
            counterexample.class(),
            ProductionW4CounterexampleClassV1::RaceConflict
        );
        assert!(
            counterexample
                .detail()
                .contains("conflicting global writes")
        );
        assert_eq!(
            diagnostic.source_map_identity(),
            Some(source_map.identity())
        );
        assert_eq!(result.subject().final_epoch(), 5);
        assert!(result.subject().diagnostic_source_map().is_some());
        assert!(!result.grants_any_authority());
        result.require_exact_subject_v1(&subject).unwrap();
        ProductionW4CanonicalEncodingV1::decode(result.canonical_encoding()).unwrap();

        let incomplete = non_clean_without_pipeline(
            subject.clone(),
            ProductionCapabilityAnalysisKindV1::CapabilityProvenance,
            None,
            KernelCheckStatusV1::Incomplete,
            "live typed safety carrier unavailable".to_owned(),
        )
        .unwrap();
        let ProductionW4FinalGraphExecutionV1::NonClean(incomplete) = incomplete else {
            panic!("expected incomplete W4 result");
        };
        assert_eq!(
            incomplete.outcome().status(),
            KernelCheckStatusV1::Incomplete
        );
        assert_eq!(
            incomplete.outcome().diagnostic_code(),
            Some(PRODUCTION_W4_INCOMPLETE_DIAGNOSTIC_V1)
        );
        assert_eq!(incomplete.subject(), &subject);
        let ProductionW4CapabilityOutcomeV1::Incomplete(diagnostic) = incomplete.outcome() else {
            panic!("expected incomplete diagnostic");
        };
        assert!(diagnostic.counterexample().is_none());
        assert_eq!(incomplete.subject().final_epoch(), 5);
        assert!(incomplete.subject().diagnostic_source_map().is_some());
        assert!(!incomplete.grants_any_authority());
        incomplete.require_exact_subject_v1(&subject).unwrap();

        let unsupported = non_clean_unsupported_without_pipeline(
            subject.clone(),
            ProductionCapabilityAnalysisKindV1::MemoryBounds,
            Some(FunctionId::new("entry")),
            "unsupported final-graph operation at source block 4 op 2".to_owned(),
        )
        .unwrap();
        let ProductionW4FinalGraphExecutionV1::NonClean(unsupported) = unsupported else {
            panic!("expected unsupported W4 result");
        };
        assert!(matches!(
            unsupported.outcome(),
            ProductionW4CapabilityOutcomeV1::Unsupported(_)
        ));
        assert_eq!(
            unsupported.outcome().diagnostic_code(),
            Some(PRODUCTION_W4_UNSUPPORTED_DIAGNOSTIC_V1)
        );
        let ProductionW4CapabilityOutcomeV1::Unsupported(diagnostic) = unsupported.outcome() else {
            panic!("expected unsupported diagnostic");
        };
        assert!(diagnostic.counterexample().is_none());
        assert!(unsupported.to_string().contains("source block 4 op 2"));

        let mut substituted = subject.clone();
        substituted.policy_identity[0] ^= 1;
        assert!(matches!(
            result.require_exact_subject_v1(&substituted),
            Err(ProductionW4HandoffErrorV1::DiagnosticSubjectSubstituted)
        ));

        result.canonical_encoding[WITNESS_DOMAIN_V1.len() + size_of::<u16>() + 1] ^= 1;
        assert!(matches!(
            result.require_exact_subject_v1(&subject),
            Err(ProductionW4HandoffErrorV1::DiagnosticEncoding(
                ProductionW4EncodingErrorV1::ChecksumMismatch
            ))
        ));

        let wrong_map = DebugSourceMapDocumentV2::new(
            DebugSourceMapBindingV1::new([9; 32], [8; 32], canonical.identity().canonical_length())
                .unwrap(),
            vec![DebugSourceMapFileV1::new([7; 32], 1, "kernel.rs".to_owned()).unwrap()],
            vec![],
            vec![],
            vec![],
            vec![],
        )
        .unwrap()
        .to_canonical_json_bytes()
        .unwrap();
        assert!(matches!(
            subject_and_target(&canonical)
                .0
                .with_diagnostic_source_map_v1(wrong_map),
            Err(ProductionW4InputErrorV1::DiagnosticSourceMapGraphMismatch)
        ));
    }

    #[test]
    fn schedule_epoch_root_target_launch_and_mutation_substitutions_fail() {
        let module = kir_module();
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let function_id = &module.functions[0].id;
        let mut context = context();
        let (function, ret) = clean_function(&mut context);
        let (subject, target) = subject_and_target(&canonical);
        let witness = complete_witness(
            &canonical,
            &module,
            subject.clone(),
            target.clone(),
            &context,
            function_id,
            &function,
        );
        let live = [ProductionW4LiveFunctionV1::new(function_id, &function)];
        witness
            .require_exact_w6_handoff_v1(
                &canonical,
                &module,
                &subject,
                &target,
                &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                &context,
                &live,
            )
            .unwrap();

        let omitted = &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1[..15];
        assert!(matches!(
            witness.require_exact_w6_handoff_v1(
                &canonical, &module, &subject, &target, omitted, &context, &live,
            ),
            Err(ProductionW4HandoffErrorV1::Execution(
                ProductionW4ExecutionErrorV1::ScheduleLength { .. }
            ))
        ));
        let mut reordered = PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.to_vec();
        reordered.swap(9, 10);
        assert!(matches!(
            witness.require_exact_w6_handoff_v1(
                &canonical, &module, &subject, &target, &reordered, &context, &live,
            ),
            Err(ProductionW4HandoffErrorV1::Execution(
                ProductionW4ExecutionErrorV1::ScheduleStage { .. }
            ))
        ));

        let (closure, closure_identity) = target_closure();
        let stale_target = ProductionW4TargetResourceInputV1::try_new(
            *canonical.identity(),
            6,
            target_identity(),
            [3; 32],
            closure_identity,
            closure,
            target_decisions(),
            atomic_target(),
            launch_contract(),
        )
        .unwrap();
        assert!(matches!(
            execute_production_w4_final_graph_capability_witness_v1(
                &canonical,
                &module,
                subject.clone(),
                stale_target,
                &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                &context,
                &live,
            ),
            Err(ProductionW4ExecutionErrorV1::TargetResourceSubstituted)
        ));
        let other_canonical =
            VerifiedCanonicalKernelIrV13::from_module(Module::new("other")).unwrap();
        assert!(matches!(
            ProductionW4FinalGraphSubjectV1::try_new(
                *other_canonical.identity(),
                5,
                *canonical.identity(),
                5,
                [1; 32],
                [2; 32],
                target_identity(),
                [3; 32],
                target_closure().1,
                [ProductionW4KernelRootV1::try_new(
                    KernelId::new("kernel"),
                    FunctionId::new("entry"),
                    [6; 32],
                )
                .unwrap()],
            ),
            Err(ProductionW4InputErrorV1::ChangedGraphWithoutEpochAdvance { epoch: 5 })
        ));

        let cross_graph_subject = ProductionW4FinalGraphSubjectV1::try_new(
            *canonical.identity(),
            4,
            *other_canonical.identity(),
            5,
            [1; 32],
            [2; 32],
            target_identity(),
            [3; 32],
            target_closure().1,
            [ProductionW4KernelRootV1::try_new(
                KernelId::new("kernel"),
                FunctionId::new("entry"),
                [6; 32],
            )
            .unwrap()],
        )
        .unwrap();
        assert!(matches!(
            execute_production_w4_final_graph_capability_witness_v1(
                &canonical,
                &module,
                cross_graph_subject,
                target.clone(),
                &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                &context,
                &live,
            ),
            Err(ProductionW4ExecutionErrorV1::FinalGraphSubstituted)
        ));
        let (closure, closure_identity) = target_closure();
        let wrong_target = ProductionW4TargetResourceInputV1::try_new(
            *canonical.identity(),
            5,
            target_identity(),
            [9; 32],
            closure_identity,
            closure,
            target_decisions(),
            atomic_target(),
            launch_contract(),
        )
        .unwrap();
        assert!(matches!(
            execute_production_w4_final_graph_capability_witness_v1(
                &canonical,
                &module,
                subject.clone(),
                wrong_target,
                &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                &context,
                &live,
            ),
            Err(ProductionW4ExecutionErrorV1::TargetResourceSubstituted)
        ));
        let wrong_root = ProductionW4KernelRootV1::try_new(
            KernelId::new("other"),
            FunctionId::new("entry"),
            [6; 32],
        )
        .unwrap();
        let wrong_subject = ProductionW4FinalGraphSubjectV1::try_new(
            *canonical.identity(),
            4,
            *canonical.identity(),
            5,
            [1; 32],
            [2; 32],
            target_identity(),
            [3; 32],
            target_closure().1,
            [wrong_root],
        )
        .unwrap();
        assert!(matches!(
            execute_production_w4_final_graph_capability_witness_v1(
                &canonical,
                &module,
                wrong_subject,
                target.clone(),
                &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                &context,
                &live,
            ),
            Err(ProductionW4ExecutionErrorV1::KernelRootsSubstituted)
        ));

        let original = ret.get_operation().deref(&context).attributes.clone();
        ret.get_operation().deref_mut(&context).attributes = original;
        assert!(matches!(
            witness.require_exact_w6_handoff_v1(
                &canonical,
                &module,
                &subject,
                &target,
                &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                &context,
                &live,
            ),
            Err(ProductionW4HandoffErrorV1::ContextSubstituted)
        ));
    }

    fn parse_fixture(source: &str) -> (Context, FuncOp) {
        let ir = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut context = context();
        let operation = parse_from_str(Operation::top_level_parser(), &mut context, &ir).unwrap();
        verify_operation(operation, &context).unwrap();
        (context, FuncOp::from_operation(operation))
    }

    fn fixture_error(source: &str) -> ProductionPlironPreloweringErrorV2 {
        let (context, function) = parse_fixture(source);
        require_production_pliron_checks_with_atomic_target_before_lowering_v2(
            &context,
            &function,
            &atomic_target(),
        )
        .unwrap_err()
    }

    fn fixture_report(source: &str) -> ProductionPlironPreloweringReportV2 {
        let (context, function) = parse_fixture(source);
        require_production_pliron_checks_with_atomic_target_before_lowering_v2(
            &context,
            &function,
            &atomic_target(),
        )
        .unwrap()
    }

    fn assert_exact_fixture_passes(
        source: &str,
        passes: &[(KernelCheckPassKindV1, ProductionAnalysisWitnessCheckerV1)],
    ) {
        let report = fixture_report(source);
        assert!(report.is_clean());
        assert!(report.preservation().is_exact_identity());
        let pliron_epoch = report.preservation().input_mutation_epoch();
        assert_eq!(report.preservation().output_mutation_epoch(), pliron_epoch);
        for &(pass, checker) in passes {
            let validation = exact_pass_validation(&report, pass, pliron_epoch)
                .unwrap_or_else(|| panic!("missing exact independent evidence for {pass:?}"));
            assert_eq!(validation.witness().checker(), checker);
            assert!(validation.witness().coverage().is_complete());
            assert_ne!(validation.witness().coverage().obligation_count(), 0);
        }
    }

    #[test]
    fn live_barrier_control_flow_and_loop_analyses_are_retained_not_fabricated() {
        for source in [
            include_str!("../tests/lit/barrier_divergent.pliron"),
            include_str!("../tests/lit/barrier_different_sites.pliron"),
        ] {
            assert!(matches!(
                fixture_error(source),
                ProductionPlironPreloweringErrorV2::Barrier(_)
            ));
        }
        assert!(matches!(
            fixture_error(include_str!(
                "../tests/lit/tensor_layout_dynamic_early_return.pliron"
            )),
            ProductionPlironPreloweringErrorV2::TensorLayout(_)
        ));
        let (context, function) = parse_fixture(include_str!(
            "../tests/lit/ownership_total_finite_induction_loop.pliron"
        ));
        let report = require_production_pliron_checks_with_atomic_target_before_lowering_v2(
            &context,
            &function,
            &atomic_target(),
        )
        .unwrap();
        assert!(report.is_clean());
    }

    #[test]
    fn cyclic_and_unproved_affine_loops_remain_incomplete() {
        for source in [
            include_str!("../tests/lit/ownership_nonterminating_induction_loop.pliron"),
            include_str!("../tests/lit/tensor_layout_loop_incomplete.pliron"),
            include_str!("../tests/lit/tensor_layout_uniform_induction_loop.pliron"),
        ] {
            let error = fixture_error(source);
            let (_, status) = classify_pipeline_error(&error);
            assert_eq!(status, KernelCheckStatusV1::Incomplete);
        }

        assert_exact_fixture_passes(
            include_str!("../tests/lit/ownership_total_finite_induction_loop.pliron"),
            &[(
                KernelCheckPassKindV1::HierarchicalOwnership,
                ProductionAnalysisWitnessCheckerV1::OwnershipFreshLiveIrReplayV2,
            )],
        );
    }

    #[test]
    fn advanced_synchronization_requires_exact_independent_pass_evidence() {
        assert_exact_fixture_passes(
            include_str!("../tests/lit/pipeline_dynamic_double_buffer.pliron"),
            &[(
                KernelCheckPassKindV1::PipelineProtocol,
                ProductionAnalysisWitnessCheckerV1::PipelineFreshLiveIrReplayV2,
            )],
        );
        assert_exact_fixture_passes(
            include_str!("../tests/lit/pipeline_dynamic_triple_buffer.pliron"),
            &[(
                KernelCheckPassKindV1::PipelineProtocol,
                ProductionAnalysisWitnessCheckerV1::PipelineFreshLiveIrReplayV2,
            )],
        );
        assert_exact_fixture_passes(
            include_str!("../tests/lit/scoped_subgroup_collective.pliron"),
            &[(
                KernelCheckPassKindV1::BarrierConvergence,
                ProductionAnalysisWitnessCheckerV1::BarrierFreshLiveIrReplayV2,
            )],
        );
        assert_exact_fixture_passes(
            include_str!("../tests/lit/scoped_cross_workgroup_atomic.pliron"),
            &[
                (
                    KernelCheckPassKindV1::AtomicLegality,
                    ProductionAnalysisWitnessCheckerV1::AtomicFreshLiveIrReplayV2,
                ),
                (
                    KernelCheckPassKindV1::RaceFreedom,
                    ProductionAnalysisWitnessCheckerV1::RaceFreshLiveIrReplayV2,
                ),
            ],
        );
        assert_exact_fixture_passes(
            include_str!("../tests/lit/workgroup_published.pliron"),
            &[
                (
                    KernelCheckPassKindV1::BarrierConvergence,
                    ProductionAnalysisWitnessCheckerV1::BarrierFreshLiveIrReplayV2,
                ),
                (
                    KernelCheckPassKindV1::WorkgroupMemory,
                    ProductionAnalysisWitnessCheckerV1::WorkgroupFreshLiveIrReplayV2,
                ),
            ],
        );
    }

    #[test]
    fn unsupported_and_unproved_synchronization_are_incomplete_not_rejected() {
        for source in [
            include_str!("../tests/lit/scoped_grid_barrier_unsupported.pliron"),
            include_str!("../tests/lit/workgroup_fence_publication_incomplete.pliron"),
        ] {
            let error = fixture_error(source);
            let (_, status) = classify_pipeline_error(&error);
            assert_eq!(status, KernelCheckStatusV1::Incomplete);
        }
    }

    #[test]
    fn live_initialization_visibility_epoch_and_pipeline_failures_are_typed() {
        assert!(matches!(
            fixture_error(include_str!("../tests/lit/workgroup_uninitialized.pliron")),
            ProductionPlironPreloweringErrorV2::Workgroup(_)
        ));
        assert!(matches!(
            fixture_error(include_str!(
                "../tests/lit/workgroup_missing_publish.pliron"
            )),
            ProductionPlironPreloweringErrorV2::Workgroup(_)
        ));
        assert!(matches!(
            fixture_error(include_str!(
                "../tests/lit/pipeline_overwrite_before_release.pliron"
            )),
            ProductionPlironPreloweringErrorV2::PipelineProtocol(_)
        ));
    }

    #[test]
    fn live_atomic_race_happens_before_and_effect_failures_are_typed() {
        assert!(matches!(
            fixture_error(include_str!(
                "../tests/lit/atomic_invalid_load_order.pliron"
            )),
            ProductionPlironPreloweringErrorV2::Atomic(_)
        ));
        assert!(matches!(
            fixture_error(include_str!(
                "../tests/lit/scoped_cross_workgroup_atomic_too_narrow.pliron"
            )),
            ProductionPlironPreloweringErrorV2::Race(_)
        ));
        assert!(matches!(
            fixture_error(include_str!(
                "../tests/lit/race_allocation_effect_concurrent_alias.pliron"
            )),
            ProductionPlironPreloweringErrorV2::Race(_)
        ));
        assert!(matches!(
            fixture_error(include_str!(
                "../tests/lit/effect_refinement_value_mismatch.pliron"
            )),
            ProductionPlironPreloweringErrorV2::Semantic(_)
        ));
    }

    #[test]
    fn live_bounds_alias_and_noninjective_output_failures_are_typed() {
        assert!(matches!(
            fixture_error(include_str!("../tests/lit/bounds_static_oob.pliron")),
            ProductionPlironPreloweringErrorV2::Bounds(_)
        ));
        assert!(matches!(
            fixture_error(include_str!("../tests/lit/race_duplicate_output.pliron")),
            ProductionPlironPreloweringErrorV2::Race(_)
        ));
        assert!(matches!(
            fixture_error(include_str!("../tests/lit/ownership_hole.pliron")),
            ProductionPlironPreloweringErrorV2::Ownership(_)
        ));
    }

    #[test]
    fn input_and_diagnostic_resources_are_bounded() {
        let module = kir_module();
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let oversized = vec![0_u8; MAX_PRODUCTION_W4_TARGET_CLOSURE_BYTES_V1 + 1];
        assert!(matches!(
            ProductionW4TargetResourceInputV1::try_new(
                *canonical.identity(),
                5,
                target_identity(),
                [3; 32],
                [4; 32],
                oversized,
                target_decisions(),
                atomic_target(),
                launch_contract(),
            ),
            Err(ProductionW4InputErrorV1::ResourceLimitExceeded {
                resource: "target closure bytes",
                ..
            })
        ));
        let (subject, _) = subject_and_target(&canonical);
        let diagnostic = bounded_diagnostic(
            &subject,
            ProductionCapabilityAnalysisKindV1::RaceFreedom,
            Some(FunctionId::new("entry")),
            "x".repeat(MAX_PRODUCTION_W4_DIAGNOSTIC_BYTES_V1 + 100),
        );
        assert_eq!(
            diagnostic.detail().len(),
            MAX_PRODUCTION_W4_DIAGNOSTIC_BYTES_V1
        );
        assert!(diagnostic.truncated());

        let roots = (0..=MAX_PRODUCTION_W4_KERNEL_ROOTS_V1).map(|index| {
            ProductionW4KernelRootV1::try_new(
                KernelId::new(format!("kernel-{index}")),
                FunctionId::new(format!("entry-{index}")),
                [7; 32],
            )
            .unwrap()
        });
        assert!(matches!(
            ProductionW4FinalGraphSubjectV1::try_new(
                *canonical.identity(),
                4,
                *canonical.identity(),
                5,
                [1; 32],
                [2; 32],
                target_identity(),
                [3; 32],
                target_closure().1,
                roots,
            ),
            Err(ProductionW4InputErrorV1::ResourceLimitExceeded {
                resource: "kernel roots",
                ..
            })
        ));
    }
}
