//! Independent, move-only validation of current multi-root compiler proof inputs.
//!
//! This module owns exact content association and replays source-side evidence only. It does not
//! authenticate compiler origin, establish LLVM or machine refinement, or grant runtime authority.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationErrorV4, InertProofBindingAssociationV4,
    InertProofBindingReceiptIdentityV3, InertProofBindingReceiptV3, MultiRootCanonicalKirVersionV2,
    MultiRootProofRosterErrorV2, MultiRootProofRosterKindV2, MultiRootProofRosterRootV2,
    MultiRootProofRosterTranscriptV2,
};
use fe2o3_kernel_ir::{
    AddressSpace, AmdGpuDiagnosticOperation, BasicBlock, BinaryOp, CheckedBinaryOperator,
    FormalMemoryReceiptErrorV1, FunctionBody, FunctionRole,
    InertCanonicalFormalMemoryObligationReceiptV1, MemoryAccess, Module, OperationKind, Terminator,
    VerifiedCanonicalKernelIrErrorV8, VerifiedCanonicalKernelIrErrorV9,
    VerifiedCanonicalKernelIrV8, VerifiedCanonicalKernelIrV9,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticCheckedBinaryOpV1, SemanticFunctionDeclV1,
    SemanticFunctionIdV1, SemanticLocalRoleV1, SemanticMirDecodeErrorV1, SemanticMirLimitsV1,
    SemanticRvalueKindV1, SemanticStatementKindV1,
};
use fe2o3_mir_model::{
    InertCanonicalSemanticU32InductionEvidenceV1, SemanticU32InductionAnalysisErrorV1,
    SemanticU32InductionEvidenceErrorV1, analyze_semantic_u32_induction_no_overflow_v1,
};
use fe2o3_pliron::{InertProductionMiddleEndEvidenceV5, ProductionMiddleEndEvidenceCodecErrorV5};
use sha2::{Digest, Sha256};

use crate::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1,
    ProductionMirPlironVerusExecutionEvidenceErrorV1,
};

const CORRESPONDENCE_MAGIC_V1: [u8; 8] = *b"F2MRCOP2";
const CORRESPONDENCE_VERSION_V1: u16 = 2;
const CORRESPONDENCE_POLICY_V1: u16 = 1;
const RANKED_ROSTER_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION-RANKED-KERNEL-ROSTER-IDENTITY/V1\0";

/// Move-only ownership of exact canonical neutral Kernel IR accepted for a multi-root proof.
#[derive(Debug)]
pub enum ValidatedCompilerMultiRootKernelIrV1 {
    /// Exact canonical Kernel IR V8.
    V8(VerifiedCanonicalKernelIrV8),
    /// Exact canonical Kernel IR V9.
    V9(VerifiedCanonicalKernelIrV9),
}

impl ValidatedCompilerMultiRootKernelIrV1 {
    /// Returns the exact retained canonical bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        match self {
            Self::V8(owner) => owner.canonical_bytes(),
            Self::V9(owner) => owner.canonical_bytes(),
        }
    }

    /// Returns the exact canonical wire version.
    pub const fn version(&self) -> MultiRootCanonicalKirVersionV2 {
        match self {
            Self::V8(_) => MultiRootCanonicalKirVersionV2::V8,
            Self::V9(_) => MultiRootCanonicalKirVersionV2::V9,
        }
    }

    /// Returns the verified canonical identity digest.
    pub const fn identity_digest(&self) -> &[u8; 32] {
        match self {
            Self::V8(owner) => owner.identity().digest(),
            Self::V9(owner) => owner.identity().digest(),
        }
    }

    /// Returns the exact canonical byte length.
    pub const fn canonical_length(&self) -> u64 {
        match self {
            Self::V8(owner) => owner.identity().canonical_length(),
            Self::V9(owner) => owner.identity().canonical_length(),
        }
    }
}

/// Independently decoded evidence for one semantic root in a multi-root proof capsule.
#[derive(Debug)]
pub struct ValidatedCompilerMultiRootProofRootV1 {
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    source_rank: u8,
    workgroup: [u32; 3],
    logical_name: Box<str>,
    export_symbol: Box<str>,
    kernel_id: Box<str>,
    middle_end: InertProductionMiddleEndEvidenceV5,
    semantic_u32_induction: InertCanonicalSemanticU32InductionEvidenceV1,
    formal_memory: InertCanonicalFormalMemoryObligationReceiptV1,
    verus_execution: CanonicalProductionMirPlironVerusExecutionEvidenceV1,
}

impl ValidatedCompilerMultiRootProofRootV1 {
    /// Returns the canonical semantic root index.
    pub const fn semantic_root(&self) -> u32 {
        self.semantic_root
    }

    /// Returns the exact semantic function identity.
    pub const fn semantic_root_identity(&self) -> &[u8; 32] {
        &self.semantic_root_identity
    }

    /// Returns the exact semantic kernel-binding identity.
    pub const fn kernel_binding(&self) -> &[u8; 32] {
        &self.kernel_binding
    }

    /// Returns the source launch rank.
    pub const fn source_rank(&self) -> u8 {
        self.source_rank
    }

    /// Returns the target-selected default workgroup retained by every roster.
    pub const fn workgroup(&self) -> [u32; 3] {
        self.workgroup
    }

    /// Returns the diagnostic logical kernel name.
    pub const fn logical_name(&self) -> &str {
        &self.logical_name
    }

    /// Returns the exact semantic export symbol.
    pub const fn export_symbol(&self) -> &str {
        &self.export_symbol
    }

    /// Returns the exact Kernel IR kernel identifier.
    pub const fn kernel_id(&self) -> &str {
        &self.kernel_id
    }

    /// Returns the independently decoded V5 middle-end evidence.
    pub const fn middle_end(&self) -> &InertProductionMiddleEndEvidenceV5 {
        &self.middle_end
    }

    /// Returns the independently decoded and replayed semantic induction evidence.
    pub const fn semantic_u32_induction(&self) -> &InertCanonicalSemanticU32InductionEvidenceV1 {
        &self.semantic_u32_induction
    }

    /// Returns the independently decoded formal-memory obligation receipt.
    pub const fn formal_memory(&self) -> &InertCanonicalFormalMemoryObligationReceiptV1 {
        &self.formal_memory
    }

    /// Returns the independently imported signed Verus execution evidence.
    pub const fn verus_execution(&self) -> &CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
        &self.verus_execution
    }
}

/// Move-only ownership of one exact, independently decoded multi-root source-side proof capsule.
///
/// ```compile_fail
/// use fe2o3_verifier::ValidatedCompilerMultiRootProofInputsV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ValidatedCompilerMultiRootProofInputsV1>();
/// ```
#[derive(Debug)]
#[must_use = "dropping validated multi-root proof inputs abandons exact source-side custody"]
pub struct ValidatedCompilerMultiRootProofInputsV1 {
    association: InertProofBindingAssociationV4,
    receipt_identity: InertProofBindingReceiptIdentityV3,
    semantic_mir: AdmittedInertSemanticMirV1,
    kernel_ir: ValidatedCompilerMultiRootKernelIrV1,
    kernel_ir_module: Module,
    middle_end_roster: MultiRootProofRosterTranscriptV2,
    correspondence_roster: MultiRootProofRosterTranscriptV2,
    formal_memory_roster: MultiRootProofRosterTranscriptV2,
    verus_roster: MultiRootProofRosterTranscriptV2,
    roots: Box<[ValidatedCompilerMultiRootProofRootV1]>,
}

impl ValidatedCompilerMultiRootProofInputsV1 {
    /// Returns the independently decoded outer association.
    pub const fn association(&self) -> &InertProofBindingAssociationV4 {
        &self.association
    }

