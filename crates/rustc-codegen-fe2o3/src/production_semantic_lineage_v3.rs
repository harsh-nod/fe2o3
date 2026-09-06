//! Private join from live production compiler owners to the inert V3 capsule.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerModuleHandoffV2,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    FinalCompilerModuleCommitmentErrorV3, InertFinalCompilerModuleCommitmentV3,
    InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_compiler_lineage::{
    DataLayoutTranscriptInputsV3, DataLayoutTranscriptV3, InertAbiReceiptV3,
    InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirReceiptV3, InertDataLayoutReceiptV3,
    InertExportManifestReceiptV3, InertFinalCompilerModuleCommitmentReceiptV3,
    InertFormalMemoryReceiptV3, InertKernelIrReceiptV3, InertLineageContentIdentityV3,
    InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProductionSemanticCapsuleV3, InertProofBindingAssociationErrorV3,
    InertProofBindingAssociationErrorV4, InertProofBindingAssociationInputsV4,
    InertProofBindingAssociationV4, InertProofBindingReceiptV3,
    InertRustcIdentityInventoryReceiptV3, InertRustcPreflightPlanReceiptV3,
    InertSemanticToLlvmAssociationV3, InertSemanticToLlvmReceiptV3, InertTargetBindingReceiptV3,
    LineageErrorV3, MultiRootCanonicalKirVersionV2, MultiRootCorrespondencePayloadV2,
    MultiRootNeutralKirIdentityV2, MultiRootProofRosterErrorV2, MultiRootProofRosterInputsV2,
    MultiRootProofRosterKindV2, MultiRootProofRosterRootInputV2, MultiRootProofRosterTranscriptV2,
    MultiRootTargetBindingInputsV2, MultiRootTargetBindingTranscriptV2,
    MultiRootTargetWorkgroupInputV2, OrderedInertSemanticLineageReceiptsV3,
    ProductionTargetLineageErrorV3, SemanticToLlvmAssociationInputsV3,
    SemanticToLlvmAssociationTranscriptV3, TargetBindingTranscriptInputsV3,
    TargetBindingTranscriptV3, TargetLineageIdentityV3, derive_semantic_target_layout_identity_v1,
};
use fe2o3_kernel_ir::{
    FunctionRole, InertCanonicalFormalMemoryObligationReceiptV1, Module,
    ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugCarrierV1,
    ProductionSemanticDebugProducerGapV1, ProductionSemanticDebugReceiptExtensionV1,
    VerifiedCanonicalKernelIrErrorV8, VerifiedCanonicalKernelIrErrorV9,
    VerifiedCanonicalKernelIrErrorV11, VerifiedCanonicalKernelIrV8, VerifiedCanonicalKernelIrV9,
    VerifiedCanonicalKernelIrV11,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV4, InertCanonicalMirToKirCorrespondenceEvidenceV5,
    ProductionCanonicalKernelIrIdentityV1, ProductionCanonicalKernelIrVersionV1,
    ProductionCorrespondenceEvidenceErrorV4, ProductionCorrespondenceEvidenceErrorV5,
    ProductionFormalMemoryEvidenceErrorV4, ProductionFormalMemoryOwnerV1,
};
use fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1;
use fe2o3_pliron::InertProductionMiddleEndEvidenceV5;
use fe2o3_rustc_invocation::{
    InvocationDigestV3, RustcInvocationDescriptorV3, encode_descriptor_v3,
};
use fe2o3_verifier::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1, CompilerKirToLlvmReplayValidationErrorV1,
    CompilerMultiRootProofValidationErrorV1, CompilerTargetLineageValidationErrorV1,
    ProductionMirPlironVerusExecutionEvidenceErrorV1, validate_compiler_multi_root_proof_inputs_v1,
    validate_compiler_multi_root_target_lineage_v1,
};
use sha2::{Digest, Sha256};

use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1;
use crate::production_target_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1;
use crate::protected_rustc_invocation::{
    FinishedProtectedRustcInvocationV3, ProtectedRustcInvocationErrorV1,
};

const CODE_OBJECT_VERSION_V3: u16 = 6;
const WAVE_WIDTH_BITS_V3: u16 = 64;

fn validate_final_llvm_layout(llvm: &str) -> Result<(), ProductionSemanticLineageErrorV3> {
    let expected_header = format!(
        "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{PRODUCTION_WORKER_DATA_LAYOUT_V1}\"\n"
    );
    if !llvm.starts_with(&expected_header)
        || llvm.matches("target triple =").count() != 1
        || llvm.matches("target datalayout =").count() != 1
    {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "final LLVM does not retain the exact measured worker target layout",
        ));
    }
    Ok(())
}

/// Move-only canonical evidence prepared while the live semantic and formal
/// owners still exist. It grants no publication, load, or launch authority.
pub(crate) struct PreparedProductionSemanticLineageV3 {
    rustc_identity_inventory: InertRustcIdentityInventoryReceiptV3,
    rustc_preflight_plan: InertRustcPreflightPlanReceiptV3,
    semantic_mir: InertCanonicalSemanticMirReceiptV3,
    middle_end: InertMiddleEndReceiptV3,
    kernel_ir: InertKernelIrReceiptV3,
    mir_to_kir_correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
    proof_verus_evidence: Box<[u8]>,
    roster_custody: PreparedLineageRosterCustodyV1,
    amdgpu_lowering_replay: dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1,
    semantic_debug: crate::production_semantic_debug_v1::PreparedProductionSemanticDebugV1,
    neutral_kir_custody: ProductionCanonicalKernelIrIdentityV1,
    neutral_kir_identity: TargetLineageIdentityV3,
    bound_kir_identity: TargetLineageIdentityV3,
    semantic_layout_identity: TargetLineageIdentityV3,
    expected_exports: BTreeSet<(CompilerModuleSymbolRoleV1, String)>,
    rustc_layout: crate::semantic_layout_bridge::SemanticLayoutTargetV1,
    workgroups: Box<[(String, [u32; 3])]>,
}

enum PreparedLineageRosterCustodyV1 {
    Singleton,
    MultiRoot {
        roster_identity: [u8; 32],
        middle_end_sha256: [u8; 32],
        correspondence_sha256: [u8; 32],
        formal_memory_sha256: [u8; 32],
        verus_sha256: [u8; 32],
    },
}

struct PreparedLineageRootV1 {
    logical_name: String,
    export_symbol: Box<[u8]>,
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    source_rank: u8,
    kernel_id: String,
    workgroup: [u32; 3],
    middle_end: Box<[u8]>,
    correspondence: Box<[u8]>,
    formal_memory: Box<[u8]>,
    verus_execution: Box<[u8]>,
}

struct PreparedLineageEvidenceV1 {
    middle_end: InertMiddleEndReceiptV3,
    mir_to_kir_correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
    proof_verus_evidence: Box<[u8]>,
    roster_custody: PreparedLineageRosterCustodyV1,
    workgroups: Box<[(String, [u32; 3])]>,
}

#[derive(Clone, Copy)]
enum LineageRosterPayloadV1 {
    MiddleEnd,
    Correspondence,
    FormalMemory,
    VerusExecution,
}

impl LineageRosterPayloadV1 {
    const fn shared_kind(self) -> MultiRootProofRosterKindV2 {
        match self {
            Self::MiddleEnd => MultiRootProofRosterKindV2::MiddleEnd,
            Self::Correspondence => MultiRootProofRosterKindV2::Correspondence,
            Self::FormalMemory => MultiRootProofRosterKindV2::FormalMemory,
            Self::VerusExecution => MultiRootProofRosterKindV2::VerusExecution,
        }
    }
}

#[derive(Clone, Copy)]
struct LineageNeutralKirIdentityV1 {
    version: ProductionCanonicalKernelIrVersionV1,
    canonical_length: u64,
    digest: [u8; 32],
}

impl From<ProductionCanonicalKernelIrIdentityV1> for LineageNeutralKirIdentityV1 {
    fn from(identity: ProductionCanonicalKernelIrIdentityV1) -> Self {
        Self {
            version: identity.version(),
            canonical_length: identity.canonical_length(),
            digest: *identity.digest(),
        }
    }
}

impl LineageNeutralKirIdentityV1 {
    fn shared(self) -> Result<MultiRootNeutralKirIdentityV2, MultiRootProofRosterErrorV2> {
        MultiRootNeutralKirIdentityV2::new(
            match self.version {
                ProductionCanonicalKernelIrVersionV1::V8 => MultiRootCanonicalKirVersionV2::V8,
                ProductionCanonicalKernelIrVersionV1::V9 => MultiRootCanonicalKirVersionV2::V9,
                ProductionCanonicalKernelIrVersionV1::V11 => MultiRootCanonicalKirVersionV2::V11,
            },
            self.canonical_length,
            self.digest,
        )
    }
}

