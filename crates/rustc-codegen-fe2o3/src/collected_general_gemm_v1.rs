//! Authenticated MIR adapter for the safe general tiled-GEMM surfaces.
//!
//! The adapter recognizes exact diagnostic-item `DefId`s from the reviewed
//! standalone companion crate and derives a runtime-parameterized semantic
//! diagnostic facts from optimized MIR. Positive frontend correspondence is
//! disabled until the complete optimized-MIR authority proof is closed; this
//! module never seeds a synthetic plan or treats caller assertions as facts.

use std::collections::{BTreeSet, VecDeque};
use std::fmt;

#[cfg(test)]
use fe2o3_general_gemm_compiler::GeneralGemmFrontendSemanticBindingErrorV1;
use fe2o3_general_gemm_compiler::{
    GeneralGemmAbiArgumentV1, GeneralGemmDerivedKirBehaviorV1, GeneralGemmDerivedSourceSchemaV1,
    GeneralGemmFrontendSemanticBindingV1, GeneralGemmSymbolicKirV1,
    GeneralGemmSymbolicPlanExpressionV1, GeneralGemmSymbolicPlanV1,
};
use fe2o3_kernel_ir::{GeneralGemmKirDiagnosticV1, GeneralGemmPropertyV1};
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, BinOp, Body, Local, Operand, ProjectionElem, Rvalue, START_BLOCK,
    TerminatorKind,
};
use rustc_middle::ty::{FloatTy, Mutability, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use rustc_span::{Spanned, sym};
use sha2::{Digest, Sha256};

use crate::AmdGpuTarget;
use crate::collector::CollectionResult;
use crate::general_gemm_intrinsic_semantics_v1::{
    GeneralGemmIntrinsicSemanticsV1, GeneralGemmIntrinsicSourceFactV1,
};
use crate::trusted_device_items::{
    self, TrustedAmdGpuDiagnosticOperation, TrustedDeviceItem, TrustedGeneralGemmOperationV1,
    TrustedGeneralGemmSurfaceV1,
};

const EXACT_GENERAL_GEMM_TARGET_V1: &str = "gfx942:xnack-";
const MAX_GENERAL_GEMM_REACHABLE_CALLS_V1: usize = 512;
const MAX_GENERAL_GEMM_TERMINAL_CALLS_V1: usize = 32;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum GeneralGemmMirImportV1 {
    PositiveAnalysisBlocked,
    VerifiedMutationOracle,
    Rejected(GeneralGemmSemanticRejectionV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmSemanticRejectionV1 {
    diagnostic: GeneralGemmKirDiagnosticV1,
    root_symbol: String,
    source_span: String,
    terminal_spans: Vec<String>,
    reachable_call_chain: Vec<&'static str>,
}

impl fmt::Display for GeneralGemmSemanticRejectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}; kind=Counterexample; root symbol={}; source span={}; terminal spans={}; reachable call chain: {}",
            self.diagnostic,
            self.root_symbol,
            self.source_span,
            self.terminal_spans.join(","),
            self.reachable_call_chain.join(" -> ")
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmAbiRoleV1 {
    A,
    B,
    C,
    M,
    N,
    K,
    Lda,
    Ldb,
    Ldc,
    Alpha,
    Beta,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneralGemmAbiTypeV1 {
    SharedU16Slice,
    DisjointF32Slice,
    U32,
    F32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmAbiOperandBindingV1 {
    pub(crate) role: GeneralGemmAbiRoleV1,
    pub(crate) argument_index: u8,
    pub(crate) ty: GeneralGemmAbiTypeV1,
}

#[derive(Debug, Eq, PartialEq)]
#[cfg(test)]
pub(crate) struct ConsumedGeneralGemmSemanticTemplateV1 {
    pub(crate) kernel_instance: [u8; 32],
    pub(crate) compiled_source: [u8; 32],
    pub(crate) provider_semantics: [u8; 32],
    pub(crate) abi: [GeneralGemmAbiOperandBindingV1; 11],
    pub(crate) source_properties: [GeneralGemmSourcePropertyReceiptV1; 11],
    pub(crate) symbolic_plan: GeneralGemmSymbolicPlanV1,
    pub(crate) symbolic_kir: GeneralGemmSymbolicKirV1,
}

#[cfg(test)]
impl ConsumedGeneralGemmSemanticTemplateV1 {
    fn abi_identity(&self) -> [u8; 32] {
        general_gemm_abi_identity(&self.abi)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmFrontendCorrespondenceIdentityV1([u8; 32]);

impl GeneralGemmFrontendCorrespondenceIdentityV1 {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedGeneralGemmFrontendCorrespondenceV1 {
    binding: GeneralGemmFrontendSemanticBindingV1,
    identity: GeneralGemmFrontendCorrespondenceIdentityV1,
    source_properties: [GeneralGemmSourcePropertyReceiptV1; 11],
}

impl AuthenticatedGeneralGemmFrontendCorrespondenceV1 {
    pub(crate) const fn binding(&self) -> &GeneralGemmFrontendSemanticBindingV1 {
        &self.binding
    }

    pub(crate) const fn identity(&self) -> GeneralGemmFrontendCorrespondenceIdentityV1 {
        self.identity
    }

    pub(crate) const fn source_properties(&self) -> &[GeneralGemmSourcePropertyReceiptV1; 11] {
        &self.source_properties
    }

    pub(crate) fn revalidate(&self) -> bool {
        let Some(first) = self.source_properties.first() else {
            return false;
        };
        if self
            .source_properties
            .iter()
            .enumerate()
            .any(|(index, property)| {
                property.kind as usize != index + 1
                    || !property.revalidate()
                    || property.optimized_mir_closure != first.optimized_mir_closure
                    || property.provider_profile != first.provider_profile
            })
            || &first.provider_profile != self.binding.provider_semantics_identity()
        {
            return false;
        }
        let GeneralGemmSourceMirEvidenceV1::AllocationAndProvenance {
            abi_identity,
            root_compiled_source,
            store,
            ..
        } = &first.mir_evidence
        else {
            return false;
        };
        if abi_identity != self.binding.frontend_abi_identity()
            || root_compiled_source != self.binding.compiled_source_identity()
            || store.abi_identity != *abi_identity
        {
            return false;
        }
        let Ok(schema) = derived_schema_from_typestate_properties(&self.source_properties) else {
            return false;
        };
        let Ok(plan) = GeneralGemmSymbolicPlanV1::from_derived_source_schema(&schema) else {
            return false;
        };
        let Ok(kir) = GeneralGemmSymbolicKirV1::from_derived_source_schema(&schema) else {
            return false;
        };
        if plan != self.binding.symbolic_plan() || kir != self.binding.symbolic_kir() {
            return false;
        }
        let Ok(expected_binding) =
            GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
                *self.binding.kernel_instance_identity(),
                *self.binding.compiled_source_identity(),
                *self.binding.provider_semantics_identity(),
                *self.binding.frontend_abi_identity(),
                plan,
                kir,
            )
        else {
            return false;
        };
        if expected_binding != self.binding {
            return false;
        }
        let mut property_bytes = Vec::with_capacity(self.source_properties.len() * 33);
        for property in &self.source_properties {
            property_bytes.push(property.kind as u8);
            property_bytes.extend_from_slice(&property.evidence_identity);
        }
        self.identity.0
            == hash_fields(&[
                b"FE2O3/AUTHENTICATED-GENERAL-GEMM-FRONTEND-CORRESPONDENCE/V1\0",
                self.binding.identity().as_bytes(),
                self.binding.frontend_abi_identity(),
                &property_bytes,
            ])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum GeneralGemmSourcePropertyKindV1 {
    AllocationAndProvenance = 1,
    GuardedGlobalAccesses = 2,
    LdsWriteReadInitialization = 3,
    EffectConflictFreedom = 4,
    ControlFlowBarrierConvergence = 5,
    OutputOwnership = 6,
    LdsLifecycle = 7,
    AccumulatorPhase = 8,
    MaskedTail = 9,
    AlphaBetaEpilogue = 10,
    NumericalOperationOrder = 11,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmSourcePropertyReceiptV1 {
    kind: GeneralGemmSourcePropertyKindV1,
    intrinsic_fact: GeneralGemmIntrinsicSourceFactV1,
    mir_evidence: GeneralGemmSourceMirEvidenceV1,
    optimized_mir_closure: [u8; 32],
    provider_profile: [u8; 32],
    evidence_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmMirEventTranscriptV1 {
    operation: TrustedGeneralGemmOperationV1,
    block: u32,
    return_block: u32,
    result_local: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmAllPathsTranscriptV1 {
    from_block: u32,
    required_event: GeneralGemmMirEventTranscriptV1,
    boundary_block: u32,
    visited_region_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmPhaseCycleTranscriptV1 {
    acquire: GeneralGemmMirEventTranscriptV1,
    stage: GeneralGemmMirEventTranscriptV1,
    publish: GeneralGemmMirEventTranscriptV1,
    mfma: GeneralGemmMirEventTranscriptV1,
    reuse: GeneralGemmMirEventTranscriptV1,
    store: GeneralGemmMirEventTranscriptV1,
    stage_to_publish: GeneralGemmAllPathsTranscriptV1,
    publish_to_mfma: GeneralGemmAllPathsTranscriptV1,
    mfma_to_reuse: GeneralGemmAllPathsTranscriptV1,
    phase_split_block: u32,
    phase_cfg_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmGuardedLoaderTranscriptV1 {
    helper_def_path: [u8; 16],
    compiled_source_identity: [u8; 32],
    row_guard_block: u32,
    column_guard_block: u32,
    zero_return_block: u32,
    row_major_block: u32,
    extent_guard_block: u32,
    load_block: u32,
    trap_block: u32,
    dataflow_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmStageInputTranscriptV1 {
    stage: GeneralGemmMirEventTranscriptV1,
    guarded_loader: GeneralGemmGuardedLoaderTranscriptV1,
    coordinate_dataflow_identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmStoreTranscriptV1 {
    store: GeneralGemmMirEventTranscriptV1,
    abi_identity: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names, dead_code)]
// The variants are retained for correspondence revalidation, but production
// construction stays disabled with the positive importer boundary.
pub(crate) enum GeneralGemmSourceMirEvidenceV1 {
    AllocationAndProvenance {
        abi_identity: [u8; 32],
        root_compiled_source: [u8; 32],
        stage_inputs: GeneralGemmStageInputTranscriptV1,
        store: GeneralGemmStoreTranscriptV1,
    },
    GuardedGlobalAccesses {
        stage_inputs: GeneralGemmStageInputTranscriptV1,
        store: GeneralGemmStoreTranscriptV1,
    },
    LdsWriteReadInitialization {
        phase: GeneralGemmPhaseCycleTranscriptV1,
    },
    EffectConflictFreedom {
        phase: GeneralGemmPhaseCycleTranscriptV1,
        stage_inputs: GeneralGemmStageInputTranscriptV1,
        store: GeneralGemmStoreTranscriptV1,
    },
    ControlFlowBarrierConvergence {
        stage_to_publish: GeneralGemmAllPathsTranscriptV1,
        mfma_to_reuse: GeneralGemmAllPathsTranscriptV1,
    },
    OutputOwnership {
        phase: GeneralGemmPhaseCycleTranscriptV1,
        store: GeneralGemmStoreTranscriptV1,
    },
    LdsLifecycle {
        phase: GeneralGemmPhaseCycleTranscriptV1,
    },
    AccumulatorPhase {
        phase: GeneralGemmPhaseCycleTranscriptV1,
    },
    MaskedTail {
        stage_inputs: GeneralGemmStageInputTranscriptV1,
        store: GeneralGemmStoreTranscriptV1,
    },
    AlphaBetaEpilogue {
        store: GeneralGemmStoreTranscriptV1,
    },
    NumericalOperationOrder {
        stage_inputs: GeneralGemmStageInputTranscriptV1,
        phase: GeneralGemmPhaseCycleTranscriptV1,
        store: GeneralGemmStoreTranscriptV1,
    },
}

fn evidence_kind(evidence: &GeneralGemmSourceMirEvidenceV1) -> GeneralGemmSourcePropertyKindV1 {
    use GeneralGemmSourceMirEvidenceV1 as Evidence;
    use GeneralGemmSourcePropertyKindV1 as Kind;
    match evidence {
        Evidence::AllocationAndProvenance { .. } => Kind::AllocationAndProvenance,
        Evidence::GuardedGlobalAccesses { .. } => Kind::GuardedGlobalAccesses,
        Evidence::LdsWriteReadInitialization { .. } => Kind::LdsWriteReadInitialization,
        Evidence::EffectConflictFreedom { .. } => Kind::EffectConflictFreedom,
        Evidence::ControlFlowBarrierConvergence { .. } => Kind::ControlFlowBarrierConvergence,
        Evidence::OutputOwnership { .. } => Kind::OutputOwnership,
        Evidence::LdsLifecycle { .. } => Kind::LdsLifecycle,
        Evidence::AccumulatorPhase { .. } => Kind::AccumulatorPhase,
        Evidence::MaskedTail { .. } => Kind::MaskedTail,
        Evidence::AlphaBetaEpilogue { .. } => Kind::AlphaBetaEpilogue,
        Evidence::NumericalOperationOrder { .. } => Kind::NumericalOperationOrder,
    }
}

fn encode_event(bytes: &mut Vec<u8>, event: GeneralGemmMirEventTranscriptV1) {
    bytes.push(event.operation as u8);
    bytes.extend_from_slice(&event.block.to_le_bytes());
    bytes.extend_from_slice(&event.return_block.to_le_bytes());
    match event.result_local {
        Some(local) => {
            bytes.push(1);
            bytes.extend_from_slice(&local.to_le_bytes());
        }
        None => bytes.push(0),
    }
}

fn encode_all_paths(bytes: &mut Vec<u8>, path: GeneralGemmAllPathsTranscriptV1) {
    bytes.extend_from_slice(&path.from_block.to_le_bytes());
    encode_event(bytes, path.required_event);
    bytes.extend_from_slice(&path.boundary_block.to_le_bytes());
    bytes.extend_from_slice(&path.visited_region_identity);
}

fn encode_phase(bytes: &mut Vec<u8>, phase: GeneralGemmPhaseCycleTranscriptV1) {
    for event in [
        phase.acquire,
        phase.stage,
        phase.publish,
        phase.mfma,
        phase.reuse,
        phase.store,
    ] {
        encode_event(bytes, event);
    }
    encode_all_paths(bytes, phase.stage_to_publish);
    encode_all_paths(bytes, phase.publish_to_mfma);
    encode_all_paths(bytes, phase.mfma_to_reuse);
    bytes.extend_from_slice(&phase.phase_split_block.to_le_bytes());
    bytes.extend_from_slice(&phase.phase_cfg_identity);
}

fn encode_loader(bytes: &mut Vec<u8>, loader: GeneralGemmGuardedLoaderTranscriptV1) {
    bytes.extend_from_slice(&loader.helper_def_path);
    bytes.extend_from_slice(&loader.compiled_source_identity);
    for block in [
        loader.row_guard_block,
        loader.column_guard_block,
        loader.zero_return_block,
        loader.row_major_block,
        loader.extent_guard_block,
        loader.load_block,
        loader.trap_block,
    ] {
        bytes.extend_from_slice(&block.to_le_bytes());
    }
    bytes.extend_from_slice(&loader.dataflow_identity);
}

fn encode_stage(bytes: &mut Vec<u8>, stage: GeneralGemmStageInputTranscriptV1) {
    encode_event(bytes, stage.stage);
    encode_loader(bytes, stage.guarded_loader);
    bytes.extend_from_slice(&stage.coordinate_dataflow_identity);
}

fn encode_store(bytes: &mut Vec<u8>, store: GeneralGemmStoreTranscriptV1) {
    encode_event(bytes, store.store);
    bytes.extend_from_slice(&store.abi_identity);
}

fn encode_source_mir_evidence(evidence: &GeneralGemmSourceMirEvidenceV1) -> Vec<u8> {
    use GeneralGemmSourceMirEvidenceV1 as Evidence;
    let mut bytes = vec![evidence_kind(evidence) as u8];
    match evidence {
        Evidence::AllocationAndProvenance {
            abi_identity,
            root_compiled_source,
            stage_inputs,
            store,
        } => {
            bytes.extend_from_slice(abi_identity);
            bytes.extend_from_slice(root_compiled_source);
            encode_stage(&mut bytes, *stage_inputs);
            encode_store(&mut bytes, *store);
        }
        Evidence::GuardedGlobalAccesses {
            stage_inputs,
            store,
        }
        | Evidence::MaskedTail {
            stage_inputs,
            store,
        } => {
            encode_stage(&mut bytes, *stage_inputs);
            encode_store(&mut bytes, *store);
        }
        Evidence::LdsWriteReadInitialization { phase }
        | Evidence::LdsLifecycle { phase }
        | Evidence::AccumulatorPhase { phase } => encode_phase(&mut bytes, *phase),
        Evidence::EffectConflictFreedom {
            phase,
            stage_inputs,
            store,
        }
        | Evidence::NumericalOperationOrder {
            stage_inputs,
            phase,
            store,
        } => {
            encode_phase(&mut bytes, *phase);
            encode_stage(&mut bytes, *stage_inputs);
            encode_store(&mut bytes, *store);
        }
        Evidence::ControlFlowBarrierConvergence {
            stage_to_publish,
            mfma_to_reuse,
        } => {
            encode_all_paths(&mut bytes, *stage_to_publish);
            encode_all_paths(&mut bytes, *mfma_to_reuse);
        }
        Evidence::OutputOwnership { phase, store } => {
            encode_phase(&mut bytes, *phase);
            encode_store(&mut bytes, *store);
        }
        Evidence::AlphaBetaEpilogue { store } => encode_store(&mut bytes, *store),
    }
    bytes
}

impl GeneralGemmSourcePropertyReceiptV1 {
    pub(crate) const fn kind(&self) -> GeneralGemmSourcePropertyKindV1 {
        self.kind
    }

    pub(crate) const fn evidence_identity(&self) -> &[u8; 32] {
        &self.evidence_identity
    }

    pub(crate) fn revalidate(&self) -> bool {
        let semantics = GeneralGemmIntrinsicSemanticsV1::canonical();
        if semantics.validate().is_err()
            || self.intrinsic_fact != semantics.source_facts()[(self.kind as usize) - 1]
            || self.optimized_mir_closure == [0; 32]
            || self.provider_profile == [0; 32]
            || evidence_kind(&self.mir_evidence) != self.kind
        {
            return false;
        }
        let evidence = encode_source_mir_evidence(&self.mir_evidence);
        self.evidence_identity
            == hash_fields(&[
                b"FE2O3/GENERAL-GEMM-SOURCE-PROPERTY-RECEIPT/V1\0",
                &[self.kind as u8],
                &self.optimized_mir_closure,
                &self.provider_profile,
                &self.intrinsic_fact.identity(semantics.identity()),
                &evidence,
            ])
    }
}

#[derive(Debug, Eq, PartialEq)]
#[cfg(test)]
pub(crate) struct AuthenticatedGeneralGemmSemanticReceiptV1 {
    consumed: Option<ConsumedGeneralGemmSemanticTemplateV1>,
}

#[derive(Debug)]
#[cfg(test)]
pub(crate) enum GeneralGemmReceiptConsumptionErrorV1 {
    SourcePropertyRevalidation,
    Binding(GeneralGemmFrontendSemanticBindingErrorV1),
}

#[cfg(test)]
impl fmt::Display for GeneralGemmReceiptConsumptionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourcePropertyRevalidation => formatter.write_str(
                "authenticated general GEMM source-property receipts failed owner revalidation",
            ),
            Self::Binding(error) => write!(formatter, "{error:?}"),
        }
    }
}

#[cfg(test)]
impl AuthenticatedGeneralGemmSemanticReceiptV1 {
    pub(crate) fn into_verified_template(
        mut self,
    ) -> Result<
        AuthenticatedGeneralGemmFrontendCorrespondenceV1,
        GeneralGemmReceiptConsumptionErrorV1,
    > {
        let consumed = self
            .consumed
            .take()
            .expect("authenticated general GEMM receipt is consumed once");
        let frontend_abi = consumed.abi_identity();
        let mut property_bytes = Vec::with_capacity(consumed.source_properties.len() * 33);
        for (index, property) in consumed.source_properties.iter().enumerate() {
            if property.kind as usize != index + 1 || !property.revalidate() {
                return Err(GeneralGemmReceiptConsumptionErrorV1::SourcePropertyRevalidation);
            }
            property_bytes.push(property.kind as u8);
            property_bytes.extend_from_slice(&property.evidence_identity);
        }
        let binding =
            GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
                consumed.kernel_instance,
                consumed.compiled_source,
                consumed.provider_semantics,
                frontend_abi,
                consumed.symbolic_plan,
                consumed.symbolic_kir,
            )
            .map_err(GeneralGemmReceiptConsumptionErrorV1::Binding)?;
        let identity = GeneralGemmFrontendCorrespondenceIdentityV1(hash_fields(&[
            b"FE2O3/AUTHENTICATED-GENERAL-GEMM-FRONTEND-CORRESPONDENCE/V1\0",
            binding.identity().as_bytes(),
            &frontend_abi,
            &property_bytes,
        ]));
        let correspondence = AuthenticatedGeneralGemmFrontendCorrespondenceV1 {
            binding,
            identity,
            source_properties: consumed.source_properties,
        };
        if !correspondence.revalidate() {
            return Err(GeneralGemmReceiptConsumptionErrorV1::SourcePropertyRevalidation);
        }
        Ok(correspondence)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralGemmMirImportErrorV1 {
    message: String,
}

impl GeneralGemmMirImportErrorV1 {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for GeneralGemmMirImportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GeneralGemmMirImportErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneralGemmCallV1 {
    surface: TrustedGeneralGemmSurfaceV1,
    operation: TrustedGeneralGemmOperationV1,
    block: BasicBlock,
    return_target: BasicBlock,
    result_local: Option<Local>,
    span: rustc_span::Span,
    evidence: GeneralGemmEvidenceV1,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct GeneralGemmCallBudgetV1 {
    reachable_calls: usize,
    general_gemm_terminals: usize,
}

impl GeneralGemmCallBudgetV1 {
    fn observe_reachable_call(&mut self) -> Result<(), GeneralGemmMirImportErrorV1> {
        self.reachable_calls = self
            .reachable_calls
            .checked_add(1)
            .filter(|count| *count <= MAX_GENERAL_GEMM_REACHABLE_CALLS_V1)
            .ok_or_else(|| {
                GeneralGemmMirImportErrorV1::new(format!(
                    "general GEMM MIR closure exceeds the {MAX_GENERAL_GEMM_REACHABLE_CALLS_V1}-call analysis limit"
                ))
            })?;
        Ok(())
    }

    fn observe_general_gemm_terminal(&mut self) -> Result<(), GeneralGemmMirImportErrorV1> {
        self.general_gemm_terminals = self
            .general_gemm_terminals
            .checked_add(1)
            .filter(|count| *count <= MAX_GENERAL_GEMM_TERMINAL_CALLS_V1)
            .ok_or_else(|| {
                GeneralGemmMirImportErrorV1::new(format!(
                    "general GEMM MIR closure exceeds the {MAX_GENERAL_GEMM_TERMINAL_CALLS_V1}-terminal analysis limit"
                ))
            })?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralGemmEvidenceV1 {
    None,
    UnguardedA,
    UnguardedB,
    NonzeroTail,
    WrongEpilogue,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SymbolicValueV1 {
    KernelArgument(u8),
    WaveField(u8),
    Constant(u128),
    Add(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
    Remainder(Box<Self>, Box<Self>),
    LessThan(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SymbolicF32ValueV1 {
    KernelArgument(u8),
    OpaqueLocal(usize),
    Constant(u32),
    Add(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ProofSymbolicValueV1 {
    KernelArgument(u8),
    Lane,
    WorkgroupX,
    WorkgroupY,
    Phase,
    Component,
    Constant(u128),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    Divide(Box<Self>, Box<Self>),
    Remainder(Box<Self>, Box<Self>),
    BitXor(Box<Self>, Box<Self>),
}

pub(crate) fn try_import_general_gemm_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    target: &AmdGpuTarget,
) -> Result<Option<GeneralGemmMirImportV1>, GeneralGemmMirImportErrorV1> {
    let mut root = None;
    let mut root_function = None;
    let mut root_calls = Vec::new();
    let mut saw_general_gemm = false;
    let mut call_budget = GeneralGemmCallBudgetV1::default();

    for function in &collection.functions {
        let body = tcx.instance_mir(function.instance.def);
        let calls = general_gemm_calls(tcx, body, &mut call_budget)?;
        if calls.is_empty() {
            continue;
        }
        saw_general_gemm = true;
        if !function.is_kernel_entry() {
            return Err(GeneralGemmMirImportErrorV1::new(
                "general GEMM terminal remained in a collected helper; the V1 adapter requires direct calls in exactly one kernel root",
            ));
        }
        if root.replace(body).is_some() {
            return Err(GeneralGemmMirImportErrorV1::new(
                "general GEMM terminals occur in more than one kernel root",
            ));
        }
        root_function = Some(function);
        root_calls = calls;
    }

    if !saw_general_gemm {
        return Ok(None);
    }
    if target.as_str() != EXACT_GENERAL_GEMM_TARGET_V1 {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM V1 requires exact target `{EXACT_GENERAL_GEMM_TARGET_V1}`, found `{target}`"
        )));
    }
    let body = root.ok_or_else(|| {
        GeneralGemmMirImportErrorV1::new(
            "general GEMM terminal analysis lost its authenticated kernel root",
        )
    })?;
    let surface = unique_surface(&root_calls)?;
    let root_function = root_function.ok_or_else(|| {
        GeneralGemmMirImportErrorV1::new(
            "general GEMM terminal analysis lost its collected kernel metadata",
        )
    })?;
    require_positive_abi(tcx, root_function.instance.def_id())?;
    let dynamic_mutation_oracle = surface == TrustedGeneralGemmSurfaceV1::ProofSensitive
        && (call_count(&root_calls, TrustedGeneralGemmOperationV1::MfmaValue) != 0
            || call_count(&root_calls, TrustedGeneralGemmOperationV1::StoreEpilogue) != 0);
    if surface == TrustedGeneralGemmSurfaceV1::ProofSensitive {
        require_counterexample_abi_binding(body, &root_calls)?;
    }
    let early_counterexample = if dynamic_mutation_oracle {
        derived_dynamic_counterexample(tcx, body, &root_calls)?
    } else {
        None
    };
    let lane_conditional_publish =
        if call_count(&root_calls, TrustedGeneralGemmOperationV1::Publish) == 0 {
            false
        } else {
            publish_is_lane_conditional(body, &root_calls)?
        };
    if early_counterexample.is_none() {
        validate_call_shape(body, surface, &root_calls, lane_conditional_publish)?;
    }
    let counterexample = early_counterexample.or_else(|| {
        (!dynamic_mutation_oracle)
            .then(|| derived_counterexample(&root_calls, lane_conditional_publish))
            .flatten()
    });
    if let Some((diagnostic, call_chain)) = counterexample {
        if !call_chain
            .iter()
            .all(|call| reachable(body, START_BLOCK, call.block))
        {
            return Err(unproved(
                "counterexample event is reachable from the kernel root",
            ));
        }
        let root_span = tcx.def_span(root_function.instance.def_id());
        let start = tcx.sess.source_map().lookup_char_pos(root_span.lo());
        let end = tcx.sess.source_map().lookup_char_pos(root_span.hi());
        let source_span = format!(
            "{}:{}:{}-{}:{}",
            start
                .file
                .name
                .prefer_remapped_unconditionally()
                .to_string_lossy(),
            start.line,
            start.col.0 + 1,
            end.line,
            end.col.0 + 1
        );
        let mut reachable_call_chain = vec!["kernel-root"];
        reachable_call_chain.extend(call_chain.iter().map(|call| operation_name(call.operation)));
        let terminal_spans = call_chain
            .iter()
            .map(|call| {
                let location = tcx.sess.source_map().lookup_char_pos(call.span.lo());
                format!(
                    "{}:{}:{}",
                    location
                        .file
                        .name
                        .prefer_remapped_unconditionally()
                        .to_string_lossy(),
                    location.line,
                    location.col.0 + 1
                )
            })
            .collect();
        return Ok(Some(GeneralGemmMirImportV1::Rejected(
            GeneralGemmSemanticRejectionV1 {
                diagnostic,
                root_symbol: root_function.export_name.clone(),
                source_span,
                terminal_spans,
                reachable_call_chain,
            },
        )));
    }
    validate_call_shape(body, surface, &root_calls, lane_conditional_publish)?;
    if surface == TrustedGeneralGemmSurfaceV1::ProofSensitive {
        require_dynamic_terminal_inventory(&root_calls)?;
        require_guarded_dynamic_accesses(tcx, body, &root_calls)?;
        require_dynamic_lds_mapping(tcx, body, &root_calls)?;
        require_dynamic_accumulator_carry(tcx, body, &root_calls)?;
        return Ok(Some(GeneralGemmMirImportV1::VerifiedMutationOracle));
    }
    validate_positive_source_non_authoritative(
        tcx,
        root_function.instance.def_id(),
        body,
        &root_calls,
    )?;
    Ok(Some(GeneralGemmMirImportV1::PositiveAnalysisBlocked))
}

fn derived_counterexample(
    calls: &[GeneralGemmCallV1],
    lane_conditional_publish: bool,
) -> Option<(GeneralGemmKirDiagnosticV1, Vec<&GeneralGemmCallV1>)> {
    if let Some(call) = calls.iter().find(|call| {
        matches!(
            call.evidence,
            GeneralGemmEvidenceV1::UnguardedA | GeneralGemmEvidenceV1::UnguardedB
        )
    }) {
        return Some((diagnostic(GeneralGemmPropertyV1::BoundsSafe), vec![call]));
    }
    let stores = store_calls(calls);
    if stores.len() == 2 {
        return Some((
            diagnostic(GeneralGemmPropertyV1::OutputRegionInjective),
            stores,
        ));
    }
    if lane_conditional_publish {
        return Some((
            diagnostic(GeneralGemmPropertyV1::BarrierConvergent),
            optional_call(calls, TrustedGeneralGemmOperationV1::Publish)
                .ok()
                .flatten()
                .into_iter()
                .collect(),
        ));
    }
    if call_count(calls, TrustedGeneralGemmOperationV1::Publish) == 0 {
        return Some((
            diagnostic(GeneralGemmPropertyV1::Initialized),
            calls
                .iter()
                .find(|call| {
                    matches!(
                        call.operation,
                        TrustedGeneralGemmOperationV1::Mfma
                            | TrustedGeneralGemmOperationV1::MfmaValue
                    )
                })
                .into_iter()
                .collect(),
        ));
    }
    if let Some(call) = calls
        .iter()
        .find(|call| call.evidence == GeneralGemmEvidenceV1::NonzeroTail)
    {
        return Some((
            diagnostic(GeneralGemmPropertyV1::TailRefinement),
            vec![call],
        ));
    }
    if let Some(call) = calls
        .iter()
        .find(|call| call.evidence == GeneralGemmEvidenceV1::WrongEpilogue)
    {
        return Some((
            diagnostic(GeneralGemmPropertyV1::EpilogueRefinement),
            vec![call],
        ));
    }
    None
}

fn derived_dynamic_counterexample<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &'a [GeneralGemmCallV1],
) -> Result<
    Option<(GeneralGemmKirDiagnosticV1, Vec<&'a GeneralGemmCallV1>)>,
    GeneralGemmMirImportErrorV1,
> {
    require_dynamic_mutation_oracle_shape(calls)?;

    let load_a = unique_call(calls, TrustedGeneralGemmOperationV1::LoadA)?;
    let load_a_args = call_args(body, load_a.block)?;
    let a_row_guard = has_true_lt_guard(
        tcx,
        body,
        load_a.block,
        &load_a_args[2].node,
        &load_a_args[4].node,
    );
    let a_depth_guard = has_true_lt_guard(
        tcx,
        body,
        load_a.block,
        &load_a_args[3].node,
        &load_a_args[5].node,
    );
    if !a_row_guard && a_depth_guard {
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::BoundsSafe),
            vec![load_a],
        )));
    }
    if !a_row_guard || !a_depth_guard {
        return Err(unproved(
            "A load has exactly the row<M and depth<K guards or the named single-guard counterexample",
        ));
    }

    let load_b = unique_call(calls, TrustedGeneralGemmOperationV1::LoadB)?;
    let load_b_args = call_args(body, load_b.block)?;
    let b_depth_guard = has_true_lt_guard(
        tcx,
        body,
        load_b.block,
        &load_b_args[2].node,
        &load_b_args[4].node,
    );
    let b_column_guard = has_true_lt_guard(
        tcx,
        body,
        load_b.block,
        &load_b_args[3].node,
        &load_b_args[5].node,
    );
    if !b_depth_guard && b_column_guard {
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::BoundsSafe),
            vec![load_b],
        )));
    }
    if !b_depth_guard || !b_column_guard {
        return Err(unproved(
            "B load has exactly the depth<K and column<N guards or the named single-guard counterexample",
        ));
    }

    let phase = dynamic_phase_local(body, calls)?;
    let stores = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::StoreEpilogue)
        .collect::<Vec<_>>();
    for store in &stores {
        let args = call_args(body, store.block)?;
        let row = proof_symbolic_operand(tcx, body, calls, phase, &args[2].node)?;
        let column = proof_symbolic_operand(tcx, body, calls, phase, &args[3].node)?;
        if row == ProofSymbolicValueV1::KernelArgument(3)
            && column != ProofSymbolicValueV1::KernelArgument(4)
        {
            return Ok(Some((
                diagnostic(GeneralGemmPropertyV1::BoundsSafe),
                vec![*store],
            )));
        }
    }

    let lane = ProofSymbolicValueV1::Lane;
    let lane_row = proof_rem(lane.clone(), proof_constant(16));
    let expected_row_base = proof_add(
        proof_mul(ProofSymbolicValueV1::WorkgroupY, proof_constant(16)),
        proof_mul(
            proof_constant(4),
            proof_div(lane.clone(), proof_constant(16)),
        ),
    );
    let expected_column = proof_add(
        proof_mul(ProofSymbolicValueV1::WorkgroupX, proof_constant(16)),
        lane_row.clone(),
    );
    let lane_collision_column = proof_mul(ProofSymbolicValueV1::WorkgroupX, proof_constant(16));
    let workgroup_collision_column = lane_row.clone();
    for (component, store) in stores.iter().enumerate() {
        let args = call_args(body, store.block)?;
        let row = proof_symbolic_operand(tcx, body, calls, phase, &args[2].node)?;
        let column = proof_symbolic_operand(tcx, body, calls, phase, &args[3].node)?;
        if component == 0
            && row == expected_row_base
            && (column == lane_collision_column || column == workgroup_collision_column)
        {
            return Ok(Some((
                diagnostic(GeneralGemmPropertyV1::OutputRegionInjective),
                vec![*store],
            )));
        }
        let expected_row = if component == 0 {
            expected_row_base.clone()
        } else {
            proof_add(expected_row_base.clone(), proof_constant(component as u128))
        };
        if row != expected_row || column != expected_column {
            return Err(unproved(
                "C stores have exact grid-XY16 lane/component ownership or a derived lane/workgroup collision",
            ));
        }
        if !has_true_lt_guard(tcx, body, store.block, &args[2].node, &args[4].node)
            || !has_true_lt_guard(tcx, body, store.block, &args[3].node, &args[5].node)
        {
            return Err(unproved(
                "C stores are dominated by exact row<M and column<N guards",
            ));
        }
    }

    let stages = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::StageValue)
        .collect::<Vec<_>>();
    let depth_base = proof_mul(
        proof_constant(4),
        proof_div(lane.clone(), proof_constant(16)),
    );
    let tile_depth = proof_add(depth_base.clone(), ProofSymbolicValueV1::Component);
    let swizzle = proof_xor(
        tile_depth,
        proof_mul(
            proof_constant(4),
            proof_rem(lane_row.clone(), proof_constant(4)),
        ),
    );
    let expected_a_slot = proof_add(
        proof_mul(proof_constant(16), lane_row.clone()),
        swizzle.clone(),
    );
    let expected_b_slot = proof_add(
        proof_add(
            proof_constant(256),
            proof_mul(proof_constant(16), lane_row.clone()),
        ),
        swizzle,
    );
    let stage_slots = stages
        .iter()
        .map(|stage| {
            let args = call_args(body, stage.block)?;
            Ok((
                *stage,
                proof_symbolic_operand(tcx, body, calls, phase, &args[1].node)?,
                proof_symbolic_operand(tcx, body, calls, phase, &args[2].node)?,
            ))
        })
        .collect::<Result<Vec<_>, GeneralGemmMirImportErrorV1>>()?;
    if let Some((stage, _, _)) = stage_slots
        .iter()
        .find(|(_, slot, epoch)| *slot == lane_row && *epoch == ProofSymbolicValueV1::Phase)
    {
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::RaceFree),
            vec![*stage],
        )));
    }
    if stage_slots.len() == 1
        && stage_slots[0].1 == expected_a_slot
        && stage_slots[0].2 == ProofSymbolicValueV1::Phase
    {
        let first_b_read = calls
            .iter()
            .filter(|call| call.operation == TrustedGeneralGemmOperationV1::ReadStage)
            .nth(1)
            .ok_or_else(|| unproved("missing B stage retains a B LDS read witness"))?;
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::Initialized),
            vec![stage_slots[0].0, first_b_read],
        )));
    }
    if stage_slots.len() != 2
        || stage_slots[0].1 != expected_a_slot
        || stage_slots[1].1 != expected_b_slot
        || stage_slots
            .iter()
            .any(|(_, _, epoch)| *epoch != ProofSymbolicValueV1::Phase)
    {
        return Err(unproved(
            "two stage sites derive disjoint XOR4 A/B slots in the current phase epoch",
        ));
    }

    let publish_count = call_count(calls, TrustedGeneralGemmOperationV1::Publish);
    if publish_count == 0 {
        let first_mfma = calls
            .iter()
            .find(|call| call.operation == TrustedGeneralGemmOperationV1::MfmaValue)
            .ok_or_else(|| unproved("missing publish retains a consuming MFMA witness"))?;
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::Initialized),
            vec![first_mfma],
        )));
    }
    let publish = unique_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    if publish_is_lane_conditional(body, calls)? {
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::BarrierConvergent),
            vec![publish],
        )));
    }
    if !dominates(body, stage.block, publish.block) {
        return Err(unproved("publish is dominated by the complete stage event"));
    }

    if call_count(calls, TrustedGeneralGemmOperationV1::Reuse) == 0 {
        let last_mfma = calls
            .iter()
            .rfind(|call| call.operation == TrustedGeneralGemmOperationV1::MfmaValue)
            .ok_or_else(|| unproved("missing reuse retains a completed MFMA witness"))?;
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::LdsEpochCorrect),
            vec![last_mfma],
        )));
    }

    let reads = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::ReadStage)
        .collect::<Vec<_>>();
    for read in &reads {
        let args = call_args(body, read.block)?;
        let epoch = proof_symbolic_operand(tcx, body, calls, phase, &args[2].node)?;
        if epoch
            == ProofSymbolicValueV1::Subtract(
                Box::new(ProofSymbolicValueV1::Phase),
                Box::new(proof_constant(1)),
            )
        {
            return Ok(Some((
                diagnostic(GeneralGemmPropertyV1::LdsEpochCorrect),
                vec![*read],
            )));
        }
        if epoch != ProofSymbolicValueV1::Phase {
            return Err(unproved("every LDS read uses the current phase epoch"));
        }
    }

    let wait = unique_call(calls, TrustedGeneralGemmOperationV1::WaitStage)?;
    let wait_args = call_args(body, wait.block)?;
    if proof_symbolic_operand(tcx, body, calls, phase, &wait_args[1].node)?
        != ProofSymbolicValueV1::Phase
    {
        return Err(unproved("stage wait binds the current phase epoch"));
    }
    if let Some(read) = reads
        .iter()
        .find(|read| !dominates(body, wait.block, read.block))
    {
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::Initialized),
            vec![*read, wait],
        )));
    }
    if reads
        .iter()
        .any(|read| !dominates(body, publish.block, read.block))
    {
        return Err(unproved("convergent publish dominates every LDS read"));
    }

    let mfmas = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::MfmaValue)
        .collect::<Vec<_>>();
    for mfma in &mfmas {
        let args = call_args(body, mfma.block)?;
        if symbolic_f32_operand(tcx, body, &args[3].node, 0, &mut BTreeSet::new())
            == Some(SymbolicF32ValueV1::Constant(0.0_f32.to_bits()))
        {
            return Ok(Some((
                diagnostic(GeneralGemmPropertyV1::AccumulatorPhaseRefinement),
                vec![*mfma],
            )));
        }
    }

    for stage in &stages {
        let args = call_args(body, stage.block)?;
        let Some(value) = args.get(5).and_then(|arg| operand_local(&arg.node)) else {
            return Err(unproved("staged tail value has local MIR provenance"));
        };
        let constants = local_u16_constants(tcx, body, value, 0, &mut BTreeSet::new());
        if constants.iter().any(|value| *value != 0) {
            return Ok(Some((
                diagnostic(GeneralGemmPropertyV1::TailRefinement),
                vec![*stage],
            )));
        }
        if !constants.contains(&0) {
            return Err(unproved(
                "each staged A/B value has a CFG-derived positive-zero tail assignment",
            ));
        }
    }

    if let Some(store) = stores
        .iter()
        .find(|call| call.evidence == GeneralGemmEvidenceV1::WrongEpilogue)
    {
        return Ok(Some((
            diagnostic(GeneralGemmPropertyV1::EpilogueRefinement),
            vec![*store],
        )));
    }
    Ok(None)
}