    /// Returns the exact proof-binding receipt identity whose preimage was decoded.
    pub const fn receipt_identity(&self) -> InertProofBindingReceiptIdentityV3 {
        self.receipt_identity
    }

    /// Returns the independently decoded exact semantic MIR.
    pub const fn semantic_mir(&self) -> &AdmittedInertSemanticMirV1 {
        &self.semantic_mir
    }

    /// Returns exact verified neutral Kernel IR ownership.
    pub const fn kernel_ir(&self) -> &ValidatedCompilerMultiRootKernelIrV1 {
        &self.kernel_ir
    }

    /// Returns the same semantically verified decoded Kernel IR module.
    pub const fn kernel_ir_module(&self) -> &Module {
        &self.kernel_ir_module
    }

    /// Returns the exact middle-end proof roster.
    pub const fn middle_end_roster(&self) -> &MultiRootProofRosterTranscriptV2 {
        &self.middle_end_roster
    }

    /// Returns the exact correspondence proof roster.
    pub const fn correspondence_roster(&self) -> &MultiRootProofRosterTranscriptV2 {
        &self.correspondence_roster
    }

    /// Returns the exact formal-memory proof roster.
    pub const fn formal_memory_roster(&self) -> &MultiRootProofRosterTranscriptV2 {
        &self.formal_memory_roster
    }

    /// Returns the exact signed-Verus proof roster.
    pub const fn verus_roster(&self) -> &MultiRootProofRosterTranscriptV2 {
        &self.verus_roster
    }

    /// Returns all roots in canonical semantic-root order.
    pub fn roots(&self) -> &[ValidatedCompilerMultiRootProofRootV1] {
        &self.roots
    }

    /// Reports exact outer, roster, semantic, KIR, and nested-payload association.
    pub const fn has_exact_decoded_input_association(&self) -> bool {
        true
    }

    /// Reports deterministic replay of every retained semantic induction analysis.
    pub const fn has_replayed_semantic_induction(&self) -> bool {
        true
    }

    /// Reports that protected compiler origin remains a separate required join.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    /// Reports that source-side association establishes no LLVM or machine refinement.
    pub const fn establishes_llvm_or_machine_refinement(&self) -> bool {
        false
    }

    /// Reports that this owner grants no publication, load, or launch authority.
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Strictly decodes and cross-binds all source-side evidence for a true multi-root capsule.
#[allow(clippy::too_many_arguments)]
pub fn validate_compiler_multi_root_proof_inputs_v1(
    proof_binding: &InertProofBindingReceiptV3,
    semantic_mir: &InertCanonicalSemanticMirReceiptV3,
    middle_end: &InertMiddleEndReceiptV3,
    kernel_ir: &InertKernelIrReceiptV3,
    mir_to_kir_correspondence: &InertMirToKirCorrespondenceReceiptV3,
    formal_memory: &InertFormalMemoryReceiptV3,
) -> Result<ValidatedCompilerMultiRootProofInputsV1, CompilerMultiRootProofValidationErrorV1> {
    let association = InertProofBindingAssociationV4::decode(proof_binding.canonical_preimage())
        .map_err(CompilerMultiRootProofValidationErrorV1::ProofBindingDecode)?;
    let association_inputs = association.inputs();
    for (actual, sha256, byte_len, field) in [
        (
            association_inputs.semantic_mir(),
            semantic_mir.identity().sha256(),
            semantic_mir.identity().byte_len(),
            "semantic MIR",
        ),
        (
            association_inputs.middle_end(),
            middle_end.identity().sha256(),
            middle_end.identity().byte_len(),
            "middle end",
        ),
        (
            association_inputs.kernel_ir(),
            kernel_ir.identity().sha256(),
            kernel_ir.identity().byte_len(),
            "Kernel IR",
        ),
        (
            association_inputs.mir_to_kir_correspondence(),
            mir_to_kir_correspondence.identity().sha256(),
            mir_to_kir_correspondence.identity().byte_len(),
            "MIR-to-KIR correspondence",
        ),
        (
            association_inputs.formal_memory(),
            formal_memory.identity().sha256(),
            formal_memory.identity().byte_len(),
            "formal memory",
        ),
    ] {
        if !content_identity_matches(actual, sha256, byte_len) {
            return Err(
                CompilerMultiRootProofValidationErrorV1::ProofBindingIdentityMismatch { field },
            );
        }
    }

    let decoded_semantic_mir = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        semantic_mir.canonical_preimage(),
        SemanticMirLimitsV1::default(),
    )
    .map_err(CompilerMultiRootProofValidationErrorV1::SemanticMirDecode)?;
    let middle_end_roster =
        MultiRootProofRosterTranscriptV2::decode(middle_end.canonical_preimage())
            .map_err(CompilerMultiRootProofValidationErrorV1::MiddleEndRoster)?;
    let correspondence_roster =
        MultiRootProofRosterTranscriptV2::decode(mir_to_kir_correspondence.canonical_preimage())
            .map_err(CompilerMultiRootProofValidationErrorV1::CorrespondenceRoster)?;
    let formal_memory_roster =
        MultiRootProofRosterTranscriptV2::decode(formal_memory.canonical_preimage())
            .map_err(CompilerMultiRootProofValidationErrorV1::FormalMemoryRoster)?;
    let verus_roster =
        MultiRootProofRosterTranscriptV2::decode(association.verus_execution_evidence())
            .map_err(CompilerMultiRootProofValidationErrorV1::VerusRoster)?;
    validate_roster_set(
        &middle_end_roster,
        &correspondence_roster,
        &formal_memory_roster,
        &verus_roster,
    )?;