fn prepare_lineage_evidence_v1(
    ranked: AuthenticatedRankedVerificationRosterV1,
    admitted: &ProductionFormalMemoryOwnerV1,
    target_module: &Module,
    neutral_kir: ProductionCanonicalKernelIrIdentityV1,
) -> Result<PreparedLineageEvidenceV1, ProductionSemanticLineageErrorV3> {
    let semantic = admitted.semantic_kir().semantic().semantic();
    if ranked.root_count() == 0
        || ranked.root_count() != semantic.roots().len()
        || ranked.root_count() != target_module.kernels.len()
        || ranked.root_count() != admitted.kernels().len()
        || !ranked.every_functional_verification_is_coherent()
    {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "ranked, semantic, KIR, and formal lineage rosters differ",
        ));
    }

    let roster_identity = *ranked.canonical_roster_identity().as_bytes();
    let canonical_kernel_order = ranked.canonical_kernel_order().to_vec().into_boxed_slice();
    let mut roots = Vec::with_capacity(ranked.root_count());
    for ((((ranked_root, semantic_root), kernel), formal), ordinal) in ranked
        .roots()
        .iter()
        .zip(semantic.roots())
        .zip(&target_module.kernels)
        .zip(admitted.kernels())
        .zip(0_u32..)
    {
        let function = semantic
            .functions()
            .get(semantic_root.index() as usize)
            .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                "lineage semantic root is out of range",
            ))?;
        let entry =
            function
                .kernel_entry()
                .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                    "lineage semantic root is not an exact kernel export",
                ))?;
        let verification = ranked_root.verification();
        let induction = verification.semantic_u32_induction();
        let selected = semantic
            .select_kernel_body_for_root_v1(*semantic_root)
            .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                "lineage root has no exact selected semantic body",
            ))?;
        let workgroup =
            kernel
                .workgroup_size
                .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                    "target-bound KIR root has no exact workgroup size",
                ))?;
        if ranked_root.semantic_root() != *semantic_root
            || ranked_root.semantic_root_identity() != function.identity()
            || ranked_root.export_symbol() != entry.export_symbol().as_bytes()
            || ranked_root.kernel_binding() != entry.kernel_binding_identity().as_bytes()
            || ranked_root.source_rank() != kernel.domain.rank()
            || induction.semantic_mir_sha256() != semantic.semantic_sha256()
            || induction.function() != selected.body()
            || induction.function_identity()
                != semantic.functions()[selected.body().index() as usize].identity()
            || induction.grants_authority()
            || induction.authorizes_compiler_transform()
            || kernel.id.as_str() != std::str::from_utf8(ranked_root.export_symbol()).unwrap_or("")
            || kernel.entry.as_str() != kernel.id.as_str()
            || formal.obligations().kernel() != &kernel.id
            || formal.obligations().entry() != &kernel.entry
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "cross-wired per-root ranked, semantic, KIR, or formal lineage",
            ));
        }

        let verus = verification.aggregate_verus_execution().ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "every production root requires authenticated MIR-to-PLIRON Verus execution",
            ),
        )?;
        let verus = CanonicalProductionMirPlironVerusExecutionEvidenceV1::from_execution(verus)?;
        if verus.claims().pliron_evidence_identity().as_bytes()
            != verification.middle_end_evidence().identity().sha256()
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "per-root Verus execution names a different middle-end record",
            ));
        }
        let induction =
            fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1::from_report(induction)
                .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;
        let formal_receipt =
            InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(formal.obligations())
                .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;
        let correspondence = encode_correspondence_root_payload_v1(
            admitted.semantic_kir().correspondence(),
            *semantic_root,
            ordinal,
            induction.canonical_bytes(),
        )?;
        roots.push(PreparedLineageRootV1 {
            logical_name: ranked_root.logical_name().to_owned(),
            export_symbol: ranked_root.export_symbol().to_vec().into_boxed_slice(),
            semantic_root: semantic_root.index(),
            semantic_root_identity: *function.identity().as_bytes(),
            kernel_binding: *ranked_root.kernel_binding(),
            source_rank: ranked_root.source_rank(),
            kernel_id: kernel.id.as_str().to_owned(),
            workgroup: [workgroup.x, workgroup.y, workgroup.z],
            middle_end: verification
                .middle_end_evidence()
                .canonical_bytes()
                .to_vec()
                .into_boxed_slice(),
            correspondence: correspondence.into_boxed_slice(),
            formal_memory: formal_receipt.canonical_bytes().to_vec().into_boxed_slice(),
            verus_execution: verus.canonical_bytes().to_vec().into_boxed_slice(),
        });
    }

    let workgroups = roots
        .iter()
        .map(|root| (root.kernel_id.clone(), root.workgroup))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    if let [root] = roots.as_slice() {
        let verification = ranked
            .roots()
            .iter()
            .find(|ranked_root| ranked_root.semantic_root().index() == root.semantic_root)
            .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                "singleton lineage has no matching ranked root",
            ))?
            .verification();
        let correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV5::from_live_owner(
            admitted.semantic_kir(),
            verification.semantic_u32_induction(),
        )?;
        let formal = InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(admitted)?;
        if correspondence.nested_v4().canonical_kernel_ir_identity() != neutral_kir
            || formal.canonical_kernel_ir_identity() != neutral_kir
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "singleton lineage names a different neutral KIR",
            ));
        }
        return Ok(PreparedLineageEvidenceV1 {
            middle_end: InertMiddleEndReceiptV3::from_canonical_preimage(root.middle_end.to_vec())?,
            mir_to_kir_correspondence:
                InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
                    correspondence.canonical_bytes(),
                )?,
            formal_memory: InertFormalMemoryReceiptV3::from_canonical_preimage(
                formal.canonical_bytes(),
            )?,
            proof_verus_evidence: root.verus_execution.clone(),
            roster_custody: PreparedLineageRosterCustodyV1::Singleton,
            workgroups,
        });
    }

    let middle_end = build_lineage_roster_v2(
        semantic.semantic_sha256().as_bytes(),
        neutral_kir.into(),
        roster_identity,
        &canonical_kernel_order,
        &roots,
        LineageRosterPayloadV1::MiddleEnd,
    )?;
    let correspondence = build_lineage_roster_v2(
        semantic.semantic_sha256().as_bytes(),
        neutral_kir.into(),
        roster_identity,
        &canonical_kernel_order,
        &roots,
        LineageRosterPayloadV1::Correspondence,
    )?;
    let formal_memory = build_lineage_roster_v2(
        semantic.semantic_sha256().as_bytes(),
        neutral_kir.into(),
        roster_identity,
        &canonical_kernel_order,
        &roots,
        LineageRosterPayloadV1::FormalMemory,
    )?;
    let verus = build_lineage_roster_v2(
        semantic.semantic_sha256().as_bytes(),
        neutral_kir.into(),
        roster_identity,
        &canonical_kernel_order,
        &roots,
        LineageRosterPayloadV1::VerusExecution,
    )?;
    let middle_end_sha256 = Sha256::digest(&middle_end).into();
    let correspondence_sha256 = Sha256::digest(&correspondence).into();
    let formal_memory_sha256 = Sha256::digest(&formal_memory).into();
    let verus_sha256 = Sha256::digest(&verus).into();
    Ok(PreparedLineageEvidenceV1 {
        middle_end: InertMiddleEndReceiptV3::from_canonical_preimage(middle_end)?,
        mir_to_kir_correspondence: InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
            correspondence,
        )?,
        formal_memory: InertFormalMemoryReceiptV3::from_canonical_preimage(formal_memory)?,
        proof_verus_evidence: verus.into_boxed_slice(),
        roster_custody: PreparedLineageRosterCustodyV1::MultiRoot {
            roster_identity,
            middle_end_sha256,
            correspondence_sha256,
            formal_memory_sha256,
            verus_sha256,
        },
        workgroups,
    })
}