fn require_dynamic_mutation_oracle_shape(
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    for (operation, expected) in [
        (TrustedGeneralGemmOperationV1::Acquire, 1),
        (TrustedGeneralGemmOperationV1::Lane, 1),
        (TrustedGeneralGemmOperationV1::WorkgroupX, 1),
        (TrustedGeneralGemmOperationV1::WorkgroupY, 1),
        (TrustedGeneralGemmOperationV1::LoadA, 1),
        (TrustedGeneralGemmOperationV1::LoadB, 1),
        (TrustedGeneralGemmOperationV1::Stage, 1),
        (TrustedGeneralGemmOperationV1::WaitStage, 1),
        (TrustedGeneralGemmOperationV1::ReadStage, 8),
        (TrustedGeneralGemmOperationV1::Mfma, 0),
        (TrustedGeneralGemmOperationV1::MfmaValue, 4),
        (TrustedGeneralGemmOperationV1::Store, 0),
        (TrustedGeneralGemmOperationV1::LoadC, 4),
        (TrustedGeneralGemmOperationV1::StoreEpilogue, 4),
    ] {
        let observed = call_count(calls, operation);
        if observed != expected {
            return Err(unproved(&format!(
                "full mutation-oracle baseline has {expected} {} event(s), observed {observed}",
                operation_name(operation),
            )));
        }
    }
    for (operation, minimum, maximum) in [
        (TrustedGeneralGemmOperationV1::StageValue, 1, 2),
        (TrustedGeneralGemmOperationV1::Publish, 0, 1),
        (TrustedGeneralGemmOperationV1::Reuse, 0, 1),
    ] {
        let observed = call_count(calls, operation);
        if !(minimum..=maximum).contains(&observed) {
            return Err(unproved(&format!(
                "full mutation-oracle baseline has {minimum} through {maximum} {} event(s), observed {observed}",
                operation_name(operation),
            )));
        }
    }
    Ok(())
}