    if middle_end_roster.semantic_mir_sha256() != *decoded_semantic_mir.semantic_sha256().as_bytes()
        || middle_end_roster.root_count() != decoded_semantic_mir.roots().len()
    {
        return Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
            "proof roster differs from exact semantic MIR custody",
        ));
    }

    let neutral_identity = middle_end_roster.neutral_kir();
    let (decoded_kernel_ir, kernel_ir_module) = match neutral_identity.version() {
        MultiRootCanonicalKirVersionV2::V8 => {
            let (owner, module) = VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(
                kernel_ir.canonical_preimage().to_vec(),
            )
            .map_err(CompilerMultiRootProofValidationErrorV1::KernelIrV8)?;
            (ValidatedCompilerMultiRootKernelIrV1::V8(owner), module)
        }
        MultiRootCanonicalKirVersionV2::V9 => {
            let (owner, module) = VerifiedCanonicalKernelIrV9::from_canonical_bytes_with_module(
                kernel_ir.canonical_preimage().to_vec(),
            )
            .map_err(CompilerMultiRootProofValidationErrorV1::KernelIrV9)?;
            (ValidatedCompilerMultiRootKernelIrV1::V9(owner), module)
        }
    };
    if decoded_kernel_ir.identity_digest() != &neutral_identity.digest()
        || decoded_kernel_ir.canonical_length() != neutral_identity.canonical_length()
        || decoded_kernel_ir.version() != neutral_identity.version()
        || kernel_ir_module.kernels.len() != middle_end_roster.root_count()
    {
        return Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
            "proof roster differs from exact verified neutral Kernel IR custody",
        ));
    }

    let mut roots = Vec::new();
    roots
        .try_reserve_exact(middle_end_roster.root_count())
        .map_err(|_| {
            CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "multi-root proof owner allocation failed",
            )
        })?;
    for ordinal in 0..middle_end_roster.root_count() {
        let roster_root = middle_end_roster.root(ordinal).ok_or(
            CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "middle-end roster root is absent",
            ),
        )?;
        let correspondence_root = correspondence_roster.root(ordinal).ok_or(
            CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "correspondence roster root is absent",
            ),
        )?;
        let formal_root = formal_memory_roster.root(ordinal).ok_or(
            CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "formal-memory roster root is absent",
            ),
        )?;
        let verus_root = verus_roster.root(ordinal).ok_or(
            CompilerMultiRootProofValidationErrorV1::RosterMismatch("Verus roster root is absent"),
        )?;

        let semantic_root = decoded_semantic_mir.roots().get(ordinal).copied().ok_or(
            CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "semantic root is absent",
            },
        )?;
        let semantic_function = decoded_semantic_mir
            .functions()
            .get(semantic_root.index() as usize)
            .ok_or(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "semantic root function is absent",
            })?;
        let semantic_entry = semantic_function.kernel_entry().ok_or(
            CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "semantic root is not a kernel entry",
            },
        )?;
        let kernel = kernel_ir_module.kernels.get(ordinal).ok_or(
            CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "Kernel IR root is absent",
            },
        )?;
        let kernel_entry = kernel_ir_module.function(&kernel.entry).ok_or(
            CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "Kernel IR entry function is absent",
            },
        )?;
        if roster_root.semantic_root() != semantic_root.index()
            || roster_root.semantic_root_identity() != *semantic_function.identity().as_bytes()
            || roster_root.kernel_binding() != *semantic_entry.kernel_binding_identity().as_bytes()
            || roster_root.export_symbol().as_bytes() != semantic_entry.export_symbol().as_bytes()
            || roster_root.export_symbol() != roster_root.kernel_id()
            || kernel.id.as_str() != roster_root.kernel_id()
            || kernel.entry.as_str() != roster_root.kernel_id()
            || kernel.domain.rank() != roster_root.source_rank()
            || kernel_entry.role != FunctionRole::KernelEntry
        {
            return Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "semantic, roster, and neutral Kernel IR root axes differ",
            });
        }

        let decoded_middle_end = InertProductionMiddleEndEvidenceV5::decode(roster_root.payload())
            .map_err(
                |source| CompilerMultiRootProofValidationErrorV1::MiddleEndPayload {
                    root: ordinal,
                    source,
                },
            )?;
        if decoded_middle_end.source_semantic_identity()
            != decoded_semantic_mir.semantic_sha256().as_bytes()
        {
            return Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "middle-end payload names a different semantic MIR",
            });
        }

        let semantic_u32_induction = decode_and_validate_correspondence(
            correspondence_root.payload(),
            ordinal,
            roster_root,
            &decoded_semantic_mir,
            &kernel_ir_module,
        )?;
        let selection = decoded_semantic_mir
            .select_kernel_body_for_root_v1(semantic_root)
            .ok_or(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "semantic kernel body selection failed",
            })?;
        let selected_function = decoded_semantic_mir
            .functions()
            .get(selection.body().index() as usize)
            .ok_or(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "selected semantic kernel body is absent",
            })?;
        if semantic_u32_induction.semantic_mir_sha256()
            != decoded_semantic_mir.semantic_sha256().as_bytes()
            || semantic_u32_induction.function() != selection.body().index()
            || semantic_u32_induction.function_identity() != selected_function.identity().as_bytes()
            || semantic_u32_induction.grants_authority()
        {
            return Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "semantic induction payload changed its exact root owner",
            });
        }
        let induction_replay = analyze_semantic_u32_induction_no_overflow_v1(
            &decoded_semantic_mir,
            SemanticFunctionIdV1::from_index(semantic_u32_induction.function()),
        )
        .map_err(|source| {
            CompilerMultiRootProofValidationErrorV1::SemanticInductionAnalysis {
                root: ordinal,
                source,
            }
        })?;
        let canonical_replay =
            InertCanonicalSemanticU32InductionEvidenceV1::from_report(&induction_replay).map_err(
                |source| CompilerMultiRootProofValidationErrorV1::SemanticInductionEvidence {
                    root: ordinal,
                    source,
                },
            )?;
        if canonical_replay.canonical_bytes() != semantic_u32_induction.canonical_bytes() {
            return Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "semantic induction payload differs from deterministic replay",
            });
        }

        let decoded_formal = InertCanonicalFormalMemoryObligationReceiptV1::from_canonical_bytes(
            formal_root.payload().to_vec(),
        )
        .map_err(|source| {
            CompilerMultiRootProofValidationErrorV1::FormalMemoryPayload {
                root: ordinal,
                source,
            }
        })?;
        if decoded_formal.kernel_id() != roster_root.kernel_id()
            || decoded_formal.entry_id() != roster_root.kernel_id()
        {
            return Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "formal-memory payload names a different kernel or entry",
            });
        }

        let decoded_verus =
            CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(verus_root.payload())
                .map_err(
                    |source| CompilerMultiRootProofValidationErrorV1::VerusPayload {
                        root: ordinal,
                        source,
                    },
                )?;
        if decoded_verus.claims().pliron_evidence_identity().as_bytes()
            != decoded_middle_end.identity().sha256()
        {
            return Err(CompilerMultiRootProofValidationErrorV1::RootMismatch {
                root: ordinal,
                detail: "signed Verus payload names a different middle-end record",
            });
        }

        roots.push(ValidatedCompilerMultiRootProofRootV1 {
            semantic_root: roster_root.semantic_root(),
            semantic_root_identity: roster_root.semantic_root_identity(),
            kernel_binding: roster_root.kernel_binding(),
            source_rank: roster_root.source_rank(),
            workgroup: roster_root.workgroup(),
            logical_name: roster_root.logical_name().into(),
            export_symbol: roster_root.export_symbol().into(),
            kernel_id: roster_root.kernel_id().into(),
            middle_end: decoded_middle_end,
            semantic_u32_induction,
            formal_memory: decoded_formal,
            verus_execution: decoded_verus,
        });
    }

    if derive_ranked_roster_identity(&roots, middle_end_roster.canonical_kernel_order())?
        != middle_end_roster.roster_identity()
    {
        return Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
            "ranked roster identity does not rederive from exact nested evidence",
        ));
    }

    Ok(ValidatedCompilerMultiRootProofInputsV1 {
        association,
        receipt_identity: proof_binding.identity(),
        semantic_mir: decoded_semantic_mir,
        kernel_ir: decoded_kernel_ir,
        kernel_ir_module,
        middle_end_roster,
        correspondence_roster,
        formal_memory_roster,
        verus_roster,
        roots: roots.into_boxed_slice(),
    })
}

fn content_identity_matches(
    actual: InertLineageContentIdentityV3,
    sha256: &[u8; 32],
    byte_len: u64,
) -> bool {
    actual.sha256() == *sha256 && actual.byte_len() == byte_len
}

fn validate_roster_set(
    middle_end: &MultiRootProofRosterTranscriptV2,
    correspondence: &MultiRootProofRosterTranscriptV2,
    formal_memory: &MultiRootProofRosterTranscriptV2,
    verus: &MultiRootProofRosterTranscriptV2,
) -> Result<(), CompilerMultiRootProofValidationErrorV1> {
    for (actual, expected, field) in [
        (
            middle_end.kind(),
            MultiRootProofRosterKindV2::MiddleEnd,
            "middle-end roster kind",
        ),
        (
            correspondence.kind(),
            MultiRootProofRosterKindV2::Correspondence,
            "correspondence roster kind",
        ),
        (
            formal_memory.kind(),
            MultiRootProofRosterKindV2::FormalMemory,
            "formal-memory roster kind",
        ),
        (
            verus.kind(),
            MultiRootProofRosterKindV2::VerusExecution,
            "Verus roster kind",
        ),
    ] {
        if actual != expected {
            return Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                field,
            ));
        }
    }
    for roster in [correspondence, formal_memory, verus] {
        if roster.semantic_mir_sha256() != middle_end.semantic_mir_sha256()
            || roster.neutral_kir() != middle_end.neutral_kir()
            || roster.roster_identity() != middle_end.roster_identity()
            || roster.canonical_kernel_order() != middle_end.canonical_kernel_order()
            || roster.root_count() != middle_end.root_count()
        {
            return Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "multi-root proof roster headers differ",
            ));
        }
        for index in 0..middle_end.root_count() {
            let expected = middle_end.root(index).ok_or(
                CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                    "middle-end roster root is absent",
                ),
            )?;
            let actual = roster.root(index).ok_or(
                CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                    "parallel roster root is absent",
                ),
            )?;
            if actual.semantic_root() != expected.semantic_root()
                || actual.semantic_root_identity() != expected.semantic_root_identity()
                || actual.kernel_binding() != expected.kernel_binding()
                || actual.source_rank() != expected.source_rank()
                || actual.workgroup() != expected.workgroup()
                || actual.logical_name() != expected.logical_name()
                || actual.export_symbol() != expected.export_symbol()
                || actual.kernel_id() != expected.kernel_id()
            {
                return Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                    "multi-root proof roster root metadata differs",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct CorrespondenceFunctionBindingV1<'a> {
    semantic: &'a SemanticFunctionDeclV1,
    body: &'a FunctionBody,
}