fn encode_correspondence_root_payload_v1(
    correspondence: &fe2o3_lower_mir_kernel::SemanticKirCorrespondenceV1,
    owner: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    ordinal: u32,
    induction: &[u8],
) -> Result<Vec<u8>, ProductionSemanticLineageErrorV3> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"F2MRCOP2");
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&ordinal.to_le_bytes());
    bytes.extend_from_slice(&owner.index().to_le_bytes());
    push_lineage_bytes_v1(&mut bytes, induction)?;

    let functions = correspondence
        .lowered_functions()
        .iter()
        .filter(|record| record.correspondence_owner() == owner)
        .collect::<Vec<_>>();
    let has_functions = !functions.is_empty();
    push_lineage_count_v1(&mut bytes, functions.len())?;
    for record in functions {
        bytes.extend_from_slice(&record.semantic_function().index().to_le_bytes());
        bytes.push(match record.role() {
            fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::KernelEntry => 1,
            fe2o3_lower_mir_kernel::SemanticKirFunctionRoleV1::InternalHelper => 2,
        });
        push_lineage_bytes_v1(&mut bytes, record.kernel_ir_function().as_str().as_bytes())?;
    }

    let blocks = correspondence
        .blocks()
        .iter()
        .copied()
        .filter(|record| record.correspondence_owner() == owner)
        .collect::<Vec<_>>();
    let has_blocks = !blocks.is_empty();
    push_lineage_count_v1(&mut bytes, blocks.len())?;
    for record in blocks {
        bytes.extend_from_slice(&record.semantic_function().index().to_le_bytes());
        bytes.extend_from_slice(&record.semantic_block().index().to_le_bytes());
        bytes.extend_from_slice(&record.kernel_ir_block().0.to_le_bytes());
        bytes.extend_from_slice(&record.source_statement_count().to_le_bytes());
    }

    let statements = correspondence
        .statement_operation_spans()
        .iter()
        .copied()
        .filter(|record| record.correspondence_owner() == owner)
        .collect::<Vec<_>>();
    push_lineage_count_v1(&mut bytes, statements.len())?;
    for record in statements {
        bytes.extend_from_slice(&record.semantic_function().index().to_le_bytes());
        bytes.extend_from_slice(&record.semantic_block().index().to_le_bytes());
        bytes.extend_from_slice(&record.statement_ordinal().to_le_bytes());
        bytes.extend_from_slice(&record.kernel_ir_block().0.to_le_bytes());
        bytes.extend_from_slice(&record.first_operation_ordinal().to_le_bytes());
        bytes.extend_from_slice(&record.operation_count().to_le_bytes());
    }

    let terminators = correspondence
        .terminator_operation_spans()
        .iter()
        .copied()
        .filter(|record| record.correspondence_owner() == owner)
        .collect::<Vec<_>>();
    push_lineage_count_v1(&mut bytes, terminators.len())?;
    for record in terminators {
        bytes.extend_from_slice(&record.semantic_function().index().to_le_bytes());
        bytes.extend_from_slice(&record.semantic_block().index().to_le_bytes());
        bytes.extend_from_slice(&record.kernel_ir_block().0.to_le_bytes());
        bytes.extend_from_slice(&record.first_operation_ordinal().to_le_bytes());
        bytes.extend_from_slice(&record.operation_count().to_le_bytes());
    }

    let synthetics = correspondence
        .synthetic_operation_spans()
        .iter()
        .copied()
        .filter(|record| record.correspondence_owner() == owner)
        .collect::<Vec<_>>();
    push_lineage_count_v1(&mut bytes, synthetics.len())?;
    for record in synthetics {
        bytes.extend_from_slice(&record.semantic_function().index().to_le_bytes());
        bytes.push(match record.rule() {
            fe2o3_lower_mir_kernel::SemanticKirSyntheticOperationRuleV1::RetainedLocalStorage => 3,
            fe2o3_lower_mir_kernel::SemanticKirSyntheticOperationRuleV1::EnumPayloadStorage => 1,
            fe2o3_lower_mir_kernel::SemanticKirSyntheticOperationRuleV1::RuntimeAssertFailureTrap => 2,
        });
        bytes.extend_from_slice(&record.kernel_ir_block().0.to_le_bytes());
        bytes.extend_from_slice(&record.first_operation_ordinal().to_le_bytes());
        bytes.extend_from_slice(&record.operation_count().to_le_bytes());
    }

    let parameters = correspondence
        .parameter_bindings()
        .iter()
        .copied()
        .filter(|record| record.correspondence_owner() == owner)
        .collect::<Vec<_>>();
    push_lineage_count_v1(&mut bytes, parameters.len())?;
    for record in parameters {
        bytes.extend_from_slice(&record.semantic_function().index().to_le_bytes());
        bytes.extend_from_slice(&record.semantic_local().index().to_le_bytes());
        bytes.extend_from_slice(&record.kernel_ir_value().0.to_le_bytes());
    }
    if !has_blocks || !has_functions {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "a lineage root has no exact correspondence records",
        ));
    }
    let decoded = MultiRootCorrespondencePayloadV2::decode(&bytes).map_err(|_| {
        ProductionSemanticLineageErrorV3::AxisMismatch(
            "producer emitted a non-canonical multi-root correspondence payload",
        )
    })?;
    if decoded.root_ordinal() != ordinal || decoded.correspondence_owner() != owner.index() {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "producer cross-wired its multi-root correspondence payload header",
        ));
    }
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn build_lineage_roster_v2(
    semantic_sha256: &[u8; 32],
    neutral_kir: LineageNeutralKirIdentityV1,
    roster_identity: [u8; 32],
    canonical_kernel_order: &[usize],
    roots: &[PreparedLineageRootV1],
    payload_kind: LineageRosterPayloadV1,
) -> Result<Vec<u8>, ProductionSemanticLineageErrorV3> {
    let canonical_kernel_order = canonical_kernel_order
        .iter()
        .copied()
        .map(|index| {
            u32::try_from(index).map_err(|_| {
                ProductionSemanticLineageErrorV3::AxisMismatch("lineage index overflow")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let roots = roots
        .iter()
        .map(|root| {
            let export_symbol = std::str::from_utf8(&root.export_symbol).map_err(|_| {
                ProductionSemanticLineageErrorV3::AxisMismatch("lineage export symbol is not UTF-8")
            })?;
            let payload = match payload_kind {
                LineageRosterPayloadV1::MiddleEnd => root.middle_end.as_ref(),
                LineageRosterPayloadV1::Correspondence => root.correspondence.as_ref(),
                LineageRosterPayloadV1::FormalMemory => root.formal_memory.as_ref(),
                LineageRosterPayloadV1::VerusExecution => root.verus_execution.as_ref(),
            };
            Ok(MultiRootProofRosterRootInputV2 {
                semantic_root: root.semantic_root,
                semantic_root_identity: root.semantic_root_identity,
                kernel_binding: root.kernel_binding,
                source_rank: root.source_rank,
                workgroup: root.workgroup,
                logical_name: &root.logical_name,
                export_symbol,
                kernel_id: &root.kernel_id,
                payload,
            })
        })
        .collect::<Result<Vec<_>, ProductionSemanticLineageErrorV3>>()?;
    MultiRootProofRosterTranscriptV2::new(MultiRootProofRosterInputsV2 {
        kind: payload_kind.shared_kind(),
        semantic_mir_sha256: *semantic_sha256,
        neutral_kir: neutral_kir.shared()?,
        roster_identity,
        canonical_kernel_order: &canonical_kernel_order,
        roots: &roots,
    })
    .map(MultiRootProofRosterTranscriptV2::into_canonical_bytes)
    .map_err(Into::into)
}

fn push_lineage_count_v1(
    bytes: &mut Vec<u8>,
    count: usize,
) -> Result<(), ProductionSemanticLineageErrorV3> {
    bytes.extend_from_slice(
        &u32::try_from(count)
            .map_err(|_| ProductionSemanticLineageErrorV3::AxisMismatch("lineage count overflow"))?
            .to_le_bytes(),
    );
    Ok(())
}

fn push_lineage_bytes_v1(
    bytes: &mut Vec<u8>,
    value: &[u8],
) -> Result<(), ProductionSemanticLineageErrorV3> {
    if value.is_empty() {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "empty lineage roster field",
        ));
    }
    push_lineage_count_v1(bytes, value.len())?;
    bytes.extend_from_slice(value);
    Ok(())
}

const MAX_LINEAGE_ROOTS_V1: usize = 4_096;

struct LineageRosterReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> LineageRosterReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionSemanticLineageErrorV3> {
        let end = self.offset.checked_add(length).ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch("lineage envelope length overflow"),
        )?;
        let value = self.bytes.get(self.offset..end).ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch("truncated lineage envelope"),
        )?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ProductionSemanticLineageErrorV3> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionSemanticLineageErrorV3::AxisMismatch("truncated lineage field"))
    }

    fn u8(&mut self) -> Result<u8, ProductionSemanticLineageErrorV3> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProductionSemanticLineageErrorV3> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionSemanticLineageErrorV3> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn count(&mut self) -> Result<usize, ProductionSemanticLineageErrorV3> {
        usize::try_from(self.u32()?).map_err(|_| {
            ProductionSemanticLineageErrorV3::AxisMismatch("lineage count does not fit usize")
        })
    }

    fn bytes(&mut self) -> Result<&'a [u8], ProductionSemanticLineageErrorV3> {
        let length = self.count()?;
        if length == 0 || length > self.bytes.len() {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "invalid lineage field length",
            ));
        }
        self.take(length)
    }

    const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn validate_correspondence_root_payload_v1(
    bytes: &[u8],
    ordinal: u32,
    semantic_root: u32,
    semantic_sha256: &[u8; 32],
    kernel_id: &str,
) -> Result<(), ProductionSemanticLineageErrorV3> {
    let mut reader = LineageRosterReaderV1::new(bytes);
    if reader.fixed::<8>()? != *b"F2MRCOP2"
        || reader.u16()? != 2
        || reader.u16()? != 1
        || reader.u32()? != ordinal
        || reader.u32()? != semantic_root
    {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "cross-wired correspondence root payload",
        ));
    }
    let induction = InertCanonicalSemanticU32InductionEvidenceV1::decode(reader.bytes()?)
        .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;
    if induction.semantic_mir_sha256() != semantic_sha256 || induction.grants_authority() {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "correspondence induction payload changed semantic owner",
        ));
    }

    let function_count = reader.count()?;
    if function_count == 0 || function_count > MAX_LINEAGE_ROOTS_V1 {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "invalid correspondence function roster",
        ));
    }
    let mut functions = BTreeSet::new();
    let mut entry_count = 0_usize;
    for _ in 0..function_count {
        let semantic_function = reader.u32()?;
        let role = reader.u8()?;
        if !matches!(role, 1 | 2) {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "invalid correspondence function role",
            ));
        }
        let symbol = std::str::from_utf8(reader.bytes()?).map_err(|_| {
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "correspondence function symbol is not UTF-8",
            )
        })?;
        if !functions.insert((semantic_function, role, symbol)) {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "duplicate correspondence function record",
            ));
        }
        if role == 1 {
            entry_count += 1;
            if symbol != kernel_id {
                return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                    "correspondence entry names a different kernel",
                ));
            }
        }
    }
    if entry_count != 1 {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "correspondence payload does not contain one exact entry",
        ));
    }

    let fixed_records = [16_usize, 24, 20];
    for (index, record_bytes) in fixed_records.into_iter().enumerate() {
        let count = reader.count()?;
        if index == 0 && count == 0 {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "correspondence payload has no block records",
            ));
        }
        reader.take(count.checked_mul(record_bytes).ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch("correspondence record count overflow"),
        )?)?;
    }
    let synthetic_count = reader.count()?;
    for _ in 0..synthetic_count {
        reader.u32()?;
        if !matches!(reader.u8()?, 1 | 2) {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "invalid synthetic correspondence rule",
            ));
        }
        reader.take(12)?;
    }
    let parameter_count = reader.count()?;
    reader.take(parameter_count.checked_mul(12).ok_or(
        ProductionSemanticLineageErrorV3::AxisMismatch("correspondence parameter count overflow"),
    )?)?;
    if !reader.is_finished() {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "trailing correspondence root payload bytes",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_lineage_roster_envelope_v1(
    bytes: &[u8],
    expected_sha256: [u8; 32],
    semantic_sha256: &[u8; 32],
    neutral_kir: LineageNeutralKirIdentityV1,
    roster_identity: [u8; 32],
    expected_workgroups: &[(String, [u32; 3])],
    payload_kind: LineageRosterPayloadV1,
) -> Result<(), ProductionSemanticLineageErrorV3> {
    if <[u8; 32]>::from(Sha256::digest(bytes)) != expected_sha256 {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "multi-root lineage envelope content identity changed",
        ));
    }
    let roster = MultiRootProofRosterTranscriptV2::decode(bytes)?;
    if roster.kind() != payload_kind.shared_kind()
        || roster.semantic_mir_sha256() != *semantic_sha256
        || roster.neutral_kir() != neutral_kir.shared()?
        || roster.roster_identity() != roster_identity
    {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "multi-root lineage envelope header changed before final handoff",
        ));
    }
    if roster.root_count() != expected_workgroups.len() {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "lineage root and expected workgroup counts differ",
        ));
    }
    for (ordinal, expected_workgroup) in (0_u32..).zip(expected_workgroups) {
        let root =
            roster
                .root(ordinal as usize)
                .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                    "lineage root disappeared",
                ))?;
        if root.kernel_id() != expected_workgroup.0 || root.workgroup() != expected_workgroup.1 {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "reordered or substituted lineage workgroup root",
            ));
        }
        match payload_kind {
            LineageRosterPayloadV1::MiddleEnd => {
                InertProductionMiddleEndEvidenceV5::decode(root.payload()).map_err(|error| {
                    ProductionSemanticLineageErrorV3::LiveOwner(error.to_string())
                })?;
            }
            LineageRosterPayloadV1::Correspondence => {
                validate_correspondence_root_payload_v1(
                    root.payload(),
                    ordinal,
                    root.semantic_root(),
                    semantic_sha256,
                    root.kernel_id(),
                )?;
            }
            LineageRosterPayloadV1::FormalMemory => {
                InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
                    root.payload().to_vec(),
                )
                .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;
            }
            LineageRosterPayloadV1::VerusExecution => {
                let _ =
                    CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(root.payload())?;
            }
        }
    }
    Ok(())
}