fn dynamic_phase_local(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<Local, GeneralGemmMirImportErrorV1> {
    calls
        .iter()
        .find(|call| call.operation == TrustedGeneralGemmOperationV1::StageValue)
        .ok_or_else(|| unproved("one stage value retains the loop-carried phase"))
        .and_then(|stage| call_args(body, stage.block))?
        .get(2)
        .and_then(|arg| operand_local(&arg.node))
        .map(|local| canonical_local_alias_root(body, local))
        .ok_or_else(|| unproved("stage epoch has one loop-carried local"))
}

fn local_u16_constants<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    local: Local,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> BTreeSet<u16> {
    if depth >= 32 || !visiting.insert(local) {
        return BTreeSet::new();
    }
    let mut constants = BTreeSet::new();
    for value in body
        .basic_blocks
        .iter()
        .flat_map(|data| &data.statements)
        .filter_map(|statement| statement.kind.as_assign())
        .filter_map(|(destination, value)| (destination.as_local() == Some(local)).then_some(value))
    {
        if let Rvalue::Use(Operand::Constant(constant)) = value
            && let Some(value) = constant_u16_from_constant(tcx, constant)
        {
            constants.insert(value);
        } else if let Rvalue::Use(operand) | Rvalue::Cast(_, operand, _) = value
            && let Some(source) = operand_local(operand)
        {
            constants.extend(local_u16_constants(tcx, body, source, depth + 1, visiting));
        }
    }
    visiting.remove(&local);
    constants
}

fn diagnostic(property: GeneralGemmPropertyV1) -> GeneralGemmKirDiagnosticV1 {
    GeneralGemmKirDiagnosticV1 {
        property,
        stage: property.verification_stage(),
        code: property.diagnostic_code(),
        event_index: None,
    }
}

fn validate_positive_source_non_authoritative<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: rustc_hir::def_id::DefId,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    require_positive_abi(tcx, root)?;
    require_terminal_abi_binding(body, calls)?;
    require_positive_root_lifecycle_coverage(tcx, body, calls)?;
    derive_phase_cycle_transcript(tcx, body, calls)?;
    require_guarded_stage_inputs(tcx, body, calls)?;
    let store = store_calls(calls);
    let [store] = store.as_slice() else {
        return Err(unproved("positive typestate kernel has one output store"));
    };
    if store.operation != TrustedGeneralGemmOperationV1::Store {
        return Err(unproved(
            "positive typestate kernel uses the sealed canonical store",
        ));
    }
    Ok(())
}

fn require_positive_root_lifecycle_coverage<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let store = unique_call(calls, TrustedGeneralGemmOperationV1::Store)?;
    require_all_nontrapping_paths_reach(
        tcx,
        body,
        START_BLOCK,
        acquire.block,
        "the canonical acquire before any normal return",
    )?;
    require_all_nontrapping_paths_reach(
        tcx,
        body,
        START_BLOCK,
        store.block,
        "the canonical store before every normal return",
    )?;
    if !dominates(body, acquire.block, stage.block) || !dominates(body, acquire.block, store.block)
    {
        return Err(unproved(
            "the exact acquire dominates both phase entry and the canonical store",
        ));
    }
    if reachable(body, store.return_target, acquire.block)
        || reachable(body, store.return_target, stage.block)
    {
        return Err(unproved(
            "the canonical store has no backedge to acquire or phase entry",
        ));
    }
    Ok(())
}

fn require_all_nontrapping_paths_reach<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    from: BasicBlock,
    required: BasicBlock,
    property: &str,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mut pending = VecDeque::from([from]);
    let mut visited = BTreeSet::new();
    let mut reached_required = false;
    while let Some(block) = pending.pop_front() {
        if block == required {
            reached_required = true;
            continue;
        }
        if !visited.insert(block) || is_trusted_trap_block(tcx, body, block) {
            continue;
        }
        let terminator = body.basic_blocks[block]
            .terminator
            .as_ref()
            .ok_or_else(|| missing_terminator(block))?;
        if matches!(terminator.kind, TerminatorKind::Return) {
            return Err(unproved(property));
        }
        let successors = normal_successors(body, block)?;
        if successors.is_empty() {
            return Err(unproved(property));
        }
        pending.extend(successors);
    }
    if !reached_required {
        return Err(unproved(property));
    }
    Ok(())
}

fn is_trusted_trap_block(tcx: TyCtxt<'_>, body: &Body<'_>, block: BasicBlock) -> bool {
    matches!(
        body.basic_blocks[block]
            .terminator
            .as_ref()
            .map(|terminator| &terminator.kind),
        Some(TerminatorKind::Call {
            func: Operand::Constant(function),
            ..
        }) if matches!(
            function.const_.ty().kind(),
            TyKind::FnDef(definition, _)
                if trusted_device_items::classify(tcx, *definition)
                    == Some(TrustedDeviceItem::AmdGpuDiagnostic(
                        TrustedAmdGpuDiagnosticOperation::Trap,
                    ))
        )
    )
}

#[cfg(test)]
#[allow(dead_code)]
fn derive_typestate_source_property_receipts<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: rustc_hir::def_id::DefId,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
    abi: &[GeneralGemmAbiOperandBindingV1; 11],
) -> Result<
    (
        [GeneralGemmSourcePropertyReceiptV1; 11],
        GeneralGemmSymbolicPlanV1,
        GeneralGemmSymbolicKirV1,
    ),
    GeneralGemmMirImportErrorV1,
> {
    let phase = derive_phase_cycle_transcript(tcx, body, calls)?;
    let stage_inputs = require_guarded_stage_inputs(tcx, body, calls)?;
    let store_call = store_calls(calls)
        .into_iter()
        .next()
        .ok_or_else(|| unproved("positive typestate kernel has one output store"))?;
    if store_call.operation != TrustedGeneralGemmOperationV1::Store {
        return Err(unproved(
            "positive typestate kernel uses the sealed canonical store",
        ));
    }
    let abi_identity = general_gemm_abi_identity(abi);
    let store = GeneralGemmStoreTranscriptV1 {
        store: event_transcript(store_call),
        abi_identity,
    };
    let source_file = tcx
        .sess
        .source_map()
        .lookup_source_file(tcx.def_span(root).lo());
    let source = source_file
        .src
        .as_ref()
        .ok_or_else(|| unproved("compiled typestate kernel SourceFile bytes are retained"))?;
    let root_compiled_source = hash_fields(&[
        b"FE2O3/GENERAL-GEMM-COMPILED-SOURCE/V1\0",
        source.as_bytes(),
    ]);

    let semantics = GeneralGemmIntrinsicSemanticsV1::canonical();
    semantics
        .validate()
        .map_err(|_| unproved("reviewed typestate intrinsic-semantics schema validates"))?;
    let provider_profile =
        trusted_device_items::reviewed_general_gemm_provider_semantics_identity_v1();
    let mut closure_transcript = Vec::new();
    closure_transcript.extend_from_slice(&tcx.def_path_hash(root).0.to_le_bytes());
    closure_transcript.extend_from_slice(&root_compiled_source);
    closure_transcript.extend_from_slice(&abi_identity);
    encode_phase(&mut closure_transcript, phase);
    encode_stage(&mut closure_transcript, stage_inputs);
    encode_store(&mut closure_transcript, store);
    let mir_closure = hash_fields(&[
        b"FE2O3/GENERAL-GEMM-TYPESTATE-OPTIMIZED-MIR-CLOSURE/V1\0",
        &closure_transcript,
    ]);
    use GeneralGemmSourcePropertyKindV1 as Kind;
    let properties = [
        source_property(
            &semantics,
            Kind::AllocationAndProvenance,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::AllocationAndProvenance {
                abi_identity,
                root_compiled_source,
                stage_inputs,
                store,
            },
        ),
        source_property(
            &semantics,
            Kind::GuardedGlobalAccesses,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::GuardedGlobalAccesses {
                stage_inputs,
                store,
            },
        ),
        source_property(
            &semantics,
            Kind::LdsWriteReadInitialization,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::LdsWriteReadInitialization { phase },
        ),
        source_property(
            &semantics,
            Kind::EffectConflictFreedom,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::EffectConflictFreedom {
                phase,
                stage_inputs,
                store,
            },
        ),
        source_property(
            &semantics,
            Kind::ControlFlowBarrierConvergence,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::ControlFlowBarrierConvergence {
                stage_to_publish: phase.stage_to_publish,
                mfma_to_reuse: phase.mfma_to_reuse,
            },
        ),
        source_property(
            &semantics,
            Kind::OutputOwnership,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::OutputOwnership { phase, store },
        ),
        source_property(
            &semantics,
            Kind::LdsLifecycle,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::LdsLifecycle { phase },
        ),
        source_property(
            &semantics,
            Kind::AccumulatorPhase,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::AccumulatorPhase { phase },
        ),
        source_property(
            &semantics,
            Kind::MaskedTail,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::MaskedTail {
                stage_inputs,
                store,
            },
        ),
        source_property(
            &semantics,
            Kind::AlphaBetaEpilogue,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::AlphaBetaEpilogue { store },
        ),
        source_property(
            &semantics,
            Kind::NumericalOperationOrder,
            &mir_closure,
            &provider_profile,
            GeneralGemmSourceMirEvidenceV1::NumericalOperationOrder {
                stage_inputs,
                phase,
                store,
            },
        ),
    ];
    let derived_schema = derived_schema_from_typestate_properties(&properties)?;
    let symbolic_plan = GeneralGemmSymbolicPlanV1::from_derived_source_schema(&derived_schema)
        .map_err(|_| unproved("MIR-derived symbolic plan re-encodes exactly"))?;
    let symbolic_kir = GeneralGemmSymbolicKirV1::from_derived_source_schema(&derived_schema)
        .map_err(|_| unproved("MIR-derived symbolic KIR re-encodes exactly"))?;
    Ok((properties, symbolic_plan, symbolic_kir))
}

fn derived_schema_from_typestate_properties(
    properties: &[GeneralGemmSourcePropertyReceiptV1; 11],
) -> Result<GeneralGemmDerivedSourceSchemaV1, GeneralGemmMirImportErrorV1> {
    use GeneralGemmSourceMirEvidenceV1 as Evidence;
    let [
        allocation,
        guarded,
        initialized,
        conflict,
        convergence,
        output,
        lifecycle,
        accumulator,
        tail,
        epilogue,
        numerical,
    ] = properties;
    let (stage, store) = match &allocation.mir_evidence {
        Evidence::AllocationAndProvenance {
            stage_inputs,
            store,
            ..
        } => (*stage_inputs, *store),
        _ => {
            return Err(unproved(
                "allocation receipt retains typestate MIR evidence",
            ));
        }
    };
    let phase = match &initialized.mir_evidence {
        Evidence::LdsWriteReadInitialization { phase } => *phase,
        _ => {
            return Err(unproved(
                "initialization receipt retains typestate MIR evidence",
            ));
        }
    };
    let exact = matches!(
        &guarded.mir_evidence,
        Evidence::GuardedGlobalAccesses { stage_inputs, store: retained }
            if *stage_inputs == stage && *retained == store
    ) && matches!(
        &conflict.mir_evidence,
        Evidence::EffectConflictFreedom { phase: retained_phase, stage_inputs, store: retained_store }
            if *retained_phase == phase && *stage_inputs == stage && *retained_store == store
    ) && matches!(
        &convergence.mir_evidence,
        Evidence::ControlFlowBarrierConvergence { stage_to_publish, mfma_to_reuse }
            if *stage_to_publish == phase.stage_to_publish && *mfma_to_reuse == phase.mfma_to_reuse
    ) && matches!(
        &output.mir_evidence,
        Evidence::OutputOwnership { phase: retained_phase, store: retained_store }
            if *retained_phase == phase && *retained_store == store
    ) && matches!(
        &lifecycle.mir_evidence,
        Evidence::LdsLifecycle { phase: retained } if *retained == phase
    ) && matches!(
        &accumulator.mir_evidence,
        Evidence::AccumulatorPhase { phase: retained } if *retained == phase
    ) && matches!(
        &tail.mir_evidence,
        Evidence::MaskedTail { stage_inputs, store: retained }
            if *stage_inputs == stage && *retained == store
    ) && matches!(
        &epilogue.mir_evidence,
        Evidence::AlphaBetaEpilogue { store: retained } if *retained == store
    ) && matches!(
        &numerical.mir_evidence,
        Evidence::NumericalOperationOrder { stage_inputs, phase: retained_phase, store: retained_store }
            if *stage_inputs == stage && *retained_phase == phase && *retained_store == store
    );
    if !exact {
        return Err(unproved(
            "all source properties retain one exact typestate MIR transcript",
        ));
    }
    GeneralGemmDerivedSourceSchemaV1::checked(
        [
            GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
                rows: GeneralGemmAbiArgumentV1::M,
                columns: GeneralGemmAbiArgumentV1::K,
                stride: GeneralGemmAbiArgumentV1::Lda,
            },
            GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
                rows: GeneralGemmAbiArgumentV1::K,
                columns: GeneralGemmAbiArgumentV1::N,
                stride: GeneralGemmAbiArgumentV1::Ldb,
            },
            GeneralGemmSymbolicPlanExpressionV1::CheckedRowMajorExtent {
                rows: GeneralGemmAbiArgumentV1::M,
                columns: GeneralGemmAbiArgumentV1::N,
                stride: GeneralGemmAbiArgumentV1::Ldc,
            },
            GeneralGemmSymbolicPlanExpressionV1::CeilDiv16(GeneralGemmAbiArgumentV1::K),
            GeneralGemmSymbolicPlanExpressionV1::OutputBlockCounts,
            GeneralGemmSymbolicPlanExpressionV1::AqlGridWorkItems,
        ],
        [
            GeneralGemmDerivedKirBehaviorV1::Wave64GridXy16,
            GeneralGemmDerivedKirBehaviorV1::GuardedAbCheckedRowMajorZeroTail,
            GeneralGemmDerivedKirBehaviorV1::Xor4SingleBufferPublishReadMfmaReuse,
            GeneralGemmDerivedKirBehaviorV1::CarriedF32x4PhaseAccumulator,
            GeneralGemmDerivedKirBehaviorV1::GuardedDisjointCAlphaAccPlusBetaC,
        ],
    )
    .map_err(|_| unproved("MIR-derived plan expressions and KIR behaviors match the closed schema"))
}

fn require_positive_abi(
    tcx: TyCtxt<'_>,
    root: rustc_hir::def_id::DefId,
) -> Result<[GeneralGemmAbiOperandBindingV1; 11], GeneralGemmMirImportErrorV1> {
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(root).instantiate_identity());
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.output() != tcx.types.unit
        || signature.inputs().len() != 11
    {
        return Err(unproved("kernel has the exact safe 11-operand Rust ABI"));
    }
    let inputs = signature.inputs();
    if !is_shared_u16_slice(inputs[0])
        || !is_shared_u16_slice(inputs[1])
        || !is_disjoint_f32_slice(tcx, inputs[2])
        || inputs[3..9]
            .iter()
            .any(|ty| !matches!(ty.kind(), TyKind::Uint(UintTy::U32)))
        || inputs[9..11]
            .iter()
            .any(|ty| !matches!(ty.kind(), TyKind::Float(FloatTy::F32)))
    {
        return Err(unproved(
            "ABI roles are A/B u16 slices, C disjoint f32, M/N/K/strides u32, alpha/beta f32",
        ));
    }
    use GeneralGemmAbiRoleV1 as Role;
    use GeneralGemmAbiTypeV1 as Type;
    Ok([
        abi(Role::A, 0, Type::SharedU16Slice),
        abi(Role::B, 1, Type::SharedU16Slice),
        abi(Role::C, 2, Type::DisjointF32Slice),
        abi(Role::M, 3, Type::U32),
        abi(Role::N, 4, Type::U32),
        abi(Role::K, 5, Type::U32),
        abi(Role::Lda, 6, Type::U32),
        abi(Role::Ldb, 7, Type::U32),
        abi(Role::Ldc, 8, Type::U32),
        abi(Role::Alpha, 9, Type::F32),
        abi(Role::Beta, 10, Type::F32),
    ])
}

const fn abi(
    role: GeneralGemmAbiRoleV1,
    argument_index: u8,
    ty: GeneralGemmAbiTypeV1,
) -> GeneralGemmAbiOperandBindingV1 {
    GeneralGemmAbiOperandBindingV1 {
        role,
        argument_index,
        ty,
    }
}

#[cfg(test)]
fn general_gemm_abi_identity(abi: &[GeneralGemmAbiOperandBindingV1; 11]) -> [u8; 32] {
    let mut encoded = Vec::with_capacity(abi.len() * 3);
    for binding in abi {
        encoded.extend_from_slice(&[
            binding.role as u8,
            binding.argument_index,
            match binding.ty {
                GeneralGemmAbiTypeV1::SharedU16Slice => 1,
                GeneralGemmAbiTypeV1::DisjointF32Slice => 2,
                GeneralGemmAbiTypeV1::U32 => 3,
                GeneralGemmAbiTypeV1::F32 => 4,
            },
        ]);
    }
    hash_fields(&[b"FE2O3/GENERAL-GEMM-FRONTEND-ABI/V1\0", &encoded])
}

fn is_shared_u16_slice(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Ref(_, pointee, Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Uint(UintTy::U16)))
    )
}

fn is_disjoint_f32_slice(tcx: TyCtxt<'_>, ty: Ty<'_>) -> bool {
    let TyKind::Adt(definition, arguments) = ty.kind() else {
        return false;
    };
    trusted_device_items::classify(tcx, definition.did()) == Some(TrustedDeviceItem::DisjointSlice)
        && arguments.len() == 2
        && arguments
            .first()
            .and_then(|argument| argument.as_type())
            .is_some_and(|element| matches!(element.kind(), TyKind::Float(FloatTy::F32)))
}