#[derive(Clone, Copy)]
struct CorrespondenceOperationSpanV1 {
    kernel_ir_block: u32,
    first_operation: u32,
    operation_count: u32,
}

#[derive(Clone, Copy)]
struct CorrespondenceSyntheticSpanV1 {
    rule: u8,
    first_operation: u32,
    operation_count: u32,
}

fn decode_and_validate_correspondence(
    bytes: &[u8],
    expected_ordinal: usize,
    root: MultiRootProofRosterRootV2<'_>,
    semantic_mir: &AdmittedInertSemanticMirV1,
    kernel_ir: &Module,
) -> Result<InertCanonicalSemanticU32InductionEvidenceV1, CompilerMultiRootProofValidationErrorV1> {
    let mut reader = CorrespondenceReaderV1::new(bytes);
    let ordinal = u32::try_from(expected_ordinal).map_err(|_| {
        CompilerMultiRootProofValidationErrorV1::CorrespondencePayload {
            root: expected_ordinal,
            detail: "root ordinal does not fit the correspondence wire format",
        }
    })?;
    if reader.fixed::<8>(expected_ordinal)? != CORRESPONDENCE_MAGIC_V1
        || reader.u16(expected_ordinal)? != CORRESPONDENCE_VERSION_V1
        || reader.u16(expected_ordinal)? != CORRESPONDENCE_POLICY_V1
        || reader.u32(expected_ordinal)? != ordinal
        || reader.u32(expected_ordinal)? != root.semantic_root()
    {
        return Err(
            CompilerMultiRootProofValidationErrorV1::CorrespondencePayload {
                root: expected_ordinal,
                detail: "correspondence payload header is cross-wired",
            },
        );
    }
    let induction =
        InertCanonicalSemanticU32InductionEvidenceV1::decode(reader.bytes(expected_ordinal)?)
            .map_err(|source| {
                CompilerMultiRootProofValidationErrorV1::SemanticInductionEvidence {
                    root: expected_ordinal,
                    source,
                }
            })?;

    let semantic_root = SemanticFunctionIdV1::from_index(root.semantic_root());
    let selected = semantic_mir
        .select_kernel_body_for_root_v1(semantic_root)
        .ok_or_else(|| correspondence_error(expected_ordinal, "semantic root has no body"))?;
    let function_count = reader.count(expected_ordinal)?;
    if function_count == 0
        || function_count > semantic_mir.functions().len()
        || function_count > kernel_ir.functions.len()
    {
        return Err(correspondence_error(
            expected_ordinal,
            "correspondence function count is invalid",
        ));
    }
    let mut functions = BTreeMap::new();
    let mut kernel_symbols = BTreeSet::new();
    let mut entries = 0_usize;
    for _ in 0..function_count {
        let semantic_function_id = reader.u32(expected_ordinal)?;
        let role = reader.u8(expected_ordinal)?;
        if !matches!(role, 1 | 2) {
            return Err(correspondence_error(
                expected_ordinal,
                "correspondence payload has an invalid function role",
            ));
        }
        let symbol = std::str::from_utf8(reader.bytes(expected_ordinal)?).map_err(|_| {
            correspondence_error(
                expected_ordinal,
                "correspondence function symbol is not UTF-8",
            )
        })?;
        let semantic = semantic_mir
            .functions()
            .get(semantic_function_id as usize)
            .ok_or_else(|| {
                correspondence_error(
                    expected_ordinal,
                    "correspondence names an absent semantic function",
                )
            })?;
        let target = kernel_ir
            .functions
            .iter()
            .find(|function| function.id.as_str() == symbol)
            .ok_or_else(|| {
                correspondence_error(
                    expected_ordinal,
                    "correspondence names an absent Kernel IR function",
                )
            })?;
        let body = target.body.as_ref().ok_or_else(|| {
            correspondence_error(
                expected_ordinal,
                "correspondence names a Kernel IR declaration",
            )
        })?;
        let expected_role = if role == 1 {
            FunctionRole::KernelEntry
        } else {
            FunctionRole::InternalHelper
        };
        if target.role != expected_role
            || !kernel_symbols.insert(symbol)
            || functions
                .insert(
                    semantic_function_id,
                    CorrespondenceFunctionBindingV1 { semantic, body },
                )
                .is_some()
        {
            return Err(correspondence_error(
                expected_ordinal,
                "correspondence function roster is cross-wired or duplicated",
            ));
        }
        if role == 1 {
            entries += 1;
            if semantic_function_id != selected.body().index() || symbol != root.kernel_id() {
                return Err(correspondence_error(
                    expected_ordinal,
                    "correspondence entry names a different semantic body or kernel",
                ));
            }
        }
    }
    if entries != 1 {
        return Err(correspondence_error(
            expected_ordinal,
            "correspondence payload does not have one exact entry",
        ));
    }

    let expected_block_count = functions.values().try_fold(0_usize, |total, function| {
        total.checked_add(function.semantic.blocks().len())
    });
    let expected_block_count = expected_block_count
        .ok_or_else(|| correspondence_error(expected_ordinal, "semantic block count overflows"))?;
    let block_count = reader.count(expected_ordinal)?;
    if block_count == 0 || block_count != expected_block_count {
        return Err(correspondence_error(
            expected_ordinal,
            "correspondence block coverage differs from semantic MIR",
        ));
    }
    let mut semantic_to_kir = BTreeMap::new();
    let mut mapped_kir_blocks = BTreeSet::new();
    let mut kir_blocks = BTreeMap::<(u32, u32), &BasicBlock>::new();
    for (&semantic_function, binding) in &functions {
        for block in &binding.body.blocks {
            if kir_blocks
                .insert((semantic_function, block.id.0), block)
                .is_some()
            {
                return Err(correspondence_error(
                    expected_ordinal,
                    "Kernel IR block identities are duplicated",
                ));
            }
        }
    }
    for _ in 0..block_count {
        let semantic_function = reader.u32(expected_ordinal)?;
        let semantic_block = reader.u32(expected_ordinal)?;
        let kernel_ir_block = reader.u32(expected_ordinal)?;
        let source_statement_count = reader.u32(expected_ordinal)?;
        let binding = functions.get(&semantic_function).ok_or_else(|| {
            correspondence_error(
                expected_ordinal,
                "block correspondence names an unbound semantic function",
            )
        })?;
        let source = binding
            .semantic
            .blocks()
            .get(semantic_block as usize)
            .ok_or_else(|| {
                correspondence_error(
                    expected_ordinal,
                    "block correspondence names an absent semantic block",
                )
            })?;
        if usize::try_from(source_statement_count) != Ok(source.statements().len())
            || !kir_blocks.contains_key(&(semantic_function, kernel_ir_block))
            || !mapped_kir_blocks.insert((semantic_function, kernel_ir_block))
            || semantic_to_kir
                .insert((semantic_function, semantic_block), kernel_ir_block)
                .is_some()
        {
            return Err(correspondence_error(
                expected_ordinal,
                "block correspondence is incomplete, duplicated, or cross-wired",
            ));
        }
    }
    for (&semantic_function, binding) in &functions {
        for semantic_block in 0..binding.semantic.blocks().len() {
            let semantic_block = u32::try_from(semantic_block).map_err(|_| {
                correspondence_error(
                    expected_ordinal,
                    "semantic block index does not fit correspondence",
                )
            })?;
            if !semantic_to_kir.contains_key(&(semantic_function, semantic_block)) {
                return Err(correspondence_error(
                    expected_ordinal,
                    "semantic block has no exact Kernel IR mapping",
                ));
            }
        }
    }

    let expected_statement_count = functions.values().try_fold(0_usize, |total, function| {
        function
            .semantic
            .blocks()
            .iter()
            .try_fold(total, |total, block| {
                total.checked_add(block.statements().len())
            })
    });
    let expected_statement_count = expected_statement_count.ok_or_else(|| {
        correspondence_error(expected_ordinal, "semantic statement count overflows")
    })?;
    let statement_count = reader.count(expected_ordinal)?;
    if statement_count != expected_statement_count {
        return Err(correspondence_error(
            expected_ordinal,
            "statement-span coverage differs from semantic MIR",
        ));
    }
    let mut statement_spans = BTreeMap::new();
    for _ in 0..statement_count {
        let semantic_function = reader.u32(expected_ordinal)?;
        let semantic_block = reader.u32(expected_ordinal)?;
        let statement = reader.u32(expected_ordinal)?;
        let span = CorrespondenceOperationSpanV1 {
            kernel_ir_block: reader.u32(expected_ordinal)?,
            first_operation: reader.u32(expected_ordinal)?,
            operation_count: reader.u32(expected_ordinal)?,
        };
        if !semantic_to_kir.contains_key(&(semantic_function, semantic_block))
            || statement_spans
                .insert((semantic_function, semantic_block, statement), span)
                .is_some()
        {
            return Err(correspondence_error(
                expected_ordinal,
                "statement-span roster is duplicated or cross-wired",
            ));
        }
    }

    let terminator_count = reader.count(expected_ordinal)?;
    if terminator_count != expected_block_count {
        return Err(correspondence_error(
            expected_ordinal,
            "terminator-span coverage differs from semantic MIR",
        ));
    }
    let mut terminator_spans = BTreeMap::new();
    for _ in 0..terminator_count {
        let semantic_function = reader.u32(expected_ordinal)?;
        let semantic_block = reader.u32(expected_ordinal)?;
        let span = CorrespondenceOperationSpanV1 {
            kernel_ir_block: reader.u32(expected_ordinal)?,
            first_operation: reader.u32(expected_ordinal)?,
            operation_count: reader.u32(expected_ordinal)?,
        };
        if !semantic_to_kir.contains_key(&(semantic_function, semantic_block))
            || terminator_spans
                .insert((semantic_function, semantic_block), span)
                .is_some()
        {
            return Err(correspondence_error(
                expected_ordinal,
                "terminator-span roster is duplicated or cross-wired",
            ));
        }
    }

    let synthetic_count = reader.count(expected_ordinal)?;
    let total_kir_blocks = functions.values().try_fold(0_usize, |total, function| {
        total.checked_add(function.body.blocks.len())
    });
    if synthetic_count
        > total_kir_blocks.ok_or_else(|| {
            correspondence_error(expected_ordinal, "Kernel IR block count overflows")
        })?
    {
        return Err(correspondence_error(
            expected_ordinal,
            "synthetic-span count exceeds Kernel IR block coverage",
        ));
    }
    let mut synthetic_spans = BTreeMap::new();
    for _ in 0..synthetic_count {
        let semantic_function = reader.u32(expected_ordinal)?;
        let rule = reader.u8(expected_ordinal)?;
        let kernel_ir_block = reader.u32(expected_ordinal)?;
        let span = CorrespondenceSyntheticSpanV1 {
            rule,
            first_operation: reader.u32(expected_ordinal)?,
            operation_count: reader.u32(expected_ordinal)?,
        };
        if !matches!(rule, 1 | 2)
            || !kir_blocks.contains_key(&(semantic_function, kernel_ir_block))
            || synthetic_spans
                .insert((semantic_function, kernel_ir_block), span)
                .is_some()
        {
            return Err(correspondence_error(
                expected_ordinal,
                "synthetic-span roster is invalid or duplicated",
            ));
        }
    }

    let expected_parameter_count = functions.values().try_fold(0_usize, |total, function| {
        let arguments = function
            .semantic
            .locals()
            .iter()
            .filter(|local| matches!(local.role(), SemanticLocalRoleV1::Argument(_)))
            .count();
        total.checked_add(arguments)
    });
    let expected_parameter_count = expected_parameter_count.ok_or_else(|| {
        correspondence_error(expected_ordinal, "semantic argument count overflows")
    })?;
    let parameter_count = reader.count(expected_ordinal)?;
    if parameter_count != expected_parameter_count {
        return Err(correspondence_error(
            expected_ordinal,
            "parameter-binding coverage differs from semantic MIR",
        ));
    }
    let mut parameter_bindings = BTreeMap::new();
    for _ in 0..parameter_count {
        let semantic_function = reader.u32(expected_ordinal)?;
        let semantic_local = reader.u32(expected_ordinal)?;
        let kernel_ir_value = reader.u32(expected_ordinal)?;
        if !functions.contains_key(&semantic_function)
            || parameter_bindings
                .insert((semantic_function, semantic_local), kernel_ir_value)
                .is_some()
        {
            return Err(correspondence_error(
                expected_ordinal,
                "parameter-binding roster is duplicated or cross-wired",
            ));
        }
    }
    if !reader.is_finished() {
        return Err(correspondence_error(
            expected_ordinal,
            "correspondence payload has trailing bytes",
        ));
    }

    validate_correspondence_operation_coverage(
        expected_ordinal,
        &functions,
        &semantic_to_kir,
        &kir_blocks,
        &statement_spans,
        &terminator_spans,
        synthetic_spans,
    )?;
    validate_correspondence_parameter_bindings(expected_ordinal, &functions, &parameter_bindings)?;
    validate_correspondence_induction_anchors(
        expected_ordinal,
        &induction,
        &functions,
        &semantic_to_kir,
        &kir_blocks,
        &statement_spans,
    )?;
    Ok(induction)
}