impl PreparedProductionSemanticLineageV3 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_prepare(
        rustc_identity_inventory: &crate::collector::AuthenticatedRustcIdentityInventoryV3,
        rustc_preflight_plan: &crate::collector::AuthenticatedRustcPreflightPlanV3,
        rustc_target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
        ranked_verification: AuthenticatedRankedVerificationRosterV1,
        admitted: &ProductionFormalMemoryOwnerV1,
        target_module: &Module,
        target_optimization: &fe2o3_kernel_opt::KernelIrPlironOptimizationReportV2,
        pre_descriptor_llvm: &str,
        semantic_debug_inputs: crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1,
    ) -> Result<Self, ProductionSemanticLineageErrorV3> {
        admitted
            .verify_equivalence()
            .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;

        let semantic = admitted.semantic_kir().semantic().semantic();
        let rustc_identity_inventory =
            InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
                rustc_identity_inventory.canonical_transcript(),
            )?;
        let rustc_preflight_plan = InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
            rustc_preflight_plan.canonical_transcript(),
        )?;
        let semantic_mir = InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(
            semantic.canonical_encoding(),
        )?;

        let neutral_kir_custody = admitted.semantic_kir().canonical_kernel_ir_identity();
        let neutral_kir = admitted.semantic_kir().canonical_kernel_ir_bytes();
        let (bound_kir_digest, bound_kir_length) = match neutral_kir_custody.version() {
            ProductionCanonicalKernelIrVersionV1::V8 => {
                let bound_kir = VerifiedCanonicalKernelIrV8::from_module(target_module.clone())?;
                bound_kir.revalidate()?;
                (
                    *bound_kir.identity().digest(),
                    bound_kir.canonical_bytes().len() as u64,
                )
            }
            ProductionCanonicalKernelIrVersionV1::V9 => {
                let bound_kir = VerifiedCanonicalKernelIrV9::from_module(target_module.clone())?;
                bound_kir.revalidate()?;
                (
                    *bound_kir.identity().digest(),
                    bound_kir.canonical_bytes().len() as u64,
                )
            }
            ProductionCanonicalKernelIrVersionV1::V11 => {
                let bound_kir = VerifiedCanonicalKernelIrV11::from_module(target_module.clone())?;
                bound_kir.revalidate()?;
                (
                    *bound_kir.identity().digest(),
                    bound_kir.canonical_bytes().len() as u64,
                )
            }
        };
        let neutral_kir_identity = TargetLineageIdentityV3::new(
            *neutral_kir_custody.digest(),
            neutral_kir_custody.canonical_length(),
        )?;
        let bound_kir_identity = TargetLineageIdentityV3::new(bound_kir_digest, bound_kir_length)?;
        let kernel_ir = InertKernelIrReceiptV3::from_canonical_preimage(neutral_kir)?;
        let amdgpu_lowering_replay =
            dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1::from_optimized_live_inputs_v4(
                neutral_kir,
                target_module,
                target_optimization,
                rustc_target.profile(),
                pre_descriptor_llvm,
            )?;

        let PreparedLineageEvidenceV1 {
            middle_end,
            mir_to_kir_correspondence,
            formal_memory,
            proof_verus_evidence,
            roster_custody,
            workgroups,
        } = prepare_lineage_evidence_v1(
            ranked_verification,
            admitted,
            target_module,
            neutral_kir_custody,
        )?;
        let semantic_debug = match semantic_debug_inputs {
            crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::Unavailable(
                gap,
            ) => crate::production_semantic_debug_v1::PreparedProductionSemanticDebugV1::Unavailable(
                gap,
            ),
            crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::Available {
                source_map,
                canonical_kir_v7,
            } => match &roster_custody {
                PreparedLineageRosterCustodyV1::Singleton => {
                    let correspondence =
                        InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(
                            mir_to_kir_correspondence.canonical_preimage(),
                        )?;
                    match crate::production_semantic_debug_v1::prepare_production_semantic_debug_v1(
                        admitted.semantic_kir(),
                        &correspondence,
                        *source_map,
                        &canonical_kir_v7,
                    ) {
                        Ok(prepared) => prepared,
                        Err(error) => crate::production_semantic_debug_v1::PreparedProductionSemanticDebugV1::Unavailable(
                            semantic_debug_prepare_gap(&error),
                        ),
                    }
                }
                PreparedLineageRosterCustodyV1::MultiRoot { .. } => {
                    match crate::production_semantic_debug_v1::prepare_production_semantic_debug_multi_root_v1(
                        admitted.semantic_kir(),
                        mir_to_kir_correspondence.canonical_preimage(),
                        *source_map,
                        &canonical_kir_v7,
                    ) {
                        Ok(prepared) => prepared,
                        Err(error) => crate::production_semantic_debug_v1::PreparedProductionSemanticDebugV1::Unavailable(
                            semantic_debug_prepare_gap(&error),
                        ),
                    }
                }
            },
        };

        let semantic_layout_identity = derive_semantic_target_layout_identity_v1(
            rustc_target.rustc_layout().llvm_target(),
            rustc_target.rustc_layout().data_layout(),
            rustc_target.rustc_layout().default_pointer_width_bits(),
            rustc_target.rustc_layout().active_cpu().unwrap_or_default(),
            rustc_target
                .rustc_layout()
                .active_features()
                .unwrap_or_default(),
        )?;
        if semantic.target_layout_identity().as_bytes() != &semantic_layout_identity.sha256() {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "semantic MIR target layout differs from the authenticated rustc layout",
            ));
        }

        validate_final_llvm_layout(pre_descriptor_llvm)?;

        let expected_exports = exact_source_and_kir_exports(semantic, target_module)?;
        Ok(Self {
            rustc_identity_inventory,
            rustc_preflight_plan,
            semantic_mir,
            middle_end,
            kernel_ir,
            mir_to_kir_correspondence,
            formal_memory,
            proof_verus_evidence,
            roster_custody,
            amdgpu_lowering_replay,
            semantic_debug,
            neutral_kir_custody,
            neutral_kir_identity,
            bound_kir_identity,
            semantic_layout_identity,
            expected_exports,
            rustc_layout: rustc_target.rustc_layout().clone(),
            workgroups,
        })
    }

    pub(crate) fn finish(
        self,
        invocation_custody: &FinishedProtectedRustcInvocationV3,
        target: fe2o3_compiler_ffi::DeviceTargetV1,
        descriptor_source: &CompilerDescriptorSourceV1,
        module_handoff: CompilerModuleHandoffV2,
    ) -> Result<InertSemanticCompilerModuleHandoffV3, ProductionSemanticLineageErrorV3> {
        invocation_custody
            .revalidate_for_publication()
            .map_err(ProductionSemanticLineageErrorV3::ProtectedRustcInvocation)?;
        let invocation = invocation_custody.descriptor().clone();
        self.finish_with_inert_invocation(invocation, target, descriptor_source, module_handoff)
    }

    /// Builds the same authority-free V3 handoff for an explicit extraction descriptor.
    ///
    /// Unlike [`Self::finish`], this diagnostic boundary does not authenticate the descriptor
    /// against the running compiler process. Its output is therefore inert and cannot enter the
    /// protected publication transition.
    pub(crate) fn finish_for_inert_extraction(
        self,
        invocation: RustcInvocationDescriptorV3,
        target: fe2o3_compiler_ffi::DeviceTargetV1,
        descriptor_source: &CompilerDescriptorSourceV1,
        module_handoff: CompilerModuleHandoffV2,
    ) -> Result<InertSemanticCompilerModuleHandoffV3, ProductionSemanticLineageErrorV3> {
        self.finish_with_inert_invocation(invocation, target, descriptor_source, module_handoff)
    }

    fn finish_with_inert_invocation(
        self,
        invocation: RustcInvocationDescriptorV3,
        target: fe2o3_compiler_ffi::DeviceTargetV1,
        descriptor_source: &CompilerDescriptorSourceV1,
        module_handoff: CompilerModuleHandoffV2,
    ) -> Result<InertSemanticCompilerModuleHandoffV3, ProductionSemanticLineageErrorV3> {
        if invocation.amd_target() != target.to_string()
            || descriptor_source.table().device_target() != target
            || module_handoff.target() != target
            || descriptor_source.table().code_object_version() != CodeObjectVersion::V6
            || module_handoff.code_object_version() != CodeObjectVersion::V6
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "invocation, descriptor, and module targets or code-object versions differ",
            ));
        }
        validate_final_exports(
            &self.expected_exports,
            descriptor_source,
            module_handoff.symbol_manifest(),
        )?;
        let final_llvm = std::str::from_utf8(module_handoff.module_bytes()).map_err(|_| {
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "final compiler module is not canonical textual LLVM",
            )
        })?;
        validate_final_llvm_layout(final_llvm)?;
        match &self.roster_custody {
            PreparedLineageRosterCustodyV1::Singleton => {
                let correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV5::decode(
                    self.mir_to_kir_correspondence.canonical_preimage(),
                )?;
                let formal = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(
                    self.formal_memory.canonical_preimage(),
                )?;
                if correspondence
                    .nested_v4()
                    .semantic_u32_induction()
                    .semantic_mir_sha256()
                    != self.semantic_mir.identity().sha256()
                    || correspondence.nested_v4().canonical_kernel_ir_identity()
                        != self.neutral_kir_custody
                    || formal.canonical_kernel_ir_identity() != self.neutral_kir_custody
                    || correspondence.grants_authority()
                    || correspondence
                        .nested_v4()
                        .semantic_u32_induction()
                        .grants_authority()
                    || formal.grants_authority()
                {
                    return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                        "lossless semantic correspondence custody changed before final handoff",
                    ));
                }
            }
            PreparedLineageRosterCustodyV1::MultiRoot {
                roster_identity,
                middle_end_sha256,
                correspondence_sha256,
                formal_memory_sha256,
                verus_sha256,
            } => {
                validate_lineage_roster_envelope_v1(
                    self.middle_end.canonical_preimage(),
                    *middle_end_sha256,
                    self.semantic_mir.identity().sha256(),
                    self.neutral_kir_custody.into(),
                    *roster_identity,
                    &self.workgroups,
                    LineageRosterPayloadV1::MiddleEnd,
                )?;
                validate_lineage_roster_envelope_v1(
                    self.mir_to_kir_correspondence.canonical_preimage(),
                    *correspondence_sha256,
                    self.semantic_mir.identity().sha256(),
                    self.neutral_kir_custody.into(),
                    *roster_identity,
                    &self.workgroups,
                    LineageRosterPayloadV1::Correspondence,
                )?;
                validate_lineage_roster_envelope_v1(
                    self.formal_memory.canonical_preimage(),
                    *formal_memory_sha256,
                    self.semantic_mir.identity().sha256(),
                    self.neutral_kir_custody.into(),
                    *roster_identity,
                    &self.workgroups,
                    LineageRosterPayloadV1::FormalMemory,
                )?;
                validate_lineage_roster_envelope_v1(
                    &self.proof_verus_evidence,
                    *verus_sha256,
                    self.semantic_mir.identity().sha256(),
                    self.neutral_kir_custody.into(),
                    *roster_identity,
                    &self.workgroups,
                    LineageRosterPayloadV1::VerusExecution,
                )?;
            }
        }

        let invocation_bytes = encode_descriptor_v3(&invocation)
            .map_err(|error| ProductionSemanticLineageErrorV3::Invocation(error.to_string()))?;
        let invocation_digest = InvocationDigestV3::calculate(&invocation)
            .map_err(|error| ProductionSemanticLineageErrorV3::Invocation(error.to_string()))?;
        let invocation_identity = TargetLineageIdentityV3::new(
            invocation_digest.into_bytes(),
            invocation_bytes.len() as u64,
        )?;

        let semantic_identity = receipt_identity(
            self.semantic_mir.identity().sha256(),
            self.semantic_mir.identity().byte_len(),
        )?;
        let middle_end_identity = receipt_identity(
            self.middle_end.identity().sha256(),
            self.middle_end.identity().byte_len(),
        )?;
        let kernel_ir_identity = receipt_identity(
            self.kernel_ir.identity().sha256(),
            self.kernel_ir.identity().byte_len(),
        )?;
        let correspondence_identity = receipt_identity(
            self.mir_to_kir_correspondence.identity().sha256(),
            self.mir_to_kir_correspondence.identity().byte_len(),
        )?;
        let formal_memory_identity = receipt_identity(
            self.formal_memory.identity().sha256(),
            self.formal_memory.identity().byte_len(),
        )?;

        let proof_binding = InertProofBindingAssociationV4::new(
            InertProofBindingAssociationInputsV4::new(
                proof_association_identity(
                    self.semantic_mir.identity().sha256(),
                    self.semantic_mir.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.middle_end.identity().sha256(),
                    self.middle_end.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.kernel_ir.identity().sha256(),
                    self.kernel_ir.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.mir_to_kir_correspondence.identity().sha256(),
                    self.mir_to_kir_correspondence.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.formal_memory.identity().sha256(),
                    self.formal_memory.identity().byte_len(),
                )?,
            ),
            &self.proof_verus_evidence,
        )?;
        let proof_binding =
            InertProofBindingReceiptV3::from_canonical_preimage(proof_binding.canonical_bytes())?;
        let proof_binding_identity = receipt_identity(
            proof_binding.identity().sha256(),
            proof_binding.identity().byte_len(),
        )?;
        let independently_validated_multi_root = if matches!(
            &self.roster_custody,
            PreparedLineageRosterCustodyV1::MultiRoot { .. }
        ) {
            let independently_validated = validate_compiler_multi_root_proof_inputs_v1(
                &proof_binding,
                &self.semantic_mir,
                &self.middle_end,
                &self.kernel_ir,
                &self.mir_to_kir_correspondence,
                &self.formal_memory,
            )?;
            if independently_validated.authenticates_compiler_origin()
                || independently_validated.establishes_llvm_or_machine_refinement()
                || independently_validated.grants_runtime_authority()
            {
                return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                    "independent multi-root proof owner broadened source-side authority",
                ));
            }
            Some(independently_validated)
        } else {
            None
        };

        let rustc_cpu = self.rustc_layout.active_cpu().ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "authenticated rustc target has no active CPU",
            ),
        )?;
        let rustc_features = self.rustc_layout.active_features().ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "authenticated rustc target has no active features",
            ),
        )?;
        let configured_target = target.to_string();
        let target_binding_bytes = match &self.roster_custody {
            PreparedLineageRosterCustodyV1::Singleton => {
                let [(_, workgroup)] = self.workgroups.as_ref() else {
                    return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                        "singleton lineage changed its workgroup roster",
                    ));
                };
                TargetBindingTranscriptV3::new(TargetBindingTranscriptInputsV3 {
                    protected_rustc_invocation: invocation_identity,
                    semantic_mir: semantic_identity,
                    target_neutral_kir: self.neutral_kir_identity,
                    target_bound_kir: self.bound_kir_identity,
                    configured_target: &configured_target,
                    rustc_llvm_target: self.rustc_layout.llvm_target(),
                    target_cpu: rustc_cpu,
                    target_features: rustc_features,
                    code_object_version: CODE_OBJECT_VERSION_V3,
                    wave_width_bits: WAVE_WIDTH_BITS_V3,
                    default_workgroup: *workgroup,
                })?
                .canonical_bytes()
                .to_vec()
            }
            PreparedLineageRosterCustodyV1::MultiRoot {
                roster_identity, ..
            } => {
                let workgroups = self
                    .workgroups
                    .iter()
                    .map(|(kernel, workgroup)| MultiRootTargetWorkgroupInputV2 {
                        kernel,
                        workgroup: *workgroup,
                    })
                    .collect::<Vec<_>>();
                MultiRootTargetBindingTranscriptV2::new(MultiRootTargetBindingInputsV2 {
                    protected_rustc_invocation: invocation_identity,
                    semantic_mir: semantic_identity,
                    target_neutral_kir: self.neutral_kir_identity,
                    target_bound_kir: self.bound_kir_identity,
                    configured_target: &configured_target,
                    rustc_llvm_target: self.rustc_layout.llvm_target(),
                    target_cpu: rustc_cpu,
                    target_features: rustc_features,
                    roster_identity: *roster_identity,
                    code_object_version: CODE_OBJECT_VERSION_V3,
                    wave_width_bits: WAVE_WIDTH_BITS_V3,
                    workgroups: &workgroups,
                })?
                .into_canonical_bytes()
            }
        };
        let target_binding =
            InertTargetBindingReceiptV3::from_canonical_preimage(target_binding_bytes)?;
        let target_binding_identity = receipt_identity(
            target_binding.identity().sha256(),
            target_binding.identity().byte_len(),
        )?;

        let data_layout = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: semantic_identity,
            target_binding: target_binding_identity,
            semantic_layout: self.semantic_layout_identity,
            rustc_llvm_target: self.rustc_layout.llvm_target(),
            live_rustc_data_layout: self.rustc_layout.data_layout(),
            final_llvm_target: self.rustc_layout.llvm_target(),
            final_llvm_data_layout: PRODUCTION_WORKER_DATA_LAYOUT_V1,
            default_pointer_width_bits: self.rustc_layout.default_pointer_width_bits(),
        })?;
        let data_layout =
            InertDataLayoutReceiptV3::from_canonical_preimage(data_layout.canonical_bytes())?;
        let data_layout_identity = receipt_identity(
            data_layout.identity().sha256(),
            data_layout.identity().byte_len(),
        )?;

        // The finalizer must be able to recover and strictly decode the exact
        // zero-digest descriptor source without knowing a backend-private codec.
        let abi = InertAbiReceiptV3::from_canonical_preimage(descriptor_source.canonical_bytes())?;
        let abi_identity = receipt_identity(abi.identity().sha256(), abi.identity().byte_len())?;

        let export_manifest = InertExportManifestReceiptV3::from_canonical_preimage(
            module_handoff.symbol_manifest().canonical_bytes(),
        )?;
        let export_manifest_identity = receipt_identity(
            export_manifest.identity().sha256(),
            export_manifest.identity().byte_len(),
        )?;

        let amdgpu_lowering = InertAmdgpuLoweringReceiptV3::from_canonical_preimage(
            self.amdgpu_lowering_replay.canonical_bytes(),
        )?;
        let validated_lowering = fe2o3_verifier::validate_compiler_kir_to_llvm_replay_v1(
            &self.kernel_ir,
            &amdgpu_lowering,
        )?;
        let replay_evidence = validated_lowering.replay().evidence();
        let replay_target_identity = TargetLineageIdentityV3::new(
            replay_evidence.target_bound_kernel_ir_identity().sha256(),
            replay_evidence.target_bound_kernel_ir_identity().byte_len(),
        )?;
        if replay_target_identity != self.bound_kir_identity
            || replay_evidence.profile().device_target() != configured_target
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "independently replayed AMDGPU lowering changed target-bound KIR or target profile",
            ));
        }
        let amdgpu_lowering_identity = receipt_identity(
            amdgpu_lowering.identity().sha256(),
            amdgpu_lowering.identity().byte_len(),
        )?;

        let final_commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(&module_handoff)?;
        let final_compiler_module_commitment =
            InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(
                final_commitment.canonical_bytes(),
            )?;
        let final_commitment_identity = receipt_identity(
            final_compiler_module_commitment.identity().sha256(),
            final_compiler_module_commitment.identity().byte_len(),
        )?;
        let module_identity = module_handoff.module_identity();
        let final_llvm_identity =
            TargetLineageIdentityV3::new(*module_identity.sha256(), module_identity.byte_len())?;

        let semantic_to_llvm_association =
            SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
                semantic_mir: semantic_identity,
                middle_end: middle_end_identity,
                kernel_ir: kernel_ir_identity,
                mir_to_kir_correspondence: correspondence_identity,
                formal_memory: formal_memory_identity,
                proof_binding: proof_binding_identity,
                target_binding: target_binding_identity,
                data_layout: data_layout_identity,
                abi: abi_identity,
                export_manifest: export_manifest_identity,
                amdgpu_lowering: amdgpu_lowering_identity,
                final_llvm: final_llvm_identity,
                final_compiler_module_commitment: final_commitment_identity,
            })?;
        InertSemanticToLlvmAssociationV3::decode(semantic_to_llvm_association.canonical_bytes())
            .map_err(|_| {
                ProductionSemanticLineageErrorV3::AxisMismatch(
                    "compiler semantic-to-LLVM association is not the frozen canonical V3 schema",
                )
            })?;
        let semantic_debug = self.semantic_debug.finish(
            self.semantic_mir.canonical_preimage(),
            module_handoff.module_bytes(),
        );
        let semantic_to_llvm = attach_optional_semantic_debug_v1(
            semantic_to_llvm_association.canonical_bytes(),
            semantic_debug,
        )?;

        let receipts = OrderedInertSemanticLineageReceiptsV3::new(
            self.rustc_identity_inventory,
            self.rustc_preflight_plan,
            self.semantic_mir,
            self.middle_end,
            self.kernel_ir,
            self.mir_to_kir_correspondence,
            self.formal_memory,
            proof_binding,
            target_binding,
            data_layout,
            abi,
            export_manifest,
            amdgpu_lowering,
            semantic_to_llvm,
            final_compiler_module_commitment,
        );
        let capsule = InertProductionSemanticCapsuleV3::new(invocation, target, receipts)?;
        if let Some(proof_inputs) = independently_validated_multi_root {
            let independently_validated =
                validate_compiler_multi_root_target_lineage_v1(&capsule, &proof_inputs)?;
            if !independently_validated.has_exact_receipt_association()
                || !independently_validated.has_exact_kir_to_llvm_replay()
                || independently_validated.establishes_semantic_refinement()
                || independently_validated.establishes_llvm_to_machine_refinement()
                || independently_validated.authenticates_producer()
                || independently_validated.grants_runtime_authority()
            {
                return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                    "independent multi-root target owner changed its exact non-authority contract",
                ));
            }
        }
        InertSemanticCompilerModuleHandoffV3::new(capsule, module_handoff).map_err(Into::into)
    }
}