fn require_terminal_abi_binding(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let acquire_args = call_args(body, acquire.block)?;
    if acquire_args.len() != 1 || !is_kernel_argument(&acquire_args[0].node, 5) {
        return Err(unproved("acquire phase count is rooted in ABI K"));
    }
    let store = store_calls(calls)
        .into_iter()
        .next()
        .ok_or_else(|| unproved("one output store exists"))?;
    if store.operation != TrustedGeneralGemmOperationV1::Store {
        return Err(unproved("positive output uses the sealed canonical store"));
    }
    let args = call_args(body, store.block)?;
    if !args.get(1).is_some_and(|value| {
        is_kernel_argument_or_alias(body, &value.node, 2, 0, &mut BTreeSet::new())
    }) {
        return Err(unproved(
            "positive output store is rooted in the exact ABI C allocation",
        ));
    }
    for (operand, argument) in [(2, 3), (3, 4), (4, 8), (5, 9), (6, 10)] {
        if !args.get(operand).is_some_and(|value| {
            is_kernel_argument_or_alias(body, &value.node, argument, 0, &mut BTreeSet::new())
        }) {
            return Err(unproved(
                "store binds M/N/ldc/alpha/beta ABI operands exactly",
            ));
        }
    }
    Ok(())
}

fn require_counterexample_abi_binding(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let acquire_args = call_args(body, acquire.block)?;
    if acquire_args.len() != 1 || !is_kernel_argument(&acquire_args[0].node, 5) {
        return Err(unproved("counterexample acquire is rooted in ABI K"));
    }

    for call in calls {
        let args = call_args(body, call.block)?;
        let bindings: &[(usize, usize)] = match call.operation {
            TrustedGeneralGemmOperationV1::LoadA => &[(1, 0), (4, 3), (5, 5), (6, 6)],
            TrustedGeneralGemmOperationV1::LoadB => &[(1, 1), (4, 5), (5, 4), (6, 7)],
            TrustedGeneralGemmOperationV1::StageValue => &[(4, 5)],
            TrustedGeneralGemmOperationV1::Store => {
                &[(1, 2), (2, 3), (3, 4), (4, 8), (5, 9), (6, 10)]
            }
            TrustedGeneralGemmOperationV1::LoadC => &[(1, 2), (4, 3), (5, 4), (6, 8)],
            TrustedGeneralGemmOperationV1::StoreEpilogue => {
                &[(1, 2), (4, 3), (5, 4), (6, 8), (8, 9), (10, 10)]
            }
            _ => &[],
        };
        if let Some(&(operand, argument)) = bindings.iter().find(|&&(operand, argument)| {
            !args.get(operand).is_some_and(|value| {
                is_kernel_argument_or_alias(body, &value.node, argument, 0, &mut BTreeSet::new())
            })
        }) {
            let observed = args.get(operand).map(|value| &value.node);
            let definitions = observed
                .and_then(operand_local)
                .map(|local| {
                    body.basic_blocks
                        .iter()
                        .flat_map(|data| &data.statements)
                        .filter_map(|statement| {
                            let (destination, value) = statement.kind.as_assign()?;
                            (destination.as_local() == Some(local)).then(|| format!("{value:?}"))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            return Err(unproved(&format!(
                "counterexample {} operand {operand} retains kernel ABI argument {argument}; observed {observed:?}; definitions {definitions:?}",
                operation_name(call.operation),
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
fn source_property(
    semantics: &GeneralGemmIntrinsicSemanticsV1,
    kind: GeneralGemmSourcePropertyKindV1,
    mir_closure: &[u8; 32],
    provider_profile: &[u8; 32],
    mir_evidence: GeneralGemmSourceMirEvidenceV1,
) -> GeneralGemmSourcePropertyReceiptV1 {
    let intrinsic_fact = semantics.source_facts()[(kind as usize) - 1];
    let evidence_transcript = encode_source_mir_evidence(&mir_evidence);
    GeneralGemmSourcePropertyReceiptV1 {
        kind,
        intrinsic_fact,
        mir_evidence,
        optimized_mir_closure: *mir_closure,
        provider_profile: *provider_profile,
        evidence_identity: hash_fields(&[
            b"FE2O3/GENERAL-GEMM-SOURCE-PROPERTY-RECEIPT/V1\0",
            &[kind as u8],
            mir_closure,
            provider_profile,
            &intrinsic_fact.identity(semantics.identity()),
            &evidence_transcript,
        ]),
    }
}

fn require_dynamic_terminal_inventory(
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    for (operation, count) in [
        (TrustedGeneralGemmOperationV1::Acquire, 1),
        (TrustedGeneralGemmOperationV1::Lane, 1),
        (TrustedGeneralGemmOperationV1::WorkgroupX, 1),
        (TrustedGeneralGemmOperationV1::WorkgroupY, 1),
        (TrustedGeneralGemmOperationV1::LoadA, 1),
        (TrustedGeneralGemmOperationV1::LoadB, 1),
        (TrustedGeneralGemmOperationV1::StageValue, 2),
        (TrustedGeneralGemmOperationV1::Stage, 1),
        (TrustedGeneralGemmOperationV1::WaitStage, 1),
        (TrustedGeneralGemmOperationV1::Publish, 1),
        (TrustedGeneralGemmOperationV1::ReadStage, 8),
        (TrustedGeneralGemmOperationV1::MfmaValue, 4),
        (TrustedGeneralGemmOperationV1::Reuse, 1),
        (TrustedGeneralGemmOperationV1::LoadC, 4),
        (TrustedGeneralGemmOperationV1::StoreEpilogue, 4),
    ] {
        require_count(calls, operation, count, count)?;
    }
    require_count(calls, TrustedGeneralGemmOperationV1::Mfma, 0, 0)?;
    require_count(calls, TrustedGeneralGemmOperationV1::Store, 0, 0)
}

fn require_guarded_dynamic_accesses<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    for operation in [
        TrustedGeneralGemmOperationV1::LoadA,
        TrustedGeneralGemmOperationV1::LoadB,
    ] {
        let call = unique_call(calls, operation)?;
        let args = call_args(body, call.block)?;
        let depth_operand = match operation {
            TrustedGeneralGemmOperationV1::LoadA => 3,
            TrustedGeneralGemmOperationV1::LoadB => 2,
            _ => unreachable!("guarded dynamic access loop contains only A/B loads"),
        };
        if !has_true_lt_guard(tcx, body, call.block, &args[2].node, &args[4].node)
            || !has_true_lt_guard(tcx, body, call.block, &args[3].node, &args[5].node)
        {
            return Err(unproved(
                "A/B loads are dominated by both exact coordinate bounds",
            ));
        }
        let result = call
            .result_local
            .ok_or_else(|| unproved("guarded A/B load returns one value local"))?;
        if !local_has_u16_constant_assignment(tcx, body, result, 0) {
            return Err(unproved(
                "guarded A/B load has a positive-zero false-edge value",
            ));
        }
        let staged = calls
            .iter()
            .filter(|candidate| candidate.operation == TrustedGeneralGemmOperationV1::StageValue)
            .any(|stage| {
                call_args(body, stage.block).ok().is_some_and(|stage_args| {
                    stage_args.get(5).and_then(|arg| operand_local(&arg.node)) == Some(result)
                        && stage_args.get(3).is_some_and(|depth| {
                            same_runtime_value(&depth.node, &args[depth_operand].node)
                        })
                })
            });
        if !staged {
            let observations = calls
                .iter()
                .filter(|candidate| {
                    candidate.operation == TrustedGeneralGemmOperationV1::StageValue
                })
                .filter_map(|stage| {
                    let stage_args = call_args(body, stage.block).ok()?;
                    Some((
                        stage.block.as_usize(),
                        stage_args.get(5).and_then(|arg| operand_local(&arg.node)),
                        stage_args.get(3).and_then(|arg| operand_local(&arg.node)),
                    ))
                })
                .collect::<Vec<_>>();
            return Err(unproved(&format!(
                "guarded A/B value or positive zero reaches its exact stage ({operation:?} result={result:?}, depth={:?}, stages={observations:?})",
                args.get(depth_operand)
                    .and_then(|arg| operand_local(&arg.node)),
            )));
        }
    }

    for operation in [
        TrustedGeneralGemmOperationV1::LoadC,
        TrustedGeneralGemmOperationV1::StoreEpilogue,
    ] {
        for call in calls.iter().filter(|call| call.operation == operation) {
            let args = call_args(body, call.block)?;
            if !has_true_lt_guard(tcx, body, call.block, &args[2].node, &args[4].node)
                || !has_true_lt_guard(tcx, body, call.block, &args[3].node, &args[5].node)
            {
                return Err(unproved(
                    "C loads/stores are dominated by exact row<M and column<N",
                ));
            }
        }
    }
    Ok(())
}

fn canonical_local_alias_root(body: &Body<'_>, mut local: Local) -> Local {
    let mut visited = BTreeSet::new();
    while visited.insert(local) {
        let definitions = body
            .basic_blocks
            .iter()
            .flat_map(|data| &data.statements)
            .filter_map(|statement| statement.kind.as_assign())
            .filter_map(|(destination, value)| {
                (destination.as_local() == Some(local)).then_some(value)
            })
            .collect::<Vec<_>>();
        let [Rvalue::Use(operand) | Rvalue::Cast(_, operand, _)] = definitions.as_slice() else {
            break;
        };
        let Some(next) = operand_local(operand) else {
            break;
        };
        local = next;
    }
    local
}

fn require_dynamic_lds_mapping<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let stages = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::StageValue)
        .collect::<Vec<_>>();
    let phase = call_args(body, stages[0].block)?
        .get(2)
        .and_then(|arg| operand_local(&arg.node))
        .map(|local| canonical_local_alias_root(body, local))
        .ok_or_else(|| unproved("stage epoch has one loop-carried local"))?;
    let lane = ProofSymbolicValueV1::Lane;
    let lane_row = proof_rem(lane.clone(), proof_constant(16));
    let depth_base = proof_mul(
        proof_constant(4),
        proof_div(lane.clone(), proof_constant(16)),
    );
    let component = ProofSymbolicValueV1::Component;
    let tile_depth = proof_add(depth_base.clone(), component);
    let swizzle = proof_xor(
        tile_depth,
        proof_mul(
            proof_constant(4),
            proof_rem(lane_row.clone(), proof_constant(4)),
        ),
    );
    let expected_stage_slots = [
        proof_add(
            proof_mul(proof_constant(16), lane_row.clone()),
            swizzle.clone(),
        ),
        proof_add(
            proof_add(
                proof_constant(256),
                proof_mul(proof_constant(16), lane_row.clone()),
            ),
            swizzle,
        ),
    ];
    for (stage, expected) in stages.iter().zip(expected_stage_slots) {
        let args = call_args(body, stage.block)?;
        let slot = proof_symbolic_operand(tcx, body, calls, phase, &args[1].node)?;
        let epoch = proof_symbolic_operand(tcx, body, calls, phase, &args[2].node)?;
        if slot != expected || epoch != ProofSymbolicValueV1::Phase {
            return Err(unproved(&format!(
                "two stage sites use exact disjoint XOR4 slot and epoch maps (slot={slot:?}, expected={expected:?}, epoch={epoch:?})",
            )));
        }
    }

    let wait = unique_call(calls, TrustedGeneralGemmOperationV1::WaitStage)?;
    let wait_args = call_args(body, wait.block)?;
    if proof_symbolic_operand(tcx, body, calls, phase, &wait_args[1].node)?
        != ProofSymbolicValueV1::Phase
    {
        return Err(unproved("stage wait binds the current phase epoch"));
    }
    let publish = unique_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let reads = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::ReadStage)
        .collect::<Vec<_>>();
    for (index, read) in reads.iter().enumerate() {
        let args = call_args(body, read.block)?;
        let component = proof_constant((index / 2) as u128);
        let swizzle = proof_xor(
            proof_add(depth_base.clone(), component),
            proof_mul(
                proof_constant(4),
                proof_rem(lane_row.clone(), proof_constant(4)),
            ),
        );
        let expected = if index.is_multiple_of(2) {
            proof_add(proof_mul(proof_constant(16), lane_row.clone()), swizzle)
        } else {
            proof_add(
                proof_add(
                    proof_constant(256),
                    proof_mul(proof_constant(16), lane_row.clone()),
                ),
                swizzle,
            )
        };
        let slot = proof_symbolic_operand(tcx, body, calls, phase, &args[1].node)?;
        let epoch = proof_symbolic_operand(tcx, body, calls, phase, &args[2].node)?;
        let after_publish = reachable(body, publish.return_target, read.block);
        if slot != expected || epoch != ProofSymbolicValueV1::Phase || !after_publish {
            return Err(unproved(&format!(
                "eight LDS reads use initialized current-epoch XOR4 slots after publish (index={index}, slot={slot:?}, expected={expected:?}, epoch={epoch:?}, after_publish={after_publish})",
            )));
        }
    }

    let stores = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::StoreEpilogue)
        .collect::<Vec<_>>();
    let row_base = proof_add(
        proof_mul(ProofSymbolicValueV1::WorkgroupY, proof_constant(16)),
        proof_mul(
            proof_constant(4),
            proof_div(lane.clone(), proof_constant(16)),
        ),
    );
    let column = proof_add(
        proof_mul(ProofSymbolicValueV1::WorkgroupX, proof_constant(16)),
        lane_row,
    );
    for (component, store) in stores.iter().enumerate() {
        let args = call_args(body, store.block)?;
        let expected_row = if component == 0 {
            row_base.clone()
        } else {
            proof_add(row_base.clone(), proof_constant(component as u128))
        };
        if proof_symbolic_operand(tcx, body, calls, phase, &args[2].node)? != expected_row
            || proof_symbolic_operand(tcx, body, calls, phase, &args[3].node)? != column
        {
            return Err(unproved(
                "four stores use exact injective grid-XY16 lane/component coordinates",
            ));
        }
    }
    Ok(())
}

fn require_dynamic_accumulator_carry<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mfmas = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::MfmaValue)
        .collect::<Vec<_>>();
    let reads = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::ReadStage)
        .collect::<Vec<_>>();
    for (component, mfma) in mfmas.iter().enumerate() {
        let args = call_args(body, mfma.block)?;
        let prior = args
            .get(3)
            .and_then(|arg| operand_local(&arg.node))
            .ok_or_else(|| unproved("MFMA prior accumulator has one loop-carried local"))?;
        let result = mfma
            .result_local
            .ok_or_else(|| unproved("MFMA returns one accumulator result local"))?;
        if !same_result_operand(&args[1].node, reads[component * 2].result_local)
            || !same_result_operand(&args[2].node, reads[component * 2 + 1].result_local)
            || !local_has_f32_constant_assignment(tcx, body, prior, 0.0)
            || !local_is_assigned_from(body, prior, result)
        {
            return Err(unproved(
                "each MFMA result is the next-phase value of its zero-initialized prior",
            ));
        }
    }
    Ok(())
}

fn has_true_lt_guard<'tcx>(
    _tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    guarded: BasicBlock,
    left: &Operand<'tcx>,
    right: &Operand<'tcx>,
) -> bool {
    body.basic_blocks.iter_enumerated().any(|(block, data)| {
        let Some(terminator) = &data.terminator else {
            return false;
        };
        let TerminatorKind::SwitchInt { discr, targets } = &terminator.kind else {
            return false;
        };
        let Some(discr) = operand_local(discr) else {
            return false;
        };
        let matches = body
            .basic_blocks
            .iter()
            .flat_map(|data| &data.statements)
            .any(|statement| {
                let Some((destination, Rvalue::BinaryOp(BinOp::Lt, operands))) =
                    statement.kind.as_assign()
                else {
                    return false;
                };
                destination.as_local() == Some(discr)
                    && same_runtime_value(&operands.0, left)
                    && same_runtime_value(&operands.1, right)
            });
        if !matches {
            return false;
        }
        let true_target = targets
            .iter()
            .find_map(|(value, target)| (value == 1).then_some(target))
            .unwrap_or_else(|| targets.otherwise());
        block_dominates(body, true_target, guarded) && reachable(body, block, guarded)
    })
}

fn block_dominates(body: &Body<'_>, dominator: BasicBlock, dominated: BasicBlock) -> bool {
    if dominator == dominated || dominator == START_BLOCK {
        return true;
    }
    let mut pending = VecDeque::from([START_BLOCK]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if block == dominator || !visited.insert(block) {
            continue;
        }
        if block == dominated {
            return false;
        }
        pending.extend(normal_successors(body, block).unwrap_or_default());
    }
    true
}

fn require_bounded_phase_cycle<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let publish = unique_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let mfma = unique_call(calls, TrustedGeneralGemmOperationV1::Mfma)?;
    let reuse = unique_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    let store = store_calls(calls)[0];
    let wave = acquire
        .result_local
        .ok_or_else(|| unproved("acquire returns one wave capability local"))?;
    if !reachable(body, stage.return_target, publish.block)
        || !reachable(body, publish.return_target, mfma.block)
        || !reachable(body, mfma.return_target, reuse.block)
        || !reachable(body, reuse.return_target, stage.block)
        || !reachable(body, acquire.return_target, store.block)
        || !has_phase_split(
            tcx,
            body,
            acquire.return_target,
            stage.block,
            store.block,
            wave,
        )
    {
        return Err(unproved(
            "bounded K-phase CFG is stage/publish/MFMA/reuse with an exit to store",
        ));
    }
    Ok(())
}

fn event_transcript(call: &GeneralGemmCallV1) -> GeneralGemmMirEventTranscriptV1 {
    GeneralGemmMirEventTranscriptV1 {
        operation: call.operation,
        block: u32::try_from(call.block.as_usize()).expect("bounded MIR block index"),
        return_block: u32::try_from(call.return_target.as_usize())
            .expect("bounded MIR return block index"),
        result_local: call
            .result_local
            .map(|local| u32::try_from(local.as_usize()).expect("bounded MIR local index")),
    }
}

fn derive_all_paths_transcript(
    body: &Body<'_>,
    from: BasicBlock,
    required: &GeneralGemmCallV1,
) -> Result<GeneralGemmAllPathsTranscriptV1, GeneralGemmMirImportErrorV1> {
    let lifecycle_blocks = BTreeSet::from([required.block]);
    require_all_paths_reach_lifecycle(
        body,
        from,
        required.block,
        "prior authenticated event",
        operation_name(required.operation),
        &lifecycle_blocks,
    )?;
    let mut pending = VecDeque::from([from]);
    let mut region = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if block == required.block || !region.insert(block) {
            continue;
        }
        pending.extend(normal_successors(body, block)?);
    }
    let mut transcript = Vec::new();
    for block in &region {
        transcript.extend_from_slice(&(block.as_usize() as u32).to_le_bytes());
        transcript
            .extend_from_slice(&(body.basic_blocks[*block].statements.len() as u32).to_le_bytes());
        let successors = normal_successors(body, *block)?;
        transcript.extend_from_slice(&(successors.len() as u32).to_le_bytes());
        for successor in successors {
            transcript.extend_from_slice(&(successor.as_usize() as u32).to_le_bytes());
        }
    }
    Ok(GeneralGemmAllPathsTranscriptV1 {
        from_block: u32::try_from(from.as_usize()).expect("bounded MIR block index"),
        required_event: event_transcript(required),
        boundary_block: u32::try_from(required.block.as_usize()).expect("bounded MIR block index"),
        visited_region_identity: hash_fields(&[
            b"FE2O3/GENERAL-GEMM-ALL-PATHS-CFG-REGION/V1\0",
            &transcript,
        ]),
    })
}

fn derive_phase_cycle_transcript<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<GeneralGemmPhaseCycleTranscriptV1, GeneralGemmMirImportErrorV1> {
    require_bounded_phase_cycle(tcx, body, calls)?;
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let publish = unique_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let mfma = unique_call(calls, TrustedGeneralGemmOperationV1::Mfma)?;
    let reuse = unique_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    let store = store_calls(calls)[0];
    let wave = acquire
        .result_local
        .ok_or_else(|| unproved("acquire returns one wave capability local"))?;
    let phase_split = find_phase_split(
        tcx,
        body,
        acquire.return_target,
        stage.block,
        store.block,
        wave,
    )
    .ok_or_else(|| unproved("bounded phase split has exact wave phase/count operands"))?;
    let stage_to_publish = derive_all_paths_transcript(body, stage.return_target, publish)?;
    let publish_to_mfma = derive_all_paths_transcript(body, publish.return_target, mfma)?;
    let mfma_to_reuse = derive_all_paths_transcript(body, mfma.return_target, reuse)?;
    let mut transcript = Vec::new();
    for event in [acquire, stage, publish, mfma, reuse, store] {
        encode_event(&mut transcript, event_transcript(event));
    }
    transcript.extend_from_slice(&(phase_split.as_usize() as u32).to_le_bytes());
    for edge in [stage_to_publish, publish_to_mfma, mfma_to_reuse] {
        encode_all_paths(&mut transcript, edge);
    }
    Ok(GeneralGemmPhaseCycleTranscriptV1 {
        acquire: event_transcript(acquire),
        stage: event_transcript(stage),
        publish: event_transcript(publish),
        mfma: event_transcript(mfma),
        reuse: event_transcript(reuse),
        store: event_transcript(store),
        stage_to_publish,
        publish_to_mfma,
        mfma_to_reuse,
        phase_split_block: u32::try_from(phase_split.as_usize())
            .expect("bounded MIR phase block index"),
        phase_cfg_identity: hash_fields(&[
            b"FE2O3/GENERAL-GEMM-PHASE-CFG-TRANSCRIPT/V1\0",
            &transcript,
        ]),
    })
}