#[allow(clippy::too_many_arguments)]
fn validate_correspondence_operation_coverage(
    root: usize,
    functions: &BTreeMap<u32, CorrespondenceFunctionBindingV1<'_>>,
    semantic_to_kir: &BTreeMap<(u32, u32), u32>,
    kir_blocks: &BTreeMap<(u32, u32), &BasicBlock>,
    statement_spans: &BTreeMap<(u32, u32, u32), CorrespondenceOperationSpanV1>,
    terminator_spans: &BTreeMap<(u32, u32), CorrespondenceOperationSpanV1>,
    mut synthetic_spans: BTreeMap<(u32, u32), CorrespondenceSyntheticSpanV1>,
) -> Result<(), CompilerMultiRootProofValidationErrorV1> {
    let mut runtime_assert_blocks = 0_usize;
    for (&semantic_function, binding) in functions {
        let mut mapped_kir_blocks = BTreeSet::new();
        for (semantic_block, source) in binding.semantic.blocks().iter().enumerate() {
            let semantic_block = u32::try_from(semantic_block).map_err(|_| {
                correspondence_error(root, "semantic block index does not fit correspondence")
            })?;
            let kernel_ir_block = *semantic_to_kir
                .get(&(semantic_function, semantic_block))
                .ok_or_else(|| {
                    correspondence_error(root, "semantic block has no Kernel IR mapping")
                })?;
            mapped_kir_blocks.insert(kernel_ir_block);
            let target = *kir_blocks
                .get(&(semantic_function, kernel_ir_block))
                .ok_or_else(|| correspondence_error(root, "mapped Kernel IR block is absent"))?;
            let mut next_operation = 0_usize;
            if let Some(synthetic) = synthetic_spans.remove(&(semantic_function, kernel_ir_block)) {
                if synthetic.rule != 1
                    || synthetic.first_operation != 0
                    || synthetic.operation_count == 0
                {
                    return Err(correspondence_error(
                        root,
                        "mapped Kernel IR block has a noncanonical synthetic prologue",
                    ));
                }
                next_operation = checked_span_end(
                    root,
                    synthetic.first_operation,
                    synthetic.operation_count,
                    target.operations.len(),
                )?;
                if !target.operations[..next_operation].iter().all(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Alloca {
                            address_space: AddressSpace::Private,
                            ..
                        } | OperationKind::Load {
                            access: MemoryAccess {
                                address_space: AddressSpace::Private,
                                ..
                            },
                            ..
                        }
                    )
                }) {
                    return Err(correspondence_error(
                        root,
                        "enum-payload prologue contains a non-private-storage operation",
                    ));
                }
            }

            for statement in 0..source.statements().len() {
                let statement = u32::try_from(statement).map_err(|_| {
                    correspondence_error(
                        root,
                        "semantic statement index does not fit correspondence",
                    )
                })?;
                let span = statement_spans
                    .get(&(semantic_function, semantic_block, statement))
                    .ok_or_else(|| {
                        correspondence_error(root, "semantic statement has no exact operation span")
                    })?;
                if span.kernel_ir_block != kernel_ir_block
                    || usize::try_from(span.first_operation) != Ok(next_operation)
                {
                    return Err(correspondence_error(
                        root,
                        "semantic statement spans are not contiguous in their Kernel IR block",
                    ));
                }
                next_operation = checked_span_end(
                    root,
                    span.first_operation,
                    span.operation_count,
                    target.operations.len(),
                )?;
            }
            let terminator = terminator_spans
                .get(&(semantic_function, semantic_block))
                .ok_or_else(|| {
                    correspondence_error(root, "semantic terminator has no exact operation span")
                })?;
            if terminator.kernel_ir_block != kernel_ir_block
                || usize::try_from(terminator.first_operation) != Ok(next_operation)
            {
                return Err(correspondence_error(
                    root,
                    "semantic terminator span is not contiguous in its Kernel IR block",
                ));
            }
            next_operation = checked_span_end(
                root,
                terminator.first_operation,
                terminator.operation_count,
                target.operations.len(),
            )?;
            if next_operation != target.operations.len() || target.terminator.is_none() {
                return Err(correspondence_error(
                    root,
                    "semantic spans do not cover the complete mapped Kernel IR block",
                ));
            }
        }

        for target in &binding.body.blocks {
            if mapped_kir_blocks.contains(&target.id.0) {
                continue;
            }
            let synthetic = synthetic_spans
                .remove(&(semantic_function, target.id.0))
                .ok_or_else(|| {
                    correspondence_error(root, "unmapped Kernel IR block has no synthetic custody")
                })?;
            if synthetic.rule != 2
                || synthetic.first_operation != 0
                || synthetic.operation_count != 1
                || target.operations.as_slice() != [AmdGpuDiagnosticOperation::Trap.operation(None)]
                || !matches!(target.terminator, Some(Terminator::Unreachable))
            {
                return Err(correspondence_error(
                    root,
                    "unmapped Kernel IR block is not the canonical runtime-assert trap",
                ));
            }
            runtime_assert_blocks = runtime_assert_blocks.checked_add(1).ok_or_else(|| {
                correspondence_error(root, "runtime-assert block count overflows")
            })?;
        }
    }
    if !synthetic_spans.is_empty() || runtime_assert_blocks > 1 {
        return Err(correspondence_error(
            root,
            "synthetic operation-span coverage is incomplete or noncanonical",
        ));
    }
    Ok(())
}