fn semantic_debug_prepare_gap(
    error: &crate::production_pipeline::ProductionPipelineError,
) -> ProductionSemanticDebugProducerGapV1 {
    match error {
        crate::production_pipeline::ProductionPipelineError::SimulationProductionKirV9 => {
            ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable
        }
        crate::production_pipeline::ProductionPipelineError::SemanticDebugMap(
            fe2o3_kernel_ir::SemanticDebugMapErrorV1::InvalidLength
            | fe2o3_kernel_ir::SemanticDebugMapErrorV1::Encoding
            | fe2o3_kernel_ir::SemanticDebugMapErrorV1::ResourceLimit
            | fe2o3_kernel_ir::SemanticDebugMapErrorV1::AllocationFailure,
        )
        | crate::production_pipeline::ProductionPipelineError::SemanticDebugFragment(
            fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::ResourceLimit
            | fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::AllocationFailure,
        ) => ProductionSemanticDebugProducerGapV1::ResourceLimit,
        crate::production_pipeline::ProductionPipelineError::SimulationDebugMapCorrespondence(
            _,
        ) => ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable,
        crate::production_pipeline::ProductionPipelineError::SemanticDebugFragment(_) => {
            ProductionSemanticDebugProducerGapV1::FragmentConstructionUnavailable
        }
        crate::production_pipeline::ProductionPipelineError::SemanticDebugMap(_) => {
            ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable
        }
        _ => ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable,
    }
}