fn has_phase_split<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    from: BasicBlock,
    stage: BasicBlock,
    store: BasicBlock,
    wave: Local,
) -> bool {
    find_phase_split(tcx, body, from, stage, store, wave).is_some()
}

fn find_phase_split<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    from: BasicBlock,
    stage: BasicBlock,
    store: BasicBlock,
    wave: Local,
) -> Option<BasicBlock> {
    let mut pending = VecDeque::from([from]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if !visited.insert(block) || visited.len() > 64 {
            continue;
        }
        let Some(terminator) = &body.basic_blocks[block].terminator else {
            return None;
        };
        let successors = normal_successors(body, block).ok()?;
        if let TerminatorKind::SwitchInt { discr, targets } = &terminator.kind
            && symbolic_operand(tcx, body, discr, wave, 0, &mut BTreeSet::new())
                == Some(SymbolicValueV1::LessThan(
                    Box::new(SymbolicValueV1::WaveField(3)),
                    Box::new(SymbolicValueV1::WaveField(4)),
                ))
            && let Some((false_target, true_target)) = boolean_switch_targets(targets)
            && all_paths_reach_before(body, true_target, stage, store)
            && all_paths_reach_before(body, false_target, store, stage)
        {
            return Some(block);
        }
        pending.extend(successors);
    }
    None
}

fn boolean_switch_targets(
    targets: &rustc_middle::mir::SwitchTargets,
) -> Option<(BasicBlock, BasicBlock)> {
    let true_target = targets
        .iter()
        .find_map(|(value, target)| (value == 1).then_some(target))
        .unwrap_or_else(|| targets.otherwise());
    let false_target = targets
        .iter()
        .find_map(|(value, target)| (value == 0).then_some(target))
        .unwrap_or_else(|| targets.otherwise());
    (true_target != false_target).then_some((false_target, true_target))
}

fn all_paths_reach_before(
    body: &Body<'_>,
    from: BasicBlock,
    required: BasicBlock,
    forbidden: BasicBlock,
) -> bool {
    let mut pending = VecDeque::from([from]);
    let mut visited = BTreeSet::new();
    let mut reached_required = false;
    while let Some(block) = pending.pop_front() {
        if block == required {
            reached_required = true;
            continue;
        }
        if block == forbidden {
            return false;
        }
        if !visited.insert(block) {
            continue;
        }
        let Ok(successors) = normal_successors(body, block) else {
            return false;
        };
        if successors.is_empty() {
            return false;
        }
        pending.extend(successors);
    }
    reached_required
}

fn require_guarded_stage_inputs<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
) -> Result<GeneralGemmStageInputTranscriptV1, GeneralGemmMirImportErrorV1> {
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let stage_args = call_args(body, stage.block)?;
    let a = stage_args
        .get(1)
        .ok_or_else(|| unproved("stage carries four A values"))?;
    let b = stage_args
        .get(2)
        .ok_or_else(|| unproved("stage carries four B values"))?;
    let a_values = array_value_locals(body, &a.node, stage.block)?;
    let b_values = array_value_locals(body, &b.node, stage.block)?;
    if a_values.len() != 4 || b_values.len() != 4 {
        return Err(unproved(
            "stage carries exactly four A and four B components",
        ));
    }

    let wave = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?
        .result_local
        .ok_or_else(|| unproved("stage coordinates derive from the acquired wave"))?;
    let lane = SymbolicValueV1::WaveField(0);
    let a_row = add(
        multiply(SymbolicValueV1::WaveField(1), constant(16)),
        remainder(lane.clone(), constant(16)),
    );
    let b_column = add(
        multiply(SymbolicValueV1::WaveField(2), constant(16)),
        remainder(lane.clone(), constant(16)),
    );
    let depth_base = add(
        multiply(SymbolicValueV1::WaveField(3), constant(16)),
        multiply(divide(lane, constant(16)), constant(4)),
    );

    let mut helper = None;
    let mut coordinate_transcript = Vec::new();
    for (role, values, expected, first_coordinate, second_coordinate, varying_first) in [
        (
            0_u8,
            &a_values,
            [(0, 0), (3, 3), (4, 5), (5, 6)],
            a_row,
            depth_base.clone(),
            false,
        ),
        (
            1_u8,
            &b_values,
            [(0, 1), (3, 5), (4, 4), (5, 7)],
            depth_base,
            b_column,
            true,
        ),
    ] {
        for (component_index, value) in values.iter().enumerate() {
            let (definition, arguments, definition_block) =
                defining_call(body, *value, stage.block)?;
            if !dominates(body, definition_block, stage.block) {
                return Err(unproved(
                    "each guarded stage component definition dominates the stage event",
                ));
            }
            if helper
                .replace(definition)
                .is_some_and(|prior| prior != definition)
            {
                return Err(unproved(
                    "all staged values use one guarded row-major loader",
                ));
            }
            for (operand, argument) in expected {
                if !arguments
                    .get(operand)
                    .is_some_and(|value| is_kernel_argument(&value.node, argument))
                {
                    return Err(unproved(
                        "A/B loads bind M/K/lda and K/N/ldb ABI operands exactly",
                    ));
                }
            }
            let component = constant(component_index as u128);
            let expected_first = if varying_first && component != constant(0) {
                add(first_coordinate.clone(), component.clone())
            } else {
                first_coordinate.clone()
            };
            let expected_second = if !varying_first && component != constant(0) {
                add(second_coordinate.clone(), component)
            } else {
                second_coordinate.clone()
            };
            if symbolic_operand(tcx, body, &arguments[1].node, wave, 0, &mut BTreeSet::new())
                != Some(expected_first)
                || symbolic_operand(tcx, body, &arguments[2].node, wave, 0, &mut BTreeSet::new())
                    != Some(expected_second)
            {
                return Err(unproved(
                    "A/B coordinates are exact grid-XY16 lane and K-phase expressions",
                ));
            }
            coordinate_transcript.extend_from_slice(&[role, component_index as u8]);
            encode_symbolic_value(
                &mut coordinate_transcript,
                &symbolic_operand(tcx, body, &arguments[1].node, wave, 0, &mut BTreeSet::new())
                    .expect("checked first coordinate"),
            );
            encode_symbolic_value(
                &mut coordinate_transcript,
                &symbolic_operand(tcx, body, &arguments[2].node, wave, 0, &mut BTreeSet::new())
                    .expect("checked second coordinate"),
            );
        }
    }
    let helper = helper.ok_or_else(|| unproved("guarded stage load helper is present"))?;
    let guarded_loader = require_guarded_row_major_helper(tcx, helper)?;
    Ok(GeneralGemmStageInputTranscriptV1 {
        stage: event_transcript(stage),
        guarded_loader,
        coordinate_dataflow_identity: hash_fields(&[
            b"FE2O3/GENERAL-GEMM-STAGE-COORDINATE-DATAFLOW/V1\0",
            &coordinate_transcript,
        ]),
    })
}

fn require_guarded_row_major_helper(
    tcx: TyCtxt<'_>,
    helper: rustc_hir::def_id::DefId,
) -> Result<GeneralGemmGuardedLoaderTranscriptV1, GeneralGemmMirImportErrorV1> {
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(helper).instantiate_identity());
    if signature.inputs().len() != 6
        || !is_shared_u16_slice(signature.inputs()[0])
        || signature.inputs()[1..3]
            .iter()
            .any(|ty| !matches!(ty.kind(), TyKind::Uint(UintTy::U64)))
        || signature.inputs()[3..6]
            .iter()
            .any(|ty| !matches!(ty.kind(), TyKind::Uint(UintTy::U32)))
        || !matches!(signature.output().kind(), TyKind::Uint(UintTy::U16))
    {
        return Err(unproved(
            "guarded loader has the exact coordinate/bounds/stride ABI",
        ));
    }
    let body = tcx.optimized_mir(helper);
    let argument = |index: u8| SymbolicValueV1::KernelArgument(index);
    let mut row_guard = None;
    let mut column_guard = None;
    let mut zero_return = None;
    let mut row_major = None;
    let mut extent_guard = None;
    let mut load = None;
    let mut trap = None;
    let mut dataflow = Vec::new();
    for (block, data) in body.basic_blocks.iter_enumerated() {
        for (statement_index, statement) in data.statements.iter().enumerate() {
            let Some((destination, value)) = statement.kind.as_assign() else {
                continue;
            };
            match value {
                Rvalue::BinaryOp(BinOp::Ge, operands) => {
                    let left = symbolic_operand(
                        tcx,
                        body,
                        &operands.0,
                        rustc_middle::mir::RETURN_PLACE,
                        0,
                        &mut BTreeSet::new(),
                    );
                    let right = symbolic_operand(
                        tcx,
                        body,
                        &operands.1,
                        rustc_middle::mir::RETURN_PLACE,
                        0,
                        &mut BTreeSet::new(),
                    );
                    let guard = if left == Some(argument(1)) && right == Some(argument(3)) {
                        &mut row_guard
                    } else if left == Some(argument(2)) && right == Some(argument(4)) {
                        &mut column_guard
                    } else {
                        return Err(unproved(
                            "loader bounds compare the exact row/rows and column/columns operands",
                        ));
                    };
                    if guard.replace((block, destination.as_local())).is_some() {
                        return Err(unproved("each loader coordinate has one bounds guard"));
                    }
                    dataflow.push(1);
                    dataflow.extend_from_slice(&(block.as_usize() as u32).to_le_bytes());
                    encode_symbolic_value(&mut dataflow, &left.expect("checked guard operand"));
                    encode_symbolic_value(&mut dataflow, &right.expect("checked guard operand"));
                }
                Rvalue::BinaryOp(BinOp::Add, _) => {
                    let Some(symbolic) = symbolic_rvalue(
                        tcx,
                        body,
                        value,
                        rustc_middle::mir::RETURN_PLACE,
                        0,
                        &mut BTreeSet::new(),
                    ) else {
                        continue;
                    };
                    let expected = add(multiply(argument(1), argument(5)), argument(2));
                    if symbolic == expected {
                        let Some(local) = destination.as_local() else {
                            return Err(unproved(
                                "loader row-major offset has local MIR provenance",
                            ));
                        };
                        if row_major.replace((block, local)).is_some() {
                            return Err(unproved("loader has one exact row-major offset"));
                        }
                        dataflow.push(2);
                        dataflow.extend_from_slice(&(block.as_usize() as u32).to_le_bytes());
                        encode_symbolic_value(&mut dataflow, &symbolic);
                    }
                }
                Rvalue::Use(Operand::Constant(constant))
                    if destination.as_local() == Some(rustc_middle::mir::RETURN_PLACE)
                        && constant_u16_from_constant(tcx, constant) == Some(0) =>
                {
                    zero_return.get_or_insert(block);
                    dataflow.push(3);
                    dataflow.extend_from_slice(&(block.as_usize() as u32).to_le_bytes());
                }
                Rvalue::BinaryOp(BinOp::Lt, operands) => {
                    let Some(left) = operand_local(&operands.0) else {
                        continue;
                    };
                    let Some(right) = operand_local(&operands.1) else {
                        continue;
                    };
                    let metadata = body.basic_blocks[block].statements.iter().any(|statement| {
                        statement.kind.as_assign().is_some_and(|(place, value)| {
                            place.as_local() == Some(right)
                                && matches!(value, Rvalue::UnaryOp(rustc_middle::mir::UnOp::PtrMetadata, operand) if is_kernel_argument(operand, 0))
                        })
                    });
                    if metadata {
                        let Some(discriminant) = destination.as_local() else {
                            return Err(unproved(
                                "loader extent comparison has local MIR provenance",
                            ));
                        };
                        if extent_guard.replace((block, discriminant, left)).is_some() {
                            return Err(unproved("loader has one exact slice-extent guard"));
                        }
                        dataflow.push(4);
                        dataflow.extend_from_slice(&(block.as_usize() as u32).to_le_bytes());
                    }
                }
                Rvalue::Use(Operand::Copy(place) | Operand::Move(place))
                    if destination.as_local() == Some(rustc_middle::mir::RETURN_PLACE)
                        && place
                            .projection
                            .iter()
                            .any(|projection| matches!(projection, ProjectionElem::Deref)) =>
                {
                    if load.replace((block, statement_index, *place)).is_some() {
                        return Err(unproved("loader has one in-bounds dereference result"));
                    }
                    dataflow.push(5);
                    dataflow.extend_from_slice(&(block.as_usize() as u32).to_le_bytes());
                }
                _ => {}
            }
        }
        if let Some(terminator) = &data.terminator
            && let TerminatorKind::Call {
                func: Operand::Constant(function),
                ..
            } = &terminator.kind
            && let TyKind::FnDef(definition, _) = function.const_.ty().kind()
            && trusted_device_items::classify(tcx, *definition)
                == Some(TrustedDeviceItem::AmdGpuDiagnostic(
                    TrustedAmdGpuDiagnosticOperation::Trap,
                ))
        {
            trap = Some(block);
            dataflow.push(6);
            dataflow.extend_from_slice(&(block.as_usize() as u32).to_le_bytes());
        }
    }
    let (row_guard, row_discriminant) =
        row_guard.ok_or_else(|| unproved("loader has the exact row bounds guard"))?;
    let (column_guard, column_discriminant) =
        column_guard.ok_or_else(|| unproved("loader has the exact column bounds guard"))?;
    let zero_return = zero_return.ok_or_else(|| unproved("loader has a positive-zero tail"))?;
    let (row_major, row_major_local) =
        row_major.ok_or_else(|| unproved("loader has an exact row-major offset"))?;
    let (extent_guard, extent_discriminant, extent_index) =
        extent_guard.ok_or_else(|| unproved("loader checks the slice extent"))?;
    let (load, load_statement_index, load_place) =
        load.ok_or_else(|| unproved("loader returns the in-bounds slice element"))?;
    let trap =
        trap.ok_or_else(|| unproved("loader traps when the slice extent is insufficient"))?;
    let Some((row_in_bounds, row_out_of_bounds)) = ge_switch_targets(body, row_guard) else {
        return Err(unproved(
            "loader row guard has exact false/true branch polarity",
        ));
    };
    let Some((column_in_bounds, column_out_of_bounds)) = ge_switch_targets(body, column_guard)
    else {
        return Err(unproved(
            "loader column guard has exact false/true branch polarity",
        ));
    };
    let Some((extent_decision, extent_out_of_bounds, extent_in_bounds, extent_option)) =
        find_exact_option_decision(
            tcx,
            body,
            extent_guard,
            extent_discriminant,
            trap,
            load,
            row_major_local,
        )
    else {
        return Err(unproved(
            "loader extent comparison lowers through exact None/Some arms and a downstream Option decision whose None edge traps and Some edge loads",
        ));
    };
    let checks = [
        (
            "row switch discriminant",
            switches_on_local(body, row_guard, row_discriminant),
        ),
        (
            "column switch discriminant",
            switches_on_local(body, column_guard, column_discriminant),
        ),
        (
            "row true -> zero",
            all_paths_reach_before(body, row_out_of_bounds, zero_return, column_guard),
        ),
        (
            "row false -> column",
            all_paths_reach_before(body, row_in_bounds, column_guard, zero_return),
        ),
        (
            "column true -> zero",
            all_paths_reach_before(body, column_out_of_bounds, zero_return, row_major),
        ),
        (
            "column false -> index",
            all_paths_reach_before(body, column_in_bounds, row_major, zero_return),
        ),
        (
            "extent uses row-major index",
            canonical_local_alias_root(body, extent_index)
                == canonical_local_alias_root(body, row_major_local),
        ),
        ("extent false -> trap", true),
        ("extent true -> load", true),
        (
            "row dominates column",
            dominates(body, row_guard, column_guard),
        ),
        (
            "column dominates index",
            dominates(body, column_guard, row_major),
        ),
        (
            "index dominates extent",
            dominates(body, row_major, extent_guard),
        ),
        (
            "extent dominates decision",
            dominates(body, extent_guard, extent_decision),
        ),
        (
            "extent decision dominates load",
            dominates(body, extent_decision, load),
        ),
        (
            "load derives from slice and index",
            load_place_is_exact_option_payload(
                tcx,
                body,
                &load_place,
                extent_option,
                load,
                load_statement_index,
            ),
        ),
    ];
    let failed = checks
        .iter()
        .filter_map(|(name, passed)| (!passed).then_some(*name))
        .collect::<Vec<_>>();
    if !failed.is_empty() {
        return Err(unproved(&format!(
            "loader CFG derives both guards, zero tail, checked row-major extent, in-bounds load, and trap (failed: {}; row={row_guard:?}/{row_in_bounds:?}/{row_out_of_bounds:?}, column={column_guard:?}/{column_in_bounds:?}/{column_out_of_bounds:?}, index={row_major:?}/{row_major_local:?}, extent={extent_guard:?}/{extent_decision:?}/{extent_out_of_bounds:?}/{extent_in_bounds:?}/{extent_index:?}, load={load:?}, trap={trap:?})",
            failed.join(", "),
        )));
    }
    let source_file = tcx
        .sess
        .source_map()
        .lookup_source_file(tcx.def_span(helper).lo());
    let source = source_file
        .src
        .as_ref()
        .ok_or_else(|| unproved("compiled guarded-loader SourceFile bytes are retained"))?;
    let compiled_source_identity = hash_fields(&[
        b"FE2O3/GENERAL-GEMM-GUARDED-LOADER-COMPILED-SOURCE/V1\0",
        source.as_bytes(),
    ]);
    Ok(GeneralGemmGuardedLoaderTranscriptV1 {
        helper_def_path: tcx.def_path_hash(helper).0.to_le_bytes(),
        compiled_source_identity,
        row_guard_block: row_guard.as_usize() as u32,
        column_guard_block: column_guard.as_usize() as u32,
        zero_return_block: zero_return.as_usize() as u32,
        row_major_block: row_major.as_usize() as u32,
        extent_guard_block: extent_decision.as_usize() as u32,
        load_block: load.as_usize() as u32,
        trap_block: trap.as_usize() as u32,
        dataflow_identity: hash_fields(&[
            b"FE2O3/GENERAL-GEMM-GUARDED-LOADER-DATAFLOW/V1\0",
            &dataflow,
        ]),
    })
}

fn ge_switch_targets(body: &Body<'_>, block: BasicBlock) -> Option<(BasicBlock, BasicBlock)> {
    let TerminatorKind::SwitchInt { targets, .. } =
        &body.basic_blocks[block].terminator.as_ref()?.kind
    else {
        return None;
    };
    let (false_target, true_target) = boolean_switch_targets(targets)?;
    Some((false_target, true_target))
}