fn validate_correspondence_parameter_bindings(
    root: usize,
    functions: &BTreeMap<u32, CorrespondenceFunctionBindingV1<'_>>,
    parameter_bindings: &BTreeMap<(u32, u32), u32>,
) -> Result<(), CompilerMultiRootProofValidationErrorV1> {
    let mut expected_bindings = 0_usize;
    for (&semantic_function, binding) in functions {
        let mut arguments = binding
            .semantic
            .locals()
            .iter()
            .enumerate()
            .filter_map(|(local, declaration)| match declaration.role() {
                SemanticLocalRoleV1::Argument(argument) => Some((argument, local)),
                SemanticLocalRoleV1::Return | SemanticLocalRoleV1::Temporary => None,
            })
            .collect::<Vec<_>>();
        arguments.sort_unstable();
        if arguments.len() != binding.body.parameters.len()
            || arguments
                .iter()
                .enumerate()
                .any(|(expected, (actual, _))| usize::try_from(*actual) != Ok(expected))
        {
            return Err(correspondence_error(
                root,
                "semantic argument roster differs from Kernel IR parameters",
            ));
        }
        for (argument, (_, local)) in arguments.iter().enumerate() {
            let local = u32::try_from(*local).map_err(|_| {
                correspondence_error(root, "semantic argument local does not fit correspondence")
            })?;
            if parameter_bindings.get(&(semantic_function, local))
                != Some(&binding.body.parameters[argument].0)
            {
                return Err(correspondence_error(
                    root,
                    "semantic argument names a different Kernel IR parameter",
                ));
            }
        }
        expected_bindings = expected_bindings
            .checked_add(arguments.len())
            .ok_or_else(|| correspondence_error(root, "parameter-binding count overflows"))?;
    }
    if parameter_bindings.len() != expected_bindings {
        return Err(correspondence_error(
            root,
            "parameter-binding coverage is not exact",
        ));
    }
    Ok(())
}