fn attach_optional_semantic_debug_v1(
    association: &[u8],
    availability: ProductionSemanticDebugAvailabilityV1,
) -> Result<InertSemanticToLlvmReceiptV3, ProductionSemanticLineageErrorV3> {
    attach_optional_semantic_debug_with_fault_v1(
        association,
        availability,
        OptionalSemanticDebugAttachmentFaultV1::None,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
enum OptionalSemanticDebugAttachmentFaultV1 {
    None,
    PrimaryCarrierResource,
    PrimaryCarrierStructural,
    FallbackCarrier,
    Extension,
    ExtensionReceipt,
}

fn attach_optional_semantic_debug_with_fault_v1(
    association: &[u8],
    availability: ProductionSemanticDebugAvailabilityV1,
    fault: OptionalSemanticDebugAttachmentFaultV1,
) -> Result<InertSemanticToLlvmReceiptV3, ProductionSemanticLineageErrorV3> {
    let primary = if matches!(
        fault,
        OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierResource
            | OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierStructural
            | OptionalSemanticDebugAttachmentFaultV1::FallbackCarrier
    ) {
        None
    } else {
        Some(ProductionSemanticDebugCarrierV1::new(
            association,
            availability,
        ))
    };
    let carrier = match primary {
        Some(Ok(carrier)) => carrier,
        Some(Err(error)) => {
            let gap = if matches!(
                error,
                fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::ResourceLimit
                    | fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::AllocationFailure
            ) {
                ProductionSemanticDebugProducerGapV1::ResourceLimit
            } else {
                ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable
            };
            let Some(carrier) = fallback_debug_carrier_v1(association, gap, fault) else {
                return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
                    .map_err(Into::into);
            };
            carrier
        }
        None => {
            let gap = if fault == OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierResource {
                ProductionSemanticDebugProducerGapV1::ResourceLimit
            } else {
                ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable
            };
            let Some(carrier) = fallback_debug_carrier_v1(association, gap, fault) else {
                return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
                    .map_err(Into::into);
            };
            carrier
        }
    };
    if fault == OptionalSemanticDebugAttachmentFaultV1::Extension {
        return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
            .map_err(Into::into);
    }
    let Ok(extension) = ProductionSemanticDebugReceiptExtensionV1::new(association, carrier) else {
        return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
            .map_err(Into::into);
    };
    if fault == OptionalSemanticDebugAttachmentFaultV1::ExtensionReceipt {
        return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
            .map_err(Into::into);
    }
    match InertSemanticToLlvmReceiptV3::from_canonical_preimage(extension.canonical_bytes()) {
        Ok(receipt) => Ok(receipt),
        Err(_) => {
            InertSemanticToLlvmReceiptV3::from_canonical_preimage(association).map_err(Into::into)
        }
    }
}

fn fallback_debug_carrier_v1(
    association: &[u8],
    gap: ProductionSemanticDebugProducerGapV1,
    fault: OptionalSemanticDebugAttachmentFaultV1,
) -> Option<ProductionSemanticDebugCarrierV1> {
    if fault == OptionalSemanticDebugAttachmentFaultV1::FallbackCarrier {
        return None;
    }
    ProductionSemanticDebugCarrierV1::new(
        association,
        ProductionSemanticDebugAvailabilityV1::Unavailable(gap),
    )
    .ok()
}

fn receipt_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<TargetLineageIdentityV3, ProductionSemanticLineageErrorV3> {
    TargetLineageIdentityV3::new(*sha256, byte_len).map_err(Into::into)
}

fn proof_association_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<InertLineageContentIdentityV3, ProductionSemanticLineageErrorV3> {
    InertLineageContentIdentityV3::new(*sha256, byte_len).map_err(Into::into)
}

fn exact_source_and_kir_exports(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    target_module: &Module,
) -> Result<BTreeSet<(CompilerModuleSymbolRoleV1, String)>, ProductionSemanticLineageErrorV3> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionExportV1;

    let source = semantic
        .functions()
        .iter()
        .filter_map(|function| match function.export()? {
            SemanticFunctionExportV1::Kernel(entry) => Some(
                semantic_link_symbol(entry.export_symbol())
                    .map(|symbol| (CompilerModuleSymbolRoleV1::KernelEntry, symbol)),
            ),
            SemanticFunctionExportV1::DeviceFfi { export_symbol } => Some(
                semantic_link_symbol(export_symbol)
                    .map(|symbol| (CompilerModuleSymbolRoleV1::DeviceFfiExport, symbol)),
            ),
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let kir = target_module
        .functions
        .iter()
        .filter_map(|function| match function.role {
            FunctionRole::KernelEntry => Some((
                CompilerModuleSymbolRoleV1::KernelEntry,
                function.id.as_str().to_owned(),
            )),
            FunctionRole::DeviceFfiExport => Some((
                CompilerModuleSymbolRoleV1::DeviceFfiExport,
                function.id.as_str().to_owned(),
            )),
            FunctionRole::InternalHelper | FunctionRole::ExternalImport => None,
        })
        .collect::<BTreeSet<_>>();
    if source.is_empty() || source != kir {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "semantic and target-bound KIR export roles differ",
        ));
    }
    Ok(source)
}

fn semantic_link_symbol(
    symbol: &fe2o3_mir_model::semantic_mir_v1::SemanticLinkSymbolV1,
) -> Result<String, ProductionSemanticLineageErrorV3> {
    std::str::from_utf8(symbol.as_bytes())
        .map(str::to_owned)
        .map_err(|_| {
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "semantic export symbol is not valid UTF-8",
            )
        })
}