fn find_exact_option_decision<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    comparison: BasicBlock,
    discriminant: Local,
    false_boundary: BasicBlock,
    true_boundary: BasicBlock,
    index: Local,
) -> Option<(BasicBlock, BasicBlock, BasicBlock, Local)> {
    let TerminatorKind::SwitchInt {
        discr: comparison_operand,
        targets: comparison_targets,
    } = &body.basic_blocks[comparison].terminator.as_ref()?.kind
    else {
        return None;
    };
    if operand_local(comparison_operand) != Some(discriminant) {
        return None;
    }
    let (none_entry, some_entry) = boolean_switch_targets(comparison_targets)?;
    let candidates = body
        .basic_blocks
        .iter_enumerated()
        .filter_map(|(block, data)| {
            let TerminatorKind::SwitchInt { discr, targets } = &data.terminator.as_ref()?.kind
            else {
                return None;
            };
            let decision_discriminant = operand_local(discr)?;
            if !dominates(body, comparison, block) {
                return None;
            }
            let option_places = data
                .statements
                .iter()
                .filter_map(|statement| statement.kind.as_assign())
                .filter_map(|(destination, value)| {
                    (destination.as_local() == Some(decision_discriminant)).then_some(value)
                })
                .filter_map(|value| match value {
                    Rvalue::Discriminant(place) if place.projection.is_empty() => place.as_local(),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [option] = option_places.as_slice() else {
                return None;
            };
            let TyKind::Adt(option_adt, _) = body.local_decls[*option].ty.kind() else {
                return None;
            };
            if !tcx.is_diagnostic_item(sym::Option, option_adt.did())
                || !all_paths_reach_before(body, none_entry, block, some_entry)
                || !all_paths_reach_before(body, some_entry, block, none_entry)
                || !option_arm_has_exact_definition(
                    tcx, body, none_entry, block, *option, index, false,
                )
                || !option_arm_has_exact_definition(
                    tcx, body, some_entry, block, *option, index, true,
                )
            {
                return None;
            }
            let (false_target, true_target) = boolean_switch_targets(targets)?;
            (all_paths_reach_before(body, false_target, false_boundary, true_boundary)
                && all_paths_reach_before(body, true_target, true_boundary, false_boundary))
            .then_some((block, false_target, true_target, *option))
        })
        .collect::<Vec<_>>();
    let [decision] = candidates.as_slice() else {
        return None;
    };
    Some(*decision)
}

fn option_arm_has_exact_definition<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    entry: BasicBlock,
    decision: BasicBlock,
    option: Local,
    index: Local,
    some: bool,
) -> bool {
    let mut pending = VecDeque::from([entry]);
    let mut visited = BTreeSet::new();
    let mut definitions = Vec::new();
    while let Some(block) = pending.pop_front() {
        if block == decision || !visited.insert(block) || visited.len() > 64 {
            continue;
        }
        for (statement_index, statement) in body.basic_blocks[block].statements.iter().enumerate() {
            let Some((destination, value)) = statement.kind.as_assign() else {
                continue;
            };
            if destination.as_local() == Some(option) {
                definitions.push((block, statement_index, value));
            }
        }
        let Ok(successors) = normal_successors(body, block) else {
            return false;
        };
        pending.extend(successors);
    }
    let [(definition_block, definition_statement_index, definition)] = definitions.as_slice()
    else {
        return false;
    };
    let TyKind::Adt(option_adt, _) = body.local_decls[option].ty.kind() else {
        return false;
    };
    if !tcx.is_diagnostic_item(sym::Option, option_adt.did()) {
        return false;
    }
    if !some {
        return matches!(
            definition,
            Rvalue::Use(Operand::Constant(constant))
                if constant.const_.ty() == body.local_decls[option].ty
                    && constant.const_.try_eval_target_usize(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                    ) == Some(0)
        );
    }
    let Rvalue::Aggregate(kind, operands) = definition else {
        return false;
    };
    let AggregateKind::Adt(definition, variant, _, _, active_field) = &**kind else {
        return false;
    };
    if *definition != option_adt.did()
        || active_field.is_some()
        || option_adt.discriminant_for_variant(tcx, *variant).val != 1
    {
        return false;
    }
    let [payload] = &operands.raw[..] else {
        return false;
    };
    loader_operand_is_exact_indexed_reference(
        body,
        payload,
        index,
        *definition_block,
        *definition_statement_index,
    )
}

fn loader_operand_is_exact_indexed_reference(
    body: &Body<'_>,
    operand: &Operand<'_>,
    index: Local,
    use_block: BasicBlock,
    use_statement_index: usize,
) -> bool {
    let Some(reference) = operand_local(operand) else {
        return false;
    };
    let Some((_, _, Rvalue::Ref(_, _, place))) =
        unique_assignment_before(body, reference, use_block, use_statement_index)
    else {
        return false;
    };
    let [ProjectionElem::Deref, ProjectionElem::Index(actual_index)] = &place.projection[..] else {
        return false;
    };
    place.local == Local::from_usize(1)
        && canonical_local_alias_root(body, *actual_index)
            == canonical_local_alias_root(body, index)
}

fn load_place_is_exact_option_payload<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    place: &rustc_middle::mir::Place<'_>,
    option: Local,
    use_block: BasicBlock,
    use_statement_index: usize,
) -> bool {
    if !matches!(&place.projection[..], [ProjectionElem::Deref]) {
        return false;
    }
    let Some((_, _, Rvalue::Use(Operand::Copy(extracted) | Operand::Move(extracted)))) =
        unique_assignment_before(body, place.local, use_block, use_statement_index)
    else {
        return false;
    };
    if extracted.local != option {
        return false;
    }
    let TyKind::Adt(option_adt, _) = body.local_decls[option].ty.kind() else {
        return false;
    };
    if !tcx.is_diagnostic_item(sym::Option, option_adt.did()) {
        return false;
    }
    let mut some_downcast = false;
    let mut payload_field = false;
    for projection in extracted.projection {
        match projection {
            ProjectionElem::Downcast(_, variant)
                if option_adt.discriminant_for_variant(tcx, variant).val == 1 =>
            {
                some_downcast = true;
            }
            ProjectionElem::Field(field, _) if field.index() == 0 => payload_field = true,
            _ => return false,
        }
    }
    some_downcast && payload_field
}

fn unique_assignment_before<'a, 'tcx>(
    body: &'a Body<'tcx>,
    local: Local,
    use_block: BasicBlock,
    use_statement_index: usize,
) -> Option<(BasicBlock, usize, &'a Rvalue<'tcx>)> {
    let definitions =
        body.basic_blocks
            .iter_enumerated()
            .flat_map(|(block, data)| {
                data.statements.iter().enumerate().filter_map(
                    move |(statement_index, statement)| {
                        let (destination, value) = statement.kind.as_assign()?;
                        (destination.as_local() == Some(local)).then_some((
                            block,
                            statement_index,
                            value,
                        ))
                    },
                )
            })
            .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return None;
    };
    let (definition_block, definition_statement_index, _) = *definition;
    ((definition_block == use_block && definition_statement_index < use_statement_index)
        || (definition_block != use_block && dominates(body, definition_block, use_block)))
    .then_some(*definition)
}

fn switches_on_local(body: &Body<'_>, block: BasicBlock, local: Option<Local>) -> bool {
    let Some(local) = local else {
        return false;
    };
    matches!(
        body.basic_blocks[block].terminator.as_ref().map(|terminator| &terminator.kind),
        Some(TerminatorKind::SwitchInt { discr, .. }) if operand_local(discr) == Some(local)
    )
}

fn dominates(body: &Body<'_>, dominator: BasicBlock, target: BasicBlock) -> bool {
    if dominator == target {
        return true;
    }
    let mut pending = VecDeque::from([START_BLOCK]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if block == dominator || !visited.insert(block) {
            continue;
        }
        if block == target {
            return false;
        }
        let Ok(successors) = normal_successors(body, block) else {
            return false;
        };
        pending.extend(successors);
    }
    true
}

fn encode_symbolic_value(bytes: &mut Vec<u8>, value: &SymbolicValueV1) {
    match value {
        SymbolicValueV1::KernelArgument(argument) => bytes.extend_from_slice(&[0, *argument]),
        SymbolicValueV1::WaveField(field) => bytes.extend_from_slice(&[1, *field]),
        SymbolicValueV1::Constant(value) => {
            bytes.push(2);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        SymbolicValueV1::Add(left, right) => {
            bytes.push(3);
            encode_symbolic_value(bytes, left);
            encode_symbolic_value(bytes, right);
        }
        SymbolicValueV1::Multiply(left, right) => {
            bytes.push(4);
            encode_symbolic_value(bytes, left);
            encode_symbolic_value(bytes, right);
        }
        SymbolicValueV1::Divide(left, right) => {
            bytes.push(5);
            encode_symbolic_value(bytes, left);
            encode_symbolic_value(bytes, right);
        }
        SymbolicValueV1::Remainder(left, right) => {
            bytes.push(6);
            encode_symbolic_value(bytes, left);
            encode_symbolic_value(bytes, right);
        }
        SymbolicValueV1::LessThan(left, right) => {
            bytes.push(7);
            encode_symbolic_value(bytes, left);
            encode_symbolic_value(bytes, right);
        }
    }
}

fn symbolic_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
    wave: Local,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> Option<SymbolicValueV1> {
    if depth >= 64 {
        return None;
    }
    match operand {
        Operand::Constant(constant) => constant
            .const_
            .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
            .map(|value| SymbolicValueV1::Constant(value.to_bits(value.size()))),
        Operand::Copy(place) | Operand::Move(place) => {
            if place.local == wave
                && let [ProjectionElem::Field(field, _)] = &place.projection[..]
            {
                return u8::try_from(field.index())
                    .ok()
                    .map(SymbolicValueV1::WaveField);
            }
            let local = place.as_local()?;
            if local.as_usize() > 0 && local.as_usize() <= body.arg_count {
                return u8::try_from(local.as_usize() - 1)
                    .ok()
                    .map(SymbolicValueV1::KernelArgument);
            }
            if !visiting.insert(local) {
                return None;
            }
            let mut definition = None;
            for block in body.basic_blocks.iter() {
                for statement in &block.statements {
                    let Some((destination, value)) = statement.kind.as_assign() else {
                        continue;
                    };
                    if destination.as_local() == Some(local) && definition.replace(value).is_some()
                    {
                        visiting.remove(&local);
                        return None;
                    }
                }
            }
            let value = symbolic_rvalue(tcx, body, definition?, wave, depth + 1, visiting);
            visiting.remove(&local);
            value
        }
        Operand::RuntimeChecks(_) => None,
    }
}

fn symbolic_rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    value: &Rvalue<'tcx>,
    wave: Local,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> Option<SymbolicValueV1> {
    match value {
        Rvalue::Use(operand) | Rvalue::Cast(_, operand, _) => {
            symbolic_operand(tcx, body, operand, wave, depth, visiting)
        }
        Rvalue::BinaryOp(operation, operands) => {
            let left = symbolic_operand(tcx, body, &operands.0, wave, depth, visiting)?;
            let right = symbolic_operand(tcx, body, &operands.1, wave, depth, visiting)?;
            match operation {
                BinOp::Add => Some(add(left, right)),
                BinOp::Mul => Some(multiply(left, right)),
                BinOp::Div => Some(divide(left, right)),
                BinOp::Rem => Some(remainder(left, right)),
                BinOp::Lt => Some(SymbolicValueV1::LessThan(Box::new(left), Box::new(right))),
                _ => None,
            }
        }
        _ => None,
    }
}

const fn constant(value: u128) -> SymbolicValueV1 {
    SymbolicValueV1::Constant(value)
}

fn add(left: SymbolicValueV1, right: SymbolicValueV1) -> SymbolicValueV1 {
    SymbolicValueV1::Add(Box::new(left), Box::new(right))
}

fn multiply(left: SymbolicValueV1, right: SymbolicValueV1) -> SymbolicValueV1 {
    SymbolicValueV1::Multiply(Box::new(left), Box::new(right))
}

fn divide(left: SymbolicValueV1, right: SymbolicValueV1) -> SymbolicValueV1 {
    SymbolicValueV1::Divide(Box::new(left), Box::new(right))
}

fn remainder(left: SymbolicValueV1, right: SymbolicValueV1) -> SymbolicValueV1 {
    SymbolicValueV1::Remainder(Box::new(left), Box::new(right))
}

fn call_args<'a, 'tcx>(
    body: &'a Body<'tcx>,
    block: BasicBlock,
) -> Result<&'a [Spanned<Operand<'tcx>>], GeneralGemmMirImportErrorV1> {
    let Some(terminator) = &body.basic_blocks[block].terminator else {
        return Err(missing_terminator(block));
    };
    let TerminatorKind::Call { args, .. } = &terminator.kind else {
        return Err(unproved("recorded semantic event remains a MIR call"));
    };
    Ok(args)
}

fn is_kernel_argument(operand: &Operand<'_>, zero_based_index: usize) -> bool {
    let expected = Local::from_usize(zero_based_index + 1);
    matches!(operand, Operand::Copy(place) | Operand::Move(place) if place.as_local() == Some(expected))
}

fn is_kernel_argument_or_alias(
    body: &Body<'_>,
    operand: &Operand<'_>,
    zero_based_index: usize,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> bool {
    if depth >= 32 {
        return false;
    }
    if is_kernel_argument(operand, zero_based_index) {
        return true;
    }
    let Some(local) = operand_local(operand) else {
        return false;
    };
    if !visiting.insert(local) {
        return false;
    }
    let definitions = body
        .basic_blocks
        .iter()
        .flat_map(|data| &data.statements)
        .filter_map(|statement| {
            let (destination, value) = statement.kind.as_assign()?;
            (destination.as_local() == Some(local)).then_some(value)
        })
        .collect::<Vec<_>>();
    let exact = !definitions.is_empty()
        && definitions.iter().all(|value| match value {
            Rvalue::Use(value) => {
                is_kernel_argument_or_alias(body, value, zero_based_index, depth + 1, visiting)
            }
            Rvalue::Ref(_, _, place) => {
                place.as_local() == Some(Local::from_usize(zero_based_index + 1))
            }
            _ => false,
        });
    visiting.remove(&local);
    exact
}

fn array_value_locals(
    body: &Body<'_>,
    operand: &Operand<'_>,
    use_block: BasicBlock,
) -> Result<Vec<Local>, GeneralGemmMirImportErrorV1> {
    let Some(array) = (match operand {
        Operand::Copy(place) | Operand::Move(place) => place.as_local(),
        _ => None,
    }) else {
        return Err(unproved("stage array has local MIR provenance"));
    };
    let mut found = None;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        for statement in &data.statements {
            let Some((destination, Rvalue::Aggregate(_, elements))) = statement.kind.as_assign()
            else {
                continue;
            };
            if destination.as_local() != Some(array) {
                continue;
            }
            if !dominates(body, block, use_block) {
                return Err(unproved(
                    "stage array aggregate definition dominates the stage event",
                ));
            }
            let values = elements
                .iter()
                .map(|element| match element {
                    Operand::Copy(place) | Operand::Move(place) => place
                        .as_local()
                        .ok_or_else(|| unproved("stage component has local MIR provenance")),
                    _ => Err(unproved("stage component has local MIR provenance")),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if found.replace(values).is_some() {
                return Err(unproved(
                    "stage array has one reaching aggregate definition",
                ));
            }
        }
    }
    found.ok_or_else(|| unproved("stage array has one aggregate definition"))
}

fn defining_call<'a, 'tcx>(
    body: &'a Body<'tcx>,
    local: Local,
    use_block: BasicBlock,
) -> Result<
    (
        rustc_hir::def_id::DefId,
        &'a [Spanned<Operand<'tcx>>],
        BasicBlock,
    ),
    GeneralGemmMirImportErrorV1,
> {
    let mut found = None;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        let Some(terminator) = &data.terminator else {
            continue;
        };
        let TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } = &terminator.kind
        else {
            continue;
        };
        if destination.as_local() != Some(local) {
            continue;
        }
        let Operand::Constant(function) = func else {
            return Err(unproved("stage component producer is a direct function"));
        };
        let TyKind::FnDef(definition, _) = function.const_.ty().kind() else {
            return Err(unproved("stage component producer is a direct function"));
        };
        if !dominates(body, block, use_block) {
            return Err(unproved(
                "stage component call definition dominates its stage use",
            ));
        }
        if found.replace((*definition, &args[..], block)).is_some() {
            return Err(unproved("stage component has one reaching call definition"));
        }
    }
    found.ok_or_else(|| unproved("stage component is returned by the guarded loader"))
}

fn constant_u16_from_constant<'tcx>(
    tcx: TyCtxt<'tcx>,
    constant: &rustc_middle::mir::ConstOperand<'tcx>,
) -> Option<u16> {
    constant
        .const_
        .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
        .and_then(|value| u16::try_from(value.to_bits(value.size())).ok())
}

fn hash_fields(fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    for field in fields {
        digest.update((field.len() as u64).to_le_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

fn general_gemm_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    budget: &mut GeneralGemmCallBudgetV1,
) -> Result<Vec<GeneralGemmCallV1>, GeneralGemmMirImportErrorV1> {
    let mut calls = Vec::new();
    calls
        .try_reserve_exact(MAX_GENERAL_GEMM_TERMINAL_CALLS_V1)
        .map_err(|_| {
            GeneralGemmMirImportErrorV1::new(
                "general GEMM terminal analysis could not reserve its fixed call budget",
            )
        })?;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        let Some(terminator) = &data.terminator else {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "general GEMM MIR block bb{} has no terminator",
                block.as_usize()
            )));
        };
        if !matches!(&terminator.kind, TerminatorKind::Call { .. }) {
            continue;
        }
        budget.observe_reachable_call()?;
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target: Some(return_target),
            ..
        } = &terminator.kind
        else {
            continue;
        };
        let Operand::Constant(constant) = func else {
            continue;
        };
        let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
            continue;
        };
        let Some(TrustedDeviceItem::GeneralGemm(surface, operation)) =
            trusted_device_items::classify(tcx, *def_id)
        else {
            continue;
        };
        budget.observe_general_gemm_terminal()?;
        calls.push(GeneralGemmCallV1 {
            surface,
            operation,
            block,
            return_target: *return_target,
            result_local: destination.as_local(),
            span: terminator.source_info.span,
            evidence: derive_evidence(tcx, body, operation, args)?,
        });
    }
    Ok(calls)
}

fn derive_evidence<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operation: TrustedGeneralGemmOperationV1,
    args: &[Spanned<Operand<'tcx>>],
) -> Result<GeneralGemmEvidenceV1, GeneralGemmMirImportErrorV1> {
    let operand = |index: usize| {
        args.get(index)
            .map(|argument| &argument.node)
            .ok_or_else(|| {
                GeneralGemmMirImportErrorV1::new(format!(
                    "proof-sensitive general GEMM {operation:?} omitted operand {index}"
                ))
            })
    };
    match operation {
        TrustedGeneralGemmOperationV1::LoadA => {
            if same_runtime_value(operand(2)?, operand(4)?)
                || same_runtime_value(operand(3)?, operand(5)?)
            {
                Ok(GeneralGemmEvidenceV1::UnguardedA)
            } else {
                Ok(GeneralGemmEvidenceV1::None)
            }
        }
        TrustedGeneralGemmOperationV1::LoadB => {
            if same_runtime_value(operand(2)?, operand(4)?)
                || same_runtime_value(operand(3)?, operand(5)?)
            {
                Ok(GeneralGemmEvidenceV1::UnguardedB)
            } else {
                Ok(GeneralGemmEvidenceV1::None)
            }
        }
        TrustedGeneralGemmOperationV1::StageValue => {
            if !same_runtime_value(operand(3)?, operand(4)?) {
                return Ok(GeneralGemmEvidenceV1::None);
            }
            let value = constant_u16(tcx, operand(5)?)
                .ok_or_else(|| unproved("K-tail staged value is a compile-time BF16 value"))?;
            if value == 0 {
                return Ok(GeneralGemmEvidenceV1::None);
            }
            Ok(GeneralGemmEvidenceV1::NonzeroTail)
        }
        TrustedGeneralGemmOperationV1::StoreEpilogue => {
            let symbolic = |operand: &Operand<'tcx>| {
                symbolic_f32_operand(tcx, body, operand, 0, &mut BTreeSet::new())
            };
            let value = symbolic(operand(7)?)
                .ok_or_else(|| unproved("epilogue result has bounded MIR value provenance"))?;
            let alpha = symbolic(operand(8)?)
                .ok_or_else(|| unproved("epilogue alpha has bounded MIR value provenance"))?;
            let accumulator = symbolic(operand(9)?)
                .ok_or_else(|| unproved("epilogue accumulator has bounded MIR value provenance"))?;
            let beta = symbolic(operand(10)?)
                .ok_or_else(|| unproved("epilogue beta has bounded MIR value provenance"))?;
            let initial = symbolic(operand(11)?)
                .ok_or_else(|| unproved("epilogue C input has bounded MIR value provenance"))?;
            let missing_beta = SymbolicF32ValueV1::Add(
                Box::new(SymbolicF32ValueV1::Multiply(
                    Box::new(alpha.clone()),
                    Box::new(accumulator.clone()),
                )),
                Box::new(initial.clone()),
            );
            if value == missing_beta
                && !matches!(beta, SymbolicF32ValueV1::Constant(bits) if bits == 1.0_f32.to_bits())
            {
                return Ok(GeneralGemmEvidenceV1::WrongEpilogue);
            }
            let canonical = SymbolicF32ValueV1::Add(
                Box::new(SymbolicF32ValueV1::Multiply(
                    Box::new(alpha),
                    Box::new(accumulator),
                )),
                Box::new(SymbolicF32ValueV1::Multiply(
                    Box::new(beta),
                    Box::new(initial),
                )),
            );
            if value == canonical {
                Ok(GeneralGemmEvidenceV1::None)
            } else {
                Err(unproved(
                    "epilogue result is exact alpha * accumulator + beta * C or a derived counterexample",
                ))
            }
        }
        _ => Ok(GeneralGemmEvidenceV1::None),
    }
}