fn validate_correspondence_induction_anchors(
    root: usize,
    induction: &InertCanonicalSemanticU32InductionEvidenceV1,
    functions: &BTreeMap<u32, CorrespondenceFunctionBindingV1<'_>>,
    semantic_to_kir: &BTreeMap<(u32, u32), u32>,
    kir_blocks: &BTreeMap<(u32, u32), &BasicBlock>,
    statement_spans: &BTreeMap<(u32, u32, u32), CorrespondenceOperationSpanV1>,
) -> Result<(), CompilerMultiRootProofValidationErrorV1> {
    let function = functions.get(&induction.function()).ok_or_else(|| {
        correspondence_error(
            root,
            "semantic induction function has no correspondence binding",
        )
    })?;
    for certificate in induction.certificates() {
        let site = certificate.checked_addition();
        let semantic_block = site.block().block();
        let semantic_statement = site.statement();
        let statement = function
            .semantic
            .blocks()
            .get(semantic_block as usize)
            .and_then(|block| block.statements().get(semantic_statement as usize))
            .ok_or_else(|| {
                correspondence_error(root, "semantic induction statement site is absent")
            })?;
        if !matches!(
            statement.kind(),
            SemanticStatementKindV1::Assign(assignment)
                if matches!(
                    assignment.value().kind(),
                    SemanticRvalueKindV1::CheckedBinary(checked)
                        if checked.operation() == SemanticCheckedBinaryOpV1::Add
                )
        ) {
            return Err(correspondence_error(
                root,
                "semantic induction certificate does not name a checked addition",
            ));
        }
        let span = statement_spans
            .get(&(induction.function(), semantic_block, semantic_statement))
            .ok_or_else(|| {
                correspondence_error(root, "semantic induction statement has no operation span")
            })?;
        let kernel_ir_block = *semantic_to_kir
            .get(&(induction.function(), semantic_block))
            .ok_or_else(|| {
                correspondence_error(root, "semantic induction block has no Kernel IR mapping")
            })?;
        if span.kernel_ir_block != kernel_ir_block {
            return Err(correspondence_error(
                root,
                "semantic induction statement names a different Kernel IR block",
            ));
        }
        let block = *kir_blocks
            .get(&(induction.function(), kernel_ir_block))
            .ok_or_else(|| correspondence_error(root, "induction Kernel IR block is absent"))?;
        let first = usize::try_from(span.first_operation).map_err(|_| {
            correspondence_error(root, "induction operation ordinal does not fit this host")
        })?;
        let end = checked_span_end(
            root,
            span.first_operation,
            span.operation_count,
            block.operations.len(),
        )?;
        let mut checked_additions = 0_usize;
        for operation in &block.operations[first..end] {
            if !matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                    ..
                }
            ) {
                continue;
            }
            if operation.results.len() != 2 {
                return Err(correspondence_error(
                    root,
                    "checked Kernel IR addition does not have value and overflow results",
                ));
            }
            checked_additions = checked_additions.checked_add(1).ok_or_else(|| {
                correspondence_error(root, "checked Kernel IR addition count overflows")
            })?;
        }
        if checked_additions != 1 {
            return Err(correspondence_error(
                root,
                "induction span does not contain one exact checked Kernel IR addition",
            ));
        }
    }
    Ok(())
}

fn checked_span_end(
    root: usize,
    first: u32,
    count: u32,
    operation_count: usize,
) -> Result<usize, CompilerMultiRootProofValidationErrorV1> {
    let first = usize::try_from(first)
        .map_err(|_| correspondence_error(root, "operation-span start does not fit this host"))?;
    let count = usize::try_from(count)
        .map_err(|_| correspondence_error(root, "operation-span count does not fit this host"))?;
    let end = first
        .checked_add(count)
        .ok_or_else(|| correspondence_error(root, "operation span overflows"))?;
    if end > operation_count {
        return Err(correspondence_error(
            root,
            "operation span exceeds its Kernel IR block",
        ));
    }
    Ok(end)
}

const fn correspondence_error(
    root: usize,
    detail: &'static str,
) -> CompilerMultiRootProofValidationErrorV1 {
    CompilerMultiRootProofValidationErrorV1::CorrespondencePayload { root, detail }
}

fn derive_ranked_roster_identity(
    roots: &[ValidatedCompilerMultiRootProofRootV1],
    canonical_kernel_order: &[u32],
) -> Result<[u8; 32], CompilerMultiRootProofValidationErrorV1> {
    if roots.len() != canonical_kernel_order.len() {
        return Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
            "canonical kernel order has the wrong length",
        ));
    }
    let mut digest = Sha256::new();
    digest.update(RANKED_ROSTER_IDENTITY_DOMAIN_V1);
    digest.update(
        u64::try_from(roots.len())
            .map_err(|_| {
                CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                    "root count does not fit roster identity",
                )
            })?
            .to_le_bytes(),
    );
    for &index in canonical_kernel_order {
        let root = roots.get(index as usize).ok_or(
            CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "canonical kernel order names an absent root",
            ),
        )?;
        update_roster_identity_frame(&mut digest, &root.kernel_binding)?;
        update_roster_identity_frame(&mut digest, root.logical_name.as_bytes())?;
        update_roster_identity_frame(&mut digest, root.export_symbol.as_bytes())?;
        update_roster_identity_frame(&mut digest, &root.semantic_root.to_le_bytes())?;
        update_roster_identity_frame(&mut digest, &root.semantic_root_identity)?;
        update_roster_identity_frame(&mut digest, &[root.source_rank])?;
        update_roster_identity_frame(&mut digest, root.middle_end.identity().sha256())?;
        update_roster_identity_frame(
            &mut digest,
            &root.middle_end.identity().byte_len().to_le_bytes(),
        )?;
        update_roster_identity_frame(
            &mut digest,
            root.semantic_u32_induction.semantic_mir_sha256(),
        )?;
        update_roster_identity_frame(
            &mut digest,
            &root.semantic_u32_induction.function().to_le_bytes(),
        )?;
        update_roster_identity_frame(&mut digest, root.semantic_u32_induction.function_identity())?;
        update_roster_identity_frame(
            &mut digest,
            &u64::from(root.semantic_u32_induction.checked_additions_examined()).to_le_bytes(),
        )?;
        update_roster_identity_frame(
            &mut digest,
            &u64::try_from(root.semantic_u32_induction.certificates().len())
                .map_err(|_| {
                    CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                        "induction certificate count does not fit roster identity",
                    )
                })?
                .to_le_bytes(),
        )?;
        update_roster_identity_frame(
            &mut digest,
            &root.semantic_u32_induction.work_units().to_le_bytes(),
        )?;
    }
    Ok(digest.finalize().into())
}

fn update_roster_identity_frame(
    digest: &mut Sha256,
    frame: &[u8],
) -> Result<(), CompilerMultiRootProofValidationErrorV1> {
    digest.update(
        u64::try_from(frame.len())
            .map_err(|_| {
                CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                    "roster identity frame length overflows",
                )
            })?
            .to_le_bytes(),
    );
    digest.update(frame);
    Ok(())
}

struct CorrespondenceReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> CorrespondenceReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
        root: usize,
    ) -> Result<&'a [u8], CompilerMultiRootProofValidationErrorV1> {
        let end = self.offset.checked_add(length).ok_or(
            CompilerMultiRootProofValidationErrorV1::CorrespondencePayload {
                root,
                detail: "correspondence payload length overflows",
            },
        )?;
        let value = self.bytes.get(self.offset..end).ok_or(
            CompilerMultiRootProofValidationErrorV1::CorrespondencePayload {
                root,
                detail: "correspondence payload is truncated",
            },
        )?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
        root: usize,
    ) -> Result<[u8; N], CompilerMultiRootProofValidationErrorV1> {
        self.take(N, root)?.try_into().map_err(|_| {
            CompilerMultiRootProofValidationErrorV1::CorrespondencePayload {
                root,
                detail: "correspondence payload field is truncated",
            }
        })
    }

    fn u8(&mut self, root: usize) -> Result<u8, CompilerMultiRootProofValidationErrorV1> {
        Ok(self.fixed::<1>(root)?[0])
    }

    fn u16(&mut self, root: usize) -> Result<u16, CompilerMultiRootProofValidationErrorV1> {
        Ok(u16::from_le_bytes(self.fixed(root)?))
    }

    fn u32(&mut self, root: usize) -> Result<u32, CompilerMultiRootProofValidationErrorV1> {
        Ok(u32::from_le_bytes(self.fixed(root)?))
    }

    fn count(&mut self, root: usize) -> Result<usize, CompilerMultiRootProofValidationErrorV1> {
        usize::try_from(self.u32(root)?).map_err(|_| {
            CompilerMultiRootProofValidationErrorV1::CorrespondencePayload {
                root,
                detail: "correspondence payload count does not fit this host",
            }
        })
    }

    fn bytes(&mut self, root: usize) -> Result<&'a [u8], CompilerMultiRootProofValidationErrorV1> {
        let length = self.count(root)?;
        if length == 0 || length > self.bytes.len() {
            return Err(
                CompilerMultiRootProofValidationErrorV1::CorrespondencePayload {
                    root,
                    detail: "correspondence payload has an invalid framed length",
                },
            );
        }
        self.take(length, root)
    }

    const fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