fn validate_final_exports(
    expected_exports: &BTreeSet<(CompilerModuleSymbolRoleV1, String)>,
    descriptor_source: &CompilerDescriptorSourceV1,
    manifest: &CompilerModuleSymbolManifestV1,
) -> Result<(), ProductionSemanticLineageErrorV3> {
    let observed_exports = manifest
        .entries()
        .filter(|(role, _)| {
            matches!(
                role,
                CompilerModuleSymbolRoleV1::KernelEntry
                    | CompilerModuleSymbolRoleV1::DeviceFfiExport
            )
        })
        .map(|(role, symbol)| (role, symbol.to_owned()))
        .collect::<BTreeSet<_>>();
    if &observed_exports != expected_exports {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "final compiler manifest export roles differ from semantic/KIR exports",
        ));
    }

    let expected_kernel_entries = expected_exports
        .iter()
        .filter(|(role, _)| *role == CompilerModuleSymbolRoleV1::KernelEntry)
        .map(|(_, symbol)| symbol.as_str())
        .collect::<BTreeSet<_>>();
    let descriptor_kernel_entries = descriptor_source
        .table()
        .kernels()
        .iter()
        .map(|kernel| kernel.entry_name().as_str())
        .collect::<BTreeSet<_>>();
    if expected_kernel_entries != descriptor_kernel_entries {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "compiler descriptor kernel entries differ from semantic/KIR entries",
        ));
    }

    let expected_descriptors = descriptor_source
        .table()
        .kernels()
        .iter()
        .map(|kernel| kernel.descriptor_symbol().as_str())
        .collect::<BTreeSet<_>>();
    let observed_descriptors = manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
        .collect::<BTreeSet<_>>();
    if expected_descriptors != observed_descriptors {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "final compiler manifest descriptor symbols differ from descriptor source",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum ProductionSemanticLineageErrorV3 {
    AxisMismatch(&'static str),
    Invocation(String),
    ProtectedRustcInvocation(ProtectedRustcInvocationErrorV1),
    LiveOwner(String),
    CanonicalKir(VerifiedCanonicalKernelIrErrorV8),
    CanonicalKirV9(VerifiedCanonicalKernelIrErrorV9),
    CanonicalKirV11(VerifiedCanonicalKernelIrErrorV11),
    Correspondence(ProductionCorrespondenceEvidenceErrorV4),
    CorrespondenceV5(ProductionCorrespondenceEvidenceErrorV5),
    FormalMemory(ProductionFormalMemoryEvidenceErrorV4),
    VerusEvidence(ProductionMirPlironVerusExecutionEvidenceErrorV1),
    KirToLlvmReplay(dialect_amdgcn::ProductionKirToLlvmReplayErrorV1),
    KirToLlvmReplayValidation(CompilerKirToLlvmReplayValidationErrorV1),
    MultiRootProofValidation(CompilerMultiRootProofValidationErrorV1),
    MultiRootTargetLineageValidation(CompilerTargetLineageValidationErrorV1),
    Receipt(LineageErrorV3),
    ProofIdentity(InertProofBindingAssociationErrorV3),
    ProofBinding(InertProofBindingAssociationErrorV4),
    MultiRootRoster(MultiRootProofRosterErrorV2),
    Transcript(ProductionTargetLineageErrorV3),
    FinalCommitment(FinalCompilerModuleCommitmentErrorV3),
    Capsule(InertSemanticCompilerModuleHandoffErrorV3),
}

impl fmt::Display for ProductionSemanticLineageErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AxisMismatch(detail) => {
                write!(formatter, "production V3 lineage mismatch: {detail}")
            }
            Self::Invocation(detail) => {
                write!(formatter, "production V3 invocation failed: {detail}")
            }
            Self::ProtectedRustcInvocation(error) => write!(
                formatter,
                "production V3 protected rustc custody failed: {error}"
            ),
            Self::LiveOwner(detail) => {
                write!(formatter, "production V3 live owner failed: {detail}")
            }
            Self::CanonicalKir(error) => {
                write!(formatter, "production V3 canonical KIR failed: {error}")
            }
            Self::CanonicalKirV9(error) => {
                write!(formatter, "production V3 canonical KIR V9 failed: {error}")
            }
            Self::CanonicalKirV11(error) => {
                write!(formatter, "production V3 canonical KIR V11 failed: {error}")
            }
            Self::Correspondence(error) => {
                write!(
                    formatter,
                    "production lossless correspondence failed: {error}"
                )
            }
            Self::CorrespondenceV5(error) => {
                write!(
                    formatter,
                    "production exact-function correspondence failed: {error}"
                )
            }
            Self::FormalMemory(error) => {
                write!(
                    formatter,
                    "production formal-memory evidence failed: {error}"
                )
            }
            Self::VerusEvidence(error) => {
                write!(
                    formatter,
                    "production V3 aggregate Verus evidence failed: {error}"
                )
            }
            Self::KirToLlvmReplay(error) => {
                write!(formatter, "production KIR-to-LLVM replay failed: {error}")
            }
            Self::KirToLlvmReplayValidation(error) => write!(
                formatter,
                "production independent KIR-to-LLVM validation failed: {error}"
            ),
            Self::MultiRootProofValidation(error) => write!(
                formatter,
                "production independent multi-root proof validation failed: {error}"
            ),
            Self::MultiRootTargetLineageValidation(error) => write!(
                formatter,
                "production independent multi-root target-lineage validation failed: {error}"
            ),
            Self::Receipt(error) => write!(formatter, "production V3 receipt failed: {error}"),
            Self::ProofIdentity(error) => {
                write!(formatter, "production V3 proof identity failed: {error}")
            }
            Self::ProofBinding(error) => {
                write!(formatter, "production V3 proof binding failed: {error}")
            }
            Self::MultiRootRoster(error) => {
                write!(
                    formatter,
                    "production multi-root proof roster failed: {error}"
                )
            }
            Self::Transcript(error) => {
                write!(formatter, "production V3 transcript failed: {error}")
            }
            Self::FinalCommitment(error) => {
                write!(formatter, "production V3 final commitment failed: {error}")
            }
            Self::Capsule(error) => write!(formatter, "production V3 capsule failed: {error}"),
        }
    }
}

impl Error for ProductionSemanticLineageErrorV3 {}

impl From<VerifiedCanonicalKernelIrErrorV8> for ProductionSemanticLineageErrorV3 {
    fn from(error: VerifiedCanonicalKernelIrErrorV8) -> Self {
        Self::CanonicalKir(error)
    }
}

impl From<VerifiedCanonicalKernelIrErrorV9> for ProductionSemanticLineageErrorV3 {
    fn from(error: VerifiedCanonicalKernelIrErrorV9) -> Self {
        Self::CanonicalKirV9(error)
    }
}

impl From<VerifiedCanonicalKernelIrErrorV11> for ProductionSemanticLineageErrorV3 {
    fn from(error: VerifiedCanonicalKernelIrErrorV11) -> Self {
        Self::CanonicalKirV11(error)
    }
}

impl From<ProductionCorrespondenceEvidenceErrorV4> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionCorrespondenceEvidenceErrorV4) -> Self {
        Self::Correspondence(error)
    }
}

impl From<ProductionCorrespondenceEvidenceErrorV5> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionCorrespondenceEvidenceErrorV5) -> Self {
        Self::CorrespondenceV5(error)
    }
}

impl From<ProductionFormalMemoryEvidenceErrorV4> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionFormalMemoryEvidenceErrorV4) -> Self {
        Self::FormalMemory(error)
    }
}

impl From<LineageErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: LineageErrorV3) -> Self {
        Self::Receipt(error)
    }
}

impl From<ProductionMirPlironVerusExecutionEvidenceErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionMirPlironVerusExecutionEvidenceErrorV1) -> Self {
        Self::VerusEvidence(error)
    }
}

impl From<dialect_amdgcn::ProductionKirToLlvmReplayErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: dialect_amdgcn::ProductionKirToLlvmReplayErrorV1) -> Self {
        Self::KirToLlvmReplay(error)
    }
}

impl From<CompilerKirToLlvmReplayValidationErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: CompilerKirToLlvmReplayValidationErrorV1) -> Self {
        Self::KirToLlvmReplayValidation(error)
    }
}

impl From<CompilerMultiRootProofValidationErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: CompilerMultiRootProofValidationErrorV1) -> Self {
        Self::MultiRootProofValidation(error)
    }
}

impl From<CompilerTargetLineageValidationErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: CompilerTargetLineageValidationErrorV1) -> Self {
        Self::MultiRootTargetLineageValidation(error)
    }
}

impl From<InertProofBindingAssociationErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: InertProofBindingAssociationErrorV3) -> Self {
        Self::ProofIdentity(error)
    }
}

impl From<InertProofBindingAssociationErrorV4> for ProductionSemanticLineageErrorV3 {
    fn from(error: InertProofBindingAssociationErrorV4) -> Self {
        Self::ProofBinding(error)
    }
}

impl From<MultiRootProofRosterErrorV2> for ProductionSemanticLineageErrorV3 {
    fn from(error: MultiRootProofRosterErrorV2) -> Self {
        Self::MultiRootRoster(error)
    }
}

impl From<ProductionTargetLineageErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionTargetLineageErrorV3) -> Self {
        Self::Transcript(error)
    }
}

impl From<FinalCompilerModuleCommitmentErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: FinalCompilerModuleCommitmentErrorV3) -> Self {
        Self::FinalCommitment(error)
    }
}

impl From<InertSemanticCompilerModuleHandoffErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: InertSemanticCompilerModuleHandoffErrorV3) -> Self {
        Self::Capsule(error)
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, ExplicitLaunchExtent, FormalIndexWidth, Function, Kernel,
        LaunchDomain, LaunchExtent, Signature, Terminator, WorkgroupSize,
        derive_kernel_memory_obligations_for_launch,
    };

    fn llvm_with_layout(layout: &str) -> String {
        format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{layout}\"\n\ndefine void @body() {{ ret void }}\n"
        )
    }

    fn formal_payload(kernel_name: &str) -> Box<[u8]> {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function = Function::kernel_entry(
            kernel_name,
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let mut kernel = Kernel::new(
            kernel_name,
            kernel_name,
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new(format!("lineage_{kernel_name}"));
        module.functions.push(function);
        module.kernels.push(kernel);
        let obligations = derive_kernel_memory_obligations_for_launch(
            &module,
            &module.kernels[0].id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(obligations.obligations())
            .unwrap()
            .into_canonical_bytes()
            .into_boxed_slice()
    }

    type FormalRosterFixture = (
        Vec<u8>,
        [u8; 32],
        [u8; 32],
        LineageNeutralKirIdentityV1,
        [u8; 32],
        Vec<(String, [u32; 3])>,
    );

    fn formal_roster_fixture() -> FormalRosterFixture {
        let semantic_sha256 = [0x31; 32];
        let roster_identity = [0x42; 32];
        let neutral = LineageNeutralKirIdentityV1 {
            version: ProductionCanonicalKernelIrVersionV1::V8,
            canonical_length: 4_096,
            digest: [0x53; 32],
        };
        let roots = vec![
            PreparedLineageRootV1 {
                logical_name: "zeta".to_owned(),
                export_symbol: b"zeta_kernel".to_vec().into_boxed_slice(),
                semantic_root: 3,
                semantic_root_identity: [0x61; 32],
                kernel_binding: [0x71; 32],
                source_rank: 1,
                kernel_id: "zeta_kernel".to_owned(),
                workgroup: [64, 1, 1],
                middle_end: vec![1].into_boxed_slice(),
                correspondence: vec![1].into_boxed_slice(),
                formal_memory: formal_payload("zeta_kernel"),
                verus_execution: vec![1].into_boxed_slice(),
            },
            PreparedLineageRootV1 {
                logical_name: "alpha".to_owned(),
                export_symbol: b"alpha_kernel".to_vec().into_boxed_slice(),
                semantic_root: 9,
                semantic_root_identity: [0x62; 32],
                kernel_binding: [0x72; 32],
                source_rank: 1,
                kernel_id: "alpha_kernel".to_owned(),
                workgroup: [128, 1, 1],
                middle_end: vec![2].into_boxed_slice(),
                correspondence: vec![2].into_boxed_slice(),
                formal_memory: formal_payload("alpha_kernel"),
                verus_execution: vec![2].into_boxed_slice(),
            },
        ];
        let workgroups = roots
            .iter()
            .map(|root| (root.kernel_id.clone(), root.workgroup))
            .collect::<Vec<_>>();
        let bytes = build_lineage_roster_v2(
            &semantic_sha256,
            neutral,
            roster_identity,
            &[0, 1],
            &roots,
            LineageRosterPayloadV1::FormalMemory,
        )
        .unwrap();
        let identity = Sha256::digest(&bytes).into();
        (
            bytes,
            identity,
            semantic_sha256,
            neutral,
            roster_identity,
            workgroups,
        )
    }

    #[test]
    fn final_llvm_requires_one_exact_measured_worker_layout() {
        let exact = llvm_with_layout(PRODUCTION_WORKER_DATA_LAYOUT_V1);
        validate_final_llvm_layout(&exact).unwrap();

        let stale_layout = format!("e-m:e-{}", &PRODUCTION_WORKER_DATA_LAYOUT_V1[2..]);
        assert!(validate_final_llvm_layout(&llvm_with_layout(&stale_layout)).is_err());
        assert!(
            validate_final_llvm_layout(&format!(
                "{exact}target datalayout = \"{PRODUCTION_WORKER_DATA_LAYOUT_V1}\"\n"
            ))
            .is_err()
        );
    }

    #[test]
    fn production_capsule_requires_shared_independent_kir_to_llvm_replay() {
        let source = include_str!("production_semantic_lineage_v3.rs");
        assert!(source.contains(
            "CanonicalProductionKirToLlvmReplayEvidenceV1::from_optimized_live_inputs_v4"
        ));
        assert!(source.contains(
            "target_optimization: &fe2o3_kernel_opt::KernelIrPlironOptimizationReportV2"
        ));
        assert!(source.contains("target_module,\n                target_optimization,"));
        assert!(source.contains("validate_compiler_kir_to_llvm_replay_v1"));
        assert!(source.contains("MultiRootProofRosterTranscriptV2::new"));
        assert!(source.contains("MultiRootProofRosterTranscriptV2::decode"));
        assert!(source.contains("validate_compiler_multi_root_proof_inputs_v1"));
        assert!(source.contains("validate_compiler_multi_root_target_lineage_v1"));
        assert!(source.contains("MultiRootTargetBindingTranscriptV2::new"));
        assert!(!source.contains(concat!("AmdgpuLoweringTranscript", "V3::new")));
        for suffix in ["MID2", "COR2", "FOR2", "VER2"] {
            assert!(!source.contains(&format!("{}{}", "F2MR", suffix)));
        }
        assert!(!source.contains(concat!("fn encode_", "multi_root_target_binding")));
        assert!(!source.contains(concat!("F2MR", "TGT2")));
    }

    #[test]
    fn multi_root_lineage_strictly_validates_every_root_field_and_payload_identity() {
        let (bytes, identity, semantic, neutral, roster, workgroups) = formal_roster_fixture();
        let mut symbol_order = (0..workgroups.len()).collect::<Vec<_>>();
        symbol_order.sort_unstable_by_key(|index| workgroups[*index].0.as_str());
        assert_eq!(symbol_order, vec![1, 0]);
        assert_eq!(
            &bytes[128..136],
            [0_u32.to_le_bytes(), 1_u32.to_le_bytes()].concat(),
            "descriptor binding order must remain independent of symbol order",
        );
        validate_lineage_roster_envelope_v1(
            &bytes,
            identity,
            &semantic,
            neutral,
            roster,
            &workgroups,
            LineageRosterPayloadV1::FormalMemory,
        )
        .unwrap();

        // Every fixed header/root field, every permutation slot, every framed
        // string, and every nested payload byte remains content-identity bound.
        let mutation_offsets = [
            0,
            8,
            10,
            12,
            16,
            48,
            50,
            52,
            60,
            92,
            124,
            128,
            132,
            136,
            140,
            144,
            176,
            208,
            240,
            241,
            244,
            248,
            252,
            256,
            260,
            264,
            bytes.len() - 1,
        ];
        for offset in mutation_offsets {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(
                validate_lineage_roster_envelope_v1(
                    &hostile,
                    identity,
                    &semantic,
                    neutral,
                    roster,
                    &workgroups,
                    LineageRosterPayloadV1::FormalMemory,
                )
                .is_err(),
                "mutation at byte {offset} was accepted",
            );
        }

        let mut wrong_permutation = bytes.clone();
        wrong_permutation[128..132].copy_from_slice(&1_u32.to_le_bytes());
        wrong_permutation[132..136].copy_from_slice(&0_u32.to_le_bytes());
        let wrong_identity = Sha256::digest(&wrong_permutation).into();
        assert!(
            validate_lineage_roster_envelope_v1(
                &wrong_permutation,
                wrong_identity,
                &semantic,
                neutral,
                roster,
                &workgroups,
                LineageRosterPayloadV1::FormalMemory,
            )
            .is_err()
        );

        let mut reordered_workgroups = workgroups.clone();
        reordered_workgroups.swap(0, 1);
        assert!(
            validate_lineage_roster_envelope_v1(
                &bytes,
                identity,
                &semantic,
                neutral,
                roster,
                &reordered_workgroups,
                LineageRosterPayloadV1::FormalMemory,
            )
            .is_err()
        );
    }

    fn test_association() -> Vec<u8> {
        let identity =
            fe2o3_compiler_lineage::InertSemanticToLlvmContentIdentityV3::new([0x41; 32], 1)
                .unwrap();
        fe2o3_compiler_lineage::InertSemanticToLlvmAssociationV3::new(
            fe2o3_compiler_lineage::InertSemanticToLlvmAssociationInputsV3::new(
                identity, identity, identity, identity, identity, identity, identity, identity,
                identity, identity, identity, identity, identity,
            ),
        )
        .unwrap()
        .canonical_bytes()
        .to_vec()
    }

    #[test]
    fn every_debug_attachment_failure_preserves_the_frozen_core_association() {
        let association = test_association();
        let availability = ProductionSemanticDebugAvailabilityV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
        );
        for (fault, expected_gap) in [
            (
                OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierResource,
                ProductionSemanticDebugProducerGapV1::ResourceLimit,
            ),
            (
                OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierStructural,
                ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable,
            ),
        ] {
            let receipt = attach_optional_semantic_debug_with_fault_v1(
                &association,
                availability.clone(),
                fault,
            )
            .unwrap();
            let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
                receipt.canonical_preimage(),
            )
            .unwrap();
            assert_eq!(extension.association_v3(), association);
            assert!(matches!(
                extension.carrier_v1().availability(),
                ProductionSemanticDebugAvailabilityV1::Unavailable(gap) if *gap == expected_gap
            ));
        }

        for fault in [
            OptionalSemanticDebugAttachmentFaultV1::FallbackCarrier,
            OptionalSemanticDebugAttachmentFaultV1::Extension,
            OptionalSemanticDebugAttachmentFaultV1::ExtensionReceipt,
        ] {
            let receipt = attach_optional_semantic_debug_with_fault_v1(
                &association,
                availability.clone(),
                fault,
            )
            .unwrap();
            assert_eq!(receipt.canonical_preimage(), association);
            assert!(
                fe2o3_compiler_lineage::InertSemanticToLlvmAssociationV3::decode(
                    receipt.canonical_preimage(),
                )
                .is_ok()
            );
        }
    }
}