fn same_runtime_value(left: &Operand<'_>, right: &Operand<'_>) -> bool {
    match (left, right) {
        (
            Operand::Copy(left) | Operand::Move(left),
            Operand::Copy(right) | Operand::Move(right),
        ) => left
            .as_local()
            .is_some_and(|left| right.as_local() == Some(left)),
        _ => false,
    }
}

fn constant_u16<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<u16> {
    let Operand::Constant(constant) = operand else {
        return None;
    };
    constant
        .const_
        .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
        .and_then(|value| u16::try_from(value.to_bits(value.size())).ok())
}

fn symbolic_f32_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> Option<SymbolicF32ValueV1> {
    if depth >= 64 {
        return None;
    }
    match operand {
        Operand::Constant(constant)
            if matches!(constant.const_.ty().kind(), TyKind::Float(FloatTy::F32)) =>
        {
            constant
                .const_
                .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
                .map(|value| SymbolicF32ValueV1::Constant(value.to_u32()))
        }
        Operand::Constant(_) | Operand::RuntimeChecks(_) => None,
        Operand::Copy(place) | Operand::Move(place) => {
            let local = place.as_local()?;
            if local.as_usize() > 0 && local.as_usize() <= body.arg_count {
                return u8::try_from(local.as_usize() - 1)
                    .ok()
                    .map(SymbolicF32ValueV1::KernelArgument);
            }
            if !visiting.insert(local) {
                return None;
            }
            let mut definition = None;
            for block in body.basic_blocks.iter() {
                for statement in &block.statements {
                    let Some((destination, value)) = statement.kind.as_assign() else {
                        continue;
                    };
                    if destination.as_local() == Some(local) && definition.replace(value).is_some()
                    {
                        visiting.remove(&local);
                        return Some(SymbolicF32ValueV1::OpaqueLocal(local.as_usize()));
                    }
                }
            }
            let Some(definition) = definition else {
                visiting.remove(&local);
                return Some(SymbolicF32ValueV1::OpaqueLocal(local.as_usize()));
            };
            let value = symbolic_f32_rvalue(tcx, body, definition, depth + 1, visiting);
            visiting.remove(&local);
            value
        }
    }
}

fn symbolic_f32_rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    value: &Rvalue<'tcx>,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> Option<SymbolicF32ValueV1> {
    match value {
        Rvalue::Use(operand) => symbolic_f32_operand(tcx, body, operand, depth, visiting),
        Rvalue::BinaryOp(operation, operands) => {
            let left = symbolic_f32_operand(tcx, body, &operands.0, depth, visiting)?;
            let right = symbolic_f32_operand(tcx, body, &operands.1, depth, visiting)?;
            match operation {
                BinOp::Add => Some(SymbolicF32ValueV1::Add(Box::new(left), Box::new(right))),
                BinOp::Mul => Some(SymbolicF32ValueV1::Multiply(
                    Box::new(left),
                    Box::new(right),
                )),
                _ => None,
            }
        }
        _ => None,
    }
}

fn proof_symbolic_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
    phase: Local,
    operand: &Operand<'tcx>,
) -> Result<ProofSymbolicValueV1, GeneralGemmMirImportErrorV1> {
    proof_symbolic_operand_inner(tcx, body, calls, phase, operand, 0, &mut BTreeSet::new())
        .ok_or_else(|| unproved("integer operand has bounded elemental GEMM provenance"))
}

fn proof_symbolic_operand_inner<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
    phase: Local,
    operand: &Operand<'tcx>,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> Option<ProofSymbolicValueV1> {
    if depth >= 64 {
        return None;
    }
    match operand {
        Operand::Constant(constant) => constant
            .const_
            .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
            .map(|value| ProofSymbolicValueV1::Constant(value.to_bits(value.size()))),
        Operand::RuntimeChecks(_) => None,
        Operand::Copy(place) | Operand::Move(place) => {
            let local = place.as_local()?;
            if local == phase {
                return Some(ProofSymbolicValueV1::Phase);
            }
            for (operation, symbolic) in [
                (
                    TrustedGeneralGemmOperationV1::Lane,
                    ProofSymbolicValueV1::Lane,
                ),
                (
                    TrustedGeneralGemmOperationV1::WorkgroupX,
                    ProofSymbolicValueV1::WorkgroupX,
                ),
                (
                    TrustedGeneralGemmOperationV1::WorkgroupY,
                    ProofSymbolicValueV1::WorkgroupY,
                ),
            ] {
                if calls
                    .iter()
                    .any(|call| call.operation == operation && call.result_local == Some(local))
                {
                    return Some(symbolic);
                }
            }
            if local.as_usize() > 0 && local.as_usize() <= body.arg_count {
                return u8::try_from(local.as_usize() - 1)
                    .ok()
                    .map(ProofSymbolicValueV1::KernelArgument);
            }
            if !visiting.insert(local) {
                return None;
            }
            let definitions = body
                .basic_blocks
                .iter()
                .flat_map(|data| &data.statements)
                .filter_map(|statement| statement.kind.as_assign())
                .filter_map(|(destination, value)| {
                    (destination.as_local() == Some(local)).then_some(value)
                })
                .collect::<Vec<_>>();
            let value = match &definitions[..] {
                [value] => {
                    proof_symbolic_rvalue(tcx, body, calls, phase, value, depth + 1, visiting)
                }
                _ if is_zero_to_four_induction(tcx, local, &definitions) => {
                    Some(ProofSymbolicValueV1::Component)
                }
                _ => None,
            };
            visiting.remove(&local);
            value
        }
    }
}

fn proof_symbolic_rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    calls: &[GeneralGemmCallV1],
    phase: Local,
    value: &Rvalue<'tcx>,
    depth: usize,
    visiting: &mut BTreeSet<Local>,
) -> Option<ProofSymbolicValueV1> {
    match value {
        Rvalue::Use(operand) | Rvalue::Cast(_, operand, _) => {
            proof_symbolic_operand_inner(tcx, body, calls, phase, operand, depth, visiting)
        }
        Rvalue::BinaryOp(operation, operands) => {
            let left = proof_symbolic_operand_inner(
                tcx,
                body,
                calls,
                phase,
                &operands.0,
                depth,
                visiting,
            )?;
            let right = proof_symbolic_operand_inner(
                tcx,
                body,
                calls,
                phase,
                &operands.1,
                depth,
                visiting,
            )?;
            match operation {
                BinOp::Add => Some(proof_add(left, right)),
                BinOp::Sub => Some(ProofSymbolicValueV1::Subtract(
                    Box::new(left),
                    Box::new(right),
                )),
                BinOp::Mul => Some(proof_mul(left, right)),
                BinOp::Div => Some(proof_div(left, right)),
                BinOp::Rem => Some(proof_rem(left, right)),
                BinOp::BitXor => Some(proof_xor(left, right)),
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_zero_to_four_induction<'tcx>(
    tcx: TyCtxt<'tcx>,
    local: Local,
    definitions: &[&Rvalue<'tcx>],
) -> bool {
    let mut zero = false;
    let mut increment = false;
    for value in definitions {
        match value {
            Rvalue::Use(Operand::Constant(constant))
                if constant_u16_from_constant(tcx, constant) == Some(0) =>
            {
                zero = true
            }
            Rvalue::BinaryOp(BinOp::Add, operands)
                if operand_local(&operands.0) == Some(local)
                    && matches!(&operands.1, Operand::Constant(constant) if constant_u16_from_constant(tcx, constant) == Some(1)) =>
            {
                increment = true;
            }
            _ => {}
        }
    }
    zero && increment
}

const fn proof_constant(value: u128) -> ProofSymbolicValueV1 {
    ProofSymbolicValueV1::Constant(value)
}

fn proof_add(left: ProofSymbolicValueV1, right: ProofSymbolicValueV1) -> ProofSymbolicValueV1 {
    if left == ProofSymbolicValueV1::Constant(0) {
        return right;
    }
    if right == ProofSymbolicValueV1::Constant(0) {
        return left;
    }
    ProofSymbolicValueV1::Add(Box::new(left), Box::new(right))
}

fn proof_mul(left: ProofSymbolicValueV1, right: ProofSymbolicValueV1) -> ProofSymbolicValueV1 {
    ProofSymbolicValueV1::Multiply(Box::new(left), Box::new(right))
}

fn proof_div(left: ProofSymbolicValueV1, right: ProofSymbolicValueV1) -> ProofSymbolicValueV1 {
    ProofSymbolicValueV1::Divide(Box::new(left), Box::new(right))
}

fn proof_rem(left: ProofSymbolicValueV1, right: ProofSymbolicValueV1) -> ProofSymbolicValueV1 {
    ProofSymbolicValueV1::Remainder(Box::new(left), Box::new(right))
}

fn proof_xor(left: ProofSymbolicValueV1, right: ProofSymbolicValueV1) -> ProofSymbolicValueV1 {
    ProofSymbolicValueV1::BitXor(Box::new(left), Box::new(right))
}

fn operand_local(operand: &Operand<'_>) -> Option<Local> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place.as_local(),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => None,
    }
}

fn local_has_u16_constant_assignment<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    local: Local,
    expected: u16,
) -> bool {
    body.basic_blocks
        .iter()
        .flat_map(|data| &data.statements)
        .any(|statement| {
            let Some((destination, Rvalue::Use(Operand::Constant(constant)))) =
                statement.kind.as_assign()
            else {
                return false;
            };
            destination.as_local() == Some(local)
                && constant_u16_from_constant(tcx, constant) == Some(expected)
        })
}

fn local_has_f32_constant_assignment<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    local: Local,
    expected: f32,
) -> bool {
    body.basic_blocks
        .iter()
        .flat_map(|data| &data.statements)
        .any(|statement| {
            let Some((destination, Rvalue::Use(Operand::Constant(constant)))) =
                statement.kind.as_assign()
            else {
                return false;
            };
            destination.as_local() == Some(local)
                && constant
                    .const_
                    .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
                    .is_some_and(|value| value.to_u32() == expected.to_bits())
        })
}

fn local_is_assigned_from(body: &Body<'_>, destination: Local, source: Local) -> bool {
    body.basic_blocks
        .iter()
        .flat_map(|data| &data.statements)
        .any(|statement| {
            let Some((place, Rvalue::Use(operand))) = statement.kind.as_assign() else {
                return false;
            };
            place.as_local() == Some(destination) && operand_local(operand) == Some(source)
        })
}

fn same_result_operand(operand: &Operand<'_>, result: Option<Local>) -> bool {
    result.is_some_and(|result| operand_local(operand) == Some(result))
}

fn unproved(property: &str) -> GeneralGemmMirImportErrorV1 {
    GeneralGemmMirImportErrorV1::new(format!(
        "general GEMM semantic fact is Unknown/Unproved: {property}"
    ))
}

fn publish_is_lane_conditional(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<bool, GeneralGemmMirImportErrorV1> {
    let Some(lane) = optional_call(calls, TrustedGeneralGemmOperationV1::Lane)? else {
        return Ok(false);
    };
    let lane = lane
        .result_local
        .ok_or_else(|| unproved("lane identity reaches the barrier condition"))?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let publish = unique_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let reuse = optional_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    if reuse.is_some_and(|reuse| {
        all_paths_reach_before(body, stage.return_target, publish.block, reuse.block)
    }) || reuse.is_none() && all_paths_reach(body, stage.return_target, publish.block)
    {
        return Ok(false);
    }
    Ok(region_has_lane_switch(
        body,
        stage.return_target,
        publish.block,
        reuse.map(|reuse| reuse.block),
        lane,
    ))
}

fn all_paths_reach(body: &Body<'_>, from: BasicBlock, to: BasicBlock) -> bool {
    let mut pending = VecDeque::from([from]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if block == to || !visited.insert(block) {
            continue;
        }
        let Ok(successors) = normal_successors(body, block) else {
            return false;
        };
        if successors.is_empty() {
            return false;
        }
        pending.extend(successors);
    }
    true
}

fn region_has_lane_switch(
    body: &Body<'_>,
    from: BasicBlock,
    boundary: BasicBlock,
    forbidden: Option<BasicBlock>,
    lane: Local,
) -> bool {
    let mut tainted = BTreeSet::from([lane]);
    for _ in 0..64 {
        let before = tainted.len();
        for block in body.basic_blocks.iter() {
            for statement in &block.statements {
                let Some((destination, value)) = statement.kind.as_assign() else {
                    continue;
                };
                let Some(destination) = destination.as_local() else {
                    continue;
                };
                if rvalue_uses_tainted(value, &tainted) {
                    tainted.insert(destination);
                }
            }
        }
        if tainted.len() == before {
            break;
        }
    }
    let mut pending = VecDeque::from([from]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if block == boundary || Some(block) == forbidden || !visited.insert(block) {
            continue;
        }
        let Some(terminator) = &body.basic_blocks[block].terminator else {
            continue;
        };
        if let TerminatorKind::SwitchInt { discr, .. } = &terminator.kind
            && operand_uses_tainted(discr, &tainted)
        {
            return true;
        }
        pending.extend(normal_successors(body, block).unwrap_or_default());
    }
    false
}

fn rvalue_uses_tainted(value: &Rvalue<'_>, tainted: &BTreeSet<Local>) -> bool {
    match value {
        Rvalue::Use(operand)
        | Rvalue::Repeat(operand, _)
        | Rvalue::Cast(_, operand, _)
        | Rvalue::UnaryOp(_, operand) => operand_uses_tainted(operand, tainted),
        Rvalue::BinaryOp(_, operands) => {
            operand_uses_tainted(&operands.0, tainted) || operand_uses_tainted(&operands.1, tainted)
        }
        _ => false,
    }
}

fn operand_uses_tainted(operand: &Operand<'_>, tainted: &BTreeSet<Local>) -> bool {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place
            .as_local()
            .is_some_and(|local| tainted.contains(&local)),
        Operand::Constant(_) | Operand::RuntimeChecks(_) => false,
    }
}

fn unique_surface(
    calls: &[GeneralGemmCallV1],
) -> Result<TrustedGeneralGemmSurfaceV1, GeneralGemmMirImportErrorV1> {
    let Some(first) = calls.first() else {
        return Err(GeneralGemmMirImportErrorV1::new(
            "general GEMM adapter was selected without a terminal call",
        ));
    };
    if calls.iter().any(|call| call.surface != first.surface) {
        return Err(GeneralGemmMirImportErrorV1::new(
            "general GEMM kernel mixes typestate and proof-sensitive terminal surfaces",
        ));
    }
    Ok(first.surface)
}

fn validate_call_shape(
    body: &Body<'_>,
    surface: TrustedGeneralGemmSurfaceV1,
    calls: &[GeneralGemmCallV1],
    lane_conditional_publish: bool,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    for operation in [
        TrustedGeneralGemmOperationV1::Lane,
        TrustedGeneralGemmOperationV1::WorkgroupX,
        TrustedGeneralGemmOperationV1::WorkgroupY,
        TrustedGeneralGemmOperationV1::LoadA,
        TrustedGeneralGemmOperationV1::LoadB,
        TrustedGeneralGemmOperationV1::WaitStage,
    ] {
        require_count(calls, operation, 0, 1)?;
    }
    require_count(calls, TrustedGeneralGemmOperationV1::StageValue, 0, 2)?;
    require_count(calls, TrustedGeneralGemmOperationV1::ReadStage, 0, 8)?;
    require_count(calls, TrustedGeneralGemmOperationV1::LoadC, 0, 4)?;
    require_count(calls, TrustedGeneralGemmOperationV1::StoreEpilogue, 0, 4)?;
    require_count(calls, TrustedGeneralGemmOperationV1::Mfma, 0, 1)?;
    require_count(calls, TrustedGeneralGemmOperationV1::MfmaValue, 0, 4)?;
    let mfma = call_count(calls, TrustedGeneralGemmOperationV1::Mfma);
    let mfma_values = call_count(calls, TrustedGeneralGemmOperationV1::MfmaValue);
    if !((mfma == 1 && mfma_values == 0) || (mfma == 0 && mfma_values == 4)) {
        return Err(unproved(
            "MFMA is one opaque fragment event or four elemental carried event sites",
        ));
    }
    for operation in [
        TrustedGeneralGemmOperationV1::Acquire,
        TrustedGeneralGemmOperationV1::Stage,
        TrustedGeneralGemmOperationV1::Reuse,
    ] {
        require_count(calls, operation, 1, 1)?;
    }
    let publish_minimum = match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => 1,
        TrustedGeneralGemmSurfaceV1::ProofSensitive => 0,
    };
    require_count(
        calls,
        TrustedGeneralGemmOperationV1::Publish,
        publish_minimum,
        1,
    )?;
    match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => {
            require_count(calls, TrustedGeneralGemmOperationV1::Store, 1, 1)?;
            require_count(calls, TrustedGeneralGemmOperationV1::StoreEpilogue, 0, 0)?;
        }
        TrustedGeneralGemmSurfaceV1::ProofSensitive => {
            require_count(calls, TrustedGeneralGemmOperationV1::Store, 0, 4)?;
            let stores = store_calls(calls).len();
            if !(1..=4).contains(&stores) {
                return Err(GeneralGemmMirImportErrorV1::new(format!(
                    "general GEMM MIR has {stores} semantic store call(s); expected 1 through 4"
                )));
            }
        }
    }

    match surface {
        TrustedGeneralGemmSurfaceV1::Typestate => validate_typestate_template_order(body, calls),
        TrustedGeneralGemmSurfaceV1::ProofSensitive
            if call_count(calls, TrustedGeneralGemmOperationV1::MfmaValue) == 4 =>
        {
            validate_dynamic_proof_sensitive_order(body, calls)
        }
        TrustedGeneralGemmSurfaceV1::ProofSensitive if lane_conditional_publish => {
            validate_typestate_template_order(body, calls)
        }
        TrustedGeneralGemmSurfaceV1::ProofSensitive => validate_proof_sensitive_order(body, calls),
    }
}

fn validate_dynamic_proof_sensitive_order(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let wait = unique_call(calls, TrustedGeneralGemmOperationV1::WaitStage)?;
    let publish = unique_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let reuse = unique_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    let mfma = calls
        .iter()
        .filter(|call| call.operation == TrustedGeneralGemmOperationV1::MfmaValue)
        .collect::<Vec<_>>();
    let stores = store_calls(calls);
    require_reachable(body, acquire.return_target, stage.block, "acquire", "stage")?;
    require_reachable(body, stage.return_target, wait.block, "stage", "stage wait")?;
    require_reachable(
        body,
        wait.return_target,
        publish.block,
        "stage wait",
        "publish",
    )?;
    require_reachable(
        body,
        publish.return_target,
        mfma[0].block,
        "publish",
        "first carried MFMA",
    )?;
    for pair in mfma.windows(2) {
        require_reachable(
            body,
            pair[0].return_target,
            pair[1].block,
            "carried MFMA",
            "next carried MFMA",
        )?;
    }
    require_reachable(
        body,
        mfma[3].return_target,
        reuse.block,
        "last carried MFMA",
        "reuse",
    )?;
    if !reachable(body, reuse.return_target, stage.block)
        || !reachable(body, reuse.return_target, stores[0].block)
    {
        return Err(unproved(
            "reuse advances the bounded phase loop or reaches the guarded epilogue",
        ));
    }
    for pair in stores.windows(2) {
        if !reachable(body, pair[0].return_target, pair[1].block) {
            return Err(unproved("four guarded C stores remain in component order"));
        }
    }
    Ok(())
}

// The move-checked typestate API owns local ordering. This intentionally loose
// template admission remains non-authoritative until runtime plan binding.
fn validate_typestate_template_order(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let mfma = unique_mfma_call(calls)?;
    let reuse = unique_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    require_reachable(body, acquire.return_target, stage.block, "acquire", "stage")?;
    if let Some(publish) = optional_call(calls, TrustedGeneralGemmOperationV1::Publish)? {
        require_reachable(body, stage.return_target, publish.block, "stage", "publish")?;
        require_reachable(body, publish.return_target, mfma.block, "publish", "MFMA")?;
    } else {
        require_reachable(body, stage.return_target, mfma.block, "stage", "MFMA")?;
    }
    require_reachable(body, mfma.return_target, reuse.block, "MFMA", "reuse")?;

    let stores = store_calls(calls);
    require_reachable(body, reuse.return_target, stores[0].block, "reuse", "store")?;
    if stores.len() == 2 {
        require_reachable(
            body,
            stores[0].return_target,
            stores[1].block,
            "first store",
            "second store",
        )?;
    }
    Ok(())
}