/// Fail-closed independent multi-root proof validation error.
#[derive(Debug)]
pub enum CompilerMultiRootProofValidationErrorV1 {
    ProofBindingDecode(InertProofBindingAssociationErrorV4),
    ProofBindingIdentityMismatch {
        field: &'static str,
    },
    SemanticMirDecode(SemanticMirDecodeErrorV1),
    MiddleEndRoster(MultiRootProofRosterErrorV2),
    CorrespondenceRoster(MultiRootProofRosterErrorV2),
    FormalMemoryRoster(MultiRootProofRosterErrorV2),
    VerusRoster(MultiRootProofRosterErrorV2),
    KernelIrV8(VerifiedCanonicalKernelIrErrorV8),
    KernelIrV9(VerifiedCanonicalKernelIrErrorV9),
    RosterMismatch(&'static str),
    RootMismatch {
        root: usize,
        detail: &'static str,
    },
    MiddleEndPayload {
        root: usize,
        source: ProductionMiddleEndEvidenceCodecErrorV5,
    },
    CorrespondencePayload {
        root: usize,
        detail: &'static str,
    },
    SemanticInductionAnalysis {
        root: usize,
        source: SemanticU32InductionAnalysisErrorV1,
    },
    SemanticInductionEvidence {
        root: usize,
        source: SemanticU32InductionEvidenceErrorV1,
    },
    FormalMemoryPayload {
        root: usize,
        source: FormalMemoryReceiptErrorV1,
    },
    VerusPayload {
        root: usize,
        source: ProductionMirPlironVerusExecutionEvidenceErrorV1,
    },
}

impl fmt::Display for CompilerMultiRootProofValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProofBindingDecode(error) => {
                write!(formatter, "proof binding decode failed: {error}")
            }
            Self::ProofBindingIdentityMismatch { field } => {
                write!(formatter, "proof binding names a different {field} receipt")
            }
            Self::SemanticMirDecode(error) => {
                write!(formatter, "semantic MIR decode failed: {error}")
            }
            Self::MiddleEndRoster(error) => {
                write!(formatter, "middle-end roster decode failed: {error}")
            }
            Self::CorrespondenceRoster(error) => {
                write!(formatter, "correspondence roster decode failed: {error}")
            }
            Self::FormalMemoryRoster(error) => {
                write!(formatter, "formal-memory roster decode failed: {error}")
            }
            Self::VerusRoster(error) => write!(formatter, "Verus roster decode failed: {error}"),
            Self::KernelIrV8(error) => write!(formatter, "Kernel IR V8 decode failed: {error}"),
            Self::KernelIrV9(error) => write!(formatter, "Kernel IR V9 decode failed: {error}"),
            Self::RosterMismatch(detail) => {
                write!(formatter, "multi-root proof roster mismatch: {detail}")
            }
            Self::RootMismatch { root, detail } => {
                write!(formatter, "multi-root proof root {root} mismatch: {detail}")
            }
            Self::MiddleEndPayload { root, source } => {
                write!(formatter, "middle-end payload {root} failed: {source}")
            }
            Self::CorrespondencePayload { root, detail } => {
                write!(formatter, "correspondence payload {root} failed: {detail}")
            }
            Self::SemanticInductionAnalysis { root, source } => write!(
                formatter,
                "semantic induction replay {root} failed: {source}"
            ),
            Self::SemanticInductionEvidence { root, source } => write!(
                formatter,
                "semantic induction evidence {root} failed: {source}"
            ),
            Self::FormalMemoryPayload { root, source } => {
                write!(formatter, "formal-memory payload {root} failed: {source}")
            }
            Self::VerusPayload { root, source } => {
                write!(formatter, "Verus payload {root} failed: {source}")
            }
        }
    }
}

impl Error for CompilerMultiRootProofValidationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProofBindingDecode(error) => Some(error),
            Self::SemanticMirDecode(error) => Some(error),
            Self::MiddleEndRoster(error)
            | Self::CorrespondenceRoster(error)
            | Self::FormalMemoryRoster(error)
            | Self::VerusRoster(error) => Some(error),
            Self::KernelIrV8(error) => Some(error),
            Self::KernelIrV9(error) => Some(error),
            Self::MiddleEndPayload { source, .. } => Some(source),
            Self::SemanticInductionAnalysis { source, .. } => Some(source),
            Self::SemanticInductionEvidence { source, .. } => Some(source),
            Self::FormalMemoryPayload { source, .. } => Some(source),
            Self::VerusPayload { source, .. } => Some(source),
            Self::ProofBindingIdentityMismatch { .. }
            | Self::RosterMismatch(_)
            | Self::RootMismatch { .. }
            | Self::CorrespondencePayload { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_compiler_lineage::{
        MultiRootNeutralKirIdentityV2, MultiRootProofRosterInputsV2,
        MultiRootProofRosterRootInputV2,
    };

    fn roster(
        kind: MultiRootProofRosterKindV2,
        roster_identity: [u8; 32],
        second_workgroup: [u32; 3],
    ) -> MultiRootProofRosterTranscriptV2 {
        let roots = [
            MultiRootProofRosterRootInputV2 {
                semantic_root: 3,
                semantic_root_identity: [0x31; 32],
                kernel_binding: [0x41; 32],
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "alpha",
                export_symbol: "alpha_kernel",
                kernel_id: "alpha_kernel",
                payload: b"alpha-payload",
            },
            MultiRootProofRosterRootInputV2 {
                semantic_root: 7,
                semantic_root_identity: [0x32; 32],
                kernel_binding: [0x42; 32],
                source_rank: 2,
                workgroup: second_workgroup,
                logical_name: "zeta",
                export_symbol: "zeta_kernel",
                kernel_id: "zeta_kernel",
                payload: b"zeta-payload",
            },
        ];
        MultiRootProofRosterTranscriptV2::new(MultiRootProofRosterInputsV2 {
            kind,
            semantic_mir_sha256: [0x11; 32],
            neutral_kir: MultiRootNeutralKirIdentityV2::new(
                MultiRootCanonicalKirVersionV2::V9,
                4096,
                [0x21; 32],
            )
            .unwrap(),
            roster_identity,
            canonical_kernel_order: &[0, 1],
            roots: &roots,
        })
        .unwrap()
    }

    #[test]
    fn proof_rosters_require_exact_cross_kind_metadata() {
        let middle = roster(MultiRootProofRosterKindV2::MiddleEnd, [0x51; 32], [8, 4, 1]);
        let correspondence = roster(
            MultiRootProofRosterKindV2::Correspondence,
            [0x51; 32],
            [8, 4, 1],
        );
        let formal = roster(
            MultiRootProofRosterKindV2::FormalMemory,
            [0x51; 32],
            [8, 4, 1],
        );
        let verus = roster(
            MultiRootProofRosterKindV2::VerusExecution,
            [0x51; 32],
            [8, 4, 1],
        );
        validate_roster_set(&middle, &correspondence, &formal, &verus).unwrap();

        let wrong_workgroup = roster(
            MultiRootProofRosterKindV2::VerusExecution,
            [0x51; 32],
            [16, 2, 1],
        );
        assert!(matches!(
            validate_roster_set(&middle, &correspondence, &formal, &wrong_workgroup),
            Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "multi-root proof roster root metadata differs"
            ))
        ));

        let wrong_identity = roster(
            MultiRootProofRosterKindV2::VerusExecution,
            [0x52; 32],
            [8, 4, 1],
        );
        assert!(matches!(
            validate_roster_set(&middle, &correspondence, &formal, &wrong_identity),
            Err(CompilerMultiRootProofValidationErrorV1::RosterMismatch(
                "multi-root proof roster headers differ"
            ))
        ));
    }
}