fn validate_proof_sensitive_order(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let acquire = unique_call(calls, TrustedGeneralGemmOperationV1::Acquire)?;
    let stage = unique_call(calls, TrustedGeneralGemmOperationV1::Stage)?;
    let publish = optional_call(calls, TrustedGeneralGemmOperationV1::Publish)?;
    let mfma = unique_mfma_call(calls)?;
    let reuse = unique_call(calls, TrustedGeneralGemmOperationV1::Reuse)?;
    let stores = store_calls(calls);

    let mut prefix = vec![acquire, stage];
    prefix.extend(publish);
    prefix.extend([mfma, reuse]);

    let mut ordered = prefix.clone();
    ordered.extend(stores.iter().copied());
    match validate_acyclic_lifecycle(body, calls, &ordered) {
        Ok(()) => Ok(()),
        Err(first_error) if stores.len() == 2 => {
            let mut reversed = prefix;
            reversed.extend([stores[1], stores[0]]);
            validate_acyclic_lifecycle(body, calls, &reversed).map_err(|_| first_error)
        }
        Err(error) => Err(error),
    }
}

fn validate_acyclic_lifecycle(
    body: &Body<'_>,
    calls: &[GeneralGemmCallV1],
    ordered: &[&GeneralGemmCallV1],
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let lifecycle_blocks = calls
        .iter()
        .filter(|call| is_lifecycle_operation(call.operation))
        .map(|call| call.block)
        .collect::<BTreeSet<_>>();
    let mut from = START_BLOCK;
    let mut from_name = "kernel entry";
    for call in ordered {
        require_all_paths_reach_lifecycle(
            body,
            from,
            call.block,
            from_name,
            operation_name(call.operation),
            &lifecycle_blocks,
        )?;
        from = call.return_target;
        from_name = operation_name(call.operation);
    }
    require_all_paths_reach_return(body, from, from_name, &lifecycle_blocks)
}

fn require_all_paths_reach_lifecycle(
    body: &Body<'_>,
    from: BasicBlock,
    to: BasicBlock,
    from_name: &str,
    to_name: &str,
    lifecycle_blocks: &BTreeSet<BasicBlock>,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mut pending = VecDeque::from([from]);
    let mut region = BTreeSet::new();
    let mut reached_target = false;
    while let Some(block) = pending.pop_front() {
        if block == to {
            reached_target = true;
            continue;
        }
        if !region.insert(block) {
            continue;
        }
        if lifecycle_blocks.contains(&block) {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM reaches a different lifecycle event before {to_name} after {from_name}"
            )));
        }
        let successors = normal_successors(body, block)?;
        if successors.is_empty() {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM has a normal path that bypasses {to_name} after {from_name}"
            )));
        }
        pending.extend(successors);
    }
    if !reached_target {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "proof-sensitive general GEMM has no normal CFG path from {from_name} to {to_name}"
        )));
    }
    require_acyclic_region(body, &region, Some(to), from_name, to_name)
}

fn require_all_paths_reach_return(
    body: &Body<'_>,
    from: BasicBlock,
    from_name: &str,
    lifecycle_blocks: &BTreeSet<BasicBlock>,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mut pending = VecDeque::from([from]);
    let mut region = BTreeSet::new();
    let mut reached_return = false;
    while let Some(block) = pending.pop_front() {
        if !region.insert(block) {
            continue;
        }
        if lifecycle_blocks.contains(&block) {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM repeats a lifecycle event after {from_name}"
            )));
        }
        let Some(terminator) = &body.basic_blocks[block].terminator else {
            return Err(missing_terminator(block));
        };
        if matches!(&terminator.kind, TerminatorKind::Return) {
            reached_return = true;
            continue;
        }
        let successors = normal_successors(body, block)?;
        if successors.is_empty() {
            return Err(GeneralGemmMirImportErrorV1::new(format!(
                "proof-sensitive general GEMM has a normal path that does not return after {from_name}"
            )));
        }
        pending.extend(successors);
    }
    if !reached_return {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "proof-sensitive general GEMM has no normal return after {from_name}"
        )));
    }
    require_acyclic_region(body, &region, None, from_name, "return")
}

fn require_acyclic_region(
    body: &Body<'_>,
    region: &BTreeSet<BasicBlock>,
    boundary: Option<BasicBlock>,
    from_name: &str,
    to_name: &str,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let mut indegree = vec![0_usize; body.basic_blocks.len()];
    for &block in region {
        for successor in normal_successors(body, block)? {
            if Some(successor) != boundary && region.contains(&successor) {
                indegree[successor.as_usize()] += 1;
            }
        }
    }
    let mut ready = region
        .iter()
        .copied()
        .filter(|block| indegree[block.as_usize()] == 0)
        .collect::<VecDeque<_>>();
    let mut removed = 0_usize;
    while let Some(block) = ready.pop_front() {
        removed += 1;
        for successor in normal_successors(body, block)? {
            if Some(successor) == boundary || !region.contains(&successor) {
                continue;
            }
            indegree[successor.as_usize()] -= 1;
            if indegree[successor.as_usize()] == 0 {
                ready.push_back(successor);
            }
        }
    }
    if removed != region.len() {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "proof-sensitive general GEMM has a cyclic normal CFG between {from_name} and {to_name}"
        )));
    }
    Ok(())
}

fn normal_successors(
    body: &Body<'_>,
    block: BasicBlock,
) -> Result<Vec<BasicBlock>, GeneralGemmMirImportErrorV1> {
    let Some(terminator) = &body.basic_blocks[block].terminator else {
        return Err(missing_terminator(block));
    };
    let successors = match &terminator.kind {
        TerminatorKind::FalseEdge { real_target, .. }
        | TerminatorKind::FalseUnwind { real_target, .. } => vec![*real_target],
        _ => terminator.successors().collect(),
    };
    Ok(successors
        .into_iter()
        .filter(|successor| !body.basic_blocks[*successor].is_cleanup)
        .collect())
}

fn missing_terminator(block: BasicBlock) -> GeneralGemmMirImportErrorV1 {
    GeneralGemmMirImportErrorV1::new(format!(
        "general GEMM MIR block bb{} has no terminator",
        block.as_usize()
    ))
}

const fn operation_name(operation: TrustedGeneralGemmOperationV1) -> &'static str {
    match operation {
        TrustedGeneralGemmOperationV1::Acquire => "acquire",
        TrustedGeneralGemmOperationV1::Lane => "lane identity",
        TrustedGeneralGemmOperationV1::WorkgroupX => "workgroup X",
        TrustedGeneralGemmOperationV1::WorkgroupY => "workgroup Y",
        TrustedGeneralGemmOperationV1::LoadA => "A load",
        TrustedGeneralGemmOperationV1::LoadB => "B load",
        TrustedGeneralGemmOperationV1::LoadC => "C load",
        TrustedGeneralGemmOperationV1::Stage => "stage",
        TrustedGeneralGemmOperationV1::StageValue => "stage value",
        TrustedGeneralGemmOperationV1::WaitStage => "stage wait",
        TrustedGeneralGemmOperationV1::ReadStage => "LDS read",
        TrustedGeneralGemmOperationV1::Publish => "publish",
        TrustedGeneralGemmOperationV1::Mfma => "MFMA",
        TrustedGeneralGemmOperationV1::MfmaValue => "carried MFMA",
        TrustedGeneralGemmOperationV1::Reuse => "reuse",
        TrustedGeneralGemmOperationV1::Store => "store",
        TrustedGeneralGemmOperationV1::StoreEpilogue => "epilogue store",
    }
}

const fn is_lifecycle_operation(operation: TrustedGeneralGemmOperationV1) -> bool {
    matches!(
        operation,
        TrustedGeneralGemmOperationV1::Acquire
            | TrustedGeneralGemmOperationV1::Stage
            | TrustedGeneralGemmOperationV1::Publish
            | TrustedGeneralGemmOperationV1::Mfma
            | TrustedGeneralGemmOperationV1::MfmaValue
            | TrustedGeneralGemmOperationV1::Reuse
            | TrustedGeneralGemmOperationV1::Store
            | TrustedGeneralGemmOperationV1::StoreEpilogue
    )
}

fn store_calls(calls: &[GeneralGemmCallV1]) -> Vec<&GeneralGemmCallV1> {
    calls
        .iter()
        .filter(|call| {
            matches!(
                call.operation,
                TrustedGeneralGemmOperationV1::Store | TrustedGeneralGemmOperationV1::StoreEpilogue
            )
        })
        .collect()
}

fn require_count(
    calls: &[GeneralGemmCallV1],
    operation: TrustedGeneralGemmOperationV1,
    minimum: usize,
    maximum: usize,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    let actual = call_count(calls, operation);
    if actual < minimum || actual > maximum {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR has {actual} {operation:?} terminal call(s); expected {minimum} through {maximum}"
        )));
    }
    Ok(())
}

fn call_count(calls: &[GeneralGemmCallV1], operation: TrustedGeneralGemmOperationV1) -> usize {
    calls
        .iter()
        .filter(|call| call.operation == operation)
        .count()
}

fn unique_call(
    calls: &[GeneralGemmCallV1],
    operation: TrustedGeneralGemmOperationV1,
) -> Result<&GeneralGemmCallV1, GeneralGemmMirImportErrorV1> {
    optional_call(calls, operation)?.ok_or_else(|| {
        GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR omitted required {operation:?} terminal"
        ))
    })
}

fn unique_mfma_call(
    calls: &[GeneralGemmCallV1],
) -> Result<&GeneralGemmCallV1, GeneralGemmMirImportErrorV1> {
    let mut candidates = calls.iter().filter(|call| {
        matches!(
            call.operation,
            TrustedGeneralGemmOperationV1::Mfma | TrustedGeneralGemmOperationV1::MfmaValue
        )
    });
    let call = candidates
        .next()
        .ok_or_else(|| unproved("one MFMA event site is present"))?;
    if candidates.next().is_some() {
        return Err(unproved("one MFMA event site is present"));
    }
    Ok(call)
}

fn optional_call(
    calls: &[GeneralGemmCallV1],
    operation: TrustedGeneralGemmOperationV1,
) -> Result<Option<&GeneralGemmCallV1>, GeneralGemmMirImportErrorV1> {
    let mut matches = calls.iter().filter(|call| call.operation == operation);
    let first = matches.next();
    if matches.next().is_some() {
        return Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR has multiple {operation:?} terminals where one was required"
        )));
    }
    Ok(first)
}

fn require_reachable(
    body: &Body<'_>,
    from: BasicBlock,
    to: BasicBlock,
    from_name: &str,
    to_name: &str,
) -> Result<(), GeneralGemmMirImportErrorV1> {
    if reachable(body, from, to) {
        Ok(())
    } else {
        Err(GeneralGemmMirImportErrorV1::new(format!(
            "general GEMM MIR has no CFG path from {from_name} to {to_name}"
        )))
    }
}

fn reachable(body: &Body<'_>, from: BasicBlock, to: BasicBlock) -> bool {
    let mut pending = VecDeque::from([from]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        if block == to {
            return true;
        }
        if !visited.insert(block) {
            continue;
        }
        let Ok(successors) = normal_successors(body, block) else {
            return false;
        };
        pending.extend(successors);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abi_bindings() -> [GeneralGemmAbiOperandBindingV1; 11] {
        use GeneralGemmAbiRoleV1 as Role;
        use GeneralGemmAbiTypeV1 as Type;
        [
            abi(Role::A, 0, Type::SharedU16Slice),
            abi(Role::B, 1, Type::SharedU16Slice),
            abi(Role::C, 2, Type::DisjointF32Slice),
            abi(Role::M, 3, Type::U32),
            abi(Role::N, 4, Type::U32),
            abi(Role::K, 5, Type::U32),
            abi(Role::Lda, 6, Type::U32),
            abi(Role::Ldb, 7, Type::U32),
            abi(Role::Ldc, 8, Type::U32),
            abi(Role::Alpha, 9, Type::F32),
            abi(Role::Beta, 10, Type::F32),
        ]
    }

    fn receipt() -> AuthenticatedGeneralGemmSemanticReceiptV1 {
        use GeneralGemmSourcePropertyKindV1 as Kind;
        let kinds = [
            Kind::AllocationAndProvenance,
            Kind::GuardedGlobalAccesses,
            Kind::LdsWriteReadInitialization,
            Kind::EffectConflictFreedom,
            Kind::ControlFlowBarrierConvergence,
            Kind::OutputOwnership,
            Kind::LdsLifecycle,
            Kind::AccumulatorPhase,
            Kind::MaskedTail,
            Kind::AlphaBetaEpilogue,
            Kind::NumericalOperationOrder,
        ];
        let semantics = GeneralGemmIntrinsicSemanticsV1::canonical();
        let mir_closure = [0x44; 32];
        let provider_profile = [0x33; 32];
        AuthenticatedGeneralGemmSemanticReceiptV1 {
            consumed: Some(ConsumedGeneralGemmSemanticTemplateV1 {
                kernel_instance: [0x11; 32],
                compiled_source: [0x22; 32],
                provider_semantics: [0x33; 32],
                abi: abi_bindings(),
                source_properties: kinds.map(|kind| {
                    source_property(
                        &semantics,
                        kind,
                        &mir_closure,
                        &provider_profile,
                        test_typestate_evidence(kind),
                    )
                }),
                symbolic_plan: GeneralGemmSymbolicPlanV1::canonical(),
                symbolic_kir: GeneralGemmSymbolicKirV1::canonical(),
            }),
        }
    }

    fn test_typestate_evidence(
        kind: GeneralGemmSourcePropertyKindV1,
    ) -> GeneralGemmSourceMirEvidenceV1 {
        use GeneralGemmSourceMirEvidenceV1 as Evidence;
        use GeneralGemmSourcePropertyKindV1 as Kind;
        let event = |operation, block| GeneralGemmMirEventTranscriptV1 {
            operation,
            block,
            return_block: block + 1,
            result_local: None,
        };
        let acquire = event(TrustedGeneralGemmOperationV1::Acquire, 1);
        let stage_event = event(TrustedGeneralGemmOperationV1::Stage, 2);
        let publish = event(TrustedGeneralGemmOperationV1::Publish, 3);
        let mfma = event(TrustedGeneralGemmOperationV1::Mfma, 4);
        let reuse = event(TrustedGeneralGemmOperationV1::Reuse, 5);
        let store_event = event(TrustedGeneralGemmOperationV1::Store, 6);
        let path = |from_block, required_event| GeneralGemmAllPathsTranscriptV1 {
            from_block,
            required_event,
            boundary_block: required_event.block,
            visited_region_identity: [required_event.operation as u8 + 1; 32],
        };
        let phase = GeneralGemmPhaseCycleTranscriptV1 {
            acquire,
            stage: stage_event,
            publish,
            mfma,
            reuse,
            store: store_event,
            stage_to_publish: path(3, publish),
            publish_to_mfma: path(4, mfma),
            mfma_to_reuse: path(5, reuse),
            phase_split_block: 7,
            phase_cfg_identity: [0x66; 32],
        };
        let stage_inputs = GeneralGemmStageInputTranscriptV1 {
            stage: stage_event,
            guarded_loader: GeneralGemmGuardedLoaderTranscriptV1 {
                helper_def_path: [0x11; 16],
                compiled_source_identity: [0x12; 32],
                row_guard_block: 1,
                column_guard_block: 2,
                zero_return_block: 3,
                row_major_block: 4,
                extent_guard_block: 4,
                load_block: 5,
                trap_block: 6,
                dataflow_identity: [0x13; 32],
            },
            coordinate_dataflow_identity: [0x14; 32],
        };
        let store = GeneralGemmStoreTranscriptV1 {
            store: store_event,
            abi_identity: general_gemm_abi_identity(&abi_bindings()),
        };
        match kind {
            Kind::AllocationAndProvenance => Evidence::AllocationAndProvenance {
                abi_identity: general_gemm_abi_identity(&abi_bindings()),
                root_compiled_source: [0x22; 32],
                stage_inputs,
                store,
            },
            Kind::GuardedGlobalAccesses => Evidence::GuardedGlobalAccesses {
                stage_inputs,
                store,
            },
            Kind::LdsWriteReadInitialization => Evidence::LdsWriteReadInitialization { phase },
            Kind::EffectConflictFreedom => Evidence::EffectConflictFreedom {
                phase,
                stage_inputs,
                store,
            },
            Kind::ControlFlowBarrierConvergence => Evidence::ControlFlowBarrierConvergence {
                stage_to_publish: phase.stage_to_publish,
                mfma_to_reuse: phase.mfma_to_reuse,
            },
            Kind::OutputOwnership => Evidence::OutputOwnership { phase, store },
            Kind::LdsLifecycle => Evidence::LdsLifecycle { phase },
            Kind::AccumulatorPhase => Evidence::AccumulatorPhase { phase },
            Kind::MaskedTail => Evidence::MaskedTail {
                stage_inputs,
                store,
            },
            Kind::AlphaBetaEpilogue => Evidence::AlphaBetaEpilogue { store },
            Kind::NumericalOperationOrder => Evidence::NumericalOperationOrder {
                stage_inputs,
                phase,
                store,
            },
        }
    }

    #[test]
    fn receipt_consumption_retains_exact_abi_and_opaque_correspondence() {
        let expected_abi = receipt()
            .consumed
            .as_ref()
            .expect("test receipt")
            .abi_identity();
        let correspondence = receipt().into_verified_template().unwrap();
        assert_eq!(
            correspondence.binding().frontend_abi_identity(),
            &expected_abi
        );
        assert_ne!(correspondence.identity().as_bytes(), &[0; 32]);
        assert_ne!(
            correspondence.identity().as_bytes(),
            correspondence.binding().identity().as_bytes()
        );
        assert_eq!(correspondence.source_properties().len(), 11);
        assert!(
            correspondence
                .source_properties()
                .iter()
                .all(|property| property.evidence_identity() != &[0; 32] && property.revalidate())
        );
    }

    #[test]
    fn receipt_consumption_rejects_tampered_property_identity() {
        let mut tampered = receipt();
        tampered
            .consumed
            .as_mut()
            .expect("test receipt")
            .source_properties[4]
            .evidence_identity[0] ^= 1;
        assert!(matches!(
            tampered.into_verified_template(),
            Err(GeneralGemmReceiptConsumptionErrorV1::SourcePropertyRevalidation)
        ));
    }

    #[test]
    fn correspondence_revalidation_binds_aggregate_and_shared_transcript() {
        let mut correspondence = receipt().into_verified_template().unwrap();
        assert!(correspondence.revalidate());
        correspondence.identity.0[0] ^= 1;
        assert!(!correspondence.revalidate());

        let mut correspondence = receipt().into_verified_template().unwrap();
        correspondence.source_properties[7].optimized_mir_closure[0] ^= 1;
        assert!(!correspondence.revalidate());
    }

    #[test]
    fn abi_identity_changes_with_role_position_or_type() {
        let exact = receipt()
            .consumed
            .as_ref()
            .expect("test receipt")
            .abi_identity();
        let mut changed_position = receipt();
        changed_position
            .consumed
            .as_mut()
            .expect("test receipt")
            .abi[0]
            .argument_index = 1;
        assert_ne!(
            changed_position
                .consumed
                .as_ref()
                .expect("test receipt")
                .abi_identity(),
            exact
        );
        let mut changed_type = receipt();
        changed_type.consumed.as_mut().expect("test receipt").abi[0].ty = GeneralGemmAbiTypeV1::F32;
        assert_ne!(
            changed_type
                .consumed
                .as_ref()
                .expect("test receipt")
                .abi_identity(),
            exact
        );
    }

    #[test]
    fn call_budget_accepts_exact_limits_and_rejects_the_next_event() {
        let mut budget = GeneralGemmCallBudgetV1::default();
        for _ in 0..MAX_GENERAL_GEMM_REACHABLE_CALLS_V1 {
            budget.observe_reachable_call().unwrap();
        }
        let error = budget.observe_reachable_call().unwrap_err();
        assert!(error.to_string().contains("512-call analysis limit"));

        let mut budget = GeneralGemmCallBudgetV1::default();
        for _ in 0..MAX_GENERAL_GEMM_TERMINAL_CALLS_V1 {
            budget.observe_general_gemm_terminal().unwrap();
        }
        let error = budget.observe_general_gemm_terminal().unwrap_err();
        assert!(error.to_string().contains("32-terminal analysis limit"));
    }
}
